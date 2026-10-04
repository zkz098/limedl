//! Chunk claim/steal bookkeeping and the per-chunk worker task.

use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Response;

use super::{Arc, BatchLimiter, CancellationToken, ChunkManifest, ChunkWorkerOutcome, Client, Database, DiskType, DownloadBuffer, DownloadError, DownloadManager, DownloadState, JoinSet, ManagedDownload, ProgressThrottle, RateLimiter, RequestBudget, Result, StatusCode, WORK_STEAL_MIN_SPLIT_SIZE, build_segment_request, cancellation_chunk_outcome, if_range_header, is_too_many_requests_error, now_ms, record_durable_bytes, record_progress_on_managed, request_with_retry, validate_segment_response, write_all_at};

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

/// Why a segment fetch did not produce a usable response.
enum FetchSegmentError {
    /// 429 while multiple threads are allocated — the supervisor downgrades.
    DowngradeSingleThread,
    /// Transport/other error to propagate.
    Fatal(DownloadError),
}

/// How draining one segment body ended.
enum SegmentBody {
    /// The server closed the body (or the dynamic end was reached).
    Exhausted,
    /// The worker must stop with this outcome.
    Stop(ChunkWorkerOutcome),
}

/// Upper bound on the number of separate segment requests one chunk may issue.
///
/// Each request may itself retry up to the configured `max_retries`, so a server
/// that answers with a partial body and immediately closes the connection would
/// otherwise drive [`ChunkWorkerCtx::run`] forever (one byte per request, each
/// with a fresh per-request retry counter). Legitimate multi-response serving
/// needs a handful of requests, never dozens, so the limit turns that loop into a
/// reported failure instead of a silent livelock.
const MAX_SEGMENT_FETCHES_PER_CHUNK: u32 = 24;
impl ChunkWorkerCtx {
    /// Run this chunk to completion, a pause/cancel, or a restart request.
    async fn run(self) -> Result<ChunkWorkerOutcome> {
        let mut current = self.chunk.start + self.chunk.downloaded;
        let end = self.chunk.end;
        if current > end {
            self.release();
            return Ok(ChunkWorkerOutcome::Finished);
        }

        let mut throttle = ProgressThrottle::new();
        let mut batch = BatchLimiter::new();
        let mut budget = RequestBudget::new(MAX_SEGMENT_FETCHES_PER_CHUNK);
        while current <= end {
            if self.token.is_cancelled() {
                return Ok(self.pause_or_cancel());
            }

            // Check if tail sprint released our chunk claim
            if self.claim_released() {
                return Ok(ChunkWorkerOutcome::Finished);
            }

            if let Err(error) = budget.charge(format_args!("chunk {}", self.chunk.index)) {
                self.release();
                return Err(error);
            }

            let response = match self.fetch_segment(current, end).await {
                Ok(response) => response,
                Err(FetchSegmentError::DowngradeSingleThread) => {
                    return Ok(ChunkWorkerOutcome::DowngradeSingleThread);
                }
                Err(FetchSegmentError::Fatal(error)) => return Err(error),
            };

            if response.status() == StatusCode::OK {
                self.release();
                return Ok(ChunkWorkerOutcome::RestartSingle);
            }

            validate_segment_response(&response, current, end)?;

            match self
                .consume_segment(response, &mut current, end, &mut batch, &mut throttle)
                .await?
            {
                SegmentBody::Exhausted => {}
                SegmentBody::Stop(outcome) => return Ok(outcome),
            }
        }

        // Flush remaining rate limiter bytes after chunk completes
        batch.flush(&self.rate_limiter).await;
        self.mark_complete();
        Ok(ChunkWorkerOutcome::Finished)
    }

