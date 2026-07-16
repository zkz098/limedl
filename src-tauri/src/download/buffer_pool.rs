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

use bytes::Bytes;
use dashmap::DashMap;
use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use parking_lot::Mutex;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;

use super::error::DownloadError;
use super::file_alloc::write_all_at;

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
            .expect("semaphore closed");
        self.active_count.fetch_add(1, Ordering::Relaxed);
        SlotGuard::new(permit)
    }

    /// Release a slot permit (called from `DownloadBuffer::Drop`).
    #[allow(dead_code)]
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
    /// Note: this does NOT resize the semaphore dynamically. The semaphore
    /// capacity was set at construction time (or from a previous
    /// `update_limits` that requires a restart). Current max-parallel values
    /// are used for `half_size()` computation and display only.
    pub fn update_limits(
        &self,
        total_limit_mb: u64,
        game_mode_limit_mb: u64,
        max_parallel: u32,
        game_mode_max_parallel: u32,
    ) {
        self.total_limit_mb
            .store(total_limit_mb, Ordering::Relaxed);
        self.game_mode_limit_mb
            .store(game_mode_limit_mb, Ordering::Relaxed);
        self.max_parallel.store(max_parallel, Ordering::Relaxed);
        self.game_mode_max_parallel
            .store(game_mode_max_parallel, Ordering::Relaxed);
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
// DownloadBuffer — per-download double buffer (HDD) or local buffer (SSD)
// ---------------------------------------------------------------------------

/// Internal mode for `DownloadBuffer`.
enum BufferMode {
    /// Double-buffer mode used for HDD downloads.
    Double {
        half_a: Arc<DashMap<u64, Bytes>>,
        half_b: Arc<DashMap<u64, Bytes>>,
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
    /// Local-limit mode used for SSD write combining.
    Local {
        chunks: Arc<DashMap<u64, Bytes>>,
        buffered_bytes: AtomicU64,
        local_limit: u64,
        file: Arc<File>,
    },
}

/// A per-download buffer that accumulates chunks in memory and flushes them
/// to disk either sequentially (local mode) or via a background double-buffer
/// ping-pong (HDD mode).
pub struct DownloadBuffer {
    mode: BufferMode,
}

impl DownloadBuffer {
    /// Create a pool-backed double-buffer for HDD downloads.
    ///
    /// `pool` — the global buffer pool.
    /// `slot` — a pre-acquired slot permit from `pool.acquire_slot()`.
    /// `file` — the output file wrapped in `Arc` for background I/O.
    pub fn new(pool: Arc<BufferPool>, slot: SlotGuard, file: Arc<File>) -> Self {
        let half_size = pool.half_size();
        Self {
            mode: BufferMode::Double {
                half_a: Arc::new(DashMap::new()),
                half_b: Arc::new(DashMap::new()),
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
        }
    }

    /// Create a local-limit buffer for SSD write combining.
    ///
    /// `limit_bytes` — the maximum bytes to accumulate before flushing.
    /// `file` — the output file wrapped in `Arc`.
    pub fn new_local(limit_bytes: u64, file: Arc<File>) -> Self {
        Self {
            mode: BufferMode::Local {
                chunks: Arc::new(DashMap::new()),
                buffered_bytes: AtomicU64::new(0),
                local_limit: limit_bytes,
                file,
            },
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
                    half_a, half_b, active_is_a, usage_a, usage_b,
                    *half_size, flush_handle, notify, error_flag, flip_token,
                    pool, file, offset, data,
                )
                .await
            }
            BufferMode::Local {
                chunks,
                buffered_bytes,
                local_limit,
                file,
            } => {
                self.buffer_chunk_local(
                    chunks, buffered_bytes, *local_limit, file, offset, data,
                );
                Ok(())
            }
        }
    }

    /// Double-buffer mode implementation.
    #[allow(clippy::too_many_arguments)]
    async fn buffer_chunk_double(
        &self,
        half_a: &Arc<DashMap<u64, Bytes>>,
        half_b: &Arc<DashMap<u64, Bytes>>,
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
            // If a background flush has failed, bail out so the caller can
            // fall back to direct I/O.
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
                active_map.insert(offset, data);
                active_usage.fetch_add(len, Ordering::Release);
                pool.add_usage(len);
                return Ok(());
            }

            // Active half is full → need to flip.
            // Acquire the flip token to serialise flips.
            if flip_token.swap(true, Ordering::Acquire) {
                // Someone else is flipping — wait for room.
                notify.notified().await;
                continue;
            }

                // We hold the flip token. Before proceeding, check if a
                // background flush from a previous flip is still running.
                let prev_handle = flush_handle.lock().take();
                if let Some(h) = prev_handle {
                    // Wait for it to finish.
                    let _ = h.await;
                // Release our token and retry — the flushed half is now empty.
                flip_token.store(false, Ordering::Release);
                notify.notify_waiters();
                if error_flag.load(Ordering::Acquire) {
                    return Err(DownloadError::Internal(
                        "background buffer flush failed".into(),
                    ));
                }
                continue;
            }

            // Sanity: the inactive half should be empty (no flush running).
            let inactive_usage_val = inactive_usage.load(Ordering::Acquire);
            if inactive_usage_val > 0 {
                let cleared: u64 =
                    inactive_map.iter().map(|e| e.value().len() as u64).sum();
                inactive_map.clear();
                inactive_usage.store(0, Ordering::Release);
                pool.sub_usage(cleared);
                tracing::warn!(
                    "buffer_chunk: inactive half had {} bytes without flush handle",
                    cleared,
                );
            }

            // ---- FLIP ----
            // Drain the active half, reset usage, subtract from pool.
            let old_entries: Vec<(u64, Bytes)> =
                active_map.iter().map(|e| (*e.key(), e.value().clone())).collect();
            active_map.clear();
            let old_bytes: u64 = old_entries.iter().map(|(_, d)| d.len() as u64).sum();
            active_usage.store(0, Ordering::Release);
            pool.sub_usage(old_bytes);

            // Spawn background flush for the old active half's data.
            let bg_file = file.clone();
            let bg_error = Arc::clone(error_flag);
            let bg_notify = notify.clone();
            let bg_handle = tokio::spawn(async move {
                let blocking_result = tokio::task::spawn_blocking(move || -> std::result::Result<(), DownloadError> {
                    let mut sorted = old_entries;
                    sorted.sort_by_key(|(k, _)| *k);
                    for (off, chunk) in &sorted {
                        write_all_at(&bg_file, chunk, *off)?;
                    }
                    Ok(())
                })
                .await;

                match blocking_result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        bg_error.store(true, Ordering::Release);
                        tracing::error!("background HDD buffer flush failed: {e}");
                    }
                    Err(join_e) => {
                        bg_error.store(true, Ordering::Release);
                        tracing::error!("background flush task panicked: {join_e}");
                    }
                }
                bg_notify.notify_waiters();
            });

            *flush_handle.lock() = Some(bg_handle);

            // Atomically flip the active half.
            active_is_a.store(!is_a, Ordering::Release);

            // Release flip token.
            flip_token.store(false, Ordering::Release);
            notify.notify_waiters();

            // Insert the current chunk into the new active half (which is empty).
            let new_is_a = active_is_a.load(Ordering::Acquire);
            let (new_map, new_usage) = if new_is_a { (half_a, usage_a) } else { (half_b, usage_b) };
            new_map.insert(offset, data);
            new_usage.fetch_add(len, Ordering::Release);
            pool.add_usage(len);

            return Ok(());
        }
    }

    /// Local (SSD) mode: insert if under the local limit; otherwise flush the
    /// current buffer synchronously (blocking the current task) and retry.
    fn buffer_chunk_local(
        &self,
        chunks: &Arc<DashMap<u64, Bytes>>,
        buffered_bytes: &AtomicU64,
        local_limit: u64,
        file: &Arc<File>,
        offset: u64,
        data: Bytes,
    ) {
        let len = data.len() as u64;

        // Fast path — room available.
        let current = buffered_bytes.load(Ordering::Relaxed);
        if local_limit == 0 || current + len <= local_limit {
            chunks.insert(offset, data);
            buffered_bytes.fetch_add(len, Ordering::Relaxed);
            return;
        }

        // Buffer is full — flush synchronously, then insert.
        // This blocks the async task but is fast for small SSD-local buffers.
            let mut entries: Vec<(u64, Bytes)> =
                chunks.iter().map(|e| (*e.key(), e.value().clone())).collect();
            chunks.clear();
        entries.sort_by_key(|(k, _)| *k);
        for (off, chunk) in &entries {
            if let Err(e) = write_all_at(file, chunk, *off) {
                tracing::error!("SSD local buffer flush failed at offset {off}: {e}");
            }
        }
        let flushed: u64 = entries.iter().map(|(_, d)| d.len() as u64).sum();
        buffered_bytes.fetch_sub(flushed, Ordering::Relaxed);

        // Now insert the new chunk.
        chunks.insert(offset, data);
        buffered_bytes.fetch_add(len, Ordering::Relaxed);
    }

    /// Flush all buffered data to disk.
    ///
    /// In double-buffer mode: waits for any active background flush, then
    /// flushes both halves synchronously (via spawn_blocking).
    /// In local mode: flushes the single map synchronously.
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
                Self::flush_one_half(active, active_usage, file, pool).await?;

                // 4. Flush inactive half (should be empty, but be safe).
                Self::flush_one_half(inactive, inactive_usage, file, pool).await?;

                // 5. Check error flag one more time.
                if error_flag.load(Ordering::Acquire) {
                    return Err(DownloadError::Internal(
                        "background buffer flush failed".into(),
                    ));
                }

                Ok(())
            }
            BufferMode::Local {
                chunks,
                buffered_bytes,
                file,
                ..
            } => {
                if chunks.is_empty() {
                    return Ok(());
                }
                let mut entries: Vec<(u64, Bytes)> =
                    chunks.iter().map(|e| (*e.key(), e.value().clone())).collect();
                chunks.clear();
                entries.sort_by_key(|(k, _)| *k);
                for (off, chunk) in &entries {
                    write_all_at(file, chunk, *off)?;
                }
                let flushed: u64 =
                    entries.iter().map(|(_, d)| d.len() as u64).sum();
                buffered_bytes.fetch_sub(flushed, Ordering::Relaxed);
                Ok(())
            }
        }
    }

    /// Helper: drain a single half's DashMap and write everything to disk
    /// via `spawn_blocking`.
    async fn flush_one_half(
        half: &Arc<DashMap<u64, Bytes>>,
        usage: &AtomicU64,
        file: &Arc<File>,
        pool: &Arc<BufferPool>,
    ) -> Result<(), DownloadError> {
        if half.is_empty() {
            return Ok(());
        }

        let entries: Vec<(u64, Bytes)> = half.iter().map(|e| (*e.key(), e.value().clone())).collect();
        half.clear();
        let bytes: u64 = entries.iter().map(|(_, d)| d.len() as u64).sum();
        usage.store(0, Ordering::Release);
        pool.sub_usage(bytes);

        let f = file.clone();
        tokio::task::spawn_blocking(move || -> std::result::Result<(), DownloadError> {
            let mut sorted = entries;
            sorted.sort_by_key(|(k, _)| *k);
            for (off, chunk) in &sorted {
                write_all_at(&f, chunk, *off)?;
            }
            Ok(())
        })
        .await
        .map_err(|e| DownloadError::Internal(format!("flush task failed: {e}")))??;

        Ok(())
    }

    /// Wait for any in-progress background flush to complete, discarding its result.
    ///
    /// Used on cancellation: we must drain the background task before closing the
    /// file handle, otherwise `cleanup_files` will fail on Windows (sharing violation
    /// because the background `spawn_blocking` still holds an `Arc<File>`).
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
            BufferMode::Local { .. } => {}
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
                    let a: u64 = half_a.iter().map(|e| e.value().len() as u64).sum();
                    let b: u64 = half_b.iter().map(|e| e.value().len() as u64).sum();
                    (a, b)
                };
                half_a.clear();
                half_b.clear();
                usage_a.store(0, Ordering::Release);
                usage_b.store(0, Ordering::Release);
                if a_bytes + b_bytes > 0 {
                    pool.sub_usage(a_bytes + b_bytes);
                }
                error_flag.store(false, Ordering::Release);
            }
            BufferMode::Local {
                chunks,
                buffered_bytes,
                ..
            } => {
                let bytes: u64 = chunks.iter().map(|e| e.value().len() as u64).sum();
                chunks.clear();
                buffered_bytes.fetch_sub(bytes, Ordering::Relaxed);
            }
        }
    }

    /// Total bytes currently buffered.
    #[allow(dead_code)]
    pub fn len(&self) -> u64 {
        match &self.mode {
            BufferMode::Double {
                usage_a, usage_b, ..
            } => usage_a.load(Ordering::Relaxed) + usage_b.load(Ordering::Relaxed),
            BufferMode::Local {
                buffered_bytes, ..
            } => buffered_bytes.load(Ordering::Relaxed),
        }
    }

    /// Whether any background flush has degraded (double mode only).
    /// Always `false` in local mode.
    #[allow(dead_code)]
    pub fn has_degraded(&self) -> bool {
        match &self.mode {
            BufferMode::Double { error_flag, .. } => error_flag.load(Ordering::Relaxed),
            BufferMode::Local { .. } => false,
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
                    let a: u64 = half_a.iter().map(|e| e.value().len() as u64).sum();
                    let b: u64 = half_b.iter().map(|e| e.value().len() as u64).sum();
                    (a, b)
                };
                half_a.clear();
                half_b.clear();
                usage_a.store(0, Ordering::Release);
                usage_b.store(0, Ordering::Release);
                error_flag.store(false, Ordering::Release);
                if a_bytes + b_bytes > 0 {
                    pool.sub_usage(a_bytes + b_bytes);
                }
                pool.release_slot();
                // `slot` (SlotGuard) and `flush_handle` (JoinHandle) are
                // dropped after this, returning the semaphore permit and
                // detaching the background task respectively.
            }
            BufferMode::Local {
                chunks,
                buffered_bytes,
                ..
            } => {
                let bytes: u64 = chunks.iter().map(|e| e.value().len() as u64).sum();
                chunks.clear();
                buffered_bytes.fetch_sub(bytes, Ordering::Relaxed);
            }
        }
    }
}
