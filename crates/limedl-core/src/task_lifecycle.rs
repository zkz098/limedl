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

#[cfg(any(test, feature = "test-utils"))]
#[allow(unused_imports)]
use crate::types::Priority;

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

                let (client, cdn_accelerated, cdn_node_ip) = manager.resolve_client(url_to_try).await;
                {
                    let mut core = managed.lock_core();
                    core.snapshot.cdn_accelerated = cdn_accelerated;
                    core.manifest.cdn_accelerated = cdn_accelerated;
                    core.snapshot.cdn_node_ip = cdn_node_ip.clone();
                    core.manifest.cdn_node_ip = cdn_node_ip;
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
            && let Some(chunk) = core.manifest.chunks.get_mut(index)
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use parking_lot::Mutex as ParkingMutex;
    use tokio::sync::Notify;

    use super::*;
    use crate::{
        aimd::AimdState,
        error::DownloadError,
        manager::{DownloadCore, ManagedDownload},
        manifest::Manifest,
        types::*,
    };

    // ── Test helpers ──────────────────────────────────────────────────

    fn make_managed(
        id: &str,
        state: DownloadState,
        created_at_ms: u64,
    ) -> ManagedDownload {
        ManagedDownload {
            core: ParkingMutex::new(DownloadCore {
                snapshot: DownloadSnapshot {
                    id: id.to_string(),
                    kind: TaskKind::Http,
                    state,
                    url: format!("https://example.com/{id}"),
                    final_url: format!("https://cdn.example.com/{id}"),
                    file_name: format!("{id}.bin"),
                    destination_path: format!("/tmp/dst/{id}.bin"),
                    temp_path: format!("/tmp/temp/{id}.part"),
                    total_bytes: None,
                    downloaded_bytes: 0,
                    supports_ranges: false,
                    connection_count: 0,
                    thread_mode: ThreadMode::Adaptive,
                    requested_thread_count: None,
                    desired_thread_count: None,
                    allocated_thread_count: None,
                    adaptive_profile: None,
                    thread_note: None,
                    checksum: None,
                    expected_checksum: None,
                    checksum_mode: ChecksumMode::None,
                    etag: None,
                    last_modified: None,
                    error: None,
                    speed_bytes_per_second: None,
                    eta_seconds: None,
                    uploaded_bytes: None,
                    upload_speed_bytes_per_second: None,
                    peer_count: None,
                    upload_status: None,
                    info_hash: None,
                    created_at_ms,
                    updated_at_ms: 0,
                    cdn_accelerated: false,
                    cdn_node_ip: None,
                    chunks: vec![],
                    seed_count: None,
                    leech_count: None,
                    download_limit_bps: None,
                    upload_limit_bps: None,
                    mirror_url: None,
                    priority: Priority::Normal,
                    degraded: false,
                    disk_type: None,
                    flushing: false,
                },
                manifest: Manifest {
                    id: id.to_string(),
                    url: format!("https://example.com/{id}"),
                    final_url: format!("https://cdn.example.com/{id}"),
                    user_agent: String::new(),
                    extra_headers: vec![],
                    destination_dir: String::new(),
                    file_name: format!("{id}.bin"),
                    file_name_locked: false,
                    destination_path: format!("/tmp/dst/{id}.bin"),
                    temp_path: format!("/tmp/temp/{id}.part"),
                    total_bytes: None,
                    downloaded_bytes: 0,
                    supports_ranges: false,
                    chunk_size: 4 * 1024 * 1024,
                    connection_count: 0,
                    thread_mode: ThreadMode::Adaptive,
                    requested_thread_count: None,
                    desired_thread_count: None,
                    allocated_thread_count: None,
                    adaptive_profile_snapshot: None,
                    thread_note: None,
                    etag: None,
                    last_modified: None,
                    state,
                    cdn_accelerated: false,
                    cdn_node_ip: None,
                    priority: Priority::Normal,
                    checksum_mode: ChecksumMode::None,
                    checksum: None,
                    expected_checksum: None,
                    error: None,
                    created_at_ms,
                    updated_at_ms: 0,
                    mirror_url: None,
                    mirror_urls: vec![],
                    current_mirror_index: 0,
                    chunks: vec![],
                },
            }),
            runtime: ParkingMutex::new(None),
            aimd: ParkingMutex::new(AimdState::default()),
            stop_notify: Notify::new(),
        }
    }

    fn make_managed_with_snapshot(
        id: &str,
        state: DownloadState,
        created_at_ms: u64,
        snapshot_mod: impl FnOnce(&mut DownloadSnapshot),
    ) -> ManagedDownload {
        let dl = make_managed(id, state, created_at_ms);
        {
            let mut core = dl.lock_core();
            snapshot_mod(&mut core.snapshot);
        }
        dl
    }

    /// Pure eviction logic extracted for testing.
    /// Same algorithm as `TaskLifecycle::evict_completed`.
    fn evict_completed_test(
        map: &mut HashMap<String, ManagedDownload>,
        limit: usize,
    ) -> usize {
        if limit == 0 {
            return 0;
        }
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
        let evicted = to_remove;
        for (id, _) in terminal.iter().take(to_remove) {
            map.remove(id);
        }

        evicted
    }

    /// Pure snapshot-building logic extracted for testing.
    /// Same algorithm as `TaskLifecycle::build_snapshot`.
    fn build_snapshot_test(managed: &ManagedDownload) -> DownloadSnapshot {
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

    // ── is_terminal ──────────────────────────────────────────────────

    #[test]
    fn is_terminal_completed() {
        assert!(is_terminal(DownloadState::Completed));
    }

    #[test]
    fn is_terminal_failed() {
        assert!(is_terminal(DownloadState::Failed));
    }

    #[test]
    fn is_terminal_canceled() {
        assert!(is_terminal(DownloadState::Canceled));
    }

    #[test]
    fn is_terminal_non_terminal_states() {
        assert!(!is_terminal(DownloadState::Queued));
        assert!(!is_terminal(DownloadState::Downloading));
        assert!(!is_terminal(DownloadState::Paused));
        assert!(!is_terminal(DownloadState::Retrying));
        assert!(!is_terminal(DownloadState::Verifying));
    }

    // ── is_network_error ─────────────────────────────────────────────

    #[test]
    fn is_network_error_http_builder_not_network() {
        // A reqwest builder error (invalid URL) is NOT connect/timeout/body
        let err = reqwest::Client::builder()
            .build()
            .unwrap()
            .get("http://")
            .build()
            .unwrap_err();
        let dl_err = DownloadError::Http(err);
        assert!(!is_network_error(&dl_err));
    }

    #[test]
    fn is_network_error_io_is_false() {
        let dl_err = DownloadError::Io(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused"));
        assert!(!is_network_error(&dl_err));
    }

    #[test]
    fn is_network_error_permission_denied_is_false() {
        let dl_err = DownloadError::PermissionDenied {
            path: "/data/file".into(),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        };
        assert!(!is_network_error(&dl_err));
    }

    #[test]
    fn is_network_error_all_non_http_variants_false() {
        let variants: Vec<DownloadError> = vec![
            DownloadError::UnsupportedScheme,
            DownloadError::NotFound,
            DownloadError::AlreadyRunning,
            DownloadError::NotResumable,
            DownloadError::Canceled,
            DownloadError::MissingFileName,
            DownloadError::Interrupted,
            DownloadError::InvalidResponse("test".into()),
            DownloadError::InvalidRequest("test".into()),
            DownloadError::InvalidProxy("test".into()),
            DownloadError::Torrent("test".into()),
            DownloadError::TorrentNetwork("test".into()),
            DownloadError::TorrentInvalidData("test".into()),
            DownloadError::TorrentIo("test".into()),
            DownloadError::InsufficientDiskSpace {
                available: 0,
                required: 100,
            },
            DownloadError::DatabaseInit("test".into()),
            DownloadError::Internal("test".into()),
            DownloadError::TooManyConcurrentDownloads,
        ];
        for (i, variant) in variants.into_iter().enumerate() {
            assert!(
                !is_network_error(&variant),
                "variant index {i} should not be a network error"
            );
        }
    }

    // ── build_snapshot ───────────────────────────────────────────────

    #[test]
    fn build_snapshot_normal_progress() {
        let dl = make_managed_with_snapshot("snap-1", DownloadState::Downloading, 1000, |s| {
            s.downloaded_bytes = 5_000_000;
            s.total_bytes = Some(10_000_000);
            s.updated_at_ms = 2000; // elapsed = 1.0s
        });

        let snap = build_snapshot_test(&dl);

        assert_eq!(snap.id, "snap-1");
        assert_eq!(snap.state, DownloadState::Downloading);
        // Average speed: 5_000_000 / 1.0 = 5_000_000 B/s
        assert!(
            snap.speed_bytes_per_second.is_some(),
            "speed should be Some for active download"
        );
        let speed = snap.speed_bytes_per_second.unwrap();
        assert!((speed - 5_000_000.0).abs() < 1.0, "expected ~5MB/s, got {speed}");

        // ETA: (10_000_000 - 5_000_000) / 5_000_000 = 1 second
        assert_eq!(snap.eta_seconds, Some(1));
    }

    #[test]
    fn build_snapshot_zero_downloaded_speed_none() {
        let dl = make_managed_with_snapshot("snap-2", DownloadState::Downloading, 1000, |s| {
            s.downloaded_bytes = 0;
            s.total_bytes = Some(10_000_000);
            s.updated_at_ms = 5000; // 4s elapsed, but 0 bytes → speed None
        });

        let snap = build_snapshot_test(&dl);

        assert_eq!(snap.speed_bytes_per_second, None);
        assert_eq!(snap.eta_seconds, None);
    }

    #[test]
    fn build_snapshot_terminal_state_speed_eta_none() {
        for &state in &[
            DownloadState::Completed,
            DownloadState::Failed,
            DownloadState::Canceled,
        ] {
            let dl = make_managed_with_snapshot("snap-term", state, 1000, |s| {
                s.downloaded_bytes = 8_000_000;
                s.total_bytes = Some(10_000_000);
                s.updated_at_ms = 3000; // elapsed = 2.0s, avg speed = 4MB/s
            });

            let snap = build_snapshot_test(&dl);

            assert!(
                snap.speed_bytes_per_second.is_none(),
                "speed should be None for terminal state {state:?}"
            );
            assert!(
                snap.eta_seconds.is_none(),
                "ETA should be None for terminal state {state:?}"
            );
        }
    }

    #[test]
    fn build_snapshot_persistent_bytes_added() {
        let dl = make_managed_with_snapshot("snap-resume", DownloadState::Downloading, 1000, |s| {
            s.downloaded_bytes = 5_000_000;
            s.total_bytes = Some(10_000_000);
            s.updated_at_ms = 2000; // elapsed = 1.0s
        });

        // Update AIMD last_throughput for realistic scenario
        {
            let mut aimd = dl.aimd.lock();
            aimd.last_throughput = Some(6_000_000.0);
        }

        let snap = build_snapshot_test(&dl);

        // speed should use AIMD last_throughput over average
        let speed = snap.speed_bytes_per_second.unwrap();
        assert!(
            (speed - 6_000_000.0).abs() < 1.0,
            "expected AIMD speed 6MB/s, got {speed}"
        );

        // ETA based on AIMD speed
        assert_eq!(snap.eta_seconds, Some(1)); // (10M - 5M) / 6M = 0.83 → ceil → 1
    }

    #[test]
    fn build_snapshot_eta_with_unknown_total_is_none() {
        let dl = make_managed_with_snapshot("snap-eta-none", DownloadState::Downloading, 1000, |s| {
            s.downloaded_bytes = 5_000_000;
            s.total_bytes = None; // unknown total
            s.updated_at_ms = 2000;
        });

        let snap = build_snapshot_test(&dl);

        assert!(snap.speed_bytes_per_second.is_some()); // still has speed
        assert!(snap.eta_seconds.is_none()); // no ETA without total
    }

    #[test]
    fn build_snapshot_total_less_than_downloaded_no_negative_eta() {
        let dl = make_managed_with_snapshot("snap-over", DownloadState::Downloading, 1000, |s| {
            s.downloaded_bytes = 12_000_000;
            s.total_bytes = Some(10_000_000); // server said 10MB, but we got 12MB
            s.updated_at_ms = 2000;
        });

        let snap = build_snapshot_test(&dl);

        // total < downloaded → ETA should be None (can't compute meaningful ETA)
        assert!(snap.eta_seconds.is_none());
    }

    #[test]
    fn build_snapshot_speed_zero_eta_none() {
        let dl = make_managed_with_snapshot("snap-zero", DownloadState::Downloading, 1000, |s| {
            s.downloaded_bytes = 5_000_000;
            s.total_bytes = Some(10_000_000);
            s.updated_at_ms = 2000;
        });

        // Force AIMD to return zero speed for this case
        {
            let mut aimd = dl.aimd.lock();
            aimd.last_throughput = Some(0.0);
        }

        let snap = build_snapshot_test(&dl);

        // speed=0 → ETA should be None (division by zero guard)
        assert!(snap.eta_seconds.is_none());
    }

    #[test]
    fn build_snapshot_exact_completion_no_eta() {
        let dl = make_managed_with_snapshot(
            "snap-done",
            DownloadState::Completed,
            1000,
            |s| {
                s.downloaded_bytes = 10_000_000;
                s.total_bytes = Some(10_000_000);
                s.updated_at_ms = 5000;
            },
        );

        let snap = build_snapshot_test(&dl);

        // Terminal state → no speed, no ETA even if download is complete
        assert!(snap.speed_bytes_per_second.is_none());
        assert!(snap.eta_seconds.is_none());
    }

    #[test]
    fn build_snapshot_non_terminal_retaining_state() {
        // Non-terminal states should preserve speed/ETA
        let non_terminal = [
            DownloadState::Queued,
            DownloadState::Downloading,
            DownloadState::Paused,
            DownloadState::Retrying,
            DownloadState::Verifying,
        ];

        for &state in &non_terminal {
            let dl = make_managed_with_snapshot("snap-active", state, 1000, |s| {
                s.downloaded_bytes = 5_000_000;
                s.total_bytes = Some(10_000_000);
                s.updated_at_ms = 2000;
            });

            let snap = build_snapshot_test(&dl);

            assert!(
                snap.speed_bytes_per_second.is_some(),
                "speed should be Some for non-terminal state {state:?}"
            );
            assert!(
                snap.eta_seconds.is_some(),
                "ETA should be Some for non-terminal state {state:?}"
            );
        }
    }

    // ── evict_completed ──────────────────────────────────────────────

    #[test]
    fn evict_completed_limit_zero_keeps_everything() {
        // With limit==0 the actual code returns 0 early (keep everything).
        // This matches the semantics: max_in_memory_downloads=0 means "no limit".
        let mut map: HashMap<String, ManagedDownload> = HashMap::new();
        map.insert("a".into(), make_managed("a", DownloadState::Completed, 100));
        map.insert("b".into(), make_managed("b", DownloadState::Failed, 200));
        map.insert("c".into(), make_managed("c", DownloadState::Canceled, 300));
        map.insert("d".into(), make_managed("d", DownloadState::Downloading, 400));

        let evicted = evict_completed_test(&mut map, 0);

        assert_eq!(evicted, 0);
        assert_eq!(map.len(), 4);
    }

    #[test]
    fn evict_completed_limit_gt_total_keeps_everything() {
        let mut map: HashMap<String, ManagedDownload> = HashMap::new();
        map.insert("a".into(), make_managed("a", DownloadState::Completed, 100));
        map.insert("b".into(), make_managed("b", DownloadState::Failed, 200));

        let evicted = evict_completed_test(&mut map, 10);

        assert_eq!(evicted, 0);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn evict_completed_no_terminal_no_op() {
        let mut map: HashMap<String, ManagedDownload> = HashMap::new();
        map.insert("a".into(), make_managed("a", DownloadState::Downloading, 100));
        map.insert("b".into(), make_managed("b", DownloadState::Paused, 200));

        let evicted = evict_completed_test(&mut map, 1);

        assert_eq!(evicted, 0);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn evict_completed_mixed_only_terminal_evicted() {
        let mut map: HashMap<String, ManagedDownload> = HashMap::new();
        map.insert("active".into(), make_managed("active", DownloadState::Downloading, 100));
        map.insert("done1".into(), make_managed("done1", DownloadState::Completed, 200));
        map.insert("done2".into(), make_managed("done2", DownloadState::Completed, 300));
        map.insert("failed".into(), make_managed("failed", DownloadState::Failed, 400));
        map.insert("paused".into(), make_managed("paused", DownloadState::Paused, 500));

        // limit=3, 5 total → excess=2 → evict 2 oldest terminal
        let evicted = evict_completed_test(&mut map, 3);

        assert_eq!(evicted, 2);
        assert_eq!(map.len(), 3);
        assert!(map.contains_key("active"), "active should survive");
        assert!(map.contains_key("paused"), "paused should survive");
        // Oldest terminal (done1, done2) removed, newest terminal (failed) stays
        assert!(!map.contains_key("done1"), "oldest completed 'done1' evicted");
        assert!(!map.contains_key("done2"), "second oldest 'done2' evicted");
        assert!(map.contains_key("failed"), "newest terminal 'failed' stays");
    }

    #[test]
    fn evict_completed_eviction_order_oldest_first() {
        let mut map: HashMap<String, ManagedDownload> = HashMap::new();
        // Insert in non-chronological order to verify sort
        map.insert("c".into(), make_managed("c", DownloadState::Completed, 300));
        map.insert("a".into(), make_managed("a", DownloadState::Completed, 100));
        map.insert("b".into(), make_managed("b", DownloadState::Completed, 200));

        // limit=1, 3 total → excess=2 → evict 2 oldest
        let evicted = evict_completed_test(&mut map, 1);

        assert_eq!(evicted, 2);
        assert_eq!(map.len(), 1);
        // Oldest two (a=100, b=200) should be evicted, newest (c=300) stays
        assert!(!map.contains_key("a"));
        assert!(!map.contains_key("b"));
        assert!(map.contains_key("c"), "newest entry 'c' should survive");
    }

    #[test]
    fn evict_completed_limit_exactly_total_keeps_all() {
        let mut map: HashMap<String, ManagedDownload> = HashMap::new();
        map.insert("a".into(), make_managed("a", DownloadState::Completed, 100));
        map.insert("b".into(), make_managed("b", DownloadState::Completed, 200));
        map.insert("c".into(), make_managed("c", DownloadState::Failed, 300));

        let evicted = evict_completed_test(&mut map, 3);

        assert_eq!(evicted, 0);
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn evict_completed_running_state_not_evicted() {
        let mut map: HashMap<String, ManagedDownload> = HashMap::new();
        map.insert("running".into(), make_managed("running", DownloadState::Downloading, 50));
        map.insert("done".into(), make_managed("done", DownloadState::Completed, 100));

        // limit=1, excess=1, but only 'done' is terminal — evict 'done'
        let evicted = evict_completed_test(&mut map, 1);

        assert_eq!(evicted, 1);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("running"), "running download survives");
        assert!(!map.contains_key("done"), "completed download evicted");
    }

    #[test]
    fn evict_completed_empty_map_no_op() {
        let mut map: HashMap<String, ManagedDownload> = HashMap::new();
        let evicted = evict_completed_test(&mut map, 100);
        assert_eq!(evicted, 0);
        assert!(map.is_empty());
    }

    #[test]
    fn evict_completed_fewer_terminal_than_excess_evicts_all_terminal() {
        let mut map: HashMap<String, ManagedDownload> = HashMap::new();
        // 6 total, limit=1, excess=5, but only 2 terminal → evict both terminal
        map.insert("dl1".into(), make_managed("dl1", DownloadState::Downloading, 100));
        map.insert("dl2".into(), make_managed("dl2", DownloadState::Downloading, 200));
        map.insert("dl3".into(), make_managed("dl3", DownloadState::Paused, 300));
        map.insert("dl4".into(), make_managed("dl4", DownloadState::Queued, 400));
        map.insert("done1".into(), make_managed("done1", DownloadState::Completed, 500));
        map.insert("done2".into(), make_managed("done2", DownloadState::Failed, 600));

        let evicted = evict_completed_test(&mut map, 1);

        assert_eq!(evicted, 2);
        assert_eq!(map.len(), 4);
        // All active survive
        assert!(map.contains_key("dl1"));
        assert!(map.contains_key("dl2"));
        assert!(map.contains_key("dl3"));
        assert!(map.contains_key("dl4"));
        // Both terminal evicted
        assert!(!map.contains_key("done1"));
        assert!(!map.contains_key("done2"));
    }

    // ── is_terminal + is_network_error combined with eviction ────────

    #[test]
    fn evict_completed_canceled_is_terminal() {
        let mut map: HashMap<String, ManagedDownload> = HashMap::new();
        map.insert("a".into(), make_managed("a", DownloadState::Canceled, 100));
        map.insert("b".into(), make_managed("b", DownloadState::Completed, 200));

        let evicted = evict_completed_test(&mut map, 1);

        assert_eq!(evicted, 1);
        assert_eq!(map.len(), 1);
        // Canceled is older → evicted
        assert!(!map.contains_key("a"));
        assert!(map.contains_key("b"));
    }
}
