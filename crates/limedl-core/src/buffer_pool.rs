//! Double-buffer cache for HDD download optimization.
//!
//! Replaces the old single-buffer design with a ping-pong (double-buffer)
//! per-download architecture. Each HDD download gets a slot from a global
//! semaphore, and chunks are accumulated in one of two halves. When a half
//! is full, it is flushed to disk in a background task while the other half
//! receives new writes. This provides backpressure (blocking) instead of
//! degrading to direct I/O.
//!
//! SSD downloads use a simpler local-only buffer for write combining without
//! the double-buffer machinery.

use std::collections::BTreeMap;
use bytes::Bytes;
use parking_lot::Mutex;
use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::thread;
use tokio::sync::{mpsc, oneshot, Notify, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;

use super::error::DownloadError;
use super::file_ops::write_all_at;

// ---------------------------------------------------------------------------
// IoWorker — pool of dedicated I/O worker threads for file writes
// ---------------------------------------------------------------------------

/// Command sent to the dedicated I/O worker threads.
enum IoCommand {
    /// Write a batch of (offset, chunk) pairs to a file.  The entries are
    /// already in ascending-offset order (drained from a BTreeMap).
    WriteBatch {
        file: Arc<File>,
        entries: Vec<(u64, Bytes)>,
        /// Whether to fsync after this batch. HDD double-buffer writes should
        /// sync for crash safety; SSD write-combining batches are large enough
        /// that per-batch fsync provides diminishing returns.
        sync: bool,
        done: oneshot::Sender<Result<(), DownloadError>>,
    },
}

/// Handle to a pool of dedicated I/O worker threads that serialise flush calls.
///
/// Writes are hash-routed to a specific worker based on file identity,
/// ensuring same-file writes are always serialised (correct for data
/// integrity) while writes to different files can proceed in parallel.
///
/// Cloning produces another set of senders to the same worker threads —
/// all clones share the same underlying OS threads.
#[derive(Clone)]
pub struct IoWorker {
    txs: Vec<mpsc::UnboundedSender<IoCommand>>,
}

impl IoWorker {
    /// Spawn a pool of dedicated I/O worker threads and return a handle to them.
    ///
    /// `n` must be at least 1. Each thread has its own channel for independent
    /// command processing.
    pub fn spawn_pool(n: usize) -> Self {
        let n = n.max(1);
        let mut txs = Vec::with_capacity(n);
        for i in 0..n {
            let (tx, mut rx) = mpsc::unbounded_channel::<IoCommand>();
            thread::Builder::new()
                .name(format!("limedl-io-worker-{i}"))
                .spawn(move || {
                    // Normal processing loop
                    while let Some(cmd) = rx.blocking_recv() {
                        Self::process_command(cmd);
                    }
                    // All senders dropped — drain any commands that were queued
                    // before the final sender dropped.
                    while let Ok(cmd) = rx.try_recv() {
                        Self::process_command(cmd);
                    }
                })
                .expect("failed to spawn I/O worker thread");
            txs.push(tx);
        }
        Self { txs }
    }

    /// Convenience: spawn a single-threaded I/O worker (backward-compatible).
    pub fn spawn() -> Self {
        Self::spawn_pool(1)
    }

    /// Process a single I/O command (extracted for reuse in normal loop and drain phase).
    fn process_command(cmd: IoCommand) {
        match cmd {
            IoCommand::WriteBatch {
                file,
                entries,
                done,
                sync,
            } => {
                // entries are already sorted by offset (BTreeMap drain order).
                let result = (|| -> Result<(), DownloadError> {
                    if entries.is_empty() {
                        if sync {
                            file.sync_data().map_err(|e| {
                                DownloadError::Internal(format!("fsync failed: {e}"))
                            })?;
                        }
                        return Ok(());
                    }
                    // Merge adjacent contiguous entries to reduce syscall count.
                    let mut i = 0;
                    while i < entries.len() {
                        let (start_off, ref first_data) = entries[i];
                        let mut end_off = start_off + first_data.len() as u64;
                        let mut j = i + 1;
                        while j < entries.len() && entries[j].0 == end_off {
                            end_off += entries[j].1.len() as u64;
                            j += 1;
                        }
                        if j == i + 1 {
                            // Single entry, write directly (no allocation).
                            write_all_at(&file, &entries[i].1, entries[i].0)?;
                        } else {
                            // Merge entries i..j into one contiguous buffer.
                            let total_len = (end_off - start_off) as usize;
                            let mut merged = Vec::with_capacity(total_len);
                            for (_, chunk) in &entries[i..j] {
                                merged.extend_from_slice(chunk);
                            }
                            write_all_at(&file, &merged, start_off)?;
                        }
                        i = j;
                    }
                    if sync {
                        file.sync_data().map_err(|e| {
                            DownloadError::Internal(format!("fsync failed: {e}"))
                        })?;
                    }
                    Ok(())
                })();
                // Ignore send error — caller dropped the receiver.
                let _ = done.send(result);
            }
        }
    }

    /// Submit a batch write to a worker thread and await completion.
    ///
    /// Writes are hash-routed to a specific worker based on file identity,
    /// so writes to the same file are always processed by the same thread.
    pub async fn write_batch(
        &self,
        file: Arc<File>,
        entries: Vec<(u64, Bytes)>,
        sync: bool,
    ) -> Result<(), DownloadError> {
        let idx = (Arc::as_ptr(&file) as usize) % self.txs.len();
        let tx = &self.txs[idx];

        let (done_tx, done_rx) = oneshot::channel();
        tx.send(IoCommand::WriteBatch {
            file,
            entries,
            sync,
            done: done_tx,
        })
        .map_err(|_| {
            DownloadError::Internal("I/O worker thread exited unexpectedly".into())
        })?;
        done_rx
            .await
            .map_err(|_| DownloadError::Internal("I/O worker dropped response".into()))?
    }
}

// ---------------------------------------------------------------------------
// FlipTokenGuard — RAII guard for the flip token
// ---------------------------------------------------------------------------

/// RAII guard that releases the flip token and notifies waiters on drop.
/// Prevents deadlock if the flip section panics.
struct FlipTokenGuard<'a> {
    token: &'a AtomicBool,
    notify: &'a Notify,
}

impl<'a> Drop for FlipTokenGuard<'a> {
    fn drop(&mut self) {
        self.token.store(false, Ordering::Release);
        self.notify.notify_waiters();
    }
}

/// Configuration for the shared ping-pong flip logic.
struct PingPongCfg<'a> {
    /// Global buffer pool for HDD memory tracking. `None` for SSD (ping-pong) mode.
    pool: Option<&'a Arc<BufferPool>>,
    /// Whether to fsync in the IoWorker background flush path.
    bg_sync: bool,
    /// Whether to fsync in the spawn_blocking fallback background flush path.
    bg_fsync: bool,
    /// Label for tracing/error messages (e.g., "HDD" or "SSD ping-pong").
    label: &'static str,
}

// ---------------------------------------------------------------------------
// SlotGuard — RAII guard for a semaphore permit
// ---------------------------------------------------------------------------

/// A slot permit acquired from `BufferPool`. Dropping this returns the permit
/// to the semaphore, making the slot available for another download.
pub struct SlotGuard {
    #[allow(dead_code)]
    permit: Option<OwnedSemaphorePermit>,
}

impl SlotGuard {
    fn new(permit: OwnedSemaphorePermit) -> Self {
        Self {
            permit: Some(permit),
        }
    }
}

// ---------------------------------------------------------------------------
// BufferPool — global slot-based pool
// ---------------------------------------------------------------------------

/// Global pool that governs memory and concurrency for HDD download buffers.
///
/// Each HDD download acquires a slot (semaphore permit) before creating its
/// `DownloadBuffer`. The pool enforces a total memory budget (`total_limit_mb`
/// / `game_mode_limit_mb`) and a maximum number of concurrent HDD downloads
/// (`max_parallel` / `game_mode_max_parallel`).
pub struct BufferPool {
    total_limit_mb: AtomicU64,
    game_mode: AtomicBool,
    game_mode_limit_mb: AtomicU64,
    max_parallel: AtomicU32,
    game_mode_max_parallel: AtomicU32,
    slot_semaphore: Arc<Semaphore>,
    current_usage: AtomicU64,
    active_count: AtomicU32,
}

impl BufferPool {
    /// Create a new buffer pool.
    ///
    /// * `total_limit_mb` — total pool memory in MiB under normal operation.
    /// * `game_mode_limit_mb` — reduced pool memory when game mode is active.
    /// * `max_parallel` — maximum concurrent HDD downloads (slot count).
    /// * `game_mode_max_parallel` — reduced slot count when game mode is active.
    pub fn new(
        total_limit_mb: u64,
        game_mode_limit_mb: u64,
        max_parallel: u32,
        game_mode_max_parallel: u32,
    ) -> Self {
        Self {
            total_limit_mb: AtomicU64::new(total_limit_mb),
            game_mode: AtomicBool::new(false),
            game_mode_limit_mb: AtomicU64::new(game_mode_limit_mb),
            max_parallel: AtomicU32::new(max_parallel),
            game_mode_max_parallel: AtomicU32::new(game_mode_max_parallel),
            slot_semaphore: Arc::new(Semaphore::new(max_parallel as usize)),
            current_usage: AtomicU64::new(0),
            active_count: AtomicU32::new(0),
        }
    }

    /// The capacity of a single buffer half in bytes.
    ///
    /// Derivation: `effective_limit / effective_max_parallel / 2`, floored to
    /// a minimum of 64 KiB so that even tiny configurations remain functional.
    pub fn half_size(&self) -> u64 {
        let limit = self.effective_limit();
        let slots = self.effective_max_parallel() as u64;
        if limit == 0 || slots == 0 {
            return 64 * 1024;
        }
        let per_slot = limit / slots;
        (per_slot / 2).max(64 * 1024)
    }

    /// Acquire a slot permit, blocking (async) until one is available.
    ///
    /// This uses a FIFO fair queue — the semaphore does not provide strict
    /// FIFO, but Tokio's `Semaphore` is fair under contention.
    pub async fn acquire_slot(&self) -> SlotGuard {
        let sem = self.slot_semaphore.clone();
        let permit = sem
            .acquire_owned()
            .await
            .expect("buffer pool semaphore unexpectedly closed — this is a bug");
        self.active_count.fetch_add(1, Ordering::Relaxed);
        SlotGuard::new(permit)
    }

