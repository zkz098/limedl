//! Work-stealing and batch-clamping synchronization helpers + Loom verification.
//!
//! Isolates the byte-range clamping logic used when flushing `ChunkWorkerCtx`'s
//! 128 KiB micro-batch buffer against concurrent `steal_chunk_with_min_size`
//! splits, plus an exhaustive Loom model verifying disjoint, complete coverage.

#[cfg(test)]
use super::WORK_STEAL_MIN_SPLIT_SIZE;
use crate::manifest::{ChunkManifest, Manifest};
use crate::now_ms;

/// Claim the next unclaimed incomplete chunk, preferring `worker_id`'s stripe.
pub(super) fn claim_next_chunk(
    manifest: &mut Manifest,
    worker_id: usize,
    total_workers: usize,
) -> Option<ChunkManifest> {
    let stripe_count = total_workers.max(1);
    let stripe = worker_id % stripe_count;
    // Try interleaved: chunks whose index matches the worker's stripe
    if let Some(chunk) = manifest
        .chunks
        .iter_mut()
        .find(|c| !c.completed && c.claimed_by.is_none() && c.index % stripe_count == stripe)
    {
        chunk.claimed_by = Some(worker_id);
        chunk.dirty = true;
        return Some(chunk.clone());
    }
    // Fallback: any unclaimed chunk (handles cases where stripe is exhausted)
    let chunk = manifest
        .chunks
        .iter_mut()
        .find(|chunk| !chunk.completed && chunk.claimed_by.is_none())?;
    chunk.claimed_by = Some(worker_id);
    chunk.dirty = true;
    Some(chunk.clone())
}

/// Dynamically steal work from the active chunk with the largest remaining un-downloaded range.
pub(super) fn steal_chunk_with_min_size(
    manifest: &mut Manifest,
    worker_id: usize,
    min_split_size: u64,
) -> Option<ChunkManifest> {
    let mut best_chunk_idx: Option<usize> = None;
    let mut max_remaining: u64 = 0;

    for (idx, chunk) in manifest.chunks.iter().enumerate() {
        if !chunk.completed && chunk.claimed_by.is_some() {
            let current_pos = chunk.start.saturating_add(chunk.downloaded);
            if chunk.end > current_pos {
                let remaining = chunk.end - current_pos;
                if remaining >= min_split_size && remaining > max_remaining {
                    max_remaining = remaining;
                    best_chunk_idx = Some(idx);
                }
            }
        }
    }

    let target_idx = best_chunk_idx?;
    let target_chunk = manifest.chunks.get_mut(target_idx)?;
    let current_pos = target_chunk.start.saturating_add(target_chunk.downloaded);
    let original_end = target_chunk.end;
    let remaining = original_end.saturating_sub(current_pos);
    if remaining < min_split_size {
        return None;
    }
    let half = remaining / 2;
    let mid = current_pos + half;

    target_chunk.end = mid;
    target_chunk.dirty = true;

    let new_index = manifest.chunks.len();
    let stolen_chunk = ChunkManifest {
        index: new_index,
        start: mid + 1,
        end: original_end,
        downloaded: 0,
        completed: false,
        durable_downloaded: 0,
        claimed_by: Some(worker_id),
        dirty: true,
    };
    manifest.chunks.push(stolen_chunk.clone());
    manifest.updated_at_ms = now_ms();

    tracing::info!(
        "Work stealing: worker {worker_id} stole bytes {}-{} from chunk {target_idx} (new chunk {new_index})",
        mid + 1,
        original_end
    );

    Some(stolen_chunk)
}

/// Dynamically steal work from the active chunk using the default minimum split size.
#[cfg(test)]
pub(super) fn steal_chunk(manifest: &mut Manifest, worker_id: usize) -> Option<ChunkManifest> {
    steal_chunk_with_min_size(manifest, worker_id, WORK_STEAL_MIN_SPLIT_SIZE)
}

/// Attempt to claim an unclaimed chunk, or dynamically steal work from an active chunk.
pub(super) fn claim_or_steal_chunk_with_min_size(
    manifest: &mut Manifest,
    worker_id: usize,
    total_workers: usize,
    min_split_size: u64,
) -> Option<ChunkManifest> {
    if let Some(chunk) = claim_next_chunk(manifest, worker_id, total_workers) {
        return Some(chunk);
    }
    steal_chunk_with_min_size(manifest, worker_id, min_split_size)
}

/// Attempt to claim an unclaimed chunk, or dynamically steal work from an active chunk using the default split size.
#[cfg(test)]
pub(super) fn claim_or_steal_chunk(
    manifest: &mut Manifest,
    worker_id: usize,
    total_workers: usize,
) -> Option<ChunkManifest> {
    claim_or_steal_chunk_with_min_size(
        manifest,
        worker_id,
        total_workers,
        WORK_STEAL_MIN_SPLIT_SIZE,
    )
}

