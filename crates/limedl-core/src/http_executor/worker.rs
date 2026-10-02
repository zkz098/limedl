//! Chunk claim/steal bookkeeping and the per-chunk worker task.

use futures_util::StreamExt;

use super::{Arc, CancellationToken, ChunkManifest, ChunkWorkerOutcome, Client, Database, DiskType, DownloadBuffer, DownloadError, DownloadManager, DownloadState, Duration, Instant, JoinSet, ManagedDownload, PERSIST_INTERVAL, RateLimiter, Result, StatusCode, WORK_STEAL_MIN_SPLIT_SIZE, build_segment_request, cancellation_chunk_outcome, if_range_header, is_too_many_requests_error, now_ms, persist_manifest_snapshot, record_progress_on_managed, request_with_retry, validate_segment_response, write_all_at};

// ── Free helper functions ─────────────────────────────────────────────────────

pub(super) fn current_allocation(managed: &Arc<ManagedDownload>) -> usize {
    managed
        .lock_core()
        .manifest
        .allocated_thread_count
        .unwrap_or(0)
}

pub(super) fn all_chunks_completed(managed: &Arc<ManagedDownload>) -> bool {
    managed
        .lock_core()
        .manifest
        .chunks
        .iter()
        .all(|chunk| chunk.completed)
}

