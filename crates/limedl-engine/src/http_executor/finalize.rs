//! Finalization: checksum verification, atomic rename and completion events.

use super::HttpExecutor;
use super::worker::finalize_was_canceled;
use crate::download::{ManagedDownload, RunOutcome};
use crate::error::{Result, io_error_with_path};
use crate::event_bus::DownloadEvent;
use crate::file_ops::finalize_temp_file;
use crate::manager_state::ManagerState;
use crate::types::{ChecksumMode, DownloadProgress, DownloadState};
use crate::{calculate_checksum_with_progress, now_ms};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

impl HttpExecutor {
    pub(super) async fn finalize_download(
        &self,
        dm: Arc<dyn ManagerState>,
        managed: Arc<ManagedDownload>,
        token: CancellationToken,
    ) -> Result<RunOutcome> {
        if finalize_was_canceled(&managed, &token) {
            return Ok(RunOutcome::Canceled);
        }
        {
            let mut core = managed.lock_core();
            if core.snapshot.state == DownloadState::Canceled || token.is_cancelled() {
                return Ok(RunOutcome::Canceled);
            }
            core.snapshot.state = DownloadState::Verifying;
            core.snapshot.connection_count = 0;
            core.snapshot.allocated_thread_count = Some(0);
            core.snapshot.updated_at_ms = now_ms();
            core.manifest.state = DownloadState::Verifying;
            core.manifest.connection_count = 0;
            core.manifest.allocated_thread_count = Some(0);
            core.manifest.updated_at_ms = now_ms();
        }
        dm.persist(managed.clone()).await?;

        if finalize_was_canceled(&managed, &token) {
            return Ok(RunOutcome::Canceled);
        }

        let (temp_path, destination_path, checksum_mode, expected_checksum, precomputed) = {
            let core = managed.lock_core();
            (
                PathBuf::from(core.manifest.temp_path.clone()),
                PathBuf::from(core.manifest.destination_path.clone()),
                core.manifest.checksum_mode,
                core.manifest.expected_checksum.clone(),
                core.manifest.checksum.clone(),
            )
        };

        let checksum = match checksum_mode {
            ChecksumMode::None => None,
            _ if precomputed.is_some() => precomputed,
            mode => {
                let dl_id = managed.lock_core().snapshot.id.clone();
                let eb = dm.event_bus().clone();
                let (prog_tx, mut prog_rx) = tokio::sync::mpsc::unbounded_channel::<(u64, u64)>();

                let progress_worker = tokio::spawn(async move {
                    let start = Instant::now();
                    while let Some((processed, total)) = prog_rx.recv().await {
                        let elapsed = start.elapsed().as_secs_f64();
                        let speed = if elapsed > 0.05 {
                            Some(processed as f64 / elapsed)
                        } else {
                            None
                        };
                        let eta = speed.and_then(|s| {
                            if s > 0.0 {
                                Some(((total.saturating_sub(processed)) as f64 / s) as u64)
                            } else {
                                None
                            }
                        });
                        eb.publish(DownloadEvent::Progress {
                            progress: DownloadProgress {
                                id: dl_id.clone(),
                                state: DownloadState::Verifying,
                                downloaded_bytes: processed,
                                total_bytes: Some(total),
                                speed_bytes_per_second: speed,
                                eta_seconds: eta,
                                connection_count: 0,
                                allocated_thread_count: Some(0),
                                error: None,
                                uploaded_bytes: None,
                                upload_speed_bytes_per_second: None,
                                peer_count: None,
                                upload_status: None,
                                degraded: false,
                                disk_type: None,
                                flushing: false,
                            },
                        });
                    }
                });

                let hash_res = calculate_checksum_with_progress(
                    temp_path.clone(),
                    mode,
                    move |done, total| {
                        let _ = prog_tx.send((done, total));
                    },
                )
                .await;

                let _ = progress_worker.await;
                Some(hash_res?)
            }
        };

        // Verify expected checksum if one was provided
        if let (Some(expected), Some(computed)) = (&expected_checksum, &checksum)
            && !expected.eq_ignore_ascii_case(computed)
        {
            let error_msg = format!("Checksum mismatch: expected {expected}, got {computed}");
            // Preserve the corrupt temp file for diagnosis instead of letting the
            // downstream cleanup delete the only evidence of what went wrong. The
            // renamed `{id}.part.corrupt` survives even if `cleanup_files` later
            // removes the original temp path (which no longer exists after the
            // rename).
            let corrupt_path = temp_path.with_extension("part.corrupt");
            match tokio::fs::rename(&temp_path, &corrupt_path).await {
                Ok(_) => tracing::warn!(
                    "checksum mismatch; preserved corrupt temp file at {}",
                    corrupt_path.display()
                ),
                Err(e) => tracing::warn!(
                    "failed to preserve corrupt temp file {}: {e}",
                    temp_path.display()
                ),
            }
            {
                let mut core = managed.lock_core();
                core.snapshot.state = DownloadState::Failed;
                core.snapshot.error = Some(error_msg.clone());
                core.snapshot.connection_count = 0;
                core.snapshot.allocated_thread_count = Some(0);
                core.snapshot.updated_at_ms = now_ms();
                core.manifest.state = DownloadState::Failed;
                core.manifest.error = Some(error_msg);
                core.manifest.connection_count = 0;
                core.manifest.allocated_thread_count = Some(0);
                core.manifest.updated_at_ms = now_ms();
            }
            dm.persist(managed.clone()).await?;
            return Ok(RunOutcome::Finished);
        }

        // Log success when checksum was computed and either matched or not checked
        if checksum_mode != ChecksumMode::None {
            tracing::info!("checksum verified");
        }

        if finalize_was_canceled(&managed, &token) {
            return Ok(RunOutcome::Canceled);
        }

        if let Some(parent) = destination_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| io_error_with_path(e, parent.to_string_lossy()))?;
        }

        if finalize_was_canceled(&managed, &token) {
            return Ok(RunOutcome::Canceled);
        }

        {
            let mut core = managed.lock_core();
            if core.manifest.state == DownloadState::Canceled || token.is_cancelled() {
                return Ok(RunOutcome::Canceled);
            }
            if core.snapshot.state == DownloadState::Canceled || token.is_cancelled() {
                return Ok(RunOutcome::Canceled);
            }

            ensure_core_fully_durable(&core)?;
            finalize_temp_file(&temp_path, &destination_path)?;

            core.snapshot.state = DownloadState::Completed;
            core.snapshot.downloaded_bytes = core
                .snapshot
                .total_bytes
                .unwrap_or(core.snapshot.downloaded_bytes);
            core.snapshot.checksum = checksum.clone();
            core.snapshot.destination_path = destination_path.to_string_lossy().to_string();
            core.snapshot.error = None;
            core.snapshot.updated_at_ms = now_ms();

            core.manifest.state = DownloadState::Completed;
            core.manifest.downloaded_bytes = core
                .manifest
                .total_bytes
                .unwrap_or(core.manifest.downloaded_bytes);
            core.manifest.checksum = checksum;
            core.manifest.destination_path = destination_path.to_string_lossy().to_string();
            core.manifest.error = None;
            core.manifest.updated_at_ms = now_ms();
            for chunk in &mut core.manifest.chunks {
                chunk.completed = true;
                chunk.downloaded = chunk.end.saturating_sub(chunk.start) + 1;
                chunk.claimed_by = None;
                chunk.dirty = true;
            }
        }
        dm.persist(managed.clone()).await?;

        // Broadcast aria2.onDownloadComplete via EventBus
        let download_id = managed.lock_core().snapshot.id.clone();
        let gid = format!(
            "{:016x}",
            xxhash_rust::xxh3::xxh3_64(download_id.as_bytes())
        );
        dm.event_bus().publish(DownloadEvent::Aria2Notification {
            event_name: "aria2.onDownloadComplete".into(),
            gid,
        });

        Ok(RunOutcome::Finished)
    }
}

/// Refuse to publish a file whose chunks are not all on disk.
///
/// The write buffer is flushed before finalize runs, so a shortfall here means a
/// flush silently dropped data (a failed background flush, a dropped buffer).
/// Publishing anyway would hand the user a truncated file that then reports — and
/// resumes as — complete. Chunks are absent for a single-stream download and for a
/// task whose rows were already terminal when it was loaded, and then there is
/// nothing to check.
///
/// Takes the already-held core guard on purpose: calling `lock_core()` here would
/// deadlock against the caller's own guard (`parking_lot` mutexes are not
/// reentrant).
pub(super) fn ensure_core_fully_durable(core: &crate::download::DownloadCore) -> Result<()> {
    let shortfall: u64 = core
        .manifest
        .chunks
        .iter()
        .map(|chunk| chunk.byte_len().saturating_sub(chunk.durable_bytes()))
        .sum();
    if shortfall > 0 {
        return Err(crate::error::DownloadError::Internal(format!(
            "{shortfall} byte(s) never reached the file (write buffer flush failed); \
             refusing to publish a truncated download"
        )));
    }
    Ok(())
}
