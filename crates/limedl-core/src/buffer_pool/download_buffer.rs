use std::collections::BTreeMap;
use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use parking_lot::Mutex;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use super::worker::IoWorker;
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
            let current = active_usage.load(Ordering::Acquire);
            if current + len <= half_size {
                let mut active_guard = active_map.lock();
                if active_is_a.load(Ordering::Acquire) != is_a {
                    // The active half switched while we awaited the lock — retry.
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

            // We hold the flip token. Check if a background flush is still running.
            let prev_handle = flush_handle.lock().take();
            if let Some(h) = prev_handle {
                let _ = h.await;
                if error_flag.load(Ordering::Acquire) {
                    return Err(DownloadError::Internal(
                        "background buffer flush failed".into(),
                    ));
                }
                continue;
            }

            // Sanity: fold leftovers of previous inactive half if any.
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
            let old_entries: Vec<(u64, Bytes)> = {
                let mut map = active_map.lock();
                let mut merged = std::mem::take(&mut *map);
                merged.extend(folded);
                merged.into_iter().collect()
            };
            let old_bytes: u64 = old_entries.iter().map(|(_, d)| d.len() as u64).sum();
            active_usage.store(0, Ordering::Release);
            if let Some(p) = cfg.pool {
                p.sub_usage(old_bytes);
            }

            // Spawn background flush for the old active half's data.
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

            // Insert the current chunk into the new active half.
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