pub(super) fn claim_next_chunk(
    manifest: &mut crate::manifest::Manifest,
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
pub(super) fn steal_chunk(
    manifest: &mut crate::manifest::Manifest,
    worker_id: usize,
) -> Option<ChunkManifest> {
    let mut best_chunk_idx: Option<usize> = None;
    let mut max_remaining: u64 = 0;

    for (idx, chunk) in manifest.chunks.iter().enumerate() {
        if !chunk.completed && chunk.claimed_by.is_some() {
            let current_pos = chunk.start.saturating_add(chunk.downloaded);
            if chunk.end > current_pos {
                let remaining = chunk.end - current_pos;
                if remaining >= WORK_STEAL_MIN_SPLIT_SIZE && remaining > max_remaining {
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
    if remaining < WORK_STEAL_MIN_SPLIT_SIZE {
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

/// Attempt to claim an unclaimed chunk, or dynamically steal work from an active chunk.
pub(super) fn claim_or_steal_chunk(
    manifest: &mut crate::manifest::Manifest,
    worker_id: usize,
    total_workers: usize,
) -> Option<ChunkManifest> {
    if let Some(chunk) = claim_next_chunk(manifest, worker_id, total_workers) {
        return Some(chunk);
    }
    steal_chunk(manifest, worker_id)
}

pub(crate) fn mark_chunk_released(
    managed: &Arc<ManagedDownload>,
    chunk_index: usize,
    worker_id: usize,
) {
    let mut core = managed.lock_core();
    if let Some(chunk) = core.manifest.chunks.get_mut(chunk_index) {
        // Only clear the claim if it still belongs to this worker.
        // If tail sprint released it and a new worker claimed it,
        // we must not clear the new worker's claim.
        if chunk.claimed_by == Some(worker_id) {
            chunk.claimed_by = None;
            chunk.dirty = true;
        }
    }
}

pub(super) async fn shutdown_chunk_workers(
    managed: &Arc<ManagedDownload>,
    workers: &mut JoinSet<Result<ChunkWorkerOutcome>>,
) {
    workers.abort_all();
    while workers.join_next().await.is_some() {}
    release_all_chunk_claims(managed);
}

pub(super) fn release_all_chunk_claims(managed: &Arc<ManagedDownload>) {
    let mut core = managed.lock_core();
    for chunk in &mut core.manifest.chunks {
        chunk.claimed_by = None;
        chunk.dirty = true;
    }
    core.manifest.connection_count = 0;
    core.manifest.updated_at_ms = now_ms();
}

pub(super) fn finalize_was_canceled(
    managed: &Arc<ManagedDownload>,
    token: &CancellationToken,
) -> bool {
    if token.is_cancelled() {
        return true;
    }
    let core = managed.lock_core();
    core.snapshot.state == DownloadState::Canceled || core.manifest.state == DownloadState::Canceled
}

pub(super) struct ChunkWorkerCtx {
    pub(super) managed: Arc<ManagedDownload>,
    pub(super) client: Client,
    pub(super) token: CancellationToken,
    pub(super) file: Arc<std::fs::File>,
    pub(super) chunk: ChunkManifest,
    pub(super) max_retries: u32,
    pub(super) db: Arc<Database>,
    pub(super) rate_limiter: Arc<RateLimiter>,
    pub(super) manager: Arc<DownloadManager>,
    pub(super) write_buffer: Option<Arc<DownloadBuffer>>,
    pub(super) disk_type: DiskType,
    pub(super) worker_id: usize,
}

pub(super) async fn download_chunk(ctx: ChunkWorkerCtx) -> Result<ChunkWorkerOutcome> {
    let mut current = ctx.chunk.start + ctx.chunk.downloaded;
    let end = ctx.chunk.end;
    if current > end {
        mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
        return Ok(ChunkWorkerOutcome::Finished);
    }

    let mut last_persist = Instant::now();
    // ── progress throttling ──
    let mut last_progress_emit = Instant::now();
    // ── rate limiter batch consume ──
    let mut bytes_since_consume: usize = 0;
    let mut chunks_since_consume: usize = 0;
    while current <= end {
        if ctx.token.is_cancelled() {
            mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
            return Ok(match ctx.managed.lock_core().snapshot.state {
                DownloadState::Canceled => ChunkWorkerOutcome::Canceled,
                _ => ChunkWorkerOutcome::Paused,
            });
        }

        // Check if tail sprint released our chunk claim
        {
            let core = ctx.managed.lock_core();
            if let Some(chunk) = core.manifest.chunks.get(ctx.chunk.index)
                && chunk.claimed_by != Some(ctx.worker_id)
            {
                return Ok(ChunkWorkerOutcome::Finished);
            }
        }

        let (url, user_agent, extra_headers, validator) = {
            let core = ctx.managed.lock_core();
            (
                core.manifest.final_url.clone(),
                core.manifest.user_agent.clone(),
                core.manifest.extra_headers.clone(),
                if_range_header(&core.manifest),
            )
        };

        let response = match request_with_retry(
            || {
                let client = ctx.client.clone();
                let url = url.clone();
                let user_agent = user_agent.clone();
                let extra_headers = extra_headers.clone();
                let validator = validator.clone();
                async move {
                    build_segment_request(
                        &client,
                        &url,
                        &user_agent,
                        &extra_headers,
                        current,
                        end,
                        validator,
                    )
                    .send()
                    .await
                }
            },
            ctx.token.clone(),
            ctx.max_retries,
            ctx.managed.clone(),
        )
        .await
        {
            Ok(resp) => resp,
            Err(err) => {
                mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
                if is_too_many_requests_error(&err) {
                    let concurrency = ctx
                        .managed
                        .lock_core()
                        .manifest
                        .allocated_thread_count
                        .unwrap_or(1);
                    if concurrency > 1 {
                        return Ok(ChunkWorkerOutcome::DowngradeSingleThread);
                    }
                }
                return Err(err);
            }
        };

        if response.status() == StatusCode::OK {
            mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
            return Ok(ChunkWorkerOutcome::RestartSingle);
        }

        validate_segment_response(&response, current, end)?;

        let mut stream = response.bytes_stream();
        while let Some(bytes) = tokio::select! {
            _ = ctx.token.cancelled() => {
                // Flush remaining rate limiter bytes before exiting
                if bytes_since_consume > 0 {
                    ctx.rate_limiter.consume(bytes_since_consume).await;
                }
                mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
                return Ok(cancellation_chunk_outcome(&ctx.managed));
            }
            next = stream.next() => next,
        } {
            let bytes = bytes?;
            // ── batch rate limiter consume ──
            const BATCH_BYTES: usize = 256 * 1024; // 256 KB
            const BATCH_CHUNKS: usize = 8;
            bytes_since_consume += bytes.len();
            chunks_since_consume += 1;
            if bytes_since_consume >= BATCH_BYTES || chunks_since_consume >= BATCH_CHUNKS {
                ctx.rate_limiter.consume(bytes_since_consume).await;
                bytes_since_consume = 0;
                chunks_since_consume = 0;
            }

            // Check dynamic chunk end (which may have been shortened if work was stolen)
            let dynamic_end = {
                let core = ctx.managed.lock_core();
                core.manifest
                    .chunks
                    .get(ctx.chunk.index)
                    .map(|c| c.end)
                    .unwrap_or(end)
            };

            if current > dynamic_end {
                break;
            }

            if current + bytes.len() as u64 - 1 > end {
                mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
                return Err(DownloadError::InvalidResponse(String::from(
                    "segment body exceeded requested range",
                )));
            }

            let (to_write, reached_dynamic_end) = if current + bytes.len() as u64 - 1 > dynamic_end
            {
                let allowed = (dynamic_end.saturating_sub(current) + 1) as usize;
                (bytes.slice(..allowed.min(bytes.len())), true)
            } else {
                (bytes.clone(), false)
            };

            if let Some(ref buf) = ctx.write_buffer {
                if buf.buffer_chunk(current, to_write.clone()).await.is_err() {
                    // Background flush failed — fall back to direct write.
                    write_all_at(&ctx.file, &to_write, current)?;
                    if ctx.disk_type == DiskType::Hdd {
                        let mut core = ctx.managed.lock_core();
                        core.snapshot.degraded = true;
                    }
                }
            } else {
                write_all_at(&ctx.file, &to_write, current)?;
            }

            current += to_write.len() as u64;
            {
                record_progress_on_managed(
                    &ctx.managed,
                    Some(ctx.chunk.index),
                    to_write.len() as u64,
                );
            }

            if reached_dynamic_end {
                break;
            }
            // Check if tail sprint released our chunk claim — exit early to avoid
            // wasting bandwidth competing with a new worker on the same chunk.
            {
                let claimed_by = {
                    let core = ctx.managed.lock_core();
                    core.manifest
                        .chunks
                        .get(ctx.chunk.index)
                        .and_then(|c| c.claimed_by)
                };
                if claimed_by != Some(ctx.worker_id) {
                    // Flush remaining rate limiter bytes before exiting
                    if bytes_since_consume > 0 {
                        ctx.rate_limiter.consume(bytes_since_consume).await;
                    }
                    return Ok(ChunkWorkerOutcome::Finished);
                }
            }
            if last_persist.elapsed() >= PERSIST_INTERVAL {
                persist_manifest_snapshot(&ctx.db, &ctx.managed).await?;
                last_persist = Instant::now();
                // Throttle progress events: at most once per 500ms
                if last_progress_emit.elapsed() >= Duration::from_millis(500) {
                    ctx.manager
                        .task_lifecycle
                        .emit_progress(&ctx.manager, &ctx.managed);
                    last_progress_emit = Instant::now();
                }
            }
        }
    }

    // Flush remaining rate limiter bytes after chunk completes
    if bytes_since_consume > 0 {
        ctx.rate_limiter.consume(bytes_since_consume).await;
    }

    {
        let mut core = ctx.managed.lock_core();
        if let Some(target) = core.manifest.chunks.get_mut(ctx.chunk.index) {
            // Only mark completed if we still own the claim
            if target.claimed_by == Some(ctx.worker_id) {
                target.completed = true;
                target.downloaded = target.end.saturating_sub(target.start) + 1;
                target.claimed_by = None;
                target.dirty = true;
            }
        }
        core.manifest.updated_at_ms = now_ms();
    }
    Ok(ChunkWorkerOutcome::Finished)
}
