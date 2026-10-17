//! Chunk claim/steal bookkeeping and the per-chunk worker task.

use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Response;

#[cfg(test)]
use super::WORK_STEAL_MIN_SPLIT_SIZE;
use super::{BatchLimiter, ProgressThrottle, RequestBudget};
use crate::buffer_pool::DownloadBuffer;
use crate::checksum::hash_slices;
use crate::database::Database;
use crate::download::{
    ChunkWorkerOutcome, ManagedDownload, cancellation_chunk_outcome, record_durable_bytes,
    record_progress_on_managed,
};
use crate::error::{DownloadError, Result};
use crate::file_ops::{read_all_at, write_all_at};
use crate::http::{
    build_segment_request, if_range_header, is_too_many_requests_error, validate_segment_response,
};
use crate::manager_state::ManagerState;
use crate::manifest::ChunkManifest;
use crate::metalink::{MirrorLease, MirrorPool};
use crate::now_ms;
use crate::rate_limiter::RateLimiter;
use crate::retry::{jittered_backoff_delay, register_retry_penalty, request_with_retry};
use crate::types::{DiskType, DownloadState};
use reqwest::{Client, StatusCode};
use std::sync::Arc;
use tokio::task::JoinSet;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

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
pub(super) fn steal_chunk_with_min_size(
    manifest: &mut crate::manifest::Manifest,
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
pub(super) fn steal_chunk(
    manifest: &mut crate::manifest::Manifest,
    worker_id: usize,
) -> Option<ChunkManifest> {
    steal_chunk_with_min_size(manifest, worker_id, WORK_STEAL_MIN_SPLIT_SIZE)
}

/// Attempt to claim an unclaimed chunk, or dynamically steal work from an active chunk.
pub(super) fn claim_or_steal_chunk_with_min_size(
    manifest: &mut crate::manifest::Manifest,
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
    manifest: &mut crate::manifest::Manifest,
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
    workers: &mut JoinSet<(usize, Result<ChunkWorkerOutcome>)>,
    active_worker_ids: &mut std::collections::HashSet<usize>,
) {
    workers.abort_all();
    while workers.join_next().await.is_some() {}
    active_worker_ids.clear();
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
    pub(super) max_retries: u32,
    pub(super) db: Arc<Database>,
    pub(super) rate_limiter: Arc<RateLimiter>,
    pub(super) manager: Arc<dyn ManagerState>,
    pub(super) write_buffer: Option<Arc<DownloadBuffer>>,
    pub(super) disk_type: DiskType,
    pub(super) worker_id: usize,
    pub(super) work_steal_min_split: u64,
    pub(super) mirror_pool: Option<Arc<MirrorPool>>,
    pub(super) claim_times:
        Arc<parking_lot::Mutex<std::collections::HashMap<usize, std::time::Instant>>>,
}

/// Report a failed segment to the mirror it was leased from, if any. The lease
/// already carries its pool, so there is no second `Option` to check.
fn note_mirror_failure(lease: Option<&MirrorLease>) {
    if let Some(lease) = lease {
        lease.report_failure();
    }
}

/// Report how many bytes a mirror delivered.
fn note_mirror_success(lease: Option<&MirrorLease>, bytes: u64) {
    if let Some(lease) = lease {
        lease.report_success(bytes);
    }
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
    /// The body stream was interrupted before reaching the requested end.
    StreamInterrupted(reqwest::Error),
}

/// How one pass of the chunk loop ended.
enum SegmentStep {
    /// Loop again.
    Continue,
    /// Leave `process_chunk` with this outcome.
    Done(ChunkWorkerOutcome),
}

/// Stream-level retry bookkeeping for one chunk's attempt loop.
struct StreamRetry {
    consecutive_errors: u32,
    last_progress: u64,
}

impl StreamRetry {
    fn new(last_progress: u64) -> Self {
        Self {
            consecutive_errors: 0,
            last_progress,
        }
    }

    /// A fully drained segment resets the streak when it made forward progress.
    fn note_drained(&mut self, current: u64) {
        if current > self.last_progress {
            self.last_progress = current;
            self.consecutive_errors = 0;
        }
    }

    /// An interrupted segment restarts the streak at 1 after progress, and
    /// otherwise grows it.
    fn note_interrupted(&mut self, current: u64) {
        if current > self.last_progress {
            self.last_progress = current;
            self.consecutive_errors = 1;
        } else {
            self.consecutive_errors = self.consecutive_errors.saturating_add(1);
        }
    }
}

/// The mutable state one chunk's attempt loop threads through every segment.
struct ChunkAttempt<'a> {
    current: u64,
    original_end: u64,
    budget: RequestBudget,
    retry: StreamRetry,
    batch: &'a mut BatchLimiter,
    throttle: &'a mut ProgressThrottle,
}

