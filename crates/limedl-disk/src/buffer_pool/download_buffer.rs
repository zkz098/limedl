use std::collections::BTreeMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bytes::Bytes;
use parking_lot::Mutex;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use super::worker::IoWorker;
use super::{BufferPool, SlotGuard};
use crate::error::DownloadError;
use crate::file_ops::SyncMode;
use crate::file_ops::write_all_at;

/// RAII guard that releases the flip token and notifies waiters on drop.
/// Prevents deadlock if the flip section panics.
pub struct FlipTokenGuard<'a> {
    pub(crate) token: &'a AtomicBool,
    pub(crate) notify: &'a Notify,
}

impl<'a> Drop for FlipTokenGuard<'a> {
    fn drop(&mut self) {
        self.token.store(false, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

/// Notified once a flush has actually written its byte ranges to the file.
///
/// Each element is `(offset, len)`. This is how the download engine learns which
/// bytes are safe to persist: the write buffer is the only component that knows
/// when buffered data left memory, so the manifest's durable counters are
/// advanced from here and nowhere else.
///
/// Invoked on the flushing task (an `IoWorker` await or a blocking task) after the
/// write returned successfully, never on failure.
pub type FlushObserver = Arc<dyn Fn(&[(u64, u64)]) + Send + Sync>;

/// Configuration for the shared ping-pong flip logic.
struct PingPongCfg<'a> {
    /// Global buffer pool for HDD memory tracking. `None` for SSD (ping-pong) mode.
    pool: Option<&'a Arc<BufferPool>>,
    /// Whether to fsync in the IoWorker background flush path.
    bg_sync: SyncMode,
    /// Whether to fsync in the spawn_blocking fallback background flush path.
    bg_fsync: bool,
    /// Label for tracing/error messages (e.g., "HDD" or "SSD ping-pong").
    label: &'static str,
    /// Durable-progress notification for successful flushes.
    observer: Option<&'a FlushObserver>,
}

/// Synchronized foreground accumulation half for [`DownloadBuffer`].
///
/// Keeping the offset map and its tracked byte count inside a single
/// `parking_lot::Mutex` ensures capacity checks, overlapping-offset
/// replacements, and flip drains execute atomically without multi-variable
/// tear windows. The second ("pong") half of the double-buffer is the
/// `Vec<(u64, Bytes)>` batch owned by the in-flight background flush task in
/// `flush_handle`.
#[derive(Default)]
pub struct PingPongState {
    entries: BTreeMap<u64, Bytes>,
    usage: u64,
}

impl PingPongState {
    /// Try to insert `(offset, data)` if it fits within `half_size`.
    ///
    /// Accounts for overlapping-offset replacements so `self.usage` and the
    /// global `pool` counter always match the exact sum of buffered bytes.
    fn try_insert(
        &mut self,
        offset: u64,
        data: &Bytes,
        half_size: u64,
        pool: Option<&Arc<BufferPool>>,
    ) -> bool {
        let len = data.len() as u64;
        let prev_len = self.entries.get(&offset).map_or(0, |b| b.len() as u64);
        let next_usage = self.usage - prev_len + len;
        if next_usage > half_size {
            return false;
        }
        self.entries.insert(offset, data.clone());
        self.usage = next_usage;
        adjust_pool_usage(pool, prev_len, len);
        true
    }

    /// Insert `(offset, data)` into a freshly drained state.
    fn insert_fresh(&mut self, offset: u64, data: Bytes, pool: Option<&Arc<BufferPool>>) {
        let len = data.len() as u64;
        let prev_len = self
            .entries
            .insert(offset, data)
            .map_or(0, |b| b.len() as u64);
        self.usage = self.usage - prev_len + len;
        adjust_pool_usage(pool, prev_len, len);
    }

    /// Drain all buffered entries in offset order and reset tracked usage.
    fn drain(&mut self, pool: Option<&Arc<BufferPool>>) -> Vec<(u64, Bytes)> {
        if self.entries.is_empty() {
            self.usage = 0;
            return Vec::new();
        }
        let entries: Vec<(u64, Bytes)> = std::mem::take(&mut self.entries).into_iter().collect();
        let bytes = std::mem::replace(&mut self.usage, 0);
        if let Some(p) = pool
            && bytes > 0
        {
            p.sub_usage(bytes);
        }
        entries
    }

    /// Discard all buffered entries and return the number of bytes cleared.
    fn clear(&mut self, pool: Option<&Arc<BufferPool>>) -> u64 {
        self.entries.clear();
        let bytes = std::mem::replace(&mut self.usage, 0);
        if let Some(p) = pool
            && bytes > 0
        {
            p.sub_usage(bytes);
        }
        bytes
    }
}

/// Update global pool usage when a buffered entry changes size from `prev_len` to `new_len`.
fn adjust_pool_usage(pool: Option<&Arc<BufferPool>>, prev_len: u64, new_len: u64) {
    if let Some(p) = pool {
        if new_len > prev_len {
            p.add_usage(new_len - prev_len);
        } else if prev_len > new_len {
            p.sub_usage(prev_len - new_len);
        }
    }
}

/// Internal mode for `DownloadBuffer`.
pub enum BufferMode {
    /// Double-buffer mode used for HDD downloads.
    Double {
        state: Mutex<PingPongState>,
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
        state: Mutex<PingPongState>,
        half_size: u64,
        flush_handle: Mutex<Option<JoinHandle<()>>>,
        notify: Arc<Notify>,
        error_flag: Arc<AtomicBool>,
        flip_token: AtomicBool,
        file: Arc<File>,
    },
}

/// Where a flush goes: the device sink plus the path that resolves which device
/// queue the batch belongs to.
///
/// The two are one thing — a sink cannot route a batch without a path — so they
/// are stored together instead of as two `Option`s that could disagree, which is
/// how the deleted path-less fallback used to be reachable.
struct FlushSink {
    worker: IoWorker,
    path: PathBuf,
}

/// A per-download buffer that accumulates chunks in memory and flushes them
/// to disk via a background double-buffer ping-pong (HDD / SSD).
pub struct DownloadBuffer {
    pub(crate) mode: BufferMode,
    /// `Some` on every production path; `None` only for the test/bench-only
    /// `new()`/`new_local_pingpong()` constructors, which write directly.
    flush_sink: Option<FlushSink>,
    /// Durable-progress notification, shared with every flush path.
    flush_observer: Option<FlushObserver>,
}

impl DownloadBuffer {
    /// Create a pool-backed double-buffer with a dedicated I/O worker.
    ///
    /// `file_path` is required: it selects the device queue the flushes are
    /// routed to, so a buffer without one could not flush.
    pub fn new_with_worker(
        pool: Arc<BufferPool>,
        slot: SlotGuard,
        file: Arc<File>,
        file_path: PathBuf,
        worker: IoWorker,
        flush_observer: Option<FlushObserver>,
    ) -> Self {
        let half_size = pool.half_size();
        Self {
            mode: BufferMode::Double {
                state: Mutex::new(PingPongState::default()),
                half_size,
                flush_handle: Mutex::new(None),
                notify: Arc::new(Notify::new()),
                error_flag: Arc::new(AtomicBool::new(false)),
                flip_token: AtomicBool::new(false),
                pool,
                slot,
                file,
            },
            flush_sink: Some(FlushSink {
                worker,
                path: file_path,
            }),
            flush_observer,
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
                state: Mutex::new(PingPongState::default()),
                half_size,
                flush_handle: Mutex::new(None),
                notify: Arc::new(Notify::new()),
                error_flag: Arc::new(AtomicBool::new(false)),
                flip_token: AtomicBool::new(false),
                pool,
                slot,
                file,
            },
            flush_sink: None,
            flush_observer: None,
        }
    }

    /// Create a local ping-pong buffer for SSD write combining.
    ///
    /// Uses the same double-buffer flip logic as HDD but without global
    /// pool/slot management. `half_size` is the size of each ping-pong half.
    /// `file_path` is required for the same reason as in [`Self::new_with_worker`].
    pub fn new_local_pingpong_with_worker(
        half_size: u64,
        file: Arc<File>,
        file_path: PathBuf,
        worker: IoWorker,
        flush_observer: Option<FlushObserver>,
    ) -> Self {
        Self {
            mode: BufferMode::LocalPingPong {
                state: Mutex::new(PingPongState::default()),
                half_size,
                flush_handle: Mutex::new(None),
                notify: Arc::new(Notify::new()),
                error_flag: Arc::new(AtomicBool::new(false)),
                flip_token: AtomicBool::new(false),
                file,
            },
            flush_sink: Some(FlushSink {
                worker,
                path: file_path,
            }),
            flush_observer,
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
                state,
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
                    PingPongRefs {
                        state,
                        half_size: *half_size,
                        flush_handle,
                        notify,
                        error_flag,
                        flip_token,
                        file,
                    },
                    pool,
                    offset,
                    data,
                )
                .await
            }
            BufferMode::LocalPingPong {
                state,
                half_size,
                flush_handle,
                notify,
                error_flag,
                flip_token,
                file,
                ..
            } => {
                self.buffer_chunk_local_pingpong(
                    PingPongRefs {
                        state,
                        half_size: *half_size,
                        flush_handle,
                        notify,
                        error_flag,
                        flip_token,
                        file,
                    },
                    offset,
                    data,
                )
                .await
            }
        }
    }

    /// Unified ping-pong flip logic shared by HDD double-buffer and SSD local
    /// ping-pong modes. Parameterised via [`PingPongCfg`].
    async fn buffer_chunk_pingpong_impl(
        &self,
        cfg: PingPongCfg<'_>,
        halves: PingPongRefs<'_>,
        offset: u64,
        data: Bytes,
    ) -> Result<(), DownloadError> {
        let len = data.len() as u64;

        // Single chunk larger than a half — write directly via spawn_blocking.
        if len > halves.half_size {
            return write_oversized_direct(halves.file, data, offset, cfg.observer).await;
        }

        loop {
            // Early-fail guard: if a background flush has failed on a previous
            // flip, bail out immediately so the caller can fall back to direct
            // I/O. Without this check, the chunk worker would keep downloading
            // data that will ultimately fail checksum — wasted bandwidth.
            if halves.error_flag.load(Ordering::Acquire) {
                return Err(DownloadError::Internal(
                    "background buffer flush failed".into(),
                ));
            }

            // Fast path — check capacity and insert atomically under `state` lock.
            if halves
                .state
                .lock()
                .try_insert(offset, &data, halves.half_size, cfg.pool)
            {
                return Ok(());
            }

            // Active half is full → need to flip.
            // Register notification interest BEFORE attempting to acquire `flip_token`
            // so that a concurrent flipper dropping `FlipTokenGuard` (calling
            // `notify_waiters()`) between our `swap(true)` and `.await` cannot lose
            // the wakeup.
            let notified = halves.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            // Acquire the flip token to serialise flips.
            if halves.flip_token.swap(true, Ordering::SeqCst) {
                // Someone else is flipping — wait for room.
                notified.await;
                continue;
            }

            // Guard releases the flip token on drop (even if the section panics).
            let _guard = FlipTokenGuard {
                token: halves.flip_token,
                notify: halves.notify,
            };

            // Re-check capacity under `state` lock in case another flipper just
            // rotated the buffer right before we acquired `flip_token`.
            if halves
                .state
                .lock()
                .try_insert(offset, &data, halves.half_size, cfg.pool)
            {
                return Ok(());
            }

            // We hold the flip token and the active half is full. Wait for any
            // previous background flush to finish before rotating the active half out.
            let prev_handle = halves.flush_handle.lock().take();
            if let Some(h) = prev_handle {
                let _ = h.await;
                if halves.error_flag.load(Ordering::Acquire) {
                    return Err(DownloadError::Internal(
                        "background buffer flush failed".into(),
                    ));
                }
            }

            // Lock `state` to atomically drain the full half, spawn its background
            // flush, record the flush handle, and insert the new chunk into the
            // freshly drained active half.
            let mut state = halves.state.lock();
            if state.try_insert(offset, &data, halves.half_size, cfg.pool) {
                return Ok(());
            }

            // ---- FLIP ----
            let old_entries = state.drain(cfg.pool);
            let bg_handle = spawn_background_flush(
                self.flush_sink.as_ref(),
                &cfg,
                halves.file,
                halves.error_flag,
                halves.notify,
                old_entries,
            );
            *halves.flush_handle.lock() = Some(bg_handle);
            state.insert_fresh(offset, data, cfg.pool);

            return Ok(());
        }
    }

    /// Double-buffer mode implementation.
    async fn buffer_chunk_double(
        &self,
        halves: PingPongRefs<'_>,
        pool: &Arc<BufferPool>,
        offset: u64,
        data: Bytes,
    ) -> Result<(), DownloadError> {
        let cfg = PingPongCfg {
            pool: Some(pool),
            bg_sync: SyncMode::Adaptive,
            bg_fsync: false,
            label: "HDD",
            observer: self.flush_observer.as_ref(),
        };
        self.buffer_chunk_pingpong_impl(cfg, halves, offset, data)
            .await
    }

    /// Local ping-pong mode: same double-buffer flip logic as HDD but without
    /// global pool/slot management.
    async fn buffer_chunk_local_pingpong(
        &self,
        halves: PingPongRefs<'_>,
        offset: u64,
        data: Bytes,
    ) -> Result<(), DownloadError> {
        let cfg = PingPongCfg {
            pool: None,
            bg_sync: SyncMode::None,
            bg_fsync: false,
            label: "SSD ping-pong",
            observer: self.flush_observer.as_ref(),
        };
        self.buffer_chunk_pingpong_impl(cfg, halves, offset, data)
            .await
    }

    /// Flush all buffered data to disk.
    ///
    /// Waits for any active background flush, then flushes the foreground
    /// accumulation buffer synchronously (via `IoWorker` or `spawn_blocking`).
    ///
    /// Returns an error if any flush failed.
    pub async fn flush_all(&self) -> Result<(), DownloadError> {
        match &self.mode {
            BufferMode::Double {
                state,
                flush_handle,
                error_flag,
                pool,
                file,
                ..
            } => {
                self.flush_all_impl(
                    state,
                    flush_handle,
                    error_flag,
                    file,
                    Some(pool),
                    SyncMode::Force,
                    false,
                )
                .await
            }
            BufferMode::LocalPingPong {
                state,
                flush_handle,
                error_flag,
                file,
                ..
            } => {
                self.flush_all_impl(
                    state,
                    flush_handle,
                    error_flag,
                    file,
                    None,
                    SyncMode::None,
                    true,
                )
                .await
            }
        }
    }

    /// Shared implementation of [`Self::flush_all`] for both buffer modes.
    #[allow(clippy::too_many_arguments)]
    async fn flush_all_impl(
        &self,
        state: &Mutex<PingPongState>,
        flush_handle: &Mutex<Option<JoinHandle<()>>>,
        error_flag: &Arc<AtomicBool>,
        file: &Arc<File>,
        pool: Option<&Arc<BufferPool>>,
        sync_mode: SyncMode,
        fallback_fsync: bool,
    ) -> Result<(), DownloadError> {
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

        // 2. Drain and flush the active buffer.
        let entries = state.lock().drain(pool);
        if !entries.is_empty() {
            let ranges = entry_ranges(&entries);
            if let Some(sink) = self.flush_sink.as_ref() {
                sink.worker
                    .write_batch(&sink.path, file.clone(), entries, sync_mode)
                    .await?;
            } else {
                let f = file.clone();
                tokio::task::spawn_blocking(move || -> Result<(), DownloadError> {
                    for (off, chunk) in &entries {
                        write_all_at(&f, chunk, *off)?;
                    }
                    if fallback_fsync {
                        f.sync_data()
                            .map_err(|e| DownloadError::Internal(format!("fsync failed: {e}")))?;
                    }
                    Ok(())
                })
                .await
                .map_err(|e| DownloadError::Internal(format!("flush task failed: {e}")))??;
            }
            notify_flushed(self.flush_observer.as_ref(), &ranges);
        }

        // 3. Check error flag one more time.
        if error_flag.load(Ordering::Acquire) {
            return Err(DownloadError::Internal(
                "background buffer flush failed".into(),
            ));
        }

        Ok(())
    }

    /// Wait for any in-progress background flush to complete, discarding its result.
    pub async fn drain_background(&self) {
        match &self.mode {
            BufferMode::Double { flush_handle, .. }
            | BufferMode::LocalPingPong { flush_handle, .. } => {
                let handle = flush_handle.lock().take();
                if let Some(h) = handle {
                    let _ = h.await;
                }
            }
        }
    }

    /// Wait for any in-flight background flush to finish, then clear all
    /// buffered data without flushing.
    pub async fn clear_and_wait(&self) {
        self.drain_background().await;
        self.clear();
    }

    /// Clear all buffered data without flushing.
    pub fn clear(&self) {
        match &self.mode {
            BufferMode::Double {
                state,
                error_flag,
                pool,
                ..
            } => {
                state.lock().clear(Some(pool));
                error_flag.store(false, Ordering::Release);
            }
            BufferMode::LocalPingPong {
                state, error_flag, ..
            } => {
                state.lock().clear(None);
                error_flag.store(false, Ordering::Release);
            }
        }
    }

    /// Total bytes currently buffered (test-only).
    #[cfg(any(test, feature = "test-utils"))]
    pub fn len(&self) -> u64 {
        match &self.mode {
            BufferMode::Double { state, .. } | BufferMode::LocalPingPong { state, .. } => {
                state.lock().usage
            }
        }
    }

    /// Whether the buffer is empty (test-only).
    #[cfg(any(test, feature = "test-utils"))]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether any background flush has degraded (test-only).
    #[cfg(any(test, feature = "test-utils"))]
    pub fn has_degraded(&self) -> bool {
        match &self.mode {
            BufferMode::Double { error_flag, .. }
            | BufferMode::LocalPingPong { error_flag, .. } => error_flag.load(Ordering::Relaxed),
        }
    }
}

