//! HTTP download execution — extracted from manager.rs to reduce the god object.
//!
//! Contains the HTTP-specific download flow: probing, single-stream and chunked
//! parallel downloads, chunk worker, and finalization with checksum verification.
//!
//! `HttpExecutor` is an independent actor type.  All its methods receive a
//! `&DownloadManager` or `Arc<DownloadManager>` parameter to access shared
//! state, avoiding any ownership cycle with `DownloadManager` (which holds
//! `Arc<HttpExecutor>`).

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::{Client, StatusCode, header};
use tokio::{task::JoinSet, time::sleep};
use tokio_util::sync::CancellationToken;

use crate::{
    aimd::AimdState,
    buffer_pool::DownloadBuffer,
    calculate_checksum,
    database::Database,
    error::{DownloadError, Result, io_error_with_path},
    event_bus::DownloadEvent,
    file_ops::{
        check_disk_space, finalize_temp_file, open_download_file, reset_download_file, write_all_at,
    },
    http::{
        ANTI_ABUSE_SNIFF_LIMIT, anti_abuse_forbidden_error, apply_extra_headers,
        build_segment_request, extract_total_bytes, has_header, header_string, if_range_header,
        infer_candidate_referers, infer_file_name, is_too_many_requests_error,
        looks_like_anti_abuse_page, read_body_prefix, supports_ranges, validate_probe_response,
        validate_segment_response,
    },
    download::{
        ChunkWorkerOutcome, ManagedDownload, PERSIST_INTERVAL, RunOutcome,
        cancellation_chunk_outcome, cancellation_outcome, record_progress_on_managed,
        supports_parallelism,
    },
    manager::DownloadManager,
    manifest::{
        ChunkManifest, RemoteMetadata, contiguous_prefix_end, has_partial_chunk_progress,
        plan_chunks, resolve_chunk_size, validators_changed,
    },
    now_ms,
    persistence::persist_manifest_snapshot,
    rate_limiter::RateLimiter,
    retry::request_with_retry,
    types::{AdaptiveProfile, ChecksumMode, DiskType, DownloadState, StartDownloadRequest, TaskKind, ThreadMode},
};

/// Zero-sized actor type for HTTP download execution.
///
/// All methods receive `&DownloadManager` or `Arc<DownloadManager>` to access
/// shared state.  `DownloadManager` holds `Arc<HttpExecutor>` for delegation.
pub struct HttpExecutor;

/// Tail Sprint: stall window for detecting a slow last-chunk connection.
const TAIL_SPRINT_STALL_WINDOW_SECS: u64 = 8;
/// Tail Sprint: minimum remaining bytes to qualify for chunk splitting (1 MiB).
const TAIL_SPRINT_MIN_SPLIT_SIZE: u64 = 1024 * 1024;
/// Work Stealing: minimum remaining bytes of an active chunk to qualify for splitting (2 MiB).
const WORK_STEAL_MIN_SPLIT_SIZE: u64 = 2 * 1024 * 1024;

/// Build the write buffer for a download: the shared HDD double-buffer pool,
/// or the local SSD/Network ping-pong write-combining buffer.
async fn build_write_buffer(
    dm: &DownloadManager,
    managed: &Arc<ManagedDownload>,
    file: &Arc<fs::File>,
    disk_type: DiskType,
    hdd_buffering: bool,
    ssd_write_combine_mb: u64,
) -> Arc<DownloadBuffer> {
    if disk_type == DiskType::Hdd && hdd_buffering {
        let slot = dm.buffer_pool.acquire_slot().await;
        return Arc::new(DownloadBuffer::new_with_worker(
            dm.buffer_pool.clone(),
            slot,
            file.clone(),
            dm.io_worker.clone(),
        ));
    }

    let chunk_size = managed.lock_core().manifest.chunk_size;
    let ssd_limit_bytes = if ssd_write_combine_mb == 0 {
        chunk_size // auto: use the download's chunk size
    } else {
        ssd_write_combine_mb * 1024 * 1024
    };
    // half_size = ssd_limit_bytes / 2, minimum 64 KiB, capped at 8 MiB.
    // Note: for large auto-sized buffers (chunk_size up to 128 MiB),
    // this means at most 8 MiB per download — the ping-pong double-buffer
    // caps peak untracked memory at 16 MiB per download regardless of N.
    let ssd_half_size = (ssd_limit_bytes / 2).clamp(64 * 1024, 8 * 1024 * 1024);
    Arc::new(DownloadBuffer::new_local_pingpong_with_worker(
        ssd_half_size,
        file.clone(),
        dm.io_worker.clone(),
    ))
}

/// Flush a download's write buffer if one exists.
///
/// Failures are logged, not propagated: every caller is on an exit path where a
/// failed flush must not mask the real outcome.
async fn flush_write_buffer(write_buffer: &Option<Arc<DownloadBuffer>>, reason: &str) {
    if let Some(buf) = write_buffer
        && let Err(e) = buf.flush_all().await
    {
        tracing::warn!("flush on {reason} failed: {e}");
    }
}