/// Upper bound on the number of separate segment requests one chunk may issue.
const MAX_SEGMENT_FETCHES_PER_CHUNK: u32 = 24;

/// 128 KiB micro-batching threshold to reduce BTreeMap insertion and lock contention.
const MICRO_BATCH_SIZE: usize = 128 * 1024;

impl ChunkWorkerCtx {
    /// Long-lived worker loop: repeatedly claims or steals chunks until
    /// no chunks remain, worker is demoted, or cancellation occurs.
    async fn run(self) -> Result<ChunkWorkerOutcome> {
        let mut throttle = ProgressThrottle::new();
        let mut batch = BatchLimiter::new();

        loop {
            if self.token.is_cancelled() {
                return Ok(self.pause_or_cancel(None));
            }

            // Honor demotion or reduction in target worker count
            let allocation = current_allocation(&self.managed);
            if allocation == 0 || self.worker_id >= allocation {
                break;
            }

            let chunk = {
                let mut core = self.managed.lock_core();
                claim_or_steal_chunk_with_min_size(
                    &mut core.manifest,
                    self.worker_id,
                    allocation,
                    self.work_steal_min_split,
                )
            };

            let Some(chunk) = chunk else {
                break;
            };

            self.claim_times
                .lock()
                .insert(chunk.index, std::time::Instant::now());

            let outcome = self
                .process_chunk(&chunk, &mut batch, &mut throttle)
                .await?;

            self.claim_times.lock().remove(&chunk.index);

            match outcome {
                ChunkWorkerOutcome::Finished => {
                    continue;
                }
                other => return Ok(other),
            }
        }

        batch.flush(&self.rate_limiter).await;
        Ok(ChunkWorkerOutcome::Finished)
    }

    /// Process a single chunk to completion, dynamic truncation, or error.
    async fn process_chunk(
        &self,
        chunk: &ChunkManifest,
        batch: &mut BatchLimiter,
        throttle: &mut ProgressThrottle,
    ) -> Result<ChunkWorkerOutcome> {
        let start = chunk.start + chunk.downloaded;
        let mut state = ChunkAttempt {
            current: start,
            original_end: chunk.end,
            budget: RequestBudget::new(MAX_SEGMENT_FETCHES_PER_CHUNK),
            retry: StreamRetry::new(start),
            batch,
            throttle,
        };

        if state.current > self.dynamic_end(chunk.index, state.original_end) {
            self.release(chunk.index);
            return Ok(ChunkWorkerOutcome::Finished);
        }

        while state.current <= self.dynamic_end(chunk.index, state.original_end) {
            if let SegmentStep::Done(outcome) = self.fetch_segment_once(chunk, &mut state).await? {
                return Ok(outcome);
            }
        }

        self.verify_piece_and_mark_complete(chunk).await
    }

