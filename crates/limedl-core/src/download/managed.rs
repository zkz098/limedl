//! Per-task download state: `DownloadCore` (snapshot + manifest) and
//! `ManagedDownload` (the shared, lockable handle), plus the run-outcome enums
//! and progress/cancellation helpers used by the HTTP executor and lifecycle.

use std::sync::Arc;

use parking_lot::{Mutex, MutexGuard};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::aimd::AimdState;
use crate::lock;
use crate::manifest::Manifest;
use crate::now_ms;
use crate::speed_tracker::SpeedTracker;
use crate::types::{ChunkInfo, DownloadSnapshot, DownloadState};

/// Merged core of snapshot + manifest, protected by a single Mutex.
/// This eliminates double-lock ordering in hot paths like record_progress().
pub struct DownloadCore {
    pub snapshot: DownloadSnapshot,
    pub manifest: Manifest,
    pub speed_tracker: SpeedTracker,
}
impl DownloadCore {
    pub fn new(snapshot: DownloadSnapshot, manifest: Manifest) -> Self {
        Self {
            snapshot,
            manifest,
            speed_tracker: SpeedTracker::default(),
        }
    }

    /// Sync snapshot fields from manifest.
    /// NOTE: mirror_url is intentionally NOT synced — managed by mirror retry loop.
    pub fn sync_snapshot_from_manifest(&mut self) {
        let m = &self.manifest;
        self.snapshot.state = m.state;
        self.snapshot.final_url = m.final_url.clone();
        self.snapshot.file_name = m.file_name.clone();
        self.snapshot.destination_path = m.destination_path.clone();
        self.snapshot.total_bytes = m.total_bytes;
        self.snapshot.downloaded_bytes = m.downloaded_bytes;
        self.snapshot.supports_ranges = m.supports_ranges;
        self.snapshot.connection_count = m.connection_count;
        self.snapshot.thread_mode = m.thread_mode;
        self.snapshot.requested_thread_count = m.requested_thread_count;
        self.snapshot.desired_thread_count = m.desired_thread_count;
        self.snapshot.allocated_thread_count = m.allocated_thread_count;
        self.snapshot.adaptive_profile = m.adaptive_profile_snapshot;
        self.snapshot.thread_note = m.thread_note.clone();
        self.snapshot.etag = m.etag.clone();
        self.snapshot.last_modified = m.last_modified.clone();
        self.snapshot.error = m.error.clone();
        self.snapshot.updated_at_ms = m.updated_at_ms;
        // COW optimization: only rebuild Vec<ChunkInfo> when chunk structure
        // (count + offset boundaries) changes; otherwise update state fields
        // in-place to avoid per-tick allocation churn.
        // Guard: skip the fast path when snapshot has no chunks yet (initial state);
        // both empty or structure-mismatch → full rebuild.
        if !self.snapshot.chunks.is_empty()
            && self.snapshot.chunks.len() == m.chunks.len()
            && self
                .snapshot
                .chunks
                .iter()
                .zip(m.chunks.iter())
                .all(|(sc, mc)| sc.index == mc.index && sc.start == mc.start && sc.end == mc.end)
        {
            // Structure unchanged — update only state fields in-place
            for (sc, mc) in self.snapshot.chunks.iter_mut().zip(m.chunks.iter()) {
                sc.downloaded = mc.downloaded;
                sc.completed = mc.completed;
                sc.claimed_by = mc.claimed_by;
            }
        } else {
            // Structure changed or empty — full rebuild
            self.snapshot.chunks = m
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
        }
    }
}
pub struct ManagedDownload {
    pub core: Mutex<DownloadCore>,
    pub runtime: Mutex<Option<CancellationToken>>,
    pub aimd: Mutex<AimdState>,
    pub stop_notify: Notify,
}
impl ManagedDownload {
    pub fn lock_core(&self) -> MutexGuard<'_, DownloadCore> {
        lock(&self.core)
    }

    pub fn lock_runtime(&self) -> MutexGuard<'_, Option<CancellationToken>> {
        self.runtime.lock()
    }

    pub fn lock_aimd(&self) -> MutexGuard<'_, AimdState> {
        self.aimd.lock()
    }
}
#[derive(Debug)]
pub(crate) enum RunOutcome {
    Finished,
    Paused,
    Canceled,
}
#[derive(Debug)]
pub(crate) enum ChunkWorkerOutcome {
    Finished,
    RestartSingle,
    DowngradeSingleThread,
    Paused,
    Canceled,
}
/// Result of `wait_until_active()` — used by HttpExecutor.
#[derive(Debug)]
pub(crate) enum WaitState {
    Running,
    Paused,
    Canceled,
}
pub(crate) fn sync_snapshot_with_manifest(core: &mut DownloadCore) {
    core.sync_snapshot_from_manifest();
}
/// Records download progress directly on a [`ManagedDownload`].
///
/// # Design note (duplication with [`TaskLifecycle::record_progress`])
/// This free helper exists because chunk workers in [`http_executor`] hold
/// only an `&Arc<ManagedDownload>` reference and do **not** have access to
/// a `&DownloadManager` to call through `TaskLifecycle::record_progress`.
///
/// The body is **identical** to [`TaskLifecycle::record_progress`] in
/// `task_lifecycle.rs`. If you modify one, you **must** update the other.
pub(crate) fn record_progress_on_managed(
    managed: &Arc<ManagedDownload>,
    chunk_index: Option<usize>,
    bytes: u64,
) {
    let now = now_ms();
    let mut core = managed.lock_core();
    core.speed_tracker
        .record_bytes(bytes, std::time::Instant::now());
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
pub(crate) fn cancellation_outcome(managed: &Arc<ManagedDownload>) -> RunOutcome {
    match managed.lock_core().snapshot.state {
        DownloadState::Canceled => RunOutcome::Canceled,
        _ => RunOutcome::Paused,
    }
}
pub(crate) fn cancellation_chunk_outcome(managed: &Arc<ManagedDownload>) -> ChunkWorkerOutcome {
    match managed.lock_core().snapshot.state {
        DownloadState::Canceled => ChunkWorkerOutcome::Canceled,
        _ => ChunkWorkerOutcome::Paused,
    }
}
