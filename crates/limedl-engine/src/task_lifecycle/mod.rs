//! Download task lifecycle — extracted from manager.rs to reduce the god object.
//!
//! Contains state-changing lifecycle operations: pause/resume/cancel/remove/purge
//! internal helpers, file cleanup, wait coordination, progress recording, and
//! event emission.
//!
//! `TaskLifecycle` is an independent actor type.  The methods that need shared
//! state receive a `&dyn ManagerState` (or `Arc<dyn ManagerState>`) parameter,
//! so this module does not depend on `manager` — which owns `Arc<TaskLifecycle>`
//! and would otherwise form a dependency cycle.  Methods whose `dm` parameter
//! was unused take no state at all.

use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::{
    download::{ManagedDownload, WaitState},
    error::{DownloadError, Result, io_error_with_path},
    event_bus::DownloadEvent,
    file_ops::open_download_file,
    manager_state::ManagerState,
    now_ms,
    types::{ChunkInfo, DownloadProgress, DownloadSnapshot, DownloadState, DownloadSummary},
};

#[cfg(any(test, feature = "test-utils"))]
#[allow(unused_imports)]
use crate::{speed_tracker::SpeedTracker, types::Priority};

/// Zero-sized actor type for download lifecycle operations.
///
/// Methods that need shared state receive `&dyn ManagerState`; `DownloadManager`
/// holds `Arc<TaskLifecycle>` for delegation and implements that trait.
pub struct TaskLifecycle;

impl TaskLifecycle {
    // ── Lookup helpers ────────────────────────────────────────────────

    /// Look up a managed download by ID.
    pub async fn get(
        &self,
        dm: &dyn ManagerState,
        download_id: &str,
    ) -> Result<Arc<ManagedDownload>> {
        dm.downloads()
            .read()
            .await
            .get(download_id)
            .cloned()
            .ok_or(DownloadError::NotFound)
    }

    // ── Shutdown ──────────────────────────────────────────────────────

    /// Signal the scheduler loop and all active chunk workers to stop gracefully.
    pub async fn shutdown(&self, dm: &dyn ManagerState) {
        dm.controls().shutdown_token.cancel();

        let downloads = dm.downloads().read().await;
        for managed in downloads.values() {
            if let Some(token) = managed.lock_runtime().take() {
                token.cancel();
            }
            managed.stop_notify.notify_one();
        }
    }

    // ── Spawn download (background task orchestration) ────────────────

    /// Spawn a background task that runs the full download lifecycle (mirror
    /// retry loop, HTTP execution, state transitions, cleanup, eviction).
    ///
    /// This is the core orchestration point called by `start()` and `resume()`.
    pub async fn spawn_download(
        &self,
        dm: Arc<dyn ManagerState>,
        managed: Arc<ManagedDownload>,
        max_retries: u32,
    ) -> Result<()> {
        {
            let mut runtime = managed.lock_runtime();
            if runtime.is_some() {
                return Ok(());
            }
            *runtime = Some(CancellationToken::new());
        }

        let manager = dm.clone();
        let token = managed
            .lock_runtime()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("runtime token not set after initialization"))?;

        dm.persist(managed.clone()).await?;