    /// One pass of the chunk loop: fetch a segment, drain it, and fold the result
    /// into the attempt state. `SegmentStep::Done` means the caller must return
    /// that outcome now; `Err` aborts the chunk.
    async fn fetch_segment_once(
        &self,
        chunk: &ChunkManifest,
        state: &mut ChunkAttempt<'_>,
    ) -> Result<SegmentStep> {
        let dynamic_end = self.dynamic_end(chunk.index, state.original_end);

        if self.token.is_cancelled() {
            return Ok(SegmentStep::Done(self.pause_or_cancel(Some(chunk.index))));
        }

        // Check if tail sprint released our chunk claim
        if self.claim_released(chunk.index) {
            return Ok(SegmentStep::Done(ChunkWorkerOutcome::Finished));
        }

        if let Err(error) = state.budget.charge(format_args!("chunk {}", chunk.index)) {
            self.release(chunk.index);
            return Err(error);
        }

        let mirror_lease = self
            .mirror_pool
            .as_ref()
            .and_then(|p| p.lease_best_mirror());
        let target_url = mirror_lease.as_ref().map(|l| l.url.clone());

        let response = match self
            .fetch_segment(state.current, dynamic_end, target_url.as_deref())
            .await
        {
            Ok(response) => response,
            Err(FetchSegmentError::DowngradeSingleThread) => {
                note_mirror_failure(mirror_lease.as_ref());
                self.release(chunk.index);
                return Ok(SegmentStep::Done(ChunkWorkerOutcome::DowngradeSingleThread));
            }
            Err(FetchSegmentError::Fatal(error)) => {
                note_mirror_failure(mirror_lease.as_ref());
                self.release(chunk.index);
                return Err(error);
            }
        };

        if !self.owns_claim(chunk.index) {
            return Ok(SegmentStep::Done(ChunkWorkerOutcome::Finished));
        }

        if response.status() == StatusCode::OK {
            self.release(chunk.index);
            return Ok(SegmentStep::Done(ChunkWorkerOutcome::RestartSingle));
        }

        validate_segment_response(&response, state.current, dynamic_end)?;

        self.mark_retrying_as_downloading();

        let seg_start = state.current;
        match self
            .consume_segment(
                response,
                &mut state.current,
                chunk.index,
                state.original_end,
                state.batch,
                state.throttle,
            )
            .await?
        {
            SegmentBody::Exhausted => {
                note_mirror_success(
                    mirror_lease.as_ref(),
                    state.current.saturating_sub(seg_start),
                );
                state.retry.note_drained(state.current);
            }
            SegmentBody::Stop(outcome) => return Ok(SegmentStep::Done(outcome)),
            SegmentBody::StreamInterrupted(err) => {
                tracing::warn!(
                    "Worker {} chunk {} stream interrupted at offset {}/{dynamic_end}: {err}",
                    self.worker_id,
                    chunk.index,
                    state.current
                );
                register_retry_penalty(&self.managed, err.to_string());
                note_mirror_failure(mirror_lease.as_ref());
                state.retry.note_interrupted(state.current);
                if state.retry.consecutive_errors > self.max_retries {
                    self.release(chunk.index);
                    return Err(DownloadError::from(err));
                }
                if let Some(outcome) = self
                    .backoff_or_cancel(chunk.index, state.retry.consecutive_errors)
                    .await
                {
                    return Ok(SegmentStep::Done(outcome));
                }
            }
        }
        Ok(SegmentStep::Continue)
    }

    /// Wait out the retry backoff unless the download is cancelled first.
    async fn backoff_or_cancel(
        &self,
        chunk_idx: usize,
        attempt: u32,
    ) -> Option<ChunkWorkerOutcome> {
        tokio::select! {
            _ = self.token.cancelled() => Some(self.pause_or_cancel(Some(chunk_idx))),
            _ = sleep(jittered_backoff_delay(attempt)) => None,
        }
    }

    /// Clear the "Retrying" marker a previous failure left on the task.
    fn mark_retrying_as_downloading(&self) {
        let mut core = self.managed.lock_core();
        if core.snapshot.state == DownloadState::Retrying {
            core.snapshot.state = DownloadState::Downloading;
            core.manifest.state = DownloadState::Downloading;
            core.snapshot.updated_at_ms = now_ms();
            core.manifest.updated_at_ms = now_ms();
        }
    }

