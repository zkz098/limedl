#[cfg(not(limedl_loom))]
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[cfg(limedl_loom)]
use loom::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use std::sync::Arc as NotifyArc;

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
    pub rebalance_notify: NotifyArc<Notify>,
}

impl ConcurrencyManager {
    pub fn new(max_http: usize, max_bt: usize) -> Self {
        Self {
            active_http_count: Arc::new(AtomicUsize::new(0)),
            active_bt_count: Arc::new(AtomicUsize::new(0)),
            max_concurrent_http: Arc::new(AtomicUsize::new(max_http)),
            max_concurrent_bt: Arc::new(AtomicUsize::new(max_bt)),
            overclock_mode: Arc::new(AtomicBool::new(false)),
            rebalance_notify: NotifyArc::new(Notify::new()),
        }
    }

    pub fn set_overclock_mode(&self, enabled: bool) {
        self.overclock_mode.store(enabled, Ordering::Release);
        self.rebalance_notify.notify_one();
    }

    pub fn overclock_mode(&self) -> bool {
        self.overclock_mode.load(Ordering::Acquire)
    }

    pub fn toggle_overclock_mode(&self, enabled: Option<bool>) -> bool {
        let current = self.overclock_mode.load(Ordering::Acquire);
        let new_state = enabled.unwrap_or(!current);
        self.overclock_mode.store(new_state, Ordering::Release);
        self.rebalance_notify.notify_one();
        new_state
    }

    pub fn update_limits(&self, max_http: usize, max_bt: usize) {
        self.max_concurrent_http.store(max_http, Ordering::Release);
        self.max_concurrent_bt.store(max_bt, Ordering::Release);
    }

    /// Try to acquire an HTTP download slot.
    pub fn try_acquire_http(&self) -> Result<DownloadSlotGuard, DownloadError> {
        DownloadSlotGuard::try_acquire(&self.active_http_count, &self.max_concurrent_http)
    }

    /// Try to acquire a BT download slot.
    pub fn try_acquire_bt(&self) -> Result<DownloadSlotGuard, DownloadError> {
        DownloadSlotGuard::try_acquire(&self.active_bt_count, &self.max_concurrent_bt)
    }
}

#[cfg(all(test, limedl_loom))]
mod loom_tests {
    use super::*;

    fn loom_builder() -> loom::model::Builder {
        let mut builder = loom::model::Builder::new();
        if builder.preemption_bound.is_none() {
            builder.preemption_bound = Some(3);
        }
        builder
    }

    #[test]
    fn loom_concurrency_manager_shrink_limit_vs_acquire() {
        loom_builder().check(|| {
            let mgr = loom::sync::Arc::new(ConcurrencyManager::new(2, 2));
            let shrunk = loom::sync::Arc::new(AtomicBool::new(false));
            let post_shrink_violation = loom::sync::Arc::new(AtomicUsize::new(0));

            let m1 = loom::sync::Arc::clone(&mgr);
            let t1 = loom::thread::spawn(move || {
                let _g = m1.try_acquire_http();
            });

            let m2 = loom::sync::Arc::clone(&mgr);
            let s2 = loom::sync::Arc::clone(&shrunk);
            let v2 = loom::sync::Arc::clone(&post_shrink_violation);
            let t2 = loom::thread::spawn(move || {
                // Observe whether update_limits(1, 1) already happened-before this acquire.
                let saw_shrunk_before = s2.load(Ordering::Acquire);
                if let Ok(_g) = m2.try_acquire_http()
                    && saw_shrunk_before
                    && m2.active_http_count.load(Ordering::SeqCst) > 1
                {
                    v2.fetch_add(1, Ordering::SeqCst);
                }
            });

            let m3 = loom::sync::Arc::clone(&mgr);
            let s3 = loom::sync::Arc::clone(&shrunk);
            let t3 = loom::thread::spawn(move || {
                m3.update_limits(1, 1);
                s3.store(true, Ordering::Release);
            });

            t1.join().unwrap();
            t2.join().unwrap();
            t3.join().unwrap();

            assert_eq!(post_shrink_violation.load(Ordering::SeqCst), 0);
            assert_eq!(mgr.active_http_count.load(Ordering::SeqCst), 0);
            assert_eq!(mgr.max_concurrent_http.load(Ordering::Acquire), 1);

            // Post-shrink sequential enforcement: 1st succeeds, 2nd is rejected.
            let g1 = mgr.try_acquire_http();
            assert!(g1.is_ok());
            assert!(mgr.try_acquire_http().is_err());
            drop(g1);
            assert_eq!(mgr.active_http_count.load(Ordering::SeqCst), 0);
        });
    }
}