/// Result of clamping and flushing a worker's pending micro-batch buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FlushOutcome {
    /// Bytes flushed and credited (may be 0 when buffer was empty or `chunk.end` shrank below `batch_start`).
    Written { flushed: u64, clamped: bool },
    /// Another worker took over the chunk claim (`tail sprint` / release).
    LostClaim,
}

/// Clamp a pending flush buffer `[batch_start .. batch_start + batch_len)` against
/// `chunk_idx`'s current claim and dynamic `end` boundary.
///
/// Returns `None` if `worker_id` no longer owns `chunk_idx`, or `Some(allowed_len)`
/// where `allowed_len <= batch_len` bytes fall inside `[batch_start ..= chunk.end]`.
pub(super) fn claim_clamped_batch_len(
    manifest: &Manifest,
    chunk_idx: usize,
    worker_id: usize,
    batch_start: u64,
    batch_len: usize,
) -> Option<usize> {
    let chunk = manifest.chunks.get(chunk_idx)?;
    if chunk.claimed_by != Some(worker_id) {
        return None;
    }
    if batch_start > chunk.end || batch_len == 0 {
        return Some(0);
    }
    let remaining = (chunk.end - batch_start).saturating_add(1);
    Some((batch_len as u64).min(remaining) as usize)
}

#[cfg(all(test, limedl_loom))]
mod loom_tests {
    use super::*;
    use crate::http_executor::worker::mark_chunk_released;
    use crate::manifest::{ChunkManifest, Manifest};
    use crate::types::{ChecksumMode, DownloadState, Priority, ThreadMode};
    use loom::sync::{Arc, Mutex};

    struct SimState {
        manifest: Manifest,
        written_counts: Vec<u32>,
        credited_bytes: u64,
    }

    fn make_single_chunk_manifest(total_bytes: u64) -> Manifest {
        Manifest {
            id: "http:loom-steal".to_string(),
            url: "https://example.com/file.bin".to_string(),
            final_url: "https://example.com/file.bin".to_string(),
            user_agent: "limedl".to_string(),
            extra_headers: Vec::new(),
            destination_dir: "/tmp".to_string(),
            file_name: "file.bin".to_string(),
            file_name_locked: false,
            destination_path: "/tmp/file.bin".to_string(),
            temp_path: "/tmp/file.bin.part".to_string(),
            total_bytes: Some(total_bytes),
            downloaded_bytes: 0,
            supports_ranges: true,
            chunk_size: total_bytes,
            connection_count: 2,
            thread_mode: ThreadMode::Adaptive,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: Some(2),
            adaptive_profile_snapshot: None,
            thread_note: None,
            etag: None,
            last_modified: None,
            state: DownloadState::Downloading,
            cdn_accelerated: false,
            cdn_node_ip: None,
            checksum_mode: ChecksumMode::None,
            checksum: None,
            expected_checksum: None,
            error: None,
            created_at_ms: 0,
            updated_at_ms: 0,
            chunks: vec![ChunkManifest {
                index: 0,
                start: 0,
                end: total_bytes - 1,
                downloaded: 0,
                completed: false,
                durable_downloaded: 0,
                claimed_by: None,
                dirty: false,
            }],
            mirror_url: None,
            mirror_urls: Vec::new(),
            current_mirror_index: 0,
            priority: Priority::Normal,
        }
    }

    fn commit_clamped_batch(
        state: &Arc<Mutex<SimState>>,
        chunk_idx: usize,
        worker_id: usize,
        batch_start: u64,
        batch_len: usize,
    ) -> Option<u64> {
        let allowed = {
            let guard = state.lock().expect("loom mutex poisoned");
            claim_clamped_batch_len(
                &guard.manifest,
                chunk_idx,
                worker_id,
                batch_start,
                batch_len,
            )?
        };

        // Simulate async `write_bytes` outside `lock_core()`.
        let post_allowed = {
            let mut guard = state.lock().expect("loom mutex poisoned");
            let post_allowed = claim_clamped_batch_len(
                &guard.manifest,
                chunk_idx,
                worker_id,
                batch_start,
                allowed,
            )?;
            for count in
                &mut guard.written_counts[batch_start as usize..batch_start as usize + post_allowed]
            {
                *count += 1;
            }
            guard.credited_bytes += post_allowed as u64;
            let chunk = &mut guard.manifest.chunks[chunk_idx];
            chunk.downloaded += post_allowed as u64;
            if chunk.start + chunk.downloaded > chunk.end {
                chunk.completed = true;
                chunk.claimed_by = None;
            }
            let new_pos = chunk.start + chunk.downloaded;
            drop(guard);
            let _ = &mark_chunk_released;
            new_pos
        };
        Some(post_allowed)
    }