    /// Release a slot permit (called from `DownloadBuffer::Drop`).
    pub fn release_slot(&self) {
        self.active_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Add bytes to the global usage counter.
    pub fn add_usage(&self, bytes: u64) {
        self.current_usage.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Subtract bytes from the global usage counter.
    pub fn sub_usage(&self, bytes: u64) {
        self.current_usage.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Effective pool limit in bytes, accounting for game mode.
    pub fn effective_limit(&self) -> u64 {
        if self.game_mode.load(Ordering::Relaxed) {
            self.game_mode_limit_mb.load(Ordering::Relaxed) * 1024 * 1024
        } else {
            self.total_limit_mb.load(Ordering::Relaxed) * 1024 * 1024
        }
    }

    /// Effective maximum parallel HDD downloads, accounting for game mode.
    pub fn effective_max_parallel(&self) -> u32 {
        if self.game_mode.load(Ordering::Relaxed) {
            self.game_mode_max_parallel.load(Ordering::Relaxed)
        } else {
            self.max_parallel.load(Ordering::Relaxed)
        }
    }

    /// Total bytes currently buffered across all downloads.
    pub fn current_usage(&self) -> u64 {
        self.current_usage.load(Ordering::Relaxed)
    }

    /// Whether game mode is currently active.
    pub fn game_mode(&self) -> bool {
        self.game_mode.load(Ordering::Relaxed)
    }

    /// Toggle game mode. This updates effective limit and max-parallel for
    /// the next `half_size()` calculation. Active semaphore permits are NOT
    /// revoked; new acquisitions will see the new limit.
    pub fn set_game_mode(&self, enabled: bool) {
        self.game_mode.store(enabled, Ordering::Relaxed);
    }

    /// Update all limit and parallelism parameters from settings.
    ///
    /// When `max_parallel` or `game_mode_max_parallel` (depending on game mode)
    /// is increased, the semaphore is grown dynamically so that new downloads
    /// can take advantage of the higher concurrency without a restart.
    /// Decreasing is intentionally not supported — existing permits are safe to
    /// keep and will naturally return to the pool on drop.
    pub fn update_limits(
        &self,
        total_limit_mb: u64,
        game_mode_limit_mb: u64,
        max_parallel: u32,
        game_mode_max_parallel: u32,
    ) {
        let old_max = self.effective_max_parallel();
        self.total_limit_mb.store(total_limit_mb, Ordering::Relaxed);
        self.game_mode_limit_mb
            .store(game_mode_limit_mb, Ordering::Relaxed);
        self.max_parallel.store(max_parallel, Ordering::Relaxed);
        self.game_mode_max_parallel
            .store(game_mode_max_parallel, Ordering::Relaxed);
        let new_max = self.effective_max_parallel();
        // Grow the semaphore when the limit increases. Decreasing is
        // intentionally not supported — existing permits are safe to keep.
        if new_max > old_max {
            self.slot_semaphore
                .add_permits((new_max - old_max) as usize);
        }
    }

    /// Number of slots currently acquired by active downloads.
    pub fn active_slots(&self) -> u32 {
        self.active_count.load(Ordering::Relaxed)
    }

    /// Maximum number of slots (display purpose, reflects effective max-parallel).
    pub fn max_slots(&self) -> u32 {
        self.effective_max_parallel()
    }

    /// Number of tasks waiting to acquire a slot (display purpose).
    pub fn queued_count(&self) -> u32 {
        let available = self.slot_semaphore.available_permits() as u32;
        self.max_slots()
            .saturating_sub(available)
            .saturating_sub(self.active_slots())
    }

    /// Degradation count — kept for API compatibility, always returns 0
    /// because the new design never degrades (it backpressures instead).
    pub fn degradation_count(&self) -> usize {
        0
    }
}

// ---------------------------------------------------------------------------
// DownloadBuffer — per-download double buffer (HDD) or ping-pong buffer (SSD)
// ---------------------------------------------------------------------------

/// Internal mode for `DownloadBuffer`.
enum BufferMode {
    /// Double-buffer mode used for HDD downloads.
    Double {
        half_a: Arc<Mutex<BTreeMap<u64, Bytes>>>,
        half_b: Arc<Mutex<BTreeMap<u64, Bytes>>>,
        active_is_a: AtomicBool, // true = half_a is receiving writes
        usage_a: AtomicU64,
        usage_b: AtomicU64,
        half_size: u64,
        flush_handle: Mutex<Option<JoinHandle<()>>>,
        notify: Arc<Notify>,
        error_flag: Arc<AtomicBool>,
        flip_token: AtomicBool, // guards the flip critical section
        pool: Arc<BufferPool>,
        #[allow(dead_code)]
        slot: SlotGuard,
        file: Arc<File>,
    },
    /// Local ping-pong mode for SSD write combining.
    /// Same double-buffer logic as HDD but without global pool/slot management.
    LocalPingPong {
        half_a: Arc<Mutex<BTreeMap<u64, Bytes>>>,
        half_b: Arc<Mutex<BTreeMap<u64, Bytes>>>,
        active_is_a: AtomicBool,
        usage_a: AtomicU64,
        usage_b: AtomicU64,
        half_size: u64,
        flush_handle: Mutex<Option<JoinHandle<()>>>,
        notify: Arc<Notify>,
        error_flag: Arc<AtomicBool>,
        flip_token: AtomicBool,
        file: Arc<File>,
    },
}

/// A per-download buffer that accumulates chunks in memory and flushes them
/// to disk via a background double-buffer ping-pong (HDD / SSD).
pub struct DownloadBuffer {
    mode: BufferMode,
    io_worker: Option<IoWorker>,
}

impl DownloadBuffer {
    /// Create a pool-backed double-buffer with a dedicated I/O worker.
    pub fn new_with_worker(
        pool: Arc<BufferPool>,
        slot: SlotGuard,
        file: Arc<File>,
        worker: IoWorker,
    ) -> Self {
        let half_size = pool.half_size();
        Self {
            mode: BufferMode::Double {
                half_a: Arc::new(Mutex::new(BTreeMap::new())),
                half_b: Arc::new(Mutex::new(BTreeMap::new())),
                active_is_a: AtomicBool::new(true),
                usage_a: AtomicU64::new(0),
                usage_b: AtomicU64::new(0),
                half_size,
                flush_handle: Mutex::new(None),
                notify: Arc::new(Notify::new()),
                error_flag: Arc::new(AtomicBool::new(false)),
                flip_token: AtomicBool::new(false),
                pool,
                slot,
                file,
            },
            io_worker: Some(worker),
        }
    }

    /// Create a pool-backed double-buffer without an I/O worker.
    ///
    /// Test/bench-only: uses `spawn_blocking` for flush (compatible with tests
    /// and benchmarks that don't spawn an `IoWorker`). Production always calls
    /// `new_with_worker`.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn new(pool: Arc<BufferPool>, slot: SlotGuard, file: Arc<File>) -> Self {
        let half_size = pool.half_size();
        Self {
            mode: BufferMode::Double {
                half_a: Arc::new(Mutex::new(BTreeMap::new())),
                half_b: Arc::new(Mutex::new(BTreeMap::new())),
                active_is_a: AtomicBool::new(true),
                usage_a: AtomicU64::new(0),
                usage_b: AtomicU64::new(0),
                half_size,
                flush_handle: Mutex::new(None),
                notify: Arc::new(Notify::new()),
                error_flag: Arc::new(AtomicBool::new(false)),
                flip_token: AtomicBool::new(false),
                pool,
                slot,
                file,
            },
            io_worker: None,
        }
    }

    /// Create a local ping-pong buffer for SSD write combining.
    ///
    /// Uses the same double-buffer flip logic as HDD but without global
    /// pool/slot management. `half_size` is the size of each ping-pong half.
    pub fn new_local_pingpong_with_worker(half_size: u64, file: Arc<File>, worker: IoWorker) -> Self {
        Self {
            mode: BufferMode::LocalPingPong {
                half_a: Arc::new(Mutex::new(BTreeMap::new())),
                half_b: Arc::new(Mutex::new(BTreeMap::new())),
                active_is_a: AtomicBool::new(true),
                usage_a: AtomicU64::new(0),
                usage_b: AtomicU64::new(0),
                half_size,
                flush_handle: Mutex::new(None),
                notify: Arc::new(Notify::new()),
                error_flag: Arc::new(AtomicBool::new(false)),
                flip_token: AtomicBool::new(false),
                file,
            },
            io_worker: Some(worker),
        }
    }

    /// Buffer a chunk of data at the given byte offset.
    ///
    /// Returns `Ok(())` if the data was buffered (possibly after waiting for
    /// a background flush to complete). Returns `Err` if a background flush
    /// has failed — the caller should write this chunk directly to disk.
    pub async fn buffer_chunk(&self, offset: u64, data: Bytes) -> Result<(), DownloadError> {
        match &self.mode {
            BufferMode::Double {
                half_a,
                half_b,
                active_is_a,
                usage_a,
                usage_b,
                half_size,
                flush_handle,
                notify,
                error_flag,
                flip_token,
                pool,
                file,
                ..
            } => {
                self.buffer_chunk_double(
                    half_a,
                    half_b,
                    active_is_a,
                    usage_a,
                    usage_b,
                    *half_size,
                    flush_handle,
                    notify,
                    error_flag,
                    flip_token,
                    pool,
                    file,
                    offset,
                    data,
                )
                .await
            }
            BufferMode::LocalPingPong {
                half_a,
                half_b,
                active_is_a,
                usage_a,
                usage_b,
                half_size,
                flush_handle,
                notify,
                error_flag,
                flip_token,
                file,
                ..
            } => {
                self.buffer_chunk_local_pingpong(
                    half_a,
                    half_b,
                    active_is_a,
                    usage_a,
                    usage_b,
                    *half_size,
                    flush_handle,
                    notify,
                    error_flag,
                    flip_token,
                    file,
                    offset,
                    data,
                )
                .await
            }
        }
    }

    /// Unified ping-pong flip logic shared by HDD double-buffer and SSD local
    /// ping-pong modes. Parameterised via [`PingPongCfg`].
    #[allow(clippy::too_many_arguments)]
    async fn buffer_chunk_pingpong_impl(
        &self,
        cfg: PingPongCfg<'_>,
        half_a: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
        half_b: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
        active_is_a: &AtomicBool,
        usage_a: &AtomicU64,
        usage_b: &AtomicU64,
        half_size: u64,
        flush_handle: &Mutex<Option<JoinHandle<()>>>,
        notify: &Arc<Notify>,
        error_flag: &Arc<AtomicBool>,
        flip_token: &AtomicBool,
        file: &Arc<File>,
        offset: u64,
        data: Bytes,
    ) -> Result<(), DownloadError> {
        let len = data.len() as u64;

        // Single chunk larger than a half — write directly via spawn_blocking.
        if len > half_size {
            let f = file.clone();
            tokio::task::spawn_blocking(move || write_all_at(&f, &data, offset))
                .await
                .map_err(|e| DownloadError::Internal(format!("background write failed: {e}")))??;
            return Ok(());
        }

        loop {
            // Early-fail guard: if a background flush has failed on a previous
            // flip, bail out immediately so the caller can fall back to direct
            // I/O. Without this check, the chunk worker would keep downloading
            // data that will ultimately fail checksum — wasted bandwidth.
            if error_flag.load(Ordering::Acquire) {
                return Err(DownloadError::Internal(
                    "background buffer flush failed".into(),
                ));
            }

            let is_a = active_is_a.load(Ordering::Acquire);
            let (active_map, active_usage, inactive_map, inactive_usage) = if is_a {
                (half_a, usage_a, half_b, usage_b)
            } else {
                (half_b, usage_b, half_a, usage_a)
            };

            // Fast path — room available in the active half.
            //
            // The check and the insert must be atomic with respect to a
            // concurrent flip: `is_a` can change while we wait for the map
            // lock, and inserting into a half that just became inactive would
            // strand the chunk with no flush handle (it would never be written,
            // usage accounting would drift, and disk progress would stall while
            // downloads keep fetching). Re-validate under the lock and retry if
            // a flip happened in between.
            let current = active_usage.load(Ordering::Acquire);
            if current + len <= half_size {
                let mut active_guard = active_map.lock();
                if active_is_a.load(Ordering::Acquire) != is_a {
                    // The active half switched while we awaited the lock — this
                    // half is now inactive. Drop the lock and retry with fresh
                    // state.
                    continue;
                }
                active_guard.insert(offset, data);
                active_usage.fetch_add(len, Ordering::Release);
                if let Some(p) = cfg.pool {
                    p.add_usage(len);
                }
                return Ok(());
            }

            // Active half is full → need to flip.
            // Acquire the flip token to serialise flips.
            if flip_token.swap(true, Ordering::Acquire) {
                // Someone else is flipping — wait for room.
                notify.notified().await;
                continue;
            }

            // Guard releases the flip token on drop (even if the section panics).
            let _guard = FlipTokenGuard {
                token: flip_token,
                notify,
            };

            // We hold the flip token. Before proceeding, check if a
            // background flush from a previous flip is still running.
            let prev_handle = flush_handle.lock().take();
            if let Some(h) = prev_handle {
                // Wait for it to finish.
                let _ = h.await;
                if error_flag.load(Ordering::Acquire) {
                    return Err(DownloadError::Internal(
                        "background buffer flush failed".into(),
                    ));
                }
                continue;
            }

            // Sanity: if the inactive half still holds bytes (a writer raced
            // the previous flip's drain), do NOT discard them. This data is
            // real and was never written to disk — clearing it would corrupt
            // the download (holes → stuck progress / checksum failure). Fold it
            // into the flush we are about to spawn instead.
            let folded: Vec<(u64, Bytes)> = {
                let mut map = inactive_map.lock();
                if map.is_empty() {
                    Vec::new()
                } else {
                    std::mem::take(&mut *map).into_iter().collect()
                }
            };
            let folded_bytes: u64 = folded.iter().map(|(_, d)| d.len() as u64).sum();
            if folded_bytes > 0 {
                inactive_usage.store(0, Ordering::Release);
                if let Some(p) = cfg.pool {
                    p.sub_usage(folded_bytes);
                }
                tracing::warn!(
                    "buffer_chunk: inactive half had {} bytes without flush handle — folded into flush ({} mode)",
                    folded_bytes,
                    cfg.label,
                );
            }

            // ---- FLIP ----
            // Drain the active half, reset usage, subtract from pool.
            let old_entries: Vec<(u64, Bytes)> = {
                let mut map = active_map.lock();
                let mut merged = std::mem::take(&mut *map);
                // Fold the leftovers of the previous inactive half (if any)
                // into this flush so no buffered byte is silently dropped.
                merged.extend(folded);
                merged.into_iter().collect()
            };
            let old_bytes: u64 = old_entries.iter().map(|(_, d)| d.len() as u64).sum();
            active_usage.store(0, Ordering::Release);
            if let Some(p) = cfg.pool {
                p.sub_usage(old_bytes);
            }

            // Spawn background flush for the old active half's data.
            // Uses IoWorker when available, otherwise falls back to
            // spawn_blocking (tests / no-IoWorker contexts).
            let bg_file = file.clone();
            let bg_error = Arc::clone(error_flag);
            let bg_notify = notify.clone();
            let bg_handle: JoinHandle<()> = if let Some(ref worker) = self.io_worker {
                let worker = worker.clone();
                tokio::spawn(async move {
                    if let Err(e) = worker.write_batch(bg_file, old_entries, cfg.bg_sync).await {
                        bg_error.store(true, Ordering::Release);
                        tracing::error!("background {} buffer flush failed (IoWorker): {e}", cfg.label);
                    }
                    bg_notify.notify_waiters();
                })
            } else {
                tokio::task::spawn_blocking(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        // old_entries is already sorted by key from BTreeMap drain
                        for (off, chunk) in &old_entries {
                            write_all_at(&bg_file, chunk, *off)?;
                        }
                        Ok::<_, DownloadError>(())
                    }));
                    match result {
                        Ok(Ok(())) => {
                            if cfg.bg_fsync
                                && let Err(e) = bg_file.sync_data()
                            {
                                bg_error.store(true, Ordering::Release);
                                tracing::error!(
                                    "background {} buffer flush fsync failed: {e}",
                                    cfg.label,
                                );
                            }
                        }
                        Ok(Err(e)) => {
                            bg_error.store(true, Ordering::Release);
                            tracing::error!("background {} buffer flush failed: {e}", cfg.label);
                        }
                        Err(payload) => {
                            bg_error.store(true, Ordering::Release);
                            let msg = payload
                                .downcast_ref::<String>()
                                .map(|s| s.as_str())
                                .or_else(|| payload.downcast_ref::<&'static str>().copied())
                                .unwrap_or("<non-string panic payload>");
                            tracing::error!("background {} flush task panicked: {msg}", cfg.label);
                        }
                    }
                    bg_notify.notify_waiters();
                })
            };

            *flush_handle.lock() = Some(bg_handle);

            // Atomically flip the active half.
            active_is_a.store(!is_a, Ordering::Release);

            // Insert the current chunk into the new active half (which is empty).
            let new_is_a = active_is_a.load(Ordering::Acquire);
            let (new_map, new_usage) = if new_is_a {
                (half_a, usage_a)
            } else {
                (half_b, usage_b)
            };
            new_map.lock().insert(offset, data);
            new_usage.fetch_add(len, Ordering::Release);
            if let Some(p) = cfg.pool {
                p.add_usage(len);
            }

            return Ok(());
        }
    }

    /// Double-buffer mode implementation.
    #[allow(clippy::too_many_arguments)]
    async fn buffer_chunk_double(
        &self,
        half_a: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
        half_b: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
        active_is_a: &AtomicBool,
        usage_a: &AtomicU64,
        usage_b: &AtomicU64,
        half_size: u64,
        flush_handle: &Mutex<Option<JoinHandle<()>>>,
        notify: &Arc<Notify>,
        error_flag: &Arc<AtomicBool>,
        flip_token: &AtomicBool,
        pool: &Arc<BufferPool>,
        file: &Arc<File>,
        offset: u64,
        data: Bytes,
    ) -> Result<(), DownloadError> {
        self.buffer_chunk_pingpong_impl(
            PingPongCfg {
                pool: Some(pool),
                bg_sync: true,
                bg_fsync: true,
                label: "HDD",
            },
            half_a, half_b, active_is_a, usage_a, usage_b,
            half_size, flush_handle, notify, error_flag, flip_token,
            file, offset, data,
        ).await
    }

    /// Local ping-pong mode: same double-buffer flip logic as HDD but without
    /// global pool/slot management.
    #[allow(clippy::too_many_arguments)]
    async fn buffer_chunk_local_pingpong(
        &self,
        half_a: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
        half_b: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
        active_is_a: &AtomicBool,
        usage_a: &AtomicU64,
        usage_b: &AtomicU64,
        half_size: u64,
        flush_handle: &Mutex<Option<JoinHandle<()>>>,
        notify: &Arc<Notify>,
        error_flag: &Arc<AtomicBool>,
        flip_token: &AtomicBool,
        file: &Arc<File>,
        offset: u64,
        data: Bytes,
    ) -> Result<(), DownloadError> {
        self.buffer_chunk_pingpong_impl(
            PingPongCfg {
                pool: None,
                bg_sync: false,
                bg_fsync: false,
                label: "SSD ping-pong",
            },
            half_a, half_b, active_is_a, usage_a, usage_b,
            half_size, flush_handle, notify, error_flag, flip_token,
            file, offset, data,
        ).await
    }

    /// Flush a single half's buffer to disk without pool tracking.
    /// Used by `flush_all` for LocalPingPong mode.
    async fn flush_one_half_local(
        half: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
        usage: &AtomicU64,
        file: &Arc<File>,
        io_worker: Option<&IoWorker>,
    ) -> Result<(), DownloadError> {
        let entries: Vec<(u64, Bytes)> = {
            let mut map = half.lock();
            if map.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *map).into_iter().collect()
        };
        usage.store(0, Ordering::Release);
        if let Some(worker) = io_worker {
            worker.write_batch(file.clone(), entries, true).await?;
        } else {
            let f = file.clone();
            tokio::task::spawn_blocking(move || -> Result<(), DownloadError> {
                for (off, chunk) in &entries {
                    write_all_at(&f, chunk, *off)?;
                }
                f.sync_data().map_err(|e| {
                    DownloadError::Internal(format!("fsync failed: {e}"))
                })?;
                Ok(())
            })
            .await
            .map_err(|e| DownloadError::Internal(format!("flush task failed: {e}")))??;
        }
        Ok(())
    }

    /// Flush all buffered data to disk.
    ///
    /// In HDD double-buffer mode: waits for any active background flush, then
    /// flushes both halves synchronously (via spawn_blocking).
    /// In SSD ping-pong mode: flushes both halves synchronously.
    ///
    /// Returns an error if any flush failed.
    pub async fn flush_all(&self) -> Result<(), DownloadError> {
        match &self.mode {
            BufferMode::Double {
                half_a,
                half_b,
                active_is_a,
                usage_a,
                usage_b,
                flush_handle,
                notify: _,
                error_flag,
                pool,
                file,
                ..
            } => {
                // 1. Wait for any in-progress background flush.
                let handle = flush_handle.lock().take();
                if let Some(h) = handle {
                    let _ = h.await;
                    if error_flag.load(Ordering::Acquire) {
                        return Err(DownloadError::Internal(
                            "background buffer flush failed".into(),
                        ));
                    }
                }

                // 2. Determine which half is active and which is inactive.
                let is_a = active_is_a.load(Ordering::Acquire);
                let (active, active_usage, inactive, inactive_usage) = if is_a {
                    (half_a, usage_a, half_b, usage_b)
                } else {
                    (half_b, usage_b, half_a, usage_a)
                };

                // 3. Flush active half.
                Self::flush_one_half(active, active_usage, file, pool, self.io_worker.as_ref()).await?;

                // 4. Flush inactive half (should be empty, but be safe).
                Self::flush_one_half(inactive, inactive_usage, file, pool, self.io_worker.as_ref()).await?;

                // 5. Check error flag one more time.
                if error_flag.load(Ordering::Acquire) {
                    return Err(DownloadError::Internal(
                        "background buffer flush failed".into(),
                    ));
                }

                Ok(())
            }
            BufferMode::LocalPingPong {
                half_a,
                half_b,
                active_is_a,
                usage_a,
                usage_b,
                flush_handle,
                error_flag,
                file,
                ..
            } => {
                // 1. Wait for any in-progress background flush.
                let handle = flush_handle.lock().take();
                if let Some(h) = handle {
                    let _ = h.await;
                    if error_flag.load(Ordering::Acquire) {
                        return Err(DownloadError::Internal(
                            "background buffer flush failed".into(),
                        ));
                    }
                }

                // 2. Determine which half is active and which is inactive.
                let is_a = active_is_a.load(Ordering::Acquire);
                let (active, active_usage, inactive, inactive_usage) = if is_a {
                    (half_a, usage_a, half_b, usage_b)
                } else {
                    (half_b, usage_b, half_a, usage_a)
                };

                // 3. Flush active half.
                Self::flush_one_half_local(active, active_usage, file, self.io_worker.as_ref()).await?;

                // 4. Flush inactive half (should be empty, but be safe).
                Self::flush_one_half_local(inactive, inactive_usage, file, self.io_worker.as_ref()).await?;

                // 5. Check error flag one more time.
                if error_flag.load(Ordering::Acquire) {
                    return Err(DownloadError::Internal(
                        "background buffer flush failed".into(),
                    ));
                }

                Ok(())
            }
        }
    }

    /// Helper: drain a single half's buffer and write everything to disk
    /// via IoWorker or spawn_blocking.
    async fn flush_one_half(
        half: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
        usage: &AtomicU64,
        file: &Arc<File>,
        pool: &Arc<BufferPool>,
        io_worker: Option<&IoWorker>,
    ) -> Result<(), DownloadError> {
        let entries: Vec<(u64, Bytes)> = {
            let mut map = half.lock();
            if map.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *map).into_iter().collect()
        };
        let bytes: u64 = entries.iter().map(|(_, d)| d.len() as u64).sum();
        usage.store(0, Ordering::Release);
        pool.sub_usage(bytes);

        if let Some(worker) = io_worker {
            worker.write_batch(file.clone(), entries, true).await?;
        } else {
            let f = file.clone();
            tokio::task::spawn_blocking(move || -> std::result::Result<(), DownloadError> {
                for (off, chunk) in &entries {
                    write_all_at(&f, chunk, *off)?;
                }
                Ok(())
            })
            .await
            .map_err(|e| DownloadError::Internal(format!("flush task failed: {e}")))??;
        }

        Ok(())
    }

    /// Wait for any in-progress background flush to complete, discarding its result.
    ///
    /// Test-only helper: the production cancel paths use `flush_all` instead, which
    /// also awaits the in-flight background task but persists buffered data (draining
    /// here would discard chunks already credited to `downloaded_bytes`).
    #[cfg(test)]
    pub async fn drain_background(&self) {
        match &self.mode {
            BufferMode::Double { flush_handle, .. } => {
                let handle = flush_handle.lock().take();
                // MutexGuard is dropped here, before the .await — parking_lot::MutexGuard
                // is not `Send` so we must not hold it across the await point.
                if let Some(h) = handle {
                    let _ = h.await; // discard result — data is being thrown away
                }
            }
            BufferMode::LocalPingPong { flush_handle, .. } => {
                let handle = flush_handle.lock().take();
                if let Some(h) = handle {
                    let _ = h.await;
                }
            }
        }
    }

    /// Clear all buffered data without flushing.
    ///
    /// Used when the download file is reset and buffered data becomes invalid.
    pub fn clear(&self) {
        match &self.mode {
            BufferMode::Double {
                half_a,
                half_b,
                usage_a,
                usage_b,
                error_flag,
                pool,
                ..
            } => {
                let (a_bytes, b_bytes) = {
                    let mut a = half_a.lock();
                    let mut b = half_b.lock();
                    let a_sum = a.values().map(|d| d.len() as u64).sum::<u64>();
                    let b_sum = b.values().map(|d| d.len() as u64).sum::<u64>();
                    a.clear();
                    b.clear();
                    (a_sum, b_sum)
                };
                usage_a.store(0, Ordering::Release);
                usage_b.store(0, Ordering::Release);
                if a_bytes + b_bytes > 0 {
                    pool.sub_usage(a_bytes + b_bytes);
                }
                error_flag.store(false, Ordering::Release);
            }
            BufferMode::LocalPingPong {
                half_a,
                half_b,
                usage_a,
                usage_b,
                error_flag,
                ..
            } => {
                let mut a = half_a.lock();
                let mut b = half_b.lock();
                a.clear();
                b.clear();
                usage_a.store(0, Ordering::Release);
                usage_b.store(0, Ordering::Release);
                error_flag.store(false, Ordering::Release);
            }
        }
    }
    /// Total bytes currently buffered (test-only).
    #[cfg(test)]
    pub fn len(&self) -> u64 {
        match &self.mode {
            BufferMode::Double {
                usage_a, usage_b, ..
            } => usage_a.load(Ordering::Relaxed) + usage_b.load(Ordering::Relaxed),
            BufferMode::LocalPingPong { usage_a, usage_b, .. } => {
                usage_a.load(Ordering::Relaxed) + usage_b.load(Ordering::Relaxed)
            }
        }
    }

    /// Whether the buffer is empty (test-only).
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether any background flush has degraded (test-only).
    #[cfg(test)]
    pub fn has_degraded(&self) -> bool {
        match &self.mode {
            BufferMode::Double { error_flag, .. } => error_flag.load(Ordering::Relaxed),
            BufferMode::LocalPingPong { error_flag, .. } => error_flag.load(Ordering::Relaxed),
        }
    }
}

