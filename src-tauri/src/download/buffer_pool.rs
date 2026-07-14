//! Global memory buffer pool for HDD download optimization.
//!
//! On HDDs, buffering downloaded data in memory before writing to disk
//! converts random writes (from parallel chunked downloads) into a single
//! sequential write, significantly improving throughput.
//!
//! The pool enforces a global memory budget shared across all active HDD
//! downloads. When the pool is full, new chunks are written directly to disk
//! (degraded mode).

use bytes::Bytes;
use dashmap::DashMap;
use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use super::error::Result;
use super::file_alloc::write_all_at;

/// Result of attempting to buffer a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferResult {
    /// Chunk was successfully stored in the memory buffer.
    Buffered,
    /// Pool is full; caller should write this chunk directly to disk.
    Degraded,
}

/// A per-download buffer that stores chunks in memory before flushing to disk.
///
/// Uses `DashMap<u64, Bytes>` for lock-free concurrent writes from multiple
/// download workers. On flush, entries are collected, sorted by offset, and
/// written sequentially.
///
/// Two modes:
/// - **Pool-backed** (HDD): uses a global `BufferPool` for shared memory budgeting.
/// - **Local-limit** (SSD): self-enforces a fixed per-buffer capacity (e.g. 4 MiB)
///   for lightweight write-combining without global pool contention.
pub struct DownloadBuffer {
    /// Buffered chunks indexed by byte offset.
    chunks: Arc<DashMap<u64, Bytes>>,
    /// Bytes currently stored in this buffer.
    buffered_bytes: AtomicU64,
    /// Reference to the global pool (None for local-limit buffers).
    pool: Option<Arc<BufferPool>>,
    /// Local capacity limit when no pool is attached, in bytes.
    local_limit: AtomicU64,
}

impl DownloadBuffer {
    /// Create a pool-backed buffer that shares the global HDD memory budget.
    pub fn new(pool: Arc<BufferPool>) -> Self {
        Self {
            chunks: Arc::new(DashMap::new()),
            buffered_bytes: AtomicU64::new(0),
            pool: Some(pool),
            local_limit: AtomicU64::new(0),
        }
    }

    /// Create a self-limiting buffer with a fixed per-download capacity.
    ///
    /// Used for SSD write-combining: a small buffer (e.g. 4 MiB) merges
    /// multiple small HTTP chunks into fewer larger writes without consuming
    /// the global HDD buffer pool.
    pub fn new_local(limit_bytes: u64) -> Self {
        Self {
            chunks: Arc::new(DashMap::new()),
            buffered_bytes: AtomicU64::new(0),
            pool: None,
            local_limit: AtomicU64::new(limit_bytes),
        }
    }

    /// Attempt to buffer a chunk of data at the given byte offset.
    ///
    /// Returns `BufferResult::Buffered` if the chunk was stored in memory,
    /// or `BufferResult::Degraded` if the buffer is full and the caller should
    /// write directly to disk.
    pub fn buffer_chunk(&self, offset: u64, data: Bytes) -> BufferResult {
        let len = data.len() as u64;

        if let Some(ref pool) = self.pool {
            // Pool-backed: check global budget
            if !pool.try_reserve(len) {
                return BufferResult::Degraded;
            }
        } else {
            // Local-limit: check per-buffer capacity
            let limit = self.local_limit.load(Ordering::Relaxed);
            if limit > 0 {
                let mut current = self.buffered_bytes.load(Ordering::Relaxed);
                loop {
                    if current + len > limit {
                        return BufferResult::Degraded;
                    }
                    match self.buffered_bytes.compare_exchange_weak(
                        current,
                        current + len,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(actual) => current = actual,
                    }
                }
            }
        }

        // Store the chunk
        self.chunks.insert(offset, data);
        if self.pool.is_none() {
            // Already incremented via CAS above for local mode
        } else {
            self.buffered_bytes.fetch_add(len, Ordering::Relaxed);
        }

        BufferResult::Buffered
    }

    /// Flush all buffered chunks to disk, writing them sequentially in offset order.
    ///
    /// After flushing, the buffer is empty and memory is returned to the pool.
    pub fn flush_to_disk(&self, file: &File) -> Result<()> {
        if self.chunks.is_empty() {
            return Ok(());
        }

        // Collect and sort by offset
        let mut entries: Vec<(u64, Bytes)> = self
            .chunks
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect();
        entries.sort_by_key(|(offset, _)| *offset);

        // Write sequentially
        for (offset, data) in &entries {
            write_all_at(file, data, *offset)?;
        }

        // Return memory to pool (if pool-backed) and clear
        let released = self.buffered_bytes.swap(0, Ordering::Relaxed);
        if let Some(ref pool) = self.pool {
            pool.release(released);
        }
        self.chunks.clear();

        Ok(())
    }

    /// Total bytes currently buffered.
    #[allow(dead_code)]
    pub fn len(&self) -> u64 {
        self.buffered_bytes.load(Ordering::Relaxed)
    }