/// Wait until the task is runnable; `Some(outcome)` means the caller must stop.
async fn wait_or_stop(
    dm: &DownloadManager,
    managed: &Arc<ManagedDownload>,
    token: &CancellationToken,
    write_buffer: &Option<Arc<DownloadBuffer>>,
) -> Option<RunOutcome> {
    match dm.task_lifecycle.wait_until_active(dm, managed, token).await {
        crate::download::WaitState::Running => None,
        crate::download::WaitState::Paused => {
            flush_write_buffer(write_buffer, "pause").await;
            Some(RunOutcome::Paused)
        }
        crate::download::WaitState::Canceled => {
            flush_write_buffer(write_buffer, "cancel").await;
            Some(RunOutcome::Canceled)
        }
    }
}

/// Flush the write buffer after the download finished, keeping the UI
/// "flushing" flag set around the flush.
async fn finish_buffer_flush(
    dm: &DownloadManager,
    managed: &Arc<ManagedDownload>,
    write_buffer: &Option<Arc<DownloadBuffer>>,
) -> Result<()> {
    if let Some(buf) = write_buffer {
        // Signal frontend that we're flushing to disk
        {
            let mut core = managed.lock_core();
            core.snapshot.flushing = true;
        }
        dm.task_lifecycle.emit_progress(dm, managed);

        let flush_result = buf.flush_all().await;

        // Always clear the flag, even on error
        {
            let mut core = managed.lock_core();
            core.snapshot.flushing = false;
        }
        dm.task_lifecycle.emit_progress(dm, managed);
        flush_result?;
    }
    Ok(())
}

/// Re-check free space every 30 s while downloading; fail the task when the
/// remaining bytes no longer fit.
async fn check_disk_space_periodically(
    dm: &DownloadManager,
    managed: &Arc<ManagedDownload>,
    last_check: &mut Instant,
) -> Result<()> {
    if last_check.elapsed() < Duration::from_secs(30) {
        return Ok(());
    }

    let (total_bytes, downloaded_bytes, destination_dir) = {
        let core = managed.lock_core();
        (
            core.manifest.total_bytes,
            core.manifest.downloaded_bytes,
            core.manifest.destination_dir.clone(),
        )
    };
    if let Some(total) = total_bytes {
        let remaining = total.saturating_sub(downloaded_bytes);
        if remaining > 0 && check_disk_space(Path::new(&destination_dir), remaining).is_err() {
            let msg = format!("Insufficient disk space: {remaining} bytes remaining");
            {
                let mut core = managed.lock_core();
                core.snapshot.state = DownloadState::Failed;
                core.snapshot.error = Some(msg.clone());
                core.snapshot.connection_count = 0;
                core.snapshot.updated_at_ms = now_ms();
                core.manifest.state = DownloadState::Failed;
                core.manifest.error = Some(msg);
                core.manifest.connection_count = 0;
                core.manifest.updated_at_ms = now_ms();
            }
            dm.event_bus.publish(DownloadEvent::Warning {
                id: managed.lock_core().manifest.id.clone(),
                message: "disk full".into(),
            });
            return Err(DownloadError::InsufficientDiskSpace {
                available: 0,
                required: remaining,
            });
        }
    }
    *last_check = Instant::now();
    Ok(())
}

/// Byte/chunk accumulator that amortises rate-limiter consumption over a
/// 256 KiB / 8 chunk batch instead of consuming per received chunk.
struct BatchLimiter {
    bytes: usize,
    chunks: usize,
}

impl BatchLimiter {
    fn new() -> Self {
        Self { bytes: 0, chunks: 0 }
    }

    /// Account one received chunk; consume when the batch is full.
    async fn account(&mut self, limiter: &RateLimiter, len: usize) {
        const BATCH_BYTES: usize = 256 * 1024;
        const BATCH_CHUNKS: usize = 8;
        self.bytes += len;
        self.chunks += 1;
        if self.bytes >= BATCH_BYTES || self.chunks >= BATCH_CHUNKS {
            limiter.consume(self.bytes).await;
            self.bytes = 0;
            self.chunks = 0;
        }
    }

    /// Consume any accounted-but-unconsumed bytes.
    async fn flush(&mut self, limiter: &RateLimiter) {
        if self.bytes > 0 {
            limiter.consume(self.bytes).await;
            self.bytes = 0;
            self.chunks = 0;
        }
    }
}

/// Persist + progress-emit throttle shared by the single-stream loop and the
/// chunk workers.
struct ProgressThrottle {
    last_persist: Instant,
    last_emit: Instant,
}

impl ProgressThrottle {
    fn new() -> Self {
        Self {
            last_persist: Instant::now(),
            last_emit: Instant::now(),
        }
    }

    /// Persist the manifest and (at most every 500 ms) emit progress.
    async fn tick(
        &mut self,
        db: &Arc<Database>,
        dm: &DownloadManager,
        managed: &Arc<ManagedDownload>,
    ) -> Result<()> {
        if self.last_persist.elapsed() < PERSIST_INTERVAL {
            return Ok(());
        }
        persist_manifest_snapshot(db, managed).await?;
        self.last_persist = Instant::now();
        // Throttle progress events: at most once per 500ms
        if self.last_emit.elapsed() >= Duration::from_millis(500) {
            dm.task_lifecycle.emit_progress(dm, managed);
            self.last_emit = Instant::now();
        }
        Ok(())
    }
}

mod chunked;
mod finalize;
mod run;
mod single;
mod worker;

#[cfg(test)]
pub(crate) use worker::mark_chunk_released;

#[cfg(test)]
mod tests;