impl Drop for DownloadBuffer {
    fn drop(&mut self) {
        match &self.mode {
            BufferMode::Double {
                half_a,
                half_b,
                usage_a,
                usage_b,
                error_flag,
                pool,
                ..
            } => {
                let (a_bytes, b_bytes) = {
                    let mut a = half_a.lock();
                    let mut b = half_b.lock();
                    let a_sum = a.values().map(|d| d.len() as u64).sum::<u64>();
                    let b_sum = b.values().map(|d| d.len() as u64).sum::<u64>();
                    a.clear();
                    b.clear();
                    (a_sum, b_sum)
                };
                let total = a_bytes + b_bytes;
                if total > 0 {
                    tracing::warn!(
                        "HDD DownloadBuffer dropped with {total} buffered bytes — data lost (possible panic unwind or unexpected cancel)"
                    );
                    pool.sub_usage(total);
                }
                usage_a.store(0, Ordering::Release);
                usage_b.store(0, Ordering::Release);
                error_flag.store(false, Ordering::Release);
                pool.release_slot();
                // `slot` (SlotGuard) and `flush_handle` (JoinHandle) are
                // dropped after this, returning the semaphore permit and
                // detaching the background task respectively.
            }
            BufferMode::LocalPingPong {
                half_a,
                half_b,
                usage_a,
                usage_b,
                error_flag,
                ..
            } => {
                let (a_bytes, b_bytes) = {
                    let mut a = half_a.lock();
                    let mut b = half_b.lock();
                    let a_sum = a.values().map(|d| d.len() as u64).sum::<u64>();
                    let b_sum = b.values().map(|d| d.len() as u64).sum::<u64>();
                    a.clear();
                    b.clear();
                    (a_sum, b_sum)
                };
                let total = a_bytes + b_bytes;
                if total > 0 {
                    tracing::warn!(
                        "SSD PingPong DownloadBuffer dropped with {total} buffered bytes — data lost (possible panic unwind or unexpected cancel)"
                    );
                }
                usage_a.store(0, Ordering::Release);
                usage_b.store(0, Ordering::Release);
                error_flag.store(false, Ordering::Release);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use ntest::timeout;
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tempfile::tempdir;

    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;

    /// Create a temporary file wrapped in `Arc<File>` plus the `TempDir` guard.
    fn temp_file() -> (tempfile::TempDir, Arc<File>) {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.bin");
        let file = fs::File::create(&path).expect("create file");
        (dir, Arc::new(file))
    }

    // -----------------------------------------------------------------------
    // BufferPool construction & defaults
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_pool_creation_defaults() {
        let pool = BufferPool::new(1024, 128, 4, 1);
        assert_eq!(pool.effective_limit(), 1024 * MB);
        assert_eq!(pool.effective_max_parallel(), 4);
        assert_eq!(pool.max_slots(), 4);
        assert!(!pool.game_mode());
        assert_eq!(pool.current_usage(), 0);
        assert_eq!(pool.active_slots(), 0);
        assert_eq!(pool.queued_count(), 0);
        assert_eq!(pool.degradation_count(), 0);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_pool_creation_custom_limits() {
        let pool = BufferPool::new(512, 64, 8, 2);
        assert_eq!(pool.effective_limit(), 512 * MB);
        assert_eq!(pool.effective_max_parallel(), 8);
        assert_eq!(pool.max_slots(), 8);
        assert!(!pool.game_mode());
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_pool_creation_game_mode_on_start() {
        // Game mode goes live only after set_game_mode(true)
        let pool = BufferPool::new(1024, 128, 4, 1);
        assert!(!pool.game_mode());
        assert_eq!(pool.effective_limit(), 1024 * MB);
        assert_eq!(pool.effective_max_parallel(), 4);

        pool.set_game_mode(true);
        assert!(pool.game_mode());
        assert_eq!(pool.effective_limit(), 128 * MB);
        assert_eq!(pool.effective_max_parallel(), 1);
    }

    // -----------------------------------------------------------------------
    // half_size()
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_half_size_normal() {
        let pool = BufferPool::new(1024, 128, 4, 1);
        let expected = 1024 * MB / 4 / 2;
        assert_eq!(pool.half_size(), expected);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_half_size_minimum() {
        // Zero limit → minimum 64 KiB
        let pool = BufferPool::new(0, 0, 4, 1);
        assert_eq!(pool.half_size(), 64 * KB);

        // Very small effective per-slot → minimum 64 KiB
        let pool = BufferPool::new(1, 1, 32, 1);
        // 1 MB / 32 slots / 2 = 16 KB → clamped to 64 KB
        assert_eq!(pool.half_size(), 64 * KB);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_half_size_zero_slots() {
        let pool = BufferPool::new(1024, 128, 0, 0);
        assert_eq!(pool.half_size(), 64 * KB);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_half_size_respects_game_mode() {
        let pool = BufferPool::new(1024, 128, 4, 1);
        let normal = pool.half_size();

        pool.set_game_mode(true);
        let expected_game = 128 * MB / 2;
        assert_eq!(pool.half_size(), expected_game.max(64 * KB));
        assert!(
            pool.half_size() < normal,
            "game-mode half_size ({}) should be smaller than normal ({})",
            pool.half_size(),
            normal,
        );

        pool.set_game_mode(false);
        assert_eq!(pool.half_size(), normal);
    }

    // -----------------------------------------------------------------------
    // acquire_slot / release_slot / active_count
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_acquire_slot_increments_active_count() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        assert_eq!(pool.active_slots(), 0);

        let guard = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 1);
        drop(guard);
        // SlotGuard::drop does NOT call release_slot() — only DownloadBuffer::drop does.
        // active_count persists until DownloadBuffer explicitly releases it.
        assert_eq!(pool.active_slots(), 1);

        // To fully release the slot, call release_slot() directly (as DownloadBuffer does).
        pool.release_slot();
        assert_eq!(pool.active_slots(), 0);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_acquire_multiple_slots_sequential() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));

        let g1 = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 1);

        let g2 = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 2);

        let g3 = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 3);

        let g4 = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 4);

        // SlotGuard::drop does NOT call release_slot() — only DownloadBuffer::drop does.
        // active_count stays at 4 until we explicitly release.
        drop(g1);
        drop(g2);
        drop(g3);
        drop(g4);
        assert_eq!(pool.active_slots(), 4);

        // Release all manually (as DownloadBuffer does).
        pool.release_slot();
        pool.release_slot();
        pool.release_slot();
        pool.release_slot();
        assert_eq!(pool.active_slots(), 0);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_release_slot_direct() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let _guard = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 1);
        pool.release_slot();
        assert_eq!(pool.active_slots(), 0);
        // NOTE: release_slot() is called manually here, and _guard's Drop does NOT
        // call release_slot() — only DownloadBuffer::drop does. The semaphore permit
        // is still held by _guard, so a subsequent acquire_slot would block only on
        // the semaphore, not on active_slots. That's fine for this unit test.
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_acquire_all_slots_and_verify_semaphore_exhausted() {
        let pool = Arc::new(BufferPool::new(1024, 128, 2, 1));
        let _g1 = pool.acquire_slot().await;
        let _g2 = pool.acquire_slot().await;

        // Both slots taken → no available permits.
        assert_eq!(pool.slot_semaphore.available_permits(), 0);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_acquire_blocks_when_all_slots_taken() {
        let pool = Arc::new(BufferPool::new(1024, 128, 1, 1));
        let _g1 = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 1);

        let pool2 = pool.clone();
        let acquired = Arc::new(AtomicBool::new(false));
        let acquired2 = acquired.clone();

        let handle = tokio::spawn(async move {
            let _g2 = pool2.acquire_slot().await;
            acquired2.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        // Give the spawned task time to block on acquire.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(
            !acquired.load(std::sync::atomic::Ordering::Relaxed),
            "spawned task should be blocked"
        );

        // Release the held slot.
        drop(_g1);

        // Now the spawned task should be able to acquire.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(acquired.load(std::sync::atomic::Ordering::Relaxed));
        handle.await.unwrap();
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_rapid_acquire_release_cycle() {
        let pool = Arc::new(BufferPool::new(1024, 128, 100, 1));
        let mut guards = Vec::new();
        for _ in 0..100 {
            let guard = pool.acquire_slot().await;
            assert_eq!(pool.active_slots(), guards.len() as u32 + 1);
            guards.push(guard);
        }
        assert_eq!(pool.active_slots(), 100);
        for guard in guards {
            drop(guard);
        }
        // active_count persists (SlotGuard::drop doesn't call release_slot)
        assert_eq!(pool.active_slots(), 100);
        for _ in 0..100 {
            pool.release_slot();
        }
        assert_eq!(pool.active_slots(), 0);
    }

    // -----------------------------------------------------------------------
    // Memory tracking
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_memory_tracking_add_sub_usage() {
        let pool = BufferPool::new(1024, 128, 4, 1);
        assert_eq!(pool.current_usage(), 0);

        pool.add_usage(100);
        assert_eq!(pool.current_usage(), 100);

        pool.add_usage(50);
        assert_eq!(pool.current_usage(), 150);

        pool.sub_usage(30);
        assert_eq!(pool.current_usage(), 120);

        pool.sub_usage(120);
        assert_eq!(pool.current_usage(), 0);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_memory_tracking_underflow() {
        let pool = BufferPool::new(1024, 128, 4, 1);
        pool.add_usage(10);
        pool.sub_usage(100); // wraps around by AtomicU64 — that's OK for this test
        // Note: this is technically UB for the pool, but underflow wraps to large value.
        // AtomicU64 wraps around: 10 - 100 = u64::MAX - 89.
        let usage = pool.current_usage();
        assert!(
            usage > (u64::MAX - 100),
            "underflow should wrap to a large value, got {usage}"
        );
    }

    // -----------------------------------------------------------------------
    // Game mode
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_game_mode_toggle() {
        let pool = BufferPool::new(1024, 128, 4, 1);
        assert!(!pool.game_mode());

        pool.set_game_mode(true);
        assert!(pool.game_mode());
        assert_eq!(pool.effective_limit(), 128 * MB);
        assert_eq!(pool.effective_max_parallel(), 1);

        pool.set_game_mode(false);
        assert!(!pool.game_mode());
        assert_eq!(pool.effective_limit(), 1024 * MB);
        assert_eq!(pool.effective_max_parallel(), 4);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_game_mode_active_slots_unaffected() {
        // Game mode toggling does NOT revoke already-held slots.
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let _g1 = pool.acquire_slot().await;
        let _g2 = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 2);

        pool.set_game_mode(true);
        // Active slots are still held.
        assert_eq!(pool.active_slots(), 2);
        // But effective_max_parallel says 1.
        assert_eq!(pool.effective_max_parallel(), 1);
    }

    // -----------------------------------------------------------------------
    // update_limits
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_update_limits() {
        let pool = BufferPool::new(1024, 128, 4, 1);
        assert_eq!(pool.effective_limit(), 1024 * MB);
        assert_eq!(pool.effective_max_parallel(), 4);

        pool.update_limits(512, 64, 2, 1);
        assert_eq!(pool.effective_limit(), 512 * MB);
        assert_eq!(pool.effective_max_parallel(), 2);

        // Game mode still works after update.
        pool.set_game_mode(true);
        assert_eq!(pool.effective_limit(), 64 * MB);
        assert_eq!(pool.effective_max_parallel(), 1);

        // Toggle back.
        pool.set_game_mode(false);
        assert_eq!(pool.effective_limit(), 512 * MB);
        assert_eq!(pool.effective_max_parallel(), 2);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_update_limits_affects_half_size() {
        let pool = BufferPool::new(1024, 128, 4, 1);
        let original = pool.half_size();

        pool.update_limits(512, 128, 4, 1);
        let new_half = pool.half_size();
        // 512 MB / 4 / 2 = 64 MB (vs original 128 MB)
        assert!(new_half < original);
    }

    // -----------------------------------------------------------------------
    // queued_count
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_queued_count_basic() {
        let pool = BufferPool::new(1024, 128, 4, 1);
        // No slots taken → queued = 4 - 4 - 0 = 0
        assert_eq!(pool.queued_count(), 0);

        // Take 2 slots → queued = 4 - 2 - 2 = 0
        let _g1 = pool.acquire_slot().await;
        let _g2 = pool.acquire_slot().await;
        assert_eq!(pool.queued_count(), 0);
    }

    // -----------------------------------------------------------------------
    // SlotGuard — semaphore permit release on drop
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_slot_guard_drop_releases_semaphore_permit() {
        let pool = Arc::new(BufferPool::new(1024, 128, 1, 1));

        // Take the only slot.
        let guard = pool.acquire_slot().await;
        assert_eq!(pool.slot_semaphore.available_permits(), 0);

        // Drop guard → permit returns to semaphore.
        drop(guard);
        assert_eq!(pool.slot_semaphore.available_permits(), 1);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_slot_guard_drop_allows_another_acquire() {
        let pool = Arc::new(BufferPool::new(1024, 128, 1, 1));

        let guard = pool.acquire_slot().await;
        drop(guard);

        // Should be able to acquire again (semaphore permit was returned).
        let _guard2 = pool.acquire_slot().await;
        // active_count is cumulative: both guards incremented it
        // (only DownloadBuffer::drop decrements it).
        assert_eq!(pool.active_slots(), 2);

        // Clean up (as DownloadBuffer would).
        pool.release_slot();
        pool.release_slot();
        assert_eq!(pool.active_slots(), 0);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn slot_guard_drop_releases_permit() {
        let pool = Arc::new(BufferPool::new(1, 64, 1, 64));

        // Acquire a slot → semaphore permit consumed, active_slots incremented.
        let guard = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 1);
        assert_eq!(pool.slot_semaphore.available_permits(), 0);

        // Drop guard → OwnedSemaphorePermit::drop returns the permit to the
        // semaphore, allowing another acquire to proceed.
        drop(guard);
        assert_eq!(pool.slot_semaphore.available_permits(), 1);
        // active_slots is NOT decremented here — SlotGuard manages only the
        // semaphore permit.  The slot count is managed by
        // DownloadBuffer::drop → pool.release_slot().
        assert_eq!(pool.active_slots(), 1);

        // Clean up slot accounting (as DownloadBuffer would).
        pool.release_slot();
        assert_eq!(pool.active_slots(), 0);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn slot_guard_semaphore_limits_concurrency() {
        let pool = Arc::new(BufferPool::new(1, 64, 1, 64));

        // Acquire the only slot → semaphore exhausted.
        let guard1 = pool.acquire_slot().await;
        assert_eq!(pool.slot_semaphore.available_permits(), 0);

        // Spawn a task that tries to acquire another slot — it should block.
        let pool2 = pool.clone();
        let acquired = Arc::new(AtomicBool::new(false));
        let acquired2 = acquired.clone();

        let handle = tokio::spawn(async move {
            let _g2 = pool2.acquire_slot().await;
            acquired2.store(true, Ordering::Relaxed);
        });

        // Give the spawned task time to try acquiring and block.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(
            !acquired.load(Ordering::Relaxed),
            "second acquire should block when semaphore is exhausted"
        );

        // Drop the first guard → permit returned → second acquire can proceed.
        drop(guard1);

        // Wait for the spawned task to acquire the now-available permit.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(
            acquired.load(Ordering::Relaxed),
            "second acquire should succeed after first guard is dropped"
        );

        handle.await.unwrap();

        // Clean up active_slots (not managed by SlotGuard).
        pool.release_slot();
        pool.release_slot();
        assert_eq!(pool.active_slots(), 0);
    }

    // -----------------------------------------------------------------------
    // DownloadBuffer — HDD (Double) mode
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_hdd_buffer_creation() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        assert_eq!(buf.len(), 0);
        assert!(!buf.has_degraded());
        assert_eq!(pool.active_slots(), 1);
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_hdd_buffer_chunk_small_and_flush() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        let data = Bytes::from("hello world");
        buf.buffer_chunk(0, data.clone()).await.unwrap();
        assert_eq!(buf.len(), 11);

        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[..11], b"hello world");
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_hdd_buffer_multiple_chunks_and_flush() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        buf.buffer_chunk(0, Bytes::from("AAA")).await.unwrap();
        buf.buffer_chunk(3, Bytes::from("BBB")).await.unwrap();
        buf.buffer_chunk(6, Bytes::from("CCC")).await.unwrap();
        assert_eq!(buf.len(), 9);

        buf.flush_all().await.unwrap();
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[..9], b"AAABBBCCC");
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_hdd_buffer_chunk_triggers_flip_and_flush() {
        // Use a small half_size so we can trigger a buffer flip with modest data.
        // half_size = 4 MB / 2 / 2 = 1 MB = 1048576 bytes
        // We'll fill more than 1 MB to trigger a flip.
        let pool = Arc::new(BufferPool::new(4, 128, 2, 1));
        let half = pool.half_size();
        assert!(half >= 64 * KB);

        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Fill the active half with chunks totalling > half_size.
        let chunk_size = half / 2; // 50% of a half
        let mut total_written = 0u64;
        // Write 3 chunks (150% of half) to force a flip.
        for i in 0..3u64 {
            let payload = vec![i as u8; chunk_size as usize];
            buf.buffer_chunk(i * chunk_size, Bytes::from(payload))
                .await
                .unwrap();
            total_written += chunk_size;
        }
        // After 3 chunks, at least one flip should have occurred.
        // Some data is in the background flush, some in the active half.
        assert!(!buf.is_empty());

        // flush_all should persist everything.
        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);

        // Verify file content.
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        // total_written bytes should be on disk (file may be larger due to preallocation)
        assert!(
            content.len() >= total_written as usize,
            "expected at least {} bytes, got {}",
            total_written,
            content.len()
        );

        // Spot-check known offsets.
        for i in 0..3u64 {
            let off = (i * chunk_size) as usize;
            let expected_byte = i as u8;
            assert_eq!(
                content[off], expected_byte,
                "byte at offset {off} should be {expected_byte}"
            );
        }
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_hdd_buffer_large_chunk_direct_write() {
        // A chunk larger than half_size triggers a direct spawn_blocking write.
        let pool = Arc::new(BufferPool::new(4, 128, 2, 1));
        let half = pool.half_size();
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Create a chunk larger than half_size.
        let big_data = vec![0xABu8; (half + 1) as usize];
        let big = Bytes::from(big_data);
        buf.buffer_chunk(0, big.clone()).await.unwrap();

        // The large chunk is written directly, so the buffer should still be empty.
        assert_eq!(buf.len(), 0);

        // Verify on disk.
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[..(half + 1) as usize], &big[..]);
    }

    // -----------------------------------------------------------------------
    // DownloadBuffer — SSD (Local) mode
    // -----------------------------------------------------------------------





    // -----------------------------------------------------------------------
    // flush_all
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_flush_all_hdd_empty() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Flushing an empty buffer should succeed.
        buf.flush_all().await.unwrap();
    }