        tokio::spawn(async move {
            let (urls_to_try, start_index): (Vec<String>, usize) = {
                let core = managed.lock_core();
                if core.manifest.mirror_urls.is_empty() {
                    (vec![core.manifest.url.clone()], 0)
                } else {
                    let urls = core.manifest.mirror_urls.clone();
                    let idx = core
                        .manifest
                        .current_mirror_index
                        .min(urls.len().saturating_sub(1));
                    (urls, idx)
                }
            };

            let has_mirrors = urls_to_try.len() > 1;
            let actual_retries = if has_mirrors { 1 } else { max_retries };

            for index in start_index..urls_to_try.len() {
                let url_to_try = &urls_to_try[index];
                {
                    let mut core = managed.lock_core();
                    core.manifest.current_mirror_index = index;
                    if has_mirrors {
                        core.manifest.mirror_url = Some(url_to_try.clone());
                        core.manifest.final_url = url_to_try.clone();
                    }
                    core.snapshot.mirror_url = core.manifest.mirror_url.clone();
                    core.snapshot.final_url = core.manifest.final_url.clone();
                }

                let (client, cdn_accelerated, cdn_node_ip) =
                    manager.resolve_client(url_to_try).await;
                {
                    let mut core = managed.lock_core();
                    core.snapshot.cdn_accelerated = cdn_accelerated;
                    core.manifest.cdn_accelerated = cdn_accelerated;
                    core.snapshot.cdn_node_ip = cdn_node_ip.clone();
                    core.manifest.cdn_node_ip = cdn_node_ip;
                }

                // `HttpExecutor` is a zero-sized actor; `task_lifecycle -> http_executor` is
                // a one-way edge (the executor never names this module).
                let result = crate::http_executor::HttpExecutor
                    .run_download(
                        manager.clone(),
                        managed.clone(),
                        client,
                        token.clone(),
                        actual_retries,
                    )
                    .await;

                match result {
                    Ok(()) => break,
                    Err(DownloadError::Interrupted) => {
                        {
                            let mut runtime = managed.lock_runtime();
                            *runtime = None;
                        }
                        managed.stop_notify.notify_one();
                        return;
                    }
                    Err(error) => {
                        let is_network = is_network_error(&error);
                        let has_more = index + 1 < urls_to_try.len();

                        if has_mirrors && is_network && has_more {
                            tracing::warn!(
                                "mirror {index} failed with network error, trying next: {error}"
                            );
                            continue;
                        }

                        {
                            let mut core = managed.lock_core();
                            core.snapshot.state = DownloadState::Failed;
                            core.snapshot.error = Some(error.to_string());
                            core.snapshot.connection_count = 0;
                            core.snapshot.allocated_thread_count = Some(0);
                            core.snapshot.updated_at_ms = now_ms();
                            core.manifest.state = DownloadState::Failed;
                            core.manifest.error = Some(error.to_string());
                            core.manifest.connection_count = 0;
                            core.manifest.allocated_thread_count = Some(0);
                            core.manifest.updated_at_ms = now_ms();
                        }
                        manager.emit_single_summary(&managed);

                        let download_id = managed.lock_core().snapshot.id.clone();
                        let gid = format!(
                            "{:016x}",
                            xxhash_rust::xxh3::xxh3_64(download_id.as_bytes())
                        );
                        manager
                            .event_bus()
                            .publish(DownloadEvent::Aria2Notification {
                                event_name: "aria2.onDownloadError".into(),
                                gid,
                            });

                        break;
                    }
                }
            }

            let should_persist = {
                let core = managed.lock_core();
                core.snapshot.state != DownloadState::Canceled
            };
            if should_persist && let Err(error) = manager.persist(managed.clone()).await {
                log_background_error("persist background download state", &error);
            }
            manager.evict_completed().await;
            {
                let mut runtime = managed.lock_runtime();
                *runtime = None;
            }
            managed.stop_notify.notify_one();
            manager.controls().rebalance_notify.notify_waiters();
        });

