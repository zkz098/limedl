use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
}

impl Drop for DownloadSlotGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Release);
    }
}
