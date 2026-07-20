//! Download task lifecycle — extracted from manager.rs to reduce the god object.
//!
//! Contains state-changing lifecycle operations: pause/resume/cancel/remove/purge
//! internal helpers, file cleanup, wait coordination, progress recording, and
//! event emission.
//!
//! `TaskLifecycle` is an independent actor type.  All its methods receive a
//! `&DownloadManager` or `Arc<DownloadManager>` parameter to access shared
//! state, avoiding any ownership cycle with `DownloadManager` (which holds
//! `Arc<TaskLifecycle>`).

use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::{
    error::{DownloadError, Result, io_error_with_path},
    event_bus::DownloadEvent,
    file_ops::open_download_file,
    manager::{
        DownloadManager,
        ManagedDownload, WaitState,
    },
    now_ms,
    types::{ChunkInfo, DownloadProgress, DownloadSnapshot, DownloadState, DownloadSummary},
    slot_guard::DownloadSlotGuard,
};

/// Zero-sized actor type for download lifecycle operations.
///
/// All methods receive `&DownloadManager` or `Arc<DownloadManager>` to access
/// shared state.  `DownloadManager` holds `Arc<TaskLifecycle>` for delegation.
pub struct TaskLifecycle;

impl TaskLifecycle {
    // ── Lookup helpers ────────────────────────────────────────────────

    /// Look up a managed download by ID.
    pub(crate) async fn get(
        &self,
        dm: &DownloadManager,
        download_id: &str,
    ) -> Result<Arc<ManagedDownload>> {
        dm.downloads
            .read()
            .await
            .get(download_id)
            .cloned()
            .ok_or(DownloadError::NotFound)
    }

    // ── Shutdown ──────────────────────────────────────────────────────

    /// Signal the scheduler loop and all active chunk workers to stop gracefully.
    pub(crate) async fn shutdown(&self, dm: &DownloadManager) {
        dm.controls.shutdown_token.cancel();

        let downloads = dm.downloads.read().await;
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
    pub(crate) async fn spawn_download(
        &self,
        dm: Arc<DownloadManager>,
        managed: Arc<ManagedDownload>,
        max_retries: u32,
        slot: DownloadSlotGuard,
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
            let _guard = slot;
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

                let (client, cdn_accelerated) = manager.resolve_client(url_to_try).await;
                {
                    let mut core = managed.lock_core();
                    core.snapshot.cdn_accelerated = cdn_accelerated;
                    core.manifest.cdn_accelerated = cdn_accelerated;
                }

                let result = manager
                    .http_executor
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
                        manager.task_lifecycle.emit_single_summary(&manager, &managed);

                        let download_id = managed.lock_core().snapshot.id.clone();
                        let gid = format!(
                            "{:016x}",
                            xxhash_rust::xxh3::xxh3_64(download_id.as_bytes())
                        );
                        manager.event_bus.publish(DownloadEvent::Aria2Notification {
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
            if should_persist
                && let Err(error) = manager.persist(managed.clone()).await
            {
                log_background_error("persist background download state", &error);
            }
            manager.task_lifecycle.evict_completed(&manager).await;
            {
                let mut runtime = managed.lock_runtime();
                *runtime = None;
            }
            managed.stop_notify.notify_one();
            manager.controls.rebalance_notify.notify_waiters();
        });

        Ok(())
    }

    // ── Remove / Purge internal ───────────────────────────────────────

    pub(crate) async fn remove_internal(
        &self,
        dm: &DownloadManager,
        download_id: &str,
        purge_file: bool,
    ) -> Result<DownloadSnapshot> {
        let managed = self.get(dm, download_id).await?;
        let snapshot_before = self.build_snapshot(dm, managed.clone());
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
            self.wait_until_stopped(dm, &managed).await;
        }

        self.cleanup_files(dm, &managed)?;
        if purge_file {
            self.cleanup_destination_file(dm, &managed)?;
        }
        dm.downloads.write().await.remove(download_id);
        dm.db
            .delete_download(download_id)
            .context("failed to delete download from database")?;
        dm.scheduler.rebalance_allocations(dm).await?;
        dm.controls.rebalance_notify.notify_waiters();
        Ok(self.build_snapshot(dm, managed))
    }

    // ── Wait helpers ──────────────────────────────────────────────────