        Ok(())
    }

    // ── Remove / Purge internal ───────────────────────────────────────

    pub async fn remove_internal(
        &self,
        dm: &dyn ManagerState,
        download_id: &str,
        purge_file: bool,
    ) -> Result<DownloadSnapshot> {
        let managed = self.get(dm, download_id).await?;
        let snapshot_before = self.build_snapshot(managed.clone());
        let needs_cancel_state = matches!(
            snapshot_before.state,
            DownloadState::Queued
                | DownloadState::Downloading
                | DownloadState::Retrying
                | DownloadState::Verifying
                | DownloadState::Paused
        );

        if needs_cancel_state {
            let mut core = managed.lock_core();
            core.snapshot.state = DownloadState::Canceled;
            core.snapshot.connection_count = 0;
            core.snapshot.allocated_thread_count = Some(0);
            core.snapshot.updated_at_ms = now_ms();
            core.manifest.state = DownloadState::Canceled;
            core.manifest.connection_count = 0;
            core.manifest.allocated_thread_count = Some(0);
            core.manifest.updated_at_ms = now_ms();
        }

        let token = { managed.lock_runtime().clone() };
        if let Some(token) = token {
            token.cancel();
            self.wait_until_stopped(&managed).await;
        }

        self.cleanup_files(&managed)?;
        if purge_file {
            self.cleanup_destination_file(&managed)?;
        }
        dm.downloads().write().await.remove(download_id);
        dm.db()
            .delete_download(download_id)
            .context("failed to delete download from database")?;
        dm.rebalance_allocations().await?;
        dm.controls().rebalance_notify.notify_waiters();
        Ok(self.build_snapshot(managed))
    }

    // ── Wait helpers ──────────────────────────────────────────────────

    /// Wait until a download's runtime token is dropped (worker exited).
    pub async fn wait_until_stopped(&self, managed: &Arc<ManagedDownload>) {
        if managed.lock_runtime().is_none() {
            return;
        }
        let notified = managed.stop_notify.notified();
        if managed.lock_runtime().is_none() {
            return;
        }
        notified.await;
    }

    /// Wait until a download is allocated threads and actively running.
    /// Returns the current wait state (Running, Paused, Canceled).
    pub async fn wait_until_active(
        &self,
        dm: &dyn ManagerState,
        managed: &Arc<ManagedDownload>,
        token: &CancellationToken,
    ) -> WaitState {
        loop {
            {
                let core = managed.lock_core();
                if core.manifest.state == DownloadState::Canceled {
                    return WaitState::Canceled;
                }
                if core.manifest.state == DownloadState::Paused {
                    return WaitState::Paused;
                }
                if core.manifest.allocated_thread_count.unwrap_or(0) > 0 {
                    return WaitState::Running;
                }
            }

            tokio::select! {
                _ = token.cancelled() => return match managed.lock_core().snapshot.state {
                    DownloadState::Canceled => WaitState::Canceled,
                    _ => WaitState::Paused,
                },
                _ = dm.controls().rebalance_notify.notified() => {}
                _ = sleep(Duration::from_millis(120)) => {}
            }
        }
    }

    // ── File operations ───────────────────────────────────────────────

    /// Delete the temp file (and any corrupt leftover file) for a download.
    pub fn cleanup_files(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        let temp_path = PathBuf::from(manifest.temp_path);
        if temp_path.exists() {
            remove_file_if_exists(&temp_path)?;
        }
        let corrupt_path = temp_path.with_extension("part.corrupt");
        if corrupt_path.exists() {
            remove_file_if_exists(&corrupt_path)?;
        }
        Ok(())
    }

    /// Delete the destination file for a download (used by `purge`).
    fn cleanup_destination_file(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        let destination_path = PathBuf::from(&manifest.destination_path);
        if destination_path.exists() {
            fs::remove_file(&destination_path)
                .map_err(|e| io_error_with_path(e, destination_path.to_string_lossy()))?;
        }
        let corrupt_path = PathBuf::from(&manifest.temp_path).with_extension("part.corrupt");
        if corrupt_path.exists() {
            remove_file_if_exists(&corrupt_path)?;
        }
        Ok(())
    }

    /// Prepare a fresh (empty) temp file for a download.
    pub fn prepare_fresh_temp_file(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        let temp_path = PathBuf::from(manifest.temp_path);
        if temp_path.exists() {
            fs::remove_file(&temp_path)
                .map_err(|e| io_error_with_path(e, temp_path.to_string_lossy()))?;
        }
        let corrupt_path = temp_path.with_extension("part.corrupt");
        if corrupt_path.exists() {
            remove_file_if_exists(&corrupt_path)?;
        }
        let _file = open_download_file(&temp_path, manifest.total_bytes)?;
        Ok(())
    }

    /// Reset all progress counters for a download.
    pub fn reset_progress(&self, managed: &Arc<ManagedDownload>, force_single_stream: bool) {
        let now = now_ms();
        let mut core = managed.lock_core();
        core.speed_tracker.reset();
        core.snapshot.downloaded_bytes = 0;
        core.snapshot.updated_at_ms = now;
        core.manifest.downloaded_bytes = 0;
        core.manifest.updated_at_ms = now;
        // The file is about to be recreated from scratch, so nothing written before
        // still counts as durable progress for this task.
        core.durable_bytes = 0;
        for chunk in &mut core.manifest.chunks {
            chunk.downloaded = 0;
            chunk.completed = false;
            chunk.durable_downloaded = 0;
            chunk.claimed_by = None;
            chunk.dirty = true;
        }
        if force_single_stream {
            core.snapshot.connection_count = 1;
            core.snapshot.supports_ranges = false;
            core.snapshot.desired_thread_count = Some(1);
            core.snapshot.allocated_thread_count = Some(1);
            core.snapshot.thread_note = Some(String::from("单线程（服务器不支持分段）"));
            core.manifest.connection_count = 1;
            core.manifest.supports_ranges = false;
            core.manifest.chunks.clear();
            core.manifest.desired_thread_count = Some(1);
            core.manifest.allocated_thread_count = Some(1);
            core.manifest.thread_note = Some(String::from("单线程（服务器不支持分段）"));
        }
    }

    // ── Eviction ──────────────────────────────────────────────────────

    /// Evict terminal-state downloads (Completed/Failed/Canceled) when the
    /// in-memory map exceeds `max_in_memory_downloads`.  Removes the oldest
    /// terminal entries first.  Returns the number of entries removed.
    pub async fn evict_completed(&self, dm: &dyn ManagerState) -> usize {
        let limit = dm.settings_service().get().await.max_in_memory_downloads;
        if limit == 0 {
            return 0;
        }

        let mut evicted = 0;
        let mut map = dm.downloads().write().await;
        if map.len() <= limit {
            return 0;
        }

        let mut terminal: Vec<(String, u64)> = Vec::new();
        for (id, managed) in map.iter() {
            let core = managed.lock_core();
            if is_terminal(core.snapshot.state) {
                terminal.push((id.clone(), core.snapshot.created_at_ms));
            }
        }

        terminal.sort_by_key(|(_, created)| *created);

        let excess = map.len().saturating_sub(limit);
        let to_remove = terminal.len().min(excess);
        for (id, _) in terminal.iter().take(to_remove) {
            map.remove(id);
            evicted += 1;
        }

        evicted
    }

    // ── Snapshot building ─────────────────────────────────────────────

    /// Build a `DownloadSnapshot` from a managed download, computing speed
    /// and ETA from AIMD state.
    pub fn build_snapshot(&self, managed: Arc<ManagedDownload>) -> DownloadSnapshot {
        let core = managed.lock_core();
        let mut snapshot = core.snapshot.clone();
        let chunks: Vec<ChunkInfo> = core
            .manifest
            .chunks
            .iter()
            .map(|c| ChunkInfo {
                index: c.index,
                start: c.start,
                end: c.end,
                downloaded: c.downloaded,
                completed: c.completed,
                claimed_by: c.claimed_by,
            })
            .collect();
        snapshot.chunks = chunks;

        let speed = if is_terminal(snapshot.state) {
            None
        } else if core.speed_tracker.is_initialized() {
            core.speed_tracker.current_speed(std::time::Instant::now())
        } else {
            let elapsed = (snapshot
                .updated_at_ms
                .saturating_sub(snapshot.created_at_ms))
            .max(1) as f64
                / 1000.0;
            let average_speed = if snapshot.downloaded_bytes == 0 {
                None
            } else {
                Some(snapshot.downloaded_bytes as f64 / elapsed)
            };
            managed.aimd.lock().last_throughput.or(average_speed)
        };
        drop(core);

        let eta = match (snapshot.total_bytes, speed) {
            (Some(total), Some(speed)) if speed > 0.0 && total >= snapshot.downloaded_bytes => {
                Some(((total - snapshot.downloaded_bytes) as f64 / speed).ceil() as u64)
            }
            _ => None,
        };
        snapshot.speed_bytes_per_second = if is_terminal(snapshot.state) {
            None
        } else {
            speed
        };
        snapshot.eta_seconds = if is_terminal(snapshot.state) {
            None
        } else {
            eta
        };
        snapshot
    }

    // ── Event emission ────────────────────────────────────────────────

    /// Build a [`DownloadSummary`] from a managed download and emit a
    /// `download-updated` event to the frontend.
    pub fn emit_single_summary(&self, dm: &dyn ManagerState, managed: &Arc<ManagedDownload>) {
        let snapshot = self.build_snapshot(managed.clone());
        let summary = DownloadSummary::from(&snapshot);
        dm.event_bus().publish(DownloadEvent::Updated {
            summary: Box::new(summary),
        });
    }

    /// Emit a lightweight `download-progress` event for incremental UI updates.
    pub fn emit_progress(&self, dm: &dyn ManagerState, managed: &Arc<ManagedDownload>) {
        let snapshot = self.build_snapshot(managed.clone());
        let progress = DownloadProgress::from(&snapshot);
        dm.event_bus().publish(DownloadEvent::Progress { progress });
    }

    /// Record downloaded bytes on a managed download (chunk index optional).
    /// Updates both the snapshot and the manifest chunk bookkeeping.
    ///
    /// Thin manager-side wrapper over
    /// [`record_progress_on_managed`](crate::download::record_progress_on_managed),
    /// which is the one implementation: chunk workers hold only an
    /// `&Arc<ManagedDownload>` and cannot reach this actor.
    pub fn record_progress(
        &self,
        managed: &Arc<ManagedDownload>,
        chunk_index: Option<usize>,
        bytes: u64,
    ) {
        crate::download::record_progress_on_managed(managed, chunk_index, bytes);
    }
}

// ── Free helper functions ──────────────────────────────────────────────

/// Returns `true` if the state is terminal (no further work possible).
fn is_terminal(state: DownloadState) -> bool {
    matches!(
        state,
        DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
    )
}

/// Remove a file if it exists, silently ignoring "not found" errors.
fn remove_file_if_exists(path: &std::path::Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error_with_path(error, path.to_string_lossy())),
    }
}

/// Background error logging helper.
fn log_background_error(context: &str, error: impl std::fmt::Display) {
    tracing::warn!(context, %error, "background error");
}

/// Returns `true` if the error represents a transport-level network failure.
fn is_network_error(error: &DownloadError) -> bool {
    match error {
        DownloadError::Http(e) => e.is_connect() || e.is_timeout() || e.is_body() || e.is_decode(),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