    #[test]
    fn work_steal_and_clamped_flush_preserve_disjoint_complete_coverage() {
        loom::model(|| {
            let total_bytes = 16u64;
            let state = Arc::new(Mutex::new(SimState {
                manifest: make_single_chunk_manifest(total_bytes),
                written_counts: vec![0; total_bytes as usize],
                credited_bytes: 0,
            }));

            let s0 = Arc::clone(&state);
            let t0 = loom::thread::spawn(move || {
                let Some(chunk) = ({
                    let mut guard = s0.lock().expect("loom mutex poisoned");
                    claim_or_steal_chunk_with_min_size(&mut guard.manifest, 0, 2, 4)
                }) else {
                    return;
                };
                // Worker 0 buffers 10 bytes [0..10) locally before releasing lock and flushing.
                let mut pos = chunk.start;
                let mut batch_len = 10usize;
                while let Some(next_pos) = commit_clamped_batch(&s0, chunk.index, 0, pos, batch_len)
                {
                    pos = next_pos;
                    let guard = s0.lock().expect("loom mutex poisoned");
                    let c = &guard.manifest.chunks[chunk.index];
                    if c.completed || pos > c.end {
                        break;
                    }
                    batch_len = (c.end - pos + 1) as usize;
                }
            });

            let s1 = Arc::clone(&state);
            let t1 = loom::thread::spawn(move || {
                let Some(chunk) = ({
                    let mut guard = s1.lock().expect("loom mutex poisoned");
                    claim_or_steal_chunk_with_min_size(&mut guard.manifest, 1, 2, 4)
                }) else {
                    return;
                };
                let len = (chunk.end - chunk.start + 1) as usize;
                let _ = commit_clamped_batch(&s1, chunk.index, 1, chunk.start, len);
            });

            t0.join().unwrap();
            t1.join().unwrap();

            let guard = state.lock().expect("loom mutex poisoned");
            assert_eq!(guard.credited_bytes, total_bytes);
            assert!(
                guard.written_counts.iter().all(|&count| count == 1),
                "every byte must be committed cleanly once without overlap or loss: {:?}",
                guard.written_counts
            );
            for chunk in &guard.manifest.chunks {
                assert!(chunk.completed, "chunk {} remained incomplete", chunk.index);
                assert_eq!(
                    chunk.downloaded,
                    chunk.end - chunk.start + 1,
                    "chunk {} downloaded overshot or undershot its bounds",
                    chunk.index
                );
            }
        });
    }

    #[test]
    fn mid_write_steal_clamps_progress_before_overshoot() {
        loom::model(|| {
            let total_bytes = 16u64;
            let state = Arc::new(Mutex::new(SimState {
                manifest: make_single_chunk_manifest(total_bytes),
                written_counts: vec![0; total_bytes as usize],
                credited_bytes: 0,
            }));

            // Worker 0 starts owning chunk 0 [0..=15].
            {
                let mut guard = state.lock().expect("loom mutex poisoned");
                claim_or_steal_chunk_with_min_size(&mut guard.manifest, 0, 2, 4)
                    .expect("worker 0 claims initial chunk");
            }

            let s0 = Arc::clone(&state);
            let t0 = loom::thread::spawn(move || {
                // Worker 0 holds an 8-byte batch [0..8) followed by remaining bytes.
                let pos = commit_clamped_batch(&s0, 0, 0, 0, 12).expect("still owns chunk 0");
                let guard = s0.lock().expect("loom mutex poisoned");
                let c = &guard.manifest.chunks[0];
                if !c.completed && pos <= c.end {
                    let rem = (c.end - pos + 1) as usize;
                    drop(guard);
                    let _ = commit_clamped_batch(&s0, 0, 0, pos, rem);
                }
            });

            let s1 = Arc::clone(&state);
            let t1 = loom::thread::spawn(move || {
                if let Some(stolen) = {
                    let mut guard = s1.lock().expect("loom mutex poisoned");
                    claim_or_steal_chunk_with_min_size(&mut guard.manifest, 1, 2, 4)
                } {
                    let len = (stolen.end - stolen.start + 1) as usize;
                    let _ = commit_clamped_batch(&s1, stolen.index, 1, stolen.start, len);
                }
            });

            t0.join().unwrap();
            t1.join().unwrap();

            let guard = state.lock().expect("loom mutex poisoned");
            assert_eq!(guard.credited_bytes, total_bytes);
            assert!(guard.written_counts.iter().all(|&count| count == 1));
            for chunk in &guard.manifest.chunks {
                assert_eq!(chunk.downloaded, chunk.end - chunk.start + 1);
            }
        });
    }
}