    /// Wait until a download's runtime token is dropped (worker exited).
    pub(crate) async fn wait_until_stopped(
        &self,
        _dm: &DownloadManager,
        managed: &Arc<ManagedDownload>,
    ) {
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
    pub(crate) async fn wait_until_active(
        &self,
        dm: &DownloadManager,
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
                _ = dm.controls.rebalance_notify.notified() => {}
                _ = sleep(Duration::from_millis(120)) => {}
            }
        }
    }

    // ── File operations ───────────────────────────────────────────────

    /// Delete the temp file for a download.
    pub(crate) fn cleanup_files(
        &self,
        _dm: &DownloadManager,
        managed: &Arc<ManagedDownload>,
    ) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        let temp_path = PathBuf::from(manifest.temp_path);
        if temp_path.exists() {
            remove_file_if_exists(&temp_path)?;
        }
        Ok(())
    }

    /// Delete the destination file for a download (used by `purge`).
    fn cleanup_destination_file(
        &self,
        _dm: &DownloadManager,
        managed: &Arc<ManagedDownload>,
    ) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        let destination_path = PathBuf::from(manifest.destination_path);
        if destination_path.exists() {
            fs::remove_file(&destination_path)
                .map_err(|e| io_error_with_path(e, destination_path.to_string_lossy()))?;
        }
        Ok(())
    }

    /// Prepare a fresh (empty) temp file for a download.
    pub(crate) fn prepare_fresh_temp_file(
        &self,
        _dm: &DownloadManager,
        managed: &Arc<ManagedDownload>,
    ) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        let temp_path = PathBuf::from(manifest.temp_path);
        if temp_path.exists() {
            fs::remove_file(&temp_path)
                .map_err(|e| io_error_with_path(e, temp_path.to_string_lossy()))?;
        }
        let _file = open_download_file(&temp_path, manifest.total_bytes)?;
        Ok(())
    }

    /// Reset all progress counters for a download.
    pub(crate) fn reset_progress(
        &self,
        _dm: &DownloadManager,
        managed: &Arc<ManagedDownload>,
        force_single_stream: bool,
    ) {
        let now = now_ms();
        let mut core = managed.lock_core();
        core.snapshot.downloaded_bytes = 0;
        core.snapshot.updated_at_ms = now;
        core.manifest.downloaded_bytes = 0;
        core.manifest.updated_at_ms = now;
        for chunk in &mut core.manifest.chunks {
            chunk.downloaded = 0;
            chunk.completed = false;
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
    pub(crate) async fn evict_completed(&self, dm: &DownloadManager) -> usize {
        let limit = dm.settings.read().await.max_in_memory_downloads;
        if limit == 0 {
            return 0;
        }

        let mut evicted = 0;
        let mut map = dm.downloads.write().await;
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
    pub(crate) fn build_snapshot(
        &self,
        _dm: &DownloadManager,
        managed: Arc<ManagedDownload>,
    ) -> DownloadSnapshot {
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
        drop(core);
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
        let speed = managed.aimd.lock().last_throughput.or(average_speed);
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
    pub(crate) fn emit_single_summary(
        &self,
        dm: &DownloadManager,
        managed: &Arc<ManagedDownload>,
    ) {
        let snapshot = self.build_snapshot(dm, managed.clone());
        let summary = DownloadSummary::from(&snapshot);
        let json = serde_json::to_value(&summary).unwrap_or_default();
        dm.event_bus.publish(DownloadEvent::Updated {
            id: summary.id.clone(),
            summary_json: json,
        });
    }

    /// Emit a lightweight `download-progress` event for incremental UI updates.
    pub(crate) fn emit_progress(
        &self,
        dm: &DownloadManager,
        managed: &Arc<ManagedDownload>,
    ) {
        let snapshot = self.build_snapshot(dm, managed.clone());
        let progress = DownloadProgress::from(&snapshot);
        let json = serde_json::to_value(&progress).unwrap_or_default();
        dm.event_bus.publish(DownloadEvent::Progress {
            id: progress.id.clone(),
            progress_json: json,
        });
    }

    /// Record downloaded bytes on a managed download (chunk index optional).
    /// Records download progress on a [`ManagedDownload`] and updates the snapshot.
    ///
    /// # Design note (duplication with [`record_progress_on_managed`])
    /// The body is **identical** to the free function `record_progress_on_managed`
    /// in `manager.rs`. The free version exists because chunk workers in
    /// `http_executor` only hold an `&Arc<ManagedDownload>` and cannot call
    /// through `TaskLifecycle` (which requires `&DownloadManager`).
    /// If you modify one, you **must** update the other.
    pub(crate) fn record_progress(
        &self,
        _dm: &DownloadManager,
        managed: &Arc<ManagedDownload>,
        chunk_index: Option<usize>,
        bytes: u64,
    ) {
        let now = now_ms();
        let mut core = managed.lock_core();
        core.snapshot.downloaded_bytes = core.snapshot.downloaded_bytes.saturating_add(bytes);
        core.snapshot.error = None;
        core.snapshot.updated_at_ms = now;
        core.manifest.downloaded_bytes = core.manifest.downloaded_bytes.saturating_add(bytes);
        core.manifest.error = None;
        core.manifest.updated_at_ms = now;
        if let Some(index) = chunk_index
            && let Some(chunk) = core
                .manifest
                .chunks
                .iter_mut()
                .find(|candidate| candidate.index == index)
        {
            chunk.downloaded = chunk.downloaded.saturating_add(bytes);
            chunk.dirty = true;
            if chunk.downloaded > chunk.end.saturating_sub(chunk.start) {
                chunk.completed = true;
                chunk.claimed_by = None;
            }
        }
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
        DownloadError::Http(e) => e.is_connect() || e.is_timeout() || e.is_body(),
        _ => false,
    }
}