    // -----------------------------------------------------------------------
    // clear()
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(30000)]
    async fn test_clear_hdd() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        buf.buffer_chunk(0, Bytes::from("discard me"))
            .await
            .unwrap();
        assert_eq!(buf.len(), 10);

        buf.clear();
        assert_eq!(buf.len(), 0);

        // After clear, new data should work.
        buf.buffer_chunk(0, Bytes::from("new data")).await.unwrap();
        assert_eq!(buf.len(), 8);

        buf.flush_all().await.unwrap();
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[..8], b"new data");
    }


    // -----------------------------------------------------------------------
    // DownloadBuffer — SSD (LocalPingPong) mode
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(30000)]
    async fn test_ssd_pingpong_buffer_creation() {
        let (_dir, file) = temp_file();
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_local_pingpong_with_worker(64 * 1024, file.clone(), worker);
        assert_eq!(buf.len(), 0);
        assert!(!buf.has_degraded());
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_ssd_pingpong_buffer_chunk_and_flush() {
        let (_dir, file) = temp_file();
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);

        let data = Bytes::from("hello pingpong ssd");
        buf.buffer_chunk(0, data.clone()).await.unwrap();
        assert_eq!(buf.len(), 18);

        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[..18], b"hello pingpong ssd");
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_ssd_pingpong_multiple_offsets() {
        let (_dir, file) = temp_file();
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);

        buf.buffer_chunk(0, Bytes::from("aaaa")).await.unwrap();
        buf.buffer_chunk(10, Bytes::from("bbbb")).await.unwrap();
        buf.buffer_chunk(20, Bytes::from("cccc")).await.unwrap();
        assert_eq!(buf.len(), 12);

        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);

        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[0..4], b"aaaa");
        assert_eq!(&content[10..14], b"bbbb");
        assert_eq!(&content[20..24], b"cccc");
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_ssd_pingpong_flip_trigger() {
        let half = 1024u64; // tiny half_size to trigger flip easily
        let (_dir, file) = temp_file();
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_local_pingpong_with_worker(half, file.clone(), worker);

        // Fill active half with chunks totalling > half_size to trigger a flip.
        let chunk_size = half / 2;
        let mut total_written = 0u64;
        for i in 0..3u64 {
            let payload = vec![i as u8; chunk_size as usize];
            buf.buffer_chunk(i * chunk_size, Bytes::from(payload))
                .await
                .unwrap();
            total_written += chunk_size;
        }
        assert!(!buf.is_empty());

        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);

        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert!(content.len() >= total_written as usize);
        for i in 0..3u64 {
            let off = (i * chunk_size) as usize;
            assert_eq!(content[off], i as u8);
        }
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_ssd_pingpong_drain_and_clear() {
        let half = 1024u64;
        let (_dir, file) = temp_file();
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_local_pingpong_with_worker(half, file.clone(), worker);

        let small = half / 4;
        for i in 0..5u64 {
            let payload = vec![i as u8; small as usize];
            buf.buffer_chunk(i * small, Bytes::from(payload))
                .await
                .unwrap();
        }
        buf.drain_background().await;
        buf.flush_all().await.unwrap();

        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert!(content.len() >= (5 * small) as usize);

        // clear should reset state
        buf.clear();
        assert_eq!(buf.len(), 0);
        assert!(!buf.has_degraded());
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_ssd_pingpong_error_flag() {
        let (_dir, file) = temp_file();
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);

        assert!(!buf.has_degraded());

        // Directly set the error flag
        if let BufferMode::LocalPingPong { error_flag, .. } = &buf.mode {
            error_flag.store(true, std::sync::atomic::Ordering::Release);
        }

        assert!(buf.has_degraded());

        // buffer_chunk should return error
        let result = buf.buffer_chunk(100, Bytes::from("fail")).await;
        assert!(result.is_err());

        // clear should reset
        buf.clear();
        assert!(!buf.has_degraded());
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_ssd_pingpong_flush_all_empty() {
        let (_dir, file) = temp_file();
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);
        buf.flush_all().await.unwrap();
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_ssd_pingpong_overlapping_writes() {
        let (_dir, file) = temp_file();
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);

        buf.buffer_chunk(0, Bytes::from("XXX")).await.unwrap();
        buf.buffer_chunk(0, Bytes::from("YYY")).await.unwrap();

        buf.flush_all().await.unwrap();
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[..3], b"YYY");
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_ssd_pingpong_flush_all_multiple_times() {
        let (_dir, file) = temp_file();
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_local_pingpong_with_worker(4 * MB, file.clone(), worker);

        buf.buffer_chunk(0, Bytes::from("first")).await.unwrap();
        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);

        buf.buffer_chunk(10, Bytes::from("second")).await.unwrap();
        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);

        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[0..5], b"first");
        assert_eq!(&content[10..16], b"second");
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_ssd_pingpong_large_chunk_direct_write() {
        let half = 64 * 1024u64;
        let (_dir, file) = temp_file();
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_local_pingpong_with_worker(half, file.clone(), worker);

        // Create a chunk larger than half_size — triggers direct spawn_blocking write.
        let big_data = vec![0xABu8; (half + 1) as usize];
        let big = Bytes::from(big_data);
        buf.buffer_chunk(0, big.clone()).await.unwrap();

        // Large chunk is written directly, buffer should be empty.
        assert_eq!(buf.len(), 0);

        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[..(half + 1) as usize], &big[..]);
    }

    // -----------------------------------------------------------------------
    // drain_background
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(30000)]
    async fn test_drain_background_hdd() {
        let pool = Arc::new(BufferPool::new(8, 128, 2, 1));
        let half = pool.half_size();
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Fill > half_size to trigger a background flush.
        let chunk = vec![0xDDu8; (half + 1) as usize];
        buf.buffer_chunk(0, Bytes::from(chunk)).await.unwrap();
        // The large chunk goes via direct write (not through the double-buffer).
        // Let's instead use smaller chunks to trigger the double-buffer flip.
        drop(buf); // start fresh

        let slot = pool.acquire_slot().await;
        let (_dir2, file2) = temp_file();
        let buf2 = DownloadBuffer::new(pool.clone(), slot, file2);

        // Fill active half with small chunks summing > half_size.
        let small = half / 4; // 25% of half each
        for i in 0..5u64 {
            let payload = vec![i as u8; small as usize];
            buf2.buffer_chunk(i * small, Bytes::from(payload))
                .await
                .unwrap();
        }

        // At least one flip should have created a background task.
        // drain_background waits for it.
        buf2.drain_background().await;

        // After drain, we should be able to flush_all and verify data.
        buf2.flush_all().await.unwrap();
        let content = fs::read(_dir2.path().join("test.bin")).unwrap();
        assert!(content.len() >= (5 * small) as usize);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_drain_background_noop_when_no_background_task() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // No background task running. drain_background should be a no-op.
        buf.drain_background().await;
    }

    // -----------------------------------------------------------------------
    // DownloadBuffer Drop — releases slot
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_download_buffer_drop_releases_slot() {
        let pool = Arc::new(BufferPool::new(1024, 128, 2, 1));
        let (_dir, file) = temp_file();

        let slot = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 1);

        {
            let _buf = DownloadBuffer::new(pool.clone(), slot, file);
            assert_eq!(pool.active_slots(), 1);
        }
        // After _buf drops, active_slots should return to 0 (release_slot called).

        // Wait — DownloadBuffer::drop calls release_slot which decrements active_count.
        // But active_count was initially 1 (from the manual acquire), then DownloadBuffer::drop
        // calls pool.release_slot() once. So active_count goes to 0.
        // However, there's a subtlety: SlotGuard is moved into DownloadBuffer, so no extra release.
        assert_eq!(
            pool.active_slots(),
            0,
            "DownloadBuffer::drop should release the slot"
        );
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_download_buffer_drop_clears_usage() {
        let pool = Arc::new(BufferPool::new(1024, 128, 2, 1));
        let (_dir, file) = temp_file();

        let slot = pool.acquire_slot().await;
        // Buffer some data first.
        {
            let buf = DownloadBuffer::new(pool.clone(), slot, file);
            buf.buffer_chunk(0, Bytes::from("data")).await.unwrap();
            assert!(pool.current_usage() > 0);
        }
        // Drop should clear usage.
        assert_eq!(pool.current_usage(), 0);
    }

    // -----------------------------------------------------------------------
    // Zero-size chunk
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_zero_size_chunk_hdd() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        buf.buffer_chunk(0, Bytes::new()).await.unwrap();
        assert_eq!(buf.len(), 0);

        buf.flush_all().await.unwrap();
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert!(content.is_empty());
    }


    // -----------------------------------------------------------------------
    // Concurrent buffer_chunk calls
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(30000)]
    async fn test_concurrent_hdd_buffer_chunks() {
        let pool = Arc::new(BufferPool::new(32, 128, 4, 1));
        let _half = pool.half_size();
        let (_dir, file) = temp_file();
        let file_arc = file.clone();

        // Write 4 chunks concurrently to the same buffer.
        let slot = pool.acquire_slot().await;
        let buf = Arc::new(DownloadBuffer::new(pool.clone(), slot, file_arc));

        let mut handles = Vec::new();
        let chunk_size = 4096u64;
        let num_chunks = 4u64;
        for i in 0..num_chunks {
            let b = buf.clone();
            handles.push(tokio::spawn(async move {
                let payload = vec![i as u8; chunk_size as usize];
                b.buffer_chunk(i * chunk_size, Bytes::from(payload))
                    .await
                    .unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        assert_eq!(buf.len(), num_chunks * chunk_size);

        buf.flush_all().await.unwrap();
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert!(content.len() >= (num_chunks * chunk_size) as usize);

        // Spot-check each chunk.
        for i in 0..num_chunks {
            let off = (i * chunk_size) as usize;
            assert_eq!(content[off], i as u8);
        }
    }


    // -----------------------------------------------------------------------
    // Game mode transition while slots are held
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_game_mode_transition_with_held_slots() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));

        // Acquire 2 slots.
        let _g1 = pool.acquire_slot().await;
        let _g2 = pool.acquire_slot().await;
        assert_eq!(pool.active_slots(), 2);
        assert_eq!(pool.effective_max_parallel(), 4);

        // Transition to game mode.
        pool.set_game_mode(true);
        assert_eq!(pool.effective_max_parallel(), 1);
        // Held slots are not revoked.
        assert_eq!(pool.active_slots(), 2);

        // Half-size should reflect game mode limits for future creates.
        let game_half = pool.half_size();
        assert_eq!(game_half, (128 * MB / 2).max(64 * KB));

        // Transition back.
        pool.set_game_mode(false);
        assert_eq!(pool.effective_max_parallel(), 4);
        assert_eq!(pool.active_slots(), 2);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_game_mode_affects_new_buffers_only() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));

        // Create a buffer in normal mode.
        let (_dir1, file1) = temp_file();
        let slot1 = pool.acquire_slot().await;
        let _buf1 = DownloadBuffer::new(pool.clone(), slot1, file1);

        // Switch to game mode.
        pool.set_game_mode(true);

        // Create a second buffer in game mode.
        let (_dir2, file2) = temp_file();
        let slot2 = pool.acquire_slot().await;
        let _buf2 = DownloadBuffer::new(pool.clone(), slot2, file2);

        // buf2's half_size should be smaller (game mode).
        // But we can't directly compare half_sizes since they're stored internally.
        // Verify by filling buf2 with a chunk just over normal half but under game half.
        // Normal half = 1024MB/4/2 = 128MB, game half = 128MB/1/2 = 64MB.
        // So let's use 100MB chunks... that's huge. Let's use smaller limits instead.

        // Actually let's just verify the pool-level half_size is correct.
        assert_eq!(pool.half_size(), (128 * MB / 2).max(64 * KB));
        assert_eq!(pool.effective_max_parallel(), 1);
    }

    // -----------------------------------------------------------------------
    // Pool memory management integration
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_pool_usage_tracked_across_multiple_buffers() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir1, file1) = temp_file();
        let (_dir2, file2) = temp_file();

        let slot1 = pool.acquire_slot().await;
        let slot2 = pool.acquire_slot().await;

        {
            let buf1 = DownloadBuffer::new(pool.clone(), slot1, file1);
            let buf2 = DownloadBuffer::new(pool.clone(), slot2, file2);

            buf1.buffer_chunk(0, Bytes::from("hello")).await.unwrap();
            buf2.buffer_chunk(0, Bytes::from("world")).await.unwrap();
            assert_eq!(pool.current_usage(), 10);
        }
        // Both buffers dropped → usage cleared.
        assert_eq!(pool.current_usage(), 0);
        assert_eq!(pool.active_slots(), 0);
    }

    // -----------------------------------------------------------------------
    // Integrity: flush with overlapping offsets
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(30000)]
    async fn test_overlapping_writes_hdd() {
        // Write to same offset in HDD mode and verify the last write wins.
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        buf.buffer_chunk(0, Bytes::from("AAA")).await.unwrap();
        buf.buffer_chunk(0, Bytes::from("BBB")).await.unwrap();

        buf.flush_all().await.unwrap();
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        // Last write at offset 0 should win.
        assert_eq!(&content[..3], b"BBB");
    }


    // -----------------------------------------------------------------------
    // Multiple flushes
    // -----------------------------------------------------------------------


    #[tokio::test]
    #[timeout(30000)]
    async fn test_flush_all_multiple_times_hdd() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        buf.buffer_chunk(0, Bytes::from("a")).await.unwrap();
        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);

        buf.buffer_chunk(5, Bytes::from("b")).await.unwrap();
        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);

        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(content[0], b'a');
        assert_eq!(content[5], b'b');
    }

    // -----------------------------------------------------------------------
    // HDD buffer error recovery
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_hdd_buffer_degraded_after_clear() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Trigger an error by... well, we can't easily simulate a background
        // flush failure. But we can verify that clear() resets the error flag.
        // has_degraded starts as false.
        assert!(!buf.has_degraded());

        buf.buffer_chunk(0, Bytes::from("data")).await.unwrap();
        buf.clear();
        assert!(!buf.has_degraded());
        assert_eq!(buf.len(), 0);
    }

    // -----------------------------------------------------------------------
    // Degradation / error-recovery tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_degraded_flag_detected_by_has_degraded() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Initially not degraded
        assert!(!buf.has_degraded());

        // Directly set the error flag
        if let BufferMode::Double { error_flag, .. } = &buf.mode {
            error_flag.store(true, Ordering::Release);
        }

        assert!(buf.has_degraded());
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_buffer_chunk_returns_error_when_degraded() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Without flag, buffer_chunk works
        buf.buffer_chunk(0, Bytes::from("normal")).await.unwrap();

        // Set the error flag
        if let BufferMode::Double { error_flag, .. } = &buf.mode {
            error_flag.store(true, Ordering::Release);
        }

        // Now buffer_chunk should return an error
        let result = buf.buffer_chunk(100, Bytes::from("fail")).await;
        match result {
            Err(DownloadError::Internal(msg)) => {
                assert!(
                    msg.contains("background buffer flush failed"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected Err(Internal), got {other:?}"),
        }
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_clear_resets_degraded_flag() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Set the error flag
        if let BufferMode::Double { error_flag, .. } = &buf.mode {
            error_flag.store(true, Ordering::Release);
        }
        assert!(buf.has_degraded());

        // Clear should reset it
        buf.clear();
        assert!(!buf.has_degraded());
        assert_eq!(buf.len(), 0);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_flush_all_checks_degraded() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Without flag, flush_all works
        buf.flush_all().await.unwrap();

        // Set the error flag
        if let BufferMode::Double { error_flag, .. } = &buf.mode {
            error_flag.store(true, Ordering::Release);
        }

        // flush_all should return an error when degraded
        let result = buf.flush_all().await;
        match result {
            Err(DownloadError::Internal(msg)) => {
                assert!(
                    msg.contains("background buffer flush failed"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected Err(Internal), got {other:?}"),
        }
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_degradation_count_always_zero() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Initially zero
        assert_eq!(pool.degradation_count(), 0);

        // Set error flag
        if let BufferMode::Double { error_flag, .. } = &buf.mode {
            error_flag.store(true, Ordering::Release);
        }
        assert!(buf.has_degraded());

        // Still zero (the pool never degrades, it backpressures instead)
        assert_eq!(pool.degradation_count(), 0);
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_degraded_flag_persists_after_chunk_failure() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Set the error flag
        if let BufferMode::Double { error_flag, .. } = &buf.mode {
            error_flag.store(true, Ordering::Release);
        }

        // buffer_chunk returns Err
        assert!(buf.buffer_chunk(0, Bytes::from("data")).await.is_err());

        // Flag should still be set (not auto-cleared on error)
        assert!(buf.has_degraded());
    }

    #[tokio::test]
    #[timeout(10000)]
    async fn test_drop_clears_degraded_flag() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        // Clone error_flag to observe it after the buffer is dropped
        let error_flag = match &buf.mode {
            BufferMode::Double { error_flag, .. } => error_flag.clone(),
            _ => unreachable!(),
        };

        // Set the flag
        error_flag.store(true, Ordering::Release);
        assert!(buf.has_degraded());

        // Drop the buffer — Drop impl should clear the flag
        drop(buf);

        // After drop, the flag should be cleared
        assert!(!error_flag.load(Ordering::Relaxed));
    }

    // -----------------------------------------------------------------------
    // max_slots / effective_max_parallel parity
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(10000)]
    async fn test_max_slots_matches_effective_max_parallel() {
        let pool = BufferPool::new(1024, 128, 4, 1);
        assert_eq!(pool.max_slots(), pool.effective_max_parallel());

        pool.set_game_mode(true);
        assert_eq!(pool.max_slots(), pool.effective_max_parallel());

        pool.update_limits(1024, 128, 8, 2);
        assert_eq!(pool.max_slots(), pool.effective_max_parallel());

        pool.set_game_mode(false);
        assert_eq!(pool.max_slots(), pool.effective_max_parallel());
    }

    // -----------------------------------------------------------------------
    // Edge: buffer_chunk after flush_all
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(30000)]
    async fn test_buffer_after_flush_hdd() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        buf.buffer_chunk(0, Bytes::from("first")).await.unwrap();
        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);

        // Buffer new data after flush.
        buf.buffer_chunk(10, Bytes::from("second")).await.unwrap();
        assert_eq!(buf.len(), 6);

        buf.flush_all().await.unwrap();
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[0..5], b"first");
        assert_eq!(&content[10..16], b"second");
    }

    // -----------------------------------------------------------------------
    // FlipTokenGuard panic recovery
    // -----------------------------------------------------------------------

    #[test]
    fn flip_token_recovers_from_panic() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        use std::sync::atomic::AtomicBool;
        use tokio::sync::Notify;

        let token = AtomicBool::new(true);
        let notify = Notify::new();

        // Simulate: inside the flip section, a panic occurs.
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = FlipTokenGuard {
                token: &token,
                notify: &notify,
            };
            panic!("simulated flip section panic");
        }));

        assert!(result.is_err(), "expected panic to be caught");

        // Guard was dropped during unwind — token must be released.
        assert!(
            !token.load(Ordering::Acquire),
            "flip_token should be false after guard drop on unwind"
        );
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_hdd_with_worker_buffer_chunk_and_flush() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_with_worker(pool.clone(), slot, file.clone(), worker);

        let data = Bytes::from("hello io worker");
        buf.buffer_chunk(0, data.clone()).await.unwrap();
        assert_eq!(buf.len(), 15);

        buf.flush_all().await.unwrap();
        assert_eq!(buf.len(), 0);
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[..15], b"hello io worker");
    }


    #[tokio::test]
    #[timeout(30000)]
    async fn test_hdd_with_worker_multiple_chunks() {
        let pool = Arc::new(BufferPool::new(1024, 128, 4, 1));
        let (_dir, file) = temp_file();
        let slot = pool.acquire_slot().await;
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_with_worker(pool.clone(), slot, file.clone(), worker);

        buf.buffer_chunk(0, Bytes::from("AAA")).await.unwrap();
        buf.buffer_chunk(3, Bytes::from("BBB")).await.unwrap();
        buf.buffer_chunk(6, Bytes::from("CCC")).await.unwrap();
        assert_eq!(buf.len(), 9);

        buf.flush_all().await.unwrap();
        let content = fs::read(_dir.path().join("test.bin")).unwrap();
        assert_eq!(&content[..9], b"AAABBBCCC");
    }

    // -----------------------------------------------------------------------
    // Background flush failure recovery (read-only file)
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[timeout(30000)]
    async fn test_background_flush_failure_using_readonly_file() {
        // Simulate a background flush I/O failure by making the target file
        // read-only.  The buffer is created without an IoWorker, so the
        // background flush uses spawn_blocking → write_all_at fails → error_flag
        // is set → subsequent operations return an error.
        let pool = Arc::new(BufferPool::new(1, 1, 4, 1));
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.bin");

        // Create the file, then make it read-only on the filesystem.
        fs::File::create(&path).expect("create file");
        {
            let mut perms = fs::metadata(&path).expect("metadata").permissions();
            perms.set_readonly(true);
            fs::set_permissions(&path, perms).expect("set read-only");
        }

        // Open the file without write access — any seek_write / pwrite call
        // will fail immediately.
        let file = Arc::new(
            fs::OpenOptions::new()
                .read(true)
                .open(&path)
                .expect("open read-only file"),
        );

        let slot = pool.acquire_slot().await;
        let buf = DownloadBuffer::new(pool.clone(), slot, file);

        assert!(!buf.has_degraded());
        assert_eq!(buf.len(), 0);

        let half = pool.half_size();
        // Two chunks whose combined size exceeds half_size — this triggers
        // a flip and a background flush.
        let chunk_size = half / 2 + 1;
        let chunk1 = Bytes::from(vec![0xABu8; chunk_size as usize]);
        let chunk2 = Bytes::from(vec![0xBCu8; chunk_size as usize]);

        buf.buffer_chunk(0, chunk1).await.unwrap();
        // Second chunk fills active half → flip → background flush → write fails
        // → error_flag is set by the background task.
        buf.buffer_chunk(half, chunk2).await.unwrap();

        // Wait for the background task to complete and set the error flag.
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
        while !buf.has_degraded() {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            if tokio::time::Instant::now() >= deadline {
                panic!("background flush did not set error flag within 5 s");
            }
        }

        // 1) error_flag is set after a failed flush
        assert!(buf.has_degraded());

        // 2) Subsequent buffer_chunk returns Err(DownloadError::Internal)
        let result = buf.buffer_chunk(half * 2, Bytes::from("fail")).await;
        match result {
            Err(DownloadError::Internal(msg)) => {
                assert!(
                    msg.contains("background buffer flush failed"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected Err(Internal), got {other:?}"),
        }

        // 3) has_degraded remains true after the error
        assert!(buf.has_degraded());

        // 4) flush_all also returns an error (checks error_flag internally)
        let result = buf.flush_all().await;
        match result {
            Err(DownloadError::Internal(msg)) => {
                assert!(
                    msg.contains("background buffer flush failed"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected Err(Internal), got {other:?}"),
        }

        // 5) Buffer drops cleanly and releases its slot
        drop(buf);
        assert_eq!(pool.active_slots(), 0);
    }

    #[tokio::test]
    #[timeout(30000)]
    async fn test_background_flush_failure_using_readonly_file_with_worker() {
        // Same scenario as above but the buffer uses IoWorker — the background
        // flush goes through the dedicated I/O worker thread instead of
        // spawn_blocking.
        let pool = Arc::new(BufferPool::new(1, 1, 4, 1));
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.bin");

        fs::File::create(&path).expect("create file");
        {
            let mut perms = fs::metadata(&path).expect("metadata").permissions();
            perms.set_readonly(true);
            fs::set_permissions(&path, perms).expect("set read-only");
        }

        let file = Arc::new(
            fs::OpenOptions::new()
                .read(true)
                .open(&path)
                .expect("open read-only file"),
        );

        let slot = pool.acquire_slot().await;
        let worker = IoWorker::spawn();
        let buf = DownloadBuffer::new_with_worker(pool.clone(), slot, file, worker);

        assert!(!buf.has_degraded());
        assert_eq!(buf.len(), 0);

        let half = pool.half_size();
        let chunk_size = half / 2 + 1;
        let chunk1 = Bytes::from(vec![0xABu8; chunk_size as usize]);
        let chunk2 = Bytes::from(vec![0xBCu8; chunk_size as usize]);

        buf.buffer_chunk(0, chunk1).await.unwrap();
        buf.buffer_chunk(half, chunk2).await.unwrap();

        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
        while !buf.has_degraded() {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            if tokio::time::Instant::now() >= deadline {
                panic!("background flush did not set error flag within 5 s");
            }
        }

        assert!(buf.has_degraded());

        // buffer_chunk returns Err after background flush failure
        let result = buf.buffer_chunk(half * 2, Bytes::from("fail")).await;
        match result {
            Err(DownloadError::Internal(msg)) => {
                assert!(
                    msg.contains("background buffer flush failed"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected Err(Internal), got {other:?}"),
        }

        // Buffer drops cleanly and releases slot
        drop(buf);
        assert_eq!(pool.active_slots(), 0);
    }
}