    /// Fetch the next segment, mapping the downgrade-triggering 429 to its own
    /// error so the caller does not have to inspect the transport error.
    async fn fetch_segment(
        &self,
        current: u64,
        end: u64,
    ) -> std::result::Result<Response, FetchSegmentError> {
        let (url, user_agent, extra_headers, validator) = {
            let core = self.managed.lock_core();
            (
                core.manifest.final_url.clone(),
                core.manifest.user_agent.clone(),
                core.manifest.extra_headers.clone(),
                if_range_header(&core.manifest),
            )
        };

        let response = request_with_retry(
            || {
                let client = self.client.clone();
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
            self.token.clone(),
            self.max_retries,
            self.managed.clone(),
        )
        .await;

        match response {
            Ok(response) => Ok(response),
            Err(error) => {
                self.release();
                if is_too_many_requests_error(&error) {
                    let concurrency = self
                        .managed
                        .lock_core()
                        .manifest
                        .allocated_thread_count
                        .unwrap_or(1);
                    if concurrency > 1 {
                        return Err(FetchSegmentError::DowngradeSingleThread);
                    }
                }
                Err(FetchSegmentError::Fatal(error))
            }
        }
    }

    /// Drain one segment body into the buffer/direct-write path.
    async fn consume_segment(
        &self,
        response: Response,
        current: &mut u64,
        end: u64,
        batch: &mut BatchLimiter,
        throttle: &mut ProgressThrottle,
    ) -> Result<SegmentBody> {
        let mut stream = response.bytes_stream();
        while let Some(bytes) = tokio::select! {
            _ = self.token.cancelled() => {
                // Flush remaining rate limiter bytes before exiting
                batch.flush(&self.rate_limiter).await;
                self.release();
                return Ok(SegmentBody::Stop(cancellation_chunk_outcome(&self.managed)));
            }
            next = stream.next() => next,
        } {
            let bytes = bytes?;
            batch.account(&self.rate_limiter, bytes.len()).await;

            // Check dynamic chunk end (which may have been shortened if work was stolen)
            let dynamic_end = self.dynamic_end(end);
            if *current > dynamic_end {
                break;
            }

            if *current + bytes.len() as u64 - 1 > end {
                self.release();
                return Err(DownloadError::InvalidResponse(String::from(
                    "segment body exceeded requested range",
                )));
            }

            let (to_write, reached_dynamic_end) =
                if *current + bytes.len() as u64 - 1 > dynamic_end {
                    let allowed = (dynamic_end.saturating_sub(*current) + 1) as usize;
                    (bytes.slice(..allowed.min(bytes.len())), true)
                } else {
                    (bytes.clone(), false)
                };

            self.write_bytes(*current, &to_write).await?;
            *current += to_write.len() as u64;
            record_progress_on_managed(
                &self.managed,
                Some(self.chunk.index),
                to_write.len() as u64,
            );

            if reached_dynamic_end {
                break;
            }
            // Check if tail sprint released our chunk claim — exit early to avoid
            // wasting bandwidth competing with a new worker on the same chunk.
            if !self.owns_claim() {
                // Flush remaining rate limiter bytes before exiting
                batch.flush(&self.rate_limiter).await;
                return Ok(SegmentBody::Stop(ChunkWorkerOutcome::Finished));
            }
            throttle.tick(&self.db, &self.manager, &self.managed).await?;
        }
        Ok(SegmentBody::Exhausted)
    }

    /// Write one stream chunk to the buffer, falling back to a direct write
    /// when a background flush has failed.
    async fn write_bytes(&self, offset: u64, bytes: &Bytes) -> Result<()> {
        if let Some(ref buf) = self.write_buffer {
            if buf.buffer_chunk(offset, bytes.clone()).await.is_err() {
                // Background flush failed — fall back to direct write. The bytes
                // are on the file as soon as `write_all_at` returns, so they are
                // durable immediately; the buffer never got to report them.
                write_all_at(&self.file, bytes, offset)?;
                record_durable_bytes(&self.managed, &[(offset, bytes.len() as u64)]);
                if self.disk_type == DiskType::Hdd {
                    let mut core = self.managed.lock_core();
                    core.snapshot.degraded = true;
                }
            }
        } else {
            write_all_at(&self.file, bytes, offset)?;
            record_durable_bytes(&self.managed, &[(offset, bytes.len() as u64)]);
        }
        Ok(())
    }

    /// Current end for our chunk (tail sprint may have shortened it).
    fn dynamic_end(&self, fallback: u64) -> u64 {
        let core = self.managed.lock_core();
        core.manifest
            .chunks
            .get(self.chunk.index)
            .map(|c| c.end)
            .unwrap_or(fallback)
    }

    /// `true` when tail sprint released our claim while the entry still exists.
    fn claim_released(&self) -> bool {
        let core = self.managed.lock_core();
        core.manifest
            .chunks
            .get(self.chunk.index)
            .is_some_and(|chunk| chunk.claimed_by != Some(self.worker_id))
    }

    /// `true` when we still own the claim (a missing entry counts as lost).
    fn owns_claim(&self) -> bool {
        let core = self.managed.lock_core();
        core.manifest
            .chunks
            .get(self.chunk.index)
            .and_then(|c| c.claimed_by)
            == Some(self.worker_id)
    }

    /// Mark the claim released (no-op when we already lost it).
    fn release(&self) {
        mark_chunk_released(&self.managed, self.chunk.index, self.worker_id);
    }

    /// Release the claim and map the current state to pause/cancel.
    fn pause_or_cancel(&self) -> ChunkWorkerOutcome {
        self.release();
        if self.managed.lock_core().snapshot.state == DownloadState::Canceled {
            ChunkWorkerOutcome::Canceled
        } else {
            ChunkWorkerOutcome::Paused
        }
    }

    /// Mark the chunk completed if we still own the claim.
    fn mark_complete(&self) {
        let mut core = self.managed.lock_core();
        if let Some(target) = core.manifest.chunks.get_mut(self.chunk.index) {
            // Only mark completed if we still own the claim
            if target.claimed_by == Some(self.worker_id) {
                target.completed = true;
                target.downloaded = target.end.saturating_sub(target.start) + 1;
                target.claimed_by = None;
                target.dirty = true;
            }
        }
        core.manifest.updated_at_ms = now_ms();
    }
}

pub(super) async fn download_chunk(ctx: ChunkWorkerCtx) -> Result<ChunkWorkerOutcome> {
    ctx.run().await
}
