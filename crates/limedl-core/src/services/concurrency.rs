use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

use crate::error::DownloadError;
use crate::slot_guard::DownloadSlotGuard;

/// Concurrency limits and slot manager for HTTP and BT downloads.
#[derive(Clone)]
pub struct ConcurrencyManager {
    /// Active HTTP download counter
    pub active_http_count: Arc<AtomicUsize>,
    /// Active BT download counter
    pub active_bt_count: Arc<AtomicUsize>,
    /// Maximum concurrent HTTP downloads
    pub max_concurrent_http: Arc<AtomicUsize>,
    /// Maximum concurrent BT downloads
    pub max_concurrent_bt: Arc<AtomicUsize>,
    /// Overclock mode flag
    pub overclock_mode: Arc<AtomicBool>,
    /// Notify mechanism for triggering scheduler rebalance events
    pub rebalance_notify: Arc<Notify>,
}

impl ConcurrencyManager {
    pub fn new(max_http: usize, max_bt: usize) -> Self {
        Self {
            active_http_count: Arc::new(AtomicUsize::new(0)),
            active_bt_count: Arc::new(AtomicUsize::new(0)),
            max_concurrent_http: Arc::new(AtomicUsize::new(max_http)),
            max_concurrent_bt: Arc::new(AtomicUsize::new(max_bt)),
            overclock_mode: Arc::new(AtomicBool::new(false)),
            rebalance_notify: Arc::new(Notify::new()),
        }
    }

    pub fn set_overclock_mode(&self, enabled: bool) {
        self.overclock_mode.store(enabled, Ordering::Relaxed);
        self.rebalance_notify.notify_one();
    }

    pub fn overclock_mode(&self) -> bool {
        self.overclock_mode.load(Ordering::Relaxed)
    }

    pub fn toggle_overclock_mode(&self, enabled: Option<bool>) -> bool {
        let current = self.overclock_mode.load(Ordering::Relaxed);
        let new_state = enabled.unwrap_or(!current);
        self.overclock_mode.store(new_state, Ordering::Relaxed);
        self.rebalance_notify.notify_one();
        new_state
    }

    pub fn update_limits(&self, max_http: usize, max_bt: usize) {
        self.max_concurrent_http.store(max_http, Ordering::Relaxed);
        self.max_concurrent_bt.store(max_bt, Ordering::Relaxed);
    }

    /// Try to acquire an HTTP download slot.
    pub fn try_acquire_http(&self) -> Result<DownloadSlotGuard, DownloadError> {
        let max = self.max_concurrent_http.load(Ordering::Acquire);
        let counter = &self.active_http_count;
        loop {
            let current = counter.load(Ordering::Acquire);
            if current >= max {
                return Err(DownloadError::TooManyConcurrentDownloads);
            }
            if counter
                .compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(DownloadSlotGuard::new(self.active_http_count.clone()));
            }
        }
    }

    /// Try to acquire a BT download slot.
    pub fn try_acquire_bt(&self) -> Result<DownloadSlotGuard, DownloadError> {
        let max = self.max_concurrent_bt.load(Ordering::Acquire);
        let counter = &self.active_bt_count;
        loop {
            let current = counter.load(Ordering::Acquire);
            if current >= max {
                return Err(DownloadError::TooManyConcurrentDownloads);
            }
            if counter
                .compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(DownloadSlotGuard::new(self.active_bt_count.clone()));
            }
        }
    }
}
