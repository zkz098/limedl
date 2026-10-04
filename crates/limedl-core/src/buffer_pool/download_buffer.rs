use std::collections::BTreeMap;
use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use parking_lot::Mutex;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use super::worker::{IoWorker, SyncMode};
use super::{BufferPool, SlotGuard};
use crate::error::DownloadError;
use crate::file_ops::write_all_at;

/// RAII guard that releases the flip token and notifies waiters on drop.
/// Prevents deadlock if the flip section panics.
pub(crate) struct FlipTokenGuard<'a> {
    pub(crate) token: &'a AtomicBool,
    pub(crate) notify: &'a Notify,
}

impl<'a> Drop for FlipTokenGuard<'a> {
    fn drop(&mut self) {
        self.token.store(false, Ordering::Release);
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

/// Internal mode for `DownloadBuffer`.
pub(crate) enum BufferMode {
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
    pub(crate) mode: BufferMode,
    io_worker: Option<IoWorker>,
    /// Durable-progress notification, shared with every flush path.
    flush_observer: Option<FlushObserver>,
}

impl DownloadBuffer {
    /// Create a pool-backed double-buffer with a dedicated I/O worker.
    pub fn new_with_worker(
        pool: Arc<BufferPool>,
        slot: SlotGuard,
        file: Arc<File>,
        worker: IoWorker,
        flush_observer: Option<FlushObserver>,
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
            flush_observer: None,
        }
    }

    /// Create a local ping-pong buffer for SSD write combining.
    ///
    /// Uses the same double-buffer flip logic as HDD but without global
    /// pool/slot management. `half_size` is the size of each ping-pong half.
    pub fn new_local_pingpong_with_worker(
        half_size: u64,
        file: Arc<File>,
        worker: IoWorker,
        flush_observer: Option<FlushObserver>,
    ) -> Self {
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
                    PingPongRefs {
                        half_a,
                        half_b,
                        active_is_a,
                        usage_a,
                        usage_b,
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
                    PingPongRefs {
                        half_a,
                        half_b,
                        active_is_a,
                        usage_a,
                        usage_b,
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

            let is_a = halves.active_is_a.load(Ordering::Acquire);
            let selected = halves.select(is_a);

            // Fast path — room available in the active half.
            let current = selected.active_usage.load(Ordering::Acquire);
            if current + len <= halves.half_size {
                let mut active_guard = selected.active_map.lock();
                if halves.active_is_a.load(Ordering::Acquire) != is_a {
                    // The active half switched while we awaited the lock — retry.
                    continue;
                }
                active_guard.insert(offset, data);
                selected.active_usage.fetch_add(len, Ordering::Release);
                if let Some(p) = cfg.pool {
                    p.add_usage(len);
                }
                return Ok(());
            }

            // Active half is full → need to flip.
            // Acquire the flip token to serialise flips.
            if halves.flip_token.swap(true, Ordering::Acquire) {
                // Someone else is flipping — wait for room.
                halves.notify.notified().await;
                continue;
            }

            // Guard releases the flip token on drop (even if the section panics).
            let _guard = FlipTokenGuard {
                token: halves.flip_token,
                notify: halves.notify,
            };

            // We hold the flip token. Check if a background flush is still running.
            let prev_handle = halves.flush_handle.lock().take();
            if let Some(h) = prev_handle {
                let _ = h.await;
                if halves.error_flag.load(Ordering::Acquire) {
                    return Err(DownloadError::Internal(
                        "background buffer flush failed".into(),
                    ));
                }
                continue;
            }

            let folded = fold_leftovers(selected.inactive_map, selected.inactive_usage, &cfg);

            // ---- FLIP ----
            let old_entries = take_entries_with(selected.active_map, folded);
            let old_bytes: u64 = old_entries.iter().map(|(_, d)| d.len() as u64).sum();
            selected.active_usage.store(0, Ordering::Release);
            if let Some(p) = cfg.pool {
                p.sub_usage(old_bytes);
            }

            // Spawn background flush for the old active half's data.
            let bg_handle = spawn_background_flush(
                self.io_worker.as_ref(),
                &cfg,
                halves.file,
                halves.error_flag,
                halves.notify,
                old_entries,
            );

            *halves.flush_handle.lock() = Some(bg_handle);

            // Atomically flip the active half.
            halves.active_is_a.store(!is_a, Ordering::Release);

            // Insert the current chunk into the new active half.
            insert_new_active(&halves, &cfg, offset, data, len);

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
        self.buffer_chunk_pingpong_impl(cfg, halves, offset, data).await
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
        self.buffer_chunk_pingpong_impl(cfg, halves, offset, data).await
    }

    /// Flush a single half's buffer to disk without pool tracking.
    /// Used by `flush_all` for LocalPingPong mode.
    async fn flush_one_half_local(
        half: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
        usage: &AtomicU64,
        file: &Arc<File>,
        io_worker: Option<&IoWorker>,
        observer: Option<&FlushObserver>,
    ) -> Result<(), DownloadError> {
        let entries: Vec<(u64, Bytes)> = {
            let mut map = half.lock();
            if map.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *map).into_iter().collect()
        };
        let ranges = entry_ranges(&entries);
        usage.store(0, Ordering::Release);
        if let Some(worker) = io_worker {
            worker.write_batch(file.clone(), entries, SyncMode::None).await?;
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
        notify_flushed(observer, &ranges);
        Ok(())
    }

    /// Flush all buffered data to disk.
    ///
    /// In HDD double-buffer mode: waits for any active background flush, then
    /// flushes both halves synchronously (via spawn_blocking / IoWorker).
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
                Self::flush_one_half(
                    active,
                    active_usage,
                    file,
                    pool,
                    self.io_worker.as_ref(),
                    self.flush_observer.as_ref(),
                )
                .await?;

                // 4. Flush inactive half (should be empty, but be safe).
                Self::flush_one_half(
                    inactive,
                    inactive_usage,
                    file,
                    pool,
                    self.io_worker.as_ref(),
                    self.flush_observer.as_ref(),
                )
                .await?;

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
                Self::flush_one_half_local(
                    active,
                    active_usage,
                    file,
                    self.io_worker.as_ref(),
                    self.flush_observer.as_ref(),
                )
                .await?;

                // 4. Flush inactive half (should be empty, but be safe).
                Self::flush_one_half_local(
                    inactive,
                    inactive_usage,
                    file,
                    self.io_worker.as_ref(),
                    self.flush_observer.as_ref(),
                )
                .await?;

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
        observer: Option<&FlushObserver>,
    ) -> Result<(), DownloadError> {
        let entries: Vec<(u64, Bytes)> = {
            let mut map = half.lock();
            if map.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *map).into_iter().collect()
        };
        let ranges = entry_ranges(&entries);
        let bytes: u64 = entries.iter().map(|(_, d)| d.len() as u64).sum();
        usage.store(0, Ordering::Release);
        pool.sub_usage(bytes);

        if let Some(worker) = io_worker {
            worker.write_batch(file.clone(), entries, SyncMode::Force).await?;
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

        notify_flushed(observer, &ranges);
        Ok(())
    }

    /// Wait for any in-progress background flush to complete, discarding its result.
    #[cfg(test)]
    pub async fn drain_background(&self) {
        match &self.mode {
            BufferMode::Double { flush_handle, .. } => {
                let handle = flush_handle.lock().take();
                if let Some(h) = handle {
                    let _ = h.await;
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

// ── Ping-pong helpers ────────────────────────────────────────────────────────

/// Borrowed state of one ping-pong buffer, shared by both buffer modes.
struct PingPongRefs<'a> {
    half_a: &'a Arc<Mutex<BTreeMap<u64, Bytes>>>,
    half_b: &'a Arc<Mutex<BTreeMap<u64, Bytes>>>,
    active_is_a: &'a AtomicBool,
    usage_a: &'a AtomicU64,
    usage_b: &'a AtomicU64,
    half_size: u64,
    flush_handle: &'a Mutex<Option<JoinHandle<()>>>,
    notify: &'a Arc<Notify>,
    error_flag: &'a Arc<AtomicBool>,
    flip_token: &'a AtomicBool,
    file: &'a Arc<File>,
}

/// Maps/atomics of the selected active and inactive halves.
struct Halves<'a> {
    active_map: &'a Arc<Mutex<BTreeMap<u64, Bytes>>>,
    active_usage: &'a AtomicU64,
    inactive_map: &'a Arc<Mutex<BTreeMap<u64, Bytes>>>,
    inactive_usage: &'a AtomicU64,
}

impl<'a> PingPongRefs<'a> {
    /// Active/inactive pair for the current `is_a` value.
    fn select(&self, is_a: bool) -> Halves<'a> {
        if is_a {
            Halves {
                active_map: self.half_a,
                active_usage: self.usage_a,
                inactive_map: self.half_b,
                inactive_usage: self.usage_b,
            }
        } else {
            Halves {
                active_map: self.half_b,
                active_usage: self.usage_b,
                inactive_map: self.half_a,
                inactive_usage: self.usage_a,
            }
        }
    }
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

/// Fold (and account) leftovers of the previous inactive half, if any.
fn fold_leftovers(
    inactive_map: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
    inactive_usage: &AtomicU64,
    cfg: &PingPongCfg<'_>,
) -> Vec<(u64, Bytes)> {
    let folded = take_entries(inactive_map);
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
    folded
}

/// Drain an offset map into offset-sorted entries.
fn take_entries(map: &Arc<Mutex<BTreeMap<u64, Bytes>>>) -> Vec<(u64, Bytes)> {
    let mut guard = map.lock();
    std::mem::take(&mut *guard).into_iter().collect()
}

/// Drain an offset map and append `extra` entries (folded leftovers).
fn take_entries_with(
    map: &Arc<Mutex<BTreeMap<u64, Bytes>>>,
    extra: Vec<(u64, Bytes)>,
) -> Vec<(u64, Bytes)> {
    let mut guard = map.lock();
    let mut merged = std::mem::take(&mut *guard);
    merged.extend(extra);
    merged.into_iter().collect()
}

/// Spawn the background flush for a half that was just rotated out.
///
/// Prefers the dedicated `IoWorker`; otherwise falls back to `spawn_blocking`
/// with a panic guard (unwind builds only — see the note in the body).
/// Both paths set `error_flag` on failure and notify waiters when done.
fn spawn_background_flush(
    io_worker: Option<&IoWorker>,
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

    if let Some(worker) = io_worker {
        let worker = worker.clone();
        tokio::spawn(async move {
            match worker.write_batch(bg_file, entries, bg_sync).await {
                Ok(()) => notify_flushed(bg_observer.as_ref(), &ranges),
                Err(e) => {
                    bg_error.store(true, Ordering::Release);
                    tracing::error!("background {label} buffer flush failed (IoWorker): {e}");
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

/// After a flip, insert the pending chunk into the new active half.
fn insert_new_active(
    halves: &PingPongRefs<'_>,
    cfg: &PingPongCfg<'_>,
    offset: u64,
    data: Bytes,
    len: u64,
) {
    let (new_map, new_usage) = if halves.active_is_a.load(Ordering::Acquire) {
        (halves.half_a, halves.usage_a)
    } else {
        (halves.half_b, halves.usage_b)
    };
    new_map.lock().insert(offset, data);
    new_usage.fetch_add(len, Ordering::Release);
    if let Some(p) = cfg.pool {
        p.add_usage(len);
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
