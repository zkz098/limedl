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
    /// Task-level bytes that provably reached the file.
    ///
    /// `manifest.downloaded_bytes` is the *received* counter (what the UI and the
    /// scheduler follow); this is the durable one and is what the database stores.
    /// It is seeded from the loaded manifest, because a manifest read back from
    /// SQLite is by definition durable state. Advanced only through
    /// [`record_durable_bytes`].
    pub durable_bytes: u64,
}
impl DownloadCore {
    pub fn new(snapshot: DownloadSnapshot, manifest: Manifest) -> Self {
        let durable_bytes = manifest.downloaded_bytes;
        Self {
            snapshot,
            manifest,
            speed_tracker: SpeedTracker::default(),
            durable_bytes,
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
        if chunk.downloaded > chunk.end.saturating_sub(chunk.start) {
            chunk.completed = true;
            chunk.claimed_by = None;
        }
        // Deliberately no `dirty` flag here: this counter is the *received* one.
        // The persisted row follows [`record_durable_bytes`], which runs once a
        // flush has actually put the bytes on the file.
    }
}

/// Advance the durable view of a task after bytes reached the file.
///
/// Called with the `(offset, len)` pairs a write-buffer flush just wrote, and
/// directly by the paths that write without a buffer. It is the only writer of
/// `DownloadCore::durable_bytes` / `ChunkManifest::durable_downloaded`, which are
/// the numbers the database stores: keeping them behind the file is what stops a
/// hard kill from leaving the next resume to start past data that never landed.
///
/// The pwrite boundary is the target: `write_all_at` returning is enough. An
/// fsync per flush would be needed to survive a power loss, and that is a
/// deliberate trade-off (see `docs/` troubleshooting); the HDD path already
/// syncs periodically through `SyncMode::Adaptive`.
pub(crate) fn record_durable_bytes(managed: &Arc<ManagedDownload>, entries: &[(u64, u64)]) {
    if entries.is_empty() {
        return;
    }
    let flushed: u64 = entries.iter().map(|(_, len)| *len).sum();
    let mut core = managed.lock_core();
    core.durable_bytes = core.durable_bytes.saturating_add(flushed);

    for (offset, len) in entries {
        let end = offset.saturating_add(*len); // exclusive
        for chunk in &mut core.manifest.chunks {
            if chunk.end < *offset || chunk.start >= end {
                continue;
            }
            // A coalesced write can span a chunk boundary, so every overlapping
            // chunk is credited with its own slice.
            let covered_end = end.min(chunk.end.saturating_add(1));
            let durable = covered_end.saturating_sub(chunk.start);
            if durable > chunk.durable_downloaded {
                chunk.durable_downloaded = durable;
                chunk.dirty = true;
            }
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

#[cfg(test)]
impl ManagedDownload {
    /// Test helper: wrap a manifest in a managed download with a matching
    /// snapshot, as `DownloadManager` does when it creates or loads a task.
    pub(crate) fn from_manifest(manifest: Manifest) -> Arc<Self> {
        let snapshot = crate::manifest::snapshot_from_manifest(&manifest);
        Arc::new(Self {
            core: Mutex::new(DownloadCore::new(snapshot, manifest)),
            runtime: Mutex::new(None),
            aimd: Mutex::new(AimdState::initial(None, None)),
            stop_notify: Notify::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ChunkManifest, plan_chunks};

    /// A minimal valid manifest; tests override the fields they care about with
    /// struct-update syntax.
    fn test_manifest() -> Manifest {
        Manifest {
            id: "durable-test".to_string(),
            url: "https://example.com/file.bin".to_string(),
            final_url: "https://example.com/file.bin".to_string(),
            user_agent: crate::types::default_http_user_agent(),
            extra_headers: Vec::new(),
            destination_dir: "/tmp".to_string(),
            file_name: "file.bin".to_string(),
            file_name_locked: true,
            destination_path: "/tmp/file.bin".to_string(),
            temp_path: "/tmp/file.bin.part".to_string(),
            total_bytes: Some(200),
            downloaded_bytes: 0,
            supports_ranges: true,
            chunk_size: 100,
            connection_count: 0,
            thread_mode: crate::types::ThreadMode::Adaptive,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile_snapshot: None,
            thread_note: None,
            etag: None,
            last_modified: None,
            state: crate::types::DownloadState::Downloading,
            cdn_accelerated: false,
            cdn_node_ip: None,
            checksum_mode: crate::types::ChecksumMode::None,
            checksum: None,
            expected_checksum: None,
            error: None,
            priority: crate::types::Priority::Normal,
            created_at_ms: 1000,
            updated_at_ms: 1000,
            mirror_url: None,
            mirror_urls: Vec::new(),
            current_mirror_index: 0,
            chunks: two_chunks(),
        }
    }

    /// Two 100-byte chunks covering `[0, 200)`.
    fn two_chunks() -> Vec<ChunkManifest> {
        plan_chunks(Some(200), true, 100)
    }

    fn durable_of(managed: &Arc<ManagedDownload>, index: usize) -> u64 {
        managed.lock_core().manifest.chunks[index].durable_bytes()
    }

    #[test]
    fn received_progress_does_not_make_a_chunk_durable() {
        let managed = ManagedDownload::from_manifest(Manifest {
            chunks: two_chunks(),
            ..test_manifest()
        });

        record_progress_on_managed(&managed, Some(0), 100);

        let core = managed.lock_core();
        assert_eq!(core.manifest.chunks[0].downloaded, 100, "received");
        assert!(core.manifest.chunks[0].completed, "received-complete");
        assert_eq!(core.manifest.downloaded_bytes, 100, "received total");
        assert_eq!(
            core.manifest.chunks[0].durable_bytes(),
            0,
            "nothing has been flushed yet"
        );
        assert_eq!(core.durable_bytes, 0);
    }

    #[test]
    fn flushed_ranges_advance_the_durable_counters() {
        let managed = ManagedDownload::from_manifest(Manifest {
            chunks: two_chunks(),
            ..test_manifest()
        });
        record_progress_on_managed(&managed, Some(0), 100);

        record_durable_bytes(&managed, &[(0, 40)]);
        assert_eq!(durable_of(&managed, 0), 40);
        {
            let core = managed.lock_core();
            assert_eq!(core.durable_bytes, 40);
            assert!(core.manifest.chunks[0].dirty, "durable progress is persisted");
        }

        // A second flush resumes from where the first stopped.
        record_durable_bytes(&managed, &[(40, 60)]);
        assert_eq!(durable_of(&managed, 0), 100);
        assert!(managed.lock_core().manifest.chunks[0].durable_complete());
        assert_eq!(managed.lock_core().durable_bytes, 100);
    }

    #[test]
    fn a_coalesced_write_is_credited_to_every_chunk_it_overlaps() {
        let managed = ManagedDownload::from_manifest(Manifest {
            chunks: two_chunks(),
            ..test_manifest()
        });
        // Both chunks fully received, then one vectored write spanning the
        // boundary at offset 100.
        record_progress_on_managed(&managed, Some(0), 100);
        record_progress_on_managed(&managed, Some(1), 100);

        record_durable_bytes(&managed, &[(90, 20)]);

        assert_eq!(durable_of(&managed, 0), 100);
        assert_eq!(durable_of(&managed, 1), 10);
        assert_eq!(managed.lock_core().durable_bytes, 20);
    }

    #[test]
    fn durable_bytes_never_exceeds_received_bytes() {
        let mut chunks = two_chunks();
        // A stale/over-eager counter (a reset path that forgot to clear it, a
        // chunk re-planned on top of an old one).
        chunks[0].durable_downloaded = 10_000;
        chunks[0].downloaded = 30;
        let managed =
            ManagedDownload::from_manifest(Manifest { chunks, ..test_manifest() });

        assert_eq!(durable_of(&managed, 0), 30, "clamped to what was received");
        assert!(!managed.lock_core().manifest.chunks[0].durable_complete());
    }

    #[test]
    fn a_loaded_manifest_starts_fully_durable() {
        // A manifest read back from SQLite carries both counters (the row loader
        // sets `durable_downloaded` from the stored value), so the durable view
        // starts equal to the received one.
        let mut chunks = two_chunks();
        chunks[0].downloaded = 100;
        chunks[0].completed = true;
        chunks[0].durable_downloaded = 100;
        let managed = ManagedDownload::from_manifest(Manifest {
            chunks,
            downloaded_bytes: 100,
            ..test_manifest()
        });

        let core = managed.lock_core();
        assert_eq!(core.durable_bytes, 100);
        assert_eq!(core.manifest.chunks[0].durable_bytes(), 100);
        assert!(core.manifest.chunks[0].durable_complete());
    }
}