// ── Ping-pong helpers ────────────────────────────────────────────────────────

/// Borrowed state of one ping-pong buffer, shared by both buffer modes.
struct PingPongRefs<'a> {
    state: &'a Mutex<PingPongState>,
    half_size: u64,
    flush_handle: &'a Mutex<Option<JoinHandle<()>>>,
    notify: &'a Arc<Notify>,
    error_flag: &'a Arc<AtomicBool>,
    flip_token: &'a AtomicBool,
    file: &'a Arc<File>,
}

/// A single chunk larger than a half bypasses the buffer entirely.
async fn write_oversized_direct(
    file: &Arc<File>,
    data: Bytes,
    offset: u64,
    observer: Option<&FlushObserver>,
) -> Result<(), DownloadError> {
    let len = data.len() as u64;
    let f = file.clone();
    tokio::task::spawn_blocking(move || write_all_at(&f, &data, offset))
        .await
        .map_err(|e| DownloadError::Internal(format!("background write failed: {e}")))??;
    notify_flushed(observer, &[(offset, len)]);
    Ok(())
}

/// The `(offset, len)` pairs of a flush batch, for the observer.
fn entry_ranges(entries: &[(u64, Bytes)]) -> Vec<(u64, u64)> {
    entries
        .iter()
        .map(|(offset, data)| (*offset, data.len() as u64))
        .collect()
}