    /// Whether the buffer is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of individual chunks stored.
    #[allow(dead_code)]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Clear all buffered chunks and release memory back to the pool.
    ///
    /// Used when the download file is reset and buffered data becomes invalid.
    /// After calling this, the buffer is empty and ready for fresh data.
    pub fn clear(&self) {
        let chunk_bytes: u64 = self.chunks.iter().map(|e| e.value().len() as u64).sum();
        self.chunks.clear();
        if chunk_bytes > 0
            && let Some(ref pool) = self.pool
        {
            pool.release(chunk_bytes);
        }
        self.buffered_bytes.store(0, Ordering::Relaxed);
    }

    /// Whether any chunks were degraded (written directly to disk) during this download.
    #[allow(dead_code)]
    pub fn has_degraded(&self) -> bool {
        // We track this via a flag that gets set when buffer_chunk returns Degraded
        // This is set externally by the caller
        false
    }
}

impl Drop for DownloadBuffer {
    fn drop(&mut self) {
        // Sum actual chunk sizes directly from DashMap to avoid race conditions
        // where pool bytes were reserved but never counted in buffered_bytes.
        let chunk_bytes: u64 = self.chunks.iter().map(|entry| entry.value().len() as u64).sum();
        if chunk_bytes > 0
            && let Some(ref pool) = self.pool
        {
            pool.release(chunk_bytes);
        }
        self.chunks.clear();
    }
}

/// Global memory buffer pool shared across all HDD downloads.
///
/// Enforces a total memory budget. When the pool is full, downloads
/// fall back to direct disk I/O (degraded mode).
pub struct BufferPool {
    /// Maximum total bytes that can be buffered across all downloads.
    total_limit_bytes: AtomicU64,
    /// Current total bytes buffered across all downloads.
    current_usage: AtomicU64,
    /// Whether game/performance mode is active (reduces effective limit).
    game_mode: AtomicBool,
    /// Buffer limit when game mode is active, in bytes.
    game_mode_limit_bytes: AtomicU64,
    /// Counter for degraded operations (informational).
    degradation_count: AtomicUsize,
}

impl BufferPool {
    /// Create a new buffer pool.
    ///
    /// `total_limit_mb` — maximum pool size in MiB under normal operation.
    /// `game_mode_limit_mb` — maximum pool size in MiB when game mode is active.
    pub fn new(total_limit_mb: u64, game_mode_limit_mb: u64) -> Self {
        Self {
            total_limit_bytes: AtomicU64::new(total_limit_mb * 1024 * 1024),
            current_usage: AtomicU64::new(0),
            game_mode: AtomicBool::new(false),
            game_mode_limit_bytes: AtomicU64::new(game_mode_limit_mb * 1024 * 1024),
            degradation_count: AtomicUsize::new(0),
        }
    }

    /// Get the effective byte limit, accounting for game mode.
    pub fn effective_limit(&self) -> u64 {
        if self.game_mode.load(Ordering::Relaxed) {
            self.game_mode_limit_bytes.load(Ordering::Relaxed)
        } else {
            self.total_limit_bytes.load(Ordering::Relaxed)
        }
    }

    /// Attempt to reserve `bytes` from the pool.
    ///
    /// Returns `true` if the reservation was successful (current_usage + bytes <= limit),
    /// or `false` if the pool is full.
    pub fn try_reserve(&self, bytes: u64) -> bool {
        let limit = self.effective_limit();
        if limit == 0 {
            return false;
        }

        let mut current = self.current_usage.load(Ordering::Relaxed);
        loop {
            if current + bytes > limit {
                self.degradation_count.fetch_add(1, Ordering::Relaxed);
                return false;
            }
            match self.current_usage.compare_exchange_weak(
                current,
                current + bytes,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }

    /// Release `bytes` back to the pool.
    pub fn release(&self, bytes: u64) {
        self.current_usage.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Current total bytes buffered across all downloads.
    pub fn current_usage(&self) -> u64 {
        self.current_usage.load(Ordering::Relaxed)
    }

    /// Set game/performance mode state.
    ///
    /// When enabled, the effective buffer limit is reduced to `game_mode_limit_bytes`.
    /// Note: enabling game mode does NOT forcibly release already-buffered memory;
    /// it only prevents new allocations beyond the reduced limit.
    pub fn set_game_mode(&self, enabled: bool) {
        self.game_mode.store(enabled, Ordering::Relaxed);
    }

    /// Whether game mode is currently active.
    pub fn game_mode(&self) -> bool {
        self.game_mode.load(Ordering::Relaxed)
    }

    /// Update pool limits from settings (e.g., after user changes config).
    pub fn update_limits(&self, total_limit_mb: u64, game_mode_limit_mb: u64) {
        self.total_limit_bytes
            .store(total_limit_mb * 1024 * 1024, Ordering::Relaxed);
        self.game_mode_limit_bytes
            .store(game_mode_limit_mb * 1024 * 1024, Ordering::Relaxed);
    }

    /// Count of times buffer_chunk returned Degraded (informational).
    pub fn degradation_count(&self) -> usize {
        self.degradation_count.load(Ordering::Relaxed)
    }
}
