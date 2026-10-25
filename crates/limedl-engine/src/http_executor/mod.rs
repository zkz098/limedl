//! HTTP download execution — extracted from manager.rs to reduce the god object.
//!
//! Contains the HTTP-specific download flow: probing, single-stream and chunked
//! parallel downloads, chunk worker, and finalization with checksum verification.
//!
//! `HttpExecutor` is an independent actor type.  All its methods receive a
//! `&dyn ManagerState` or `Arc<dyn ManagerState>` parameter to access shared
//! state, so this module does not depend on `manager` — which owns
//! `Arc<HttpExecutor>` and implements that trait.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio_util::sync::CancellationToken;

use crate::{
    buffer_pool::{DownloadBuffer, FlushObserver},
    database::Database,
    download::{ManagedDownload, PERSIST_INTERVAL, RunOutcome},
    error::{DownloadError, Result},
    event_bus::DownloadEvent,
    file_ops::check_disk_space,
    manager_state::ManagerState,
    manifest::Manifest,
    now_ms,
    persistence::persist_manifest_snapshot,
    rate_limiter::RateLimiter,
    types::{DiskType, DownloadState},
};

/// Zero-sized actor type for HTTP download execution.
///
/// Methods that need shared state receive `&dyn ManagerState` or
/// `Arc<dyn ManagerState>`; `DownloadManager` implements that trait and holds
/// `Arc<HttpExecutor>` for delegation.
pub struct HttpExecutor;

/// Tail Sprint: stall window for detecting a slow last-chunk connection.
const TAIL_SPRINT_STALL_WINDOW_SECS: u64 = 8;
/// Tail Sprint: minimum remaining bytes to qualify for chunk splitting (1 MiB).
const TAIL_SPRINT_MIN_SPLIT_SIZE: u64 = 1024 * 1024;
/// Work Stealing: minimum remaining bytes of an active chunk to qualify for splitting (2 MiB).
const WORK_STEAL_MIN_SPLIT_SIZE: u64 = 2 * 1024 * 1024;

/// The path whose device the write buffer targets.
///
/// `temp_path` is the natural choice.  A manifest built by hand (tests, or a
/// download whose temp path was never recorded) may leave it empty, in which
/// case the destination path — same directory, same device — serves the same
/// purpose: device resolution only needs *a* path on the target filesystem.
fn device_path_for(manifest: &Manifest) -> PathBuf {
    [
        &manifest.temp_path,
        &manifest.destination_path,
        &manifest.destination_dir,
    ]
    .into_iter()
    .find(|candidate| !candidate.is_empty())
    .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

/// Build the write buffer for a download: the shared HDD double-buffer pool,
/// or the local SSD/Network ping-pong write-combining buffer.
async fn build_write_buffer(
    dm: &dyn ManagerState,
    managed: &Arc<ManagedDownload>,
    file: &Arc<fs::File>,
    disk_type: DiskType,
    hdd_buffering: bool,
    ssd_write_combine_mb: u64,
) -> Arc<DownloadBuffer> {
    // The buffer reports every byte a flush put on the file; that report is the
    // only thing that advances the task's durable counters, and therefore the only
    // thing the database is allowed to store. See `record_durable_bytes`.
    let observer_managed = managed.clone();
    let observer: FlushObserver = Arc::new(move |ranges: &[(u64, u64)]| {
        crate::download::record_durable_bytes(&observer_managed, ranges);
    });

    let (chunk_size, device_path) = {
        let core = managed.lock_core();
        (core.manifest.chunk_size, device_path_for(&core.manifest))
    };

    if disk_type == DiskType::Hdd && hdd_buffering {
        let slot = dm.buffer_pool().acquire_slot().await;
        return Arc::new(DownloadBuffer::new_with_worker(
            dm.buffer_pool().clone(),
            slot,
            file.clone(),
            device_path,
            dm.io_worker().clone(),
            Some(observer),
        ));
    }

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
        device_path,
        dm.io_worker().clone(),
        Some(observer),
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
    dm: &dyn ManagerState,
    managed: &Arc<ManagedDownload>,
    token: &CancellationToken,
    write_buffer: &Option<Arc<DownloadBuffer>>,
) -> Option<RunOutcome> {
    match dm.wait_until_active(managed, token).await {
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
    dm: &dyn ManagerState,
    managed: &Arc<ManagedDownload>,
    write_buffer: &Option<Arc<DownloadBuffer>>,
) -> Result<()> {
    if let Some(buf) = write_buffer {
        // Signal frontend that we're flushing to disk
        {
            let mut core = managed.lock_core();
            core.snapshot.flushing = true;
        }
        dm.emit_progress(managed);

        let flush_result = buf.flush_all().await;

        // Always clear the flag, even on error
        {
            let mut core = managed.lock_core();
            core.snapshot.flushing = false;
        }
        dm.emit_progress(managed);
        flush_result?;
    }
    Ok(())
}

/// Re-check free space every 30 s while downloading; fail the task when the
/// remaining bytes no longer fit.
async fn check_disk_space_periodically(
    dm: &dyn ManagerState,
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
            dm.event_bus().publish(DownloadEvent::Warning {
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
        Self {
            bytes: 0,
            chunks: 0,
        }
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

/// Bounds how many separate HTTP requests one unit of work (a chunk, a
/// single-stream transfer) may issue.
///
/// A retry counter alone is not enough to bound the work: when a server ends a
/// response early the caller simply re-requests from the new offset, and that
/// request gets a *fresh* per-request retry counter. A server that stops making
/// progress (always closing the body immediately, or after a single byte) then
/// turns the caller's loop into a livelock that never fails and never finishes.
/// Charging the budget once per request bounds the total work and turns the
/// livelock into a reported error.
struct RequestBudget {
    used: u32,
    max: u32,
}

impl RequestBudget {
    fn new(max: u32) -> Self {
        Self { used: 0, max }
    }

    /// Charge one request against the budget, failing once it is exhausted.
    /// `what` names the unit of work for the error message.
    fn charge(&mut self, what: impl std::fmt::Display) -> Result<()> {
        self.used = self.used.saturating_add(1);
        if self.used > self.max {
            return Err(DownloadError::InvalidResponse(format!(
                "{what} was re-requested {} times without completing; the server keeps ending \
                 the response early",
                self.used
            )));
        }
        Ok(())
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
        dm: &dyn ManagerState,
        managed: &Arc<ManagedDownload>,
    ) -> Result<()> {
        if self.last_persist.elapsed() < PERSIST_INTERVAL {
            return Ok(());
        }
        persist_manifest_snapshot(db, managed).await?;
        self.last_persist = Instant::now();
        // Throttle progress events: at most once per 500ms
        if self.last_emit.elapsed() >= Duration::from_millis(500) {
            dm.emit_progress(managed);
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

#[cfg(any(test, feature = "test-utils"))]
pub use worker::mark_chunk_released;

#[cfg(test)]
mod tests;