/// Report ranges as durable. Never called after a failed write.
fn notify_flushed(observer: Option<&FlushObserver>, ranges: &[(u64, u64)]) {
    if let Some(observer) = observer {
        observer(ranges);
    }
}

/// Spawn the background flush for a half that was just rotated out.
///
/// With a sink, the batch goes to the device queue; without one (the
/// test/bench-only constructors) it is written inline on `spawn_blocking` with a
/// panic guard (unwind builds only — see the note in the body).  Both paths set
/// `error_flag` on failure and notify waiters when done.
fn spawn_background_flush(
    sink: Option<&FlushSink>,
    cfg: &PingPongCfg<'_>,
    file: &Arc<File>,
    error_flag: &Arc<AtomicBool>,
    notify: &Arc<Notify>,
    entries: Vec<(u64, Bytes)>,
) -> JoinHandle<()> {
    let bg_file = file.clone();
    let bg_error = Arc::clone(error_flag);
    let bg_notify = notify.clone();
    let bg_sync = cfg.bg_sync;
    let bg_fsync = cfg.bg_fsync;
    let label = cfg.label;
    let bg_observer = cfg.observer.cloned();
    let ranges = entry_ranges(&entries);

    if let Some(sink) = sink {
        let worker = sink.worker.clone();
        let bg_path = sink.path.clone();
        tokio::spawn(async move {
            match worker
                .write_batch(&bg_path, bg_file, entries, bg_sync)
                .await
            {
                Ok(()) => notify_flushed(bg_observer.as_ref(), &ranges),
                Err(e) => {
                    bg_error.store(true, Ordering::Release);
                    tracing::error!("background {label} buffer flush failed (device queue): {e}");
                }
            }
            bg_notify.notify_waiters();
        })
    } else {
        tokio::task::spawn_blocking(move || {
            // The panic guard is a development-build (and `test-utils`) safety
            // net only. The shipping profile builds with `panic = "abort"`
            // (workspace `Cargo.toml`), where `catch_unwind` cannot catch
            // anything: a panic here takes the process down, and the only
            // evidence is the native client's panic hook (`crash.rs`). Do not
            // treat this arm as production error recovery.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                write_all_entries(&bg_file, &entries)
            }));
            match result {
                Ok(Ok(())) => {
                    if bg_fsync && let Err(e) = bg_file.sync_data() {
                        bg_error.store(true, Ordering::Release);
                        tracing::error!("background {label} buffer flush fsync failed: {e}");
                    }
                    // `sync_data` failing still means the bytes are in the file
                    // (only the power-loss guarantee is missing), so the ranges
                    // count as durable either way.
                    notify_flushed(bg_observer.as_ref(), &ranges);
                }
                Ok(Err(e)) => {
                    bg_error.store(true, Ordering::Release);
                    tracing::error!("background {label} buffer flush failed: {e}");
                }
                Err(payload) => {
                    bg_error.store(true, Ordering::Release);
                    let msg = panic_payload_message(payload.as_ref());
                    tracing::error!("background {label} flush task panicked: {msg}");
                }
            }
            bg_notify.notify_waiters();
        })
    }
}