    /// Verify the chunk's piece hash when metalink pieces are configured, then
    /// mark it complete. A mismatch resets the chunk so it is re-downloaded.
    async fn verify_piece_and_mark_complete(
        &self,
        chunk: &ChunkManifest,
    ) -> Result<ChunkWorkerOutcome> {
        if !self.verify_chunk_piece(chunk).await? {
            tracing::warn!(
                "Worker {} chunk {} failed piece checksum verification",
                self.worker_id,
                chunk.index
            );
            self.reset_chunk_for_retry(chunk.index);
            self.release(chunk.index);
            return Err(DownloadError::InvalidResponse(format!(
                "piece checksum mismatch for chunk {}",
                chunk.index
            )));
        }

        self.mark_complete(chunk.index);
        Ok(ChunkWorkerOutcome::Finished)
    }

    /// Zero a chunk's progress so the next attempt re-downloads it.
    fn reset_chunk_for_retry(&self, chunk_idx: usize) {
        let mut core = self.managed.lock_core();
        if let Some(c) = core.manifest.chunks.get_mut(chunk_idx) {
            c.downloaded = 0;
            c.dirty = true;
        }
    }

    /// Fetch the next segment, mapping the downgrade-triggering 429 to its own
    /// error so the caller does not have to inspect the transport error.
    async fn fetch_segment(
        &self,
        current: u64,
        end: u64,
        override_url: Option<&str>,
    ) -> std::result::Result<Response, FetchSegmentError> {
        let (url, user_agent, extra_headers, validator) = {
            let core = self.managed.lock_core();
            (
                override_url
                    .map(String::from)
                    .unwrap_or_else(|| core.manifest.final_url.clone()),
                core.manifest.user_agent.clone(),
                core.manifest.extra_headers.clone(),
                if_range_header(&core.manifest),
            )
        };

        let (proxy_mode, alt_svc_enabled) = {
            let settings = self.manager.settings_service().get_blocking();
            (
                settings.proxy.mode,
                settings.scheduler.http3_alt_svc_enabled,
            )
        };
        let http3_target = self.manager.alt_svc().should_attempt_http3(
            &url,
            proxy_mode,
            alt_svc_enabled,
            now_ms(),
        );

        let mut response = None;
        if let Some((ref host, port)) = http3_target {
            let client = self.client.clone();
            let req = build_segment_request(
                &client,
                &url,
                &user_agent,
                &extra_headers,
                current,
                end,
                validator.clone(),
            )
            .version(reqwest::Version::HTTP_3);

            match tokio::time::timeout(std::time::Duration::from_secs(5), req.send()).await {
                Ok(Ok(res)) => {
                    self.manager.alt_svc().mark_success(host, port);
                    response = Some(res);
                }
                Ok(Err(err)) => {
                    tracing::warn!(
                        "HTTP/3 request to {url} failed: {err}; tripping circuit breaker and falling back to TCP"
                    );
                    self.manager.alt_svc().mark_broken(host, port, now_ms());
                }
                Err(_) => {
                    tracing::warn!(
                        "HTTP/3 request to {url} timed out; tripping circuit breaker and falling back to TCP"
                    );
                    self.manager.alt_svc().mark_broken(host, port, now_ms());
                }
            }
        }

        let response = match response {
            Some(res) => Ok(res),
            None => {
                request_with_retry(
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
                .await
            }
        };

        match response {
            Ok(response) => {
                self.manager
                    .alt_svc()
                    .record_from_response(&url, response.headers(), now_ms());
                Ok(response)
            }
            Err(error) => {
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

    /// Drain one segment body into the buffer/direct-write path with 128 KiB micro-batching.
    async fn consume_segment(
        &self,
        response: Response,
        current: &mut u64,
        chunk_idx: usize,
        original_end: u64,
        batch: &mut BatchLimiter,
        throttle: &mut ProgressThrottle,
    ) -> Result<SegmentBody> {
        let mut stream = response.bytes_stream();
        let mut local_buf = Vec::with_capacity(MICRO_BATCH_SIZE);
        let mut batch_start_offset = *current;

        while let Some(bytes) = tokio::select! {
            _ = self.token.cancelled() => {
                if !local_buf.is_empty() {
                    let b = Bytes::from(std::mem::take(&mut local_buf));
                    let _ = self.write_bytes(batch_start_offset, &b).await;
                    record_progress_on_managed(&self.managed, Some(chunk_idx), b.len() as u64);
                }
                batch.flush(&self.rate_limiter).await;
                self.release(chunk_idx);
                return Ok(SegmentBody::Stop(cancellation_chunk_outcome(&self.managed)));
            }
            next = stream.next() => next,
        } {
            let bytes = match bytes {
                Ok(bytes) => bytes,
                Err(err) => {
                    if !local_buf.is_empty() {
                        let b = Bytes::from(std::mem::take(&mut local_buf));
                        let _ = self.write_bytes(batch_start_offset, &b).await;
                        record_progress_on_managed(&self.managed, Some(chunk_idx), b.len() as u64);
                    }
                    batch.flush(&self.rate_limiter).await;
                    return Ok(SegmentBody::StreamInterrupted(err));
                }
            };
            batch.account(&self.rate_limiter, bytes.len()).await;

            let dynamic_end = self.dynamic_end(chunk_idx, original_end);
            if *current > dynamic_end {
                break;
            }

            if *current + bytes.len() as u64 - 1 > original_end {
                if !local_buf.is_empty() {
                    let b = Bytes::from(std::mem::take(&mut local_buf));
                    let _ = self.write_bytes(batch_start_offset, &b).await;
                    record_progress_on_managed(&self.managed, Some(chunk_idx), b.len() as u64);
                }
                self.release(chunk_idx);
                return Err(DownloadError::InvalidResponse(String::from(
                    "segment body exceeded requested range",
                )));
            }

            let (to_write, reached_dynamic_end) = if *current + bytes.len() as u64 - 1 > dynamic_end
            {
                let allowed = (dynamic_end.saturating_sub(*current) + 1) as usize;
                (bytes.slice(..allowed.min(bytes.len())), true)
            } else {
                (bytes, false)
            };

            local_buf.extend_from_slice(&to_write);
            *current += to_write.len() as u64;

            if local_buf.len() >= MICRO_BATCH_SIZE {
                let coalesced = Bytes::from(std::mem::replace(
                    &mut local_buf,
                    Vec::with_capacity(MICRO_BATCH_SIZE),
                ));
                self.write_bytes(batch_start_offset, &coalesced).await?;
                record_progress_on_managed(&self.managed, Some(chunk_idx), coalesced.len() as u64);
                batch_start_offset = *current;
            }

            if reached_dynamic_end {
                break;
            }

            if !self.owns_claim(chunk_idx) {
                if !local_buf.is_empty() {
                    let b = Bytes::from(std::mem::take(&mut local_buf));
                    let _ = self.write_bytes(batch_start_offset, &b).await;
                    record_progress_on_managed(&self.managed, Some(chunk_idx), b.len() as u64);
                }
                batch.flush(&self.rate_limiter).await;
                return Ok(SegmentBody::Stop(ChunkWorkerOutcome::Finished));
            }
            throttle
                .tick(&self.db, self.manager.as_ref(), &self.managed)
                .await?;
        }

        if !local_buf.is_empty() {
            let coalesced = Bytes::from(std::mem::take(&mut local_buf));
            self.write_bytes(batch_start_offset, &coalesced).await?;
            record_progress_on_managed(&self.managed, Some(chunk_idx), coalesced.len() as u64);
        }

        Ok(SegmentBody::Exhausted)
    }

    /// Verify the piece hash for a completed chunk if Metalink piece verification is configured.
    async fn verify_chunk_piece(&self, chunk: &ChunkManifest) -> Result<bool> {
        let (pieces, file_total) = {
            let pieces = self.managed.pieces.lock().clone();
            let core = self.managed.lock_core();
            (pieces, core.manifest.total_bytes)
        };
        let Some(pieces) = pieces else {
            return Ok(true);
        };
        if pieces.piece_length == 0 || pieces.hashes.is_empty() {
            return Ok(true);
        }

        // Only full pieces (or the last partial piece) aligned to piece_length can be verified
        if !chunk.start.is_multiple_of(pieces.piece_length) {
            return Ok(true);
        }
        let piece_idx = (chunk.start / pieces.piece_length) as usize;
        let Some(expected_hash) = pieces.hashes.get(piece_idx) else {
            return Ok(true);
        };

        let expected_len = if let Some(total) = file_total {
            (total.saturating_sub(chunk.start)).min(pieces.piece_length)
        } else {
            pieces.piece_length
        };

        let chunk_len = chunk.end.saturating_sub(chunk.start) + 1;
        if chunk_len != expected_len {
            // Sub-chunk from work-stealing; verification happens at whole-file finalization
            return Ok(true);
        }

        if let Some(ref buf) = self.write_buffer {
            buf.flush_all().await?;
        }

        let mut data = vec![0u8; expected_len as usize];
        let bytes_read = read_all_at(&self.file, &mut data, chunk.start)?;
        if bytes_read != expected_len as usize {
            tracing::warn!(
                "Piece verification failed: expected {expected_len} bytes, read {bytes_read} at offset {}",
                chunk.start
            );
            return Ok(false);
        }

        let computed = hash_slices(pieces.algorithm, &[&data]);
        if !computed.eq_ignore_ascii_case(expected_hash) {
            tracing::warn!(
                "Piece {piece_idx} verification mismatch: expected {expected_hash}, computed {computed}"
            );
            return Ok(false);
        }

        Ok(true)
    }

    /// Write one stream chunk to the buffer, falling back to a direct write
    /// when a background flush has failed.
    async fn write_bytes(&self, offset: u64, bytes: &Bytes) -> Result<()> {
        if let Some(ref buf) = self.write_buffer {
            if buf.buffer_chunk(offset, bytes.clone()).await.is_err() {
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

    /// Current end for chunk (dynamically updated if work was stolen).
    fn dynamic_end(&self, chunk_idx: usize, fallback: u64) -> u64 {
        let core = self.managed.lock_core();
        core.manifest
            .chunks
            .get(chunk_idx)
            .map(|c| c.end)
            .unwrap_or(fallback)
    }

    /// `true` when tail sprint released our claim while the entry still exists.
    fn claim_released(&self, chunk_idx: usize) -> bool {
        let core = self.managed.lock_core();
        core.manifest
            .chunks
            .get(chunk_idx)
            .is_some_and(|chunk| chunk.claimed_by != Some(self.worker_id))
    }

    /// `true` when we still own the claim (a missing entry counts as lost).
    fn owns_claim(&self, chunk_idx: usize) -> bool {
        let core = self.managed.lock_core();
        core.manifest
            .chunks
            .get(chunk_idx)
            .and_then(|c| c.claimed_by)
            == Some(self.worker_id)
    }

    /// Mark the claim released (no-op when we already lost it).
    fn release(&self, chunk_idx: usize) {
        mark_chunk_released(&self.managed, chunk_idx, self.worker_id);
    }

    /// Release the claim and map the current state to pause/cancel.
    fn pause_or_cancel(&self, chunk_idx: Option<usize>) -> ChunkWorkerOutcome {
        if let Some(idx) = chunk_idx {
            self.release(idx);
        }
        if self.managed.lock_core().snapshot.state == DownloadState::Canceled {
            ChunkWorkerOutcome::Canceled
        } else {
            ChunkWorkerOutcome::Paused
        }
    }

    /// Mark the chunk completed if we still own the claim.
    fn mark_complete(&self, chunk_idx: usize) {
        let mut core = self.managed.lock_core();
        if let Some(target) = core.manifest.chunks.get_mut(chunk_idx)
            && target.claimed_by == Some(self.worker_id)
        {
            target.completed = true;
            target.downloaded = target.end.saturating_sub(target.start) + 1;
            target.claimed_by = None;
            target.dirty = true;
        }
        core.manifest.updated_at_ms = now_ms();
    }
}

pub(super) async fn download_chunk(ctx: ChunkWorkerCtx) -> Result<ChunkWorkerOutcome> {
    ctx.run().await
}
