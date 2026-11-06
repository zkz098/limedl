use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::DownloadError;

/// RAII guard that holds a concurrent download slot.
///
/// The slot is automatically released (counter decremented) when the guard is dropped.
/// Used by both the HTTP download manager and the BT backend for concurrent throttling.
pub struct DownloadSlotGuard {
    counter: Arc<AtomicUsize>,
}

impl DownloadSlotGuard {
    /// Acquire a slot from the given counter. The caller must have already
    /// incremented the counter via a successful compare-and-swap.
    pub fn new(counter: Arc<AtomicUsize>) -> Self {
        Self { counter }
    }

    /// Atomically acquire a slot if `counter` is below `max_concurrent`.
    pub fn try_acquire(
        counter: &Arc<AtomicUsize>,
        max_concurrent: &AtomicUsize,
    ) -> Result<Self, DownloadError> {
        let max = max_concurrent.load(Ordering::Acquire);
        loop {
            let current = counter.load(Ordering::Acquire);
            if current >= max {
                return Err(DownloadError::TooManyConcurrentDownloads);
            }
            if counter
                .compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(Self::new(Arc::clone(counter)));
            }
        }
    }
}

impl Drop for DownloadSlotGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Release);
    }
}
