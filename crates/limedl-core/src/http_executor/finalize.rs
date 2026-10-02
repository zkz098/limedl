//! Finalization: checksum verification, atomic rename and completion events.

use super::worker::{finalize_was_canceled};
use super::{Arc, CancellationToken, ChecksumMode, DownloadEvent, DownloadManager, DownloadState, HttpExecutor, ManagedDownload, PathBuf, Result, RunOutcome, calculate_checksum, finalize_temp_file, fs, io_error_with_path, now_ms};

impl HttpExecutor {
    pub(super) async fn finalize_download(
        &self,
        dm: Arc<DownloadManager>,
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
            mode => Some(calculate_checksum(temp_path.clone(), mode).await?),
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
            fs::create_dir_all(parent)
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
        dm.event_bus.publish(DownloadEvent::Aria2Notification {
            event_name: "aria2.onDownloadComplete".into(),
            gid,
        });

        Ok(RunOutcome::Finished)
    }
}
