//! Double-buffer cache for HDD download optimization and SSD write combining.
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

pub mod download_buffer;
#[cfg(any(test, feature = "test-utils"))]
pub mod fault;
pub mod worker;

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub use download_buffer::DownloadBuffer;
pub use worker::IoWorker;

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