/// Write a batch of entries sequentially.
fn write_all_entries(file: &File, entries: &[(u64, Bytes)]) -> Result<(), DownloadError> {
    for (off, chunk) in entries {
        write_all_at(file, chunk, *off)?;
    }
    Ok(())
}

/// Message from a `catch_unwind` payload, for the flush-failure log.
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&'static str>().copied())
        .unwrap_or("<non-string panic payload>")
}

impl Drop for DownloadBuffer {
    fn drop(&mut self) {
        match &self.mode {
            BufferMode::Double {
                state,
                error_flag,
                pool,
                ..
            } => {
                let total = state.lock().clear(Some(pool));
                if total > 0 {
                    tracing::warn!(
                        "HDD DownloadBuffer dropped with {total} buffered bytes — data lost (possible panic unwind or unexpected cancel)"
                    );
                }
                error_flag.store(false, Ordering::Release);
                pool.release_slot();
            }
            BufferMode::LocalPingPong {
                state, error_flag, ..
            } => {
                let total = state.lock().clear(None);
                if total > 0 {
                    tracing::warn!(
                        "SSD PingPong DownloadBuffer dropped with {total} buffered bytes — data lost (possible panic unwind or unexpected cancel)"
                    );
                }
                error_flag.store(false, Ordering::Release);
            }
        }
    }
}
