//! Per-task download state: `DownloadCore` (manifest + runtime counters) and
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
use crate::types::{DiskType, DownloadSnapshot, DownloadState};

/// Per-task mutable state protected by a single Mutex.
///
/// [`Manifest`] is the single source of truth for task metadata and chunk
/// progress; [`DownloadSnapshot`] is derived on demand via [`Self::snapshot`]
/// (or [`crate::task_lifecycle::TaskLifecycle::build_snapshot`] when live
/// throughput / ETA from [`ManagedDownload::aimd`] is needed).
pub struct DownloadCore {
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
    /// Runtime-only flag: `true` when HDD buffered write fell back to direct write.
    pub degraded: bool,
    /// Runtime-only detected disk media type for the active download directory.
    pub disk_type: Option<DiskType>,
    /// Runtime-only flag: `true` while the write buffer is flushing at run completion.
    pub flushing: bool,
}
impl DownloadCore {
    pub fn new(manifest: Manifest) -> Self {
        let durable_bytes = manifest.downloaded_bytes;
        Self {
            manifest,
            speed_tracker: SpeedTracker::default(),
            durable_bytes,
            degraded: false,
            disk_type: None,
            flushing: false,
        }
    }

    /// Derive a point-in-time [`DownloadSnapshot`] from [`Self::manifest`] and
    /// runtime-only flags (`degraded`, `disk_type`, `flushing`).
    pub fn snapshot(&self) -> DownloadSnapshot {
        let mut snapshot = crate::manifest::snapshot_from_manifest(&self.manifest);
        snapshot.degraded = self.degraded;
        snapshot.disk_type = self.disk_type;
        snapshot.flushing = self.flushing;
        snapshot
    }
}
pub struct ManagedDownload {
    pub core: Mutex<DownloadCore>,
    pub runtime: Mutex<Option<CancellationToken>>,
    pub aimd: Mutex<AimdState>,
    pub stop_notify: Notify,
    pub pieces: Mutex<Option<crate::metalink::PieceVerification>>,
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
pub enum RunOutcome {
    Finished,
    Paused,
    Canceled,
}
#[derive(Debug)]
pub enum ChunkWorkerOutcome {
    Finished,
    RestartSingle,
    DowngradeSingleThread,
    Paused,
    Canceled,
}
/// Result of `wait_until_active()` — used by HttpExecutor.
#[derive(Debug)]
pub enum WaitState {
    Running,
    Paused,
    Canceled,
}
/// Records download progress directly on a [`ManagedDownload`].
///
/// This is the single implementation of *received* progress accounting. It is a
/// free function rather than a method because chunk workers in
/// [`http_executor`](crate::http_executor) only hold an
/// `&Arc<ManagedDownload>` and have no handle to the actor; the manager-side
/// entry point [`TaskLifecycle::record_progress`](crate::task_lifecycle::TaskLifecycle::record_progress)
/// delegates here, so the two callers cannot drift apart.
pub fn record_progress_on_managed(
    managed: &Arc<ManagedDownload>,
    chunk_index: Option<usize>,
    bytes: u64,
) {
    let now = now_ms();
    let mut core = managed.lock_core();
    core.speed_tracker
        .record_bytes(bytes, std::time::Instant::now());
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
pub fn record_durable_bytes(managed: &Arc<ManagedDownload>, entries: &[(u64, u64)]) {
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
/// Rewind the received progress to the durable one after a run failed.
///
/// A failed chunked run drops its write buffer without flushing it (see
/// `DownloadBuffer::drop`), but every byte it held was already counted as
/// received — and possibly marked `completed`. A retry starts each chunk at
/// `start + downloaded`, so keeping those counters would make it skip the
/// dropped tail for good and finalization would refuse the resulting hole
/// (`ensure_core_fully_durable`). Clamping to the durable prefix costs a
/// re-download of exactly the missing tail.
///
/// Safe against a late flush: `durable_bytes()` clamps to `downloaded`, so an
/// observer that runs after this can only re-download, never over-report.
pub fn reset_received_to_durable(managed: &Arc<ManagedDownload>) {
    let mut core = managed.lock_core();
    if core.manifest.chunks.is_empty() {
        return;
    }
    let mut received = 0u64;
    for chunk in &mut core.manifest.chunks {
        let durable = chunk.durable_bytes();
        let completed = durable >= chunk.byte_len();
        if chunk.downloaded != durable || chunk.completed != completed {
            chunk.downloaded = durable;
            chunk.completed = completed;
            chunk.dirty = true;
        }
        // The run that owned these claims is gone; a fresh attempt must be able
        // to claim the re-downloadable range again.
        chunk.claimed_by = None;
        received = received.saturating_add(chunk.downloaded);
    }
    core.manifest.downloaded_bytes = received;
    core.manifest.updated_at_ms = now_ms();
}

pub fn cancellation_outcome(managed: &Arc<ManagedDownload>) -> RunOutcome {
    match managed.lock_core().manifest.state {
        DownloadState::Canceled => RunOutcome::Canceled,
        _ => RunOutcome::Paused,
    }
}
pub fn cancellation_chunk_outcome(managed: &Arc<ManagedDownload>) -> ChunkWorkerOutcome {
    match managed.lock_core().manifest.state {
        DownloadState::Canceled => ChunkWorkerOutcome::Canceled,
        _ => ChunkWorkerOutcome::Paused,
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl ManagedDownload {
    /// Test helper: wrap a manifest in a managed download, as `DownloadManager`
    /// does when it creates or loads a task.
    pub fn from_manifest(manifest: Manifest) -> Arc<Self> {
        Arc::new(Self {
            core: Mutex::new(DownloadCore::new(manifest)),
            runtime: Mutex::new(None),
            aimd: Mutex::new(AimdState::initial(None, None)),
            stop_notify: Notify::new(),
            pieces: Mutex::new(None),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ChunkManifest, plan_chunks};
    use proptest::prelude::*;

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
            assert!(
                core.manifest.chunks[0].dirty,
                "durable progress is persisted"
            );
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
        let managed = ManagedDownload::from_manifest(Manifest {
            chunks,
            ..test_manifest()
        });

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

    #[test]
    fn a_failed_run_rewinds_received_progress_to_the_durable_prefix() {
        let managed = ManagedDownload::from_manifest(Manifest {
            chunks: two_chunks(),
            ..test_manifest()
        });
        // Received-complete but never flushed: the state a run leaves behind
        // when its write buffer is dropped on the error path.
        record_progress_on_managed(&managed, Some(0), 100);
        record_progress_on_managed(&managed, Some(1), 40);
        record_durable_bytes(&managed, &[(0, 30)]);
        {
            let mut core = managed.lock_core();
            core.manifest.chunks[0].claimed_by = Some(7);
            core.manifest.chunks[1].claimed_by = Some(8);
        }

        reset_received_to_durable(&managed);

        let core = managed.lock_core();
        assert_eq!(
            core.manifest.chunks[0].downloaded, 30,
            "kept the flushed prefix"
        );
        assert!(
            !core.manifest.chunks[0].completed,
            "a received-complete chunk with a dropped tail is not complete"
        );
        assert_eq!(
            core.manifest.chunks[1].downloaded, 0,
            "unflushed bytes are re-downloaded"
        );
        assert!(
            !core.manifest.chunks[1].completed,
            "a chunk with no durable bytes is not complete"
        );
        assert!(core.manifest.chunks.iter().all(|c| c.claimed_by.is_none()));
        assert_eq!(
            core.manifest.downloaded_bytes, 30,
            "the task-level received counter follows the chunks"
        );
        assert_eq!(core.snapshot().downloaded_bytes, 30);
    }

    #[test]
    fn reconciling_a_fully_durable_run_changes_nothing() {
        let managed = ManagedDownload::from_manifest(Manifest {
            chunks: two_chunks(),
            ..test_manifest()
        });
        record_progress_on_managed(&managed, Some(0), 100);
        record_progress_on_managed(&managed, Some(1), 100);
        record_durable_bytes(&managed, &[(0, 200)]);

        reset_received_to_durable(&managed);

        let core = managed.lock_core();
        assert_eq!(core.manifest.chunks[0].downloaded, 100);
        assert_eq!(core.manifest.chunks[1].downloaded, 100);
        assert!(core.manifest.chunks[0].completed);
        assert!(core.manifest.chunks[1].completed);
        assert_eq!(core.manifest.downloaded_bytes, 200);
    }

    #[test]
    fn reconciling_a_single_stream_download_is_a_no_op() {
        let managed = ManagedDownload::from_manifest(Manifest {
            chunks: Vec::new(),
            ..test_manifest()
        });
        record_progress_on_managed(&managed, None, 50);

        reset_received_to_durable(&managed);

        assert_eq!(managed.lock_core().manifest.downloaded_bytes, 50);
    }

    proptest! {
        /// Received counters are additive per chunk and at task level, and a
        /// chunk flips to `completed` exactly when its received bytes reach its
        /// byte length.
        #[test]
        fn received_progress_accounting_is_additive(
            total in 1u64..=200_000,
            chunk_size in 1_000u64..=50_000,
            steps in prop::collection::vec((0usize..=16, 0u64..=10_000), 1..=24),
        ) {
            let manifest = Manifest {
                total_bytes: Some(total),
                chunk_size,
                chunks: plan_chunks(Some(total), true, chunk_size),
                ..test_manifest()
            };
            let lengths: Vec<u64> = manifest
                .chunks
                .iter()
                .map(ChunkManifest::byte_len)
                .collect();
            let chunk_count = lengths.len();
            prop_assume!(chunk_count > 0);
            let managed = ManagedDownload::from_manifest(manifest);

            let mut expected = vec![0u64; chunk_count];
            let mut expected_total = 0u64;
            for (raw_index, bytes) in &steps {
                let index = raw_index % chunk_count;
                record_progress_on_managed(&managed, Some(index), *bytes);
                expected[index] = expected[index].saturating_add(*bytes);
                expected_total = expected_total.saturating_add(*bytes);

                let core = managed.lock_core();
                prop_assert_eq!(core.manifest.chunks[index].downloaded, expected[index]);
                prop_assert_eq!(
                    core.manifest.chunks[index].completed,
                    expected[index] >= lengths[index]
                );
                prop_assert_eq!(core.manifest.downloaded_bytes, expected_total);
                prop_assert_eq!(core.snapshot().downloaded_bytes, expected_total);
            }
        }

        #[test]
        fn single_stream_progress_only_moves_the_totals(
            total in 0u64..=1_000_000,
            bytes in 0u64..=1_000_000,
        ) {
            let managed = ManagedDownload::from_manifest(Manifest {
                total_bytes: Some(total),
                chunks: Vec::new(),
                ..test_manifest()
            });
            record_progress_on_managed(&managed, None, bytes);
            let core = managed.lock_core();
            prop_assert_eq!(core.manifest.downloaded_bytes, bytes);
            prop_assert_eq!(core.snapshot().downloaded_bytes, bytes);
            prop_assert!(core.manifest.chunks.is_empty());
        }

        /// A flush prefix of `[0, prefix)` is credited to every chunk it
        /// overlaps, the durable total equals the prefix, and a failed-run
        /// rewind moves the received counters back exactly to that prefix.
        #[test]
        fn durable_flush_credits_the_prefix_and_reset_rewinds(
            total in 1u64..=200_000,
            chunk_size in 1_000u64..=50_000,
            prefix_seed in 0u64..=200_000,
            splits in prop::collection::vec(any::<u8>(), 1..=8),
        ) {
            let manifest = Manifest {
                total_bytes: Some(total),
                chunk_size,
                chunks: plan_chunks(Some(total), true, chunk_size),
                ..test_manifest()
            };
            let starts: Vec<u64> = manifest.chunks.iter().map(|chunk| chunk.start).collect();
            let lengths: Vec<u64> = manifest
                .chunks
                .iter()
                .map(ChunkManifest::byte_len)
                .collect();
            let managed = ManagedDownload::from_manifest(manifest);

            // Receive everything first: `durable_bytes()` clamps to received,
            // so this keeps the assertions about the durable view exact.
            for (index, length) in lengths.iter().enumerate() {
                record_progress_on_managed(&managed, Some(index), *length);
            }

            let prefix = prefix_seed.min(total);
            let mut entries = Vec::new();
            let mut cursor = 0u64;
            for split in &splits {
                if cursor >= prefix {
                    break;
                }
                let remaining = prefix - cursor;
                let len = (u64::from(*split) % remaining).saturating_add(1).min(remaining);
                entries.push((cursor, len));
                cursor += len;
            }
            if cursor < prefix {
                entries.push((cursor, prefix - cursor));
            }
            record_durable_bytes(&managed, &entries);

            for index in 0..lengths.len() {
                let expected = prefix
                    .min(starts[index] + lengths[index])
                    .saturating_sub(starts[index]);
                prop_assert_eq!(durable_of(&managed, index), expected);
            }
            prop_assert_eq!(managed.lock_core().durable_bytes, prefix);

            reset_received_to_durable(&managed);
            let core = managed.lock_core();
            let mut received = 0u64;
            for index in 0..lengths.len() {
                let expected = prefix
                    .min(starts[index] + lengths[index])
                    .saturating_sub(starts[index]);
                prop_assert_eq!(core.manifest.chunks[index].downloaded, expected);
                prop_assert_eq!(
                    core.manifest.chunks[index].completed,
                    expected == lengths[index]
                );
                prop_assert!(core.manifest.chunks[index].claimed_by.is_none());
                received = received.saturating_add(expected);
            }
            prop_assert_eq!(core.manifest.downloaded_bytes, received);
            prop_assert_eq!(core.snapshot().downloaded_bytes, received);
        }
    }
}
