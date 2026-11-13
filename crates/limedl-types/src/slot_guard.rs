#[cfg(not(limedl_loom))]
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[cfg(limedl_loom)]
use loom::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::error::DownloadError;

/// Shared `Arc` type matching the atomic backend (`std::sync` vs `loom::sync`).
pub type SlotArc<T> = Arc<T>;
/// Shared `AtomicUsize` matching the atomic backend (`std::sync` vs `loom::sync`).
pub type SlotAtomicUsize = AtomicUsize;

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
        loop {
            // Re-read the limit on every CAS iteration so dynamic limit shrinks
            // or increases take effect promptly even under contention.
            let max = max_concurrent.load(Ordering::Acquire);
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
    fn loom_try_acquire_respects_fixed_limit() {
        loom_builder().check(|| {
            let counter = Arc::new(AtomicUsize::new(0));
            let max = Arc::new(AtomicUsize::new(2));
            let peak_violation = Arc::new(AtomicUsize::new(0));

            let handles: Vec<_> = (0..3)
                .map(|_| {
                    let counter = Arc::clone(&counter);
                    let max = Arc::clone(&max);
                    let peak_violation = Arc::clone(&peak_violation);
                    loom::thread::spawn(move || {
                        if let Ok(_guard) = DownloadSlotGuard::try_acquire(&counter, &max) {
                            let cur = counter.load(Ordering::SeqCst);
                            if cur > 2 {
                                peak_violation.store(cur, Ordering::SeqCst);
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }

            assert_eq!(peak_violation.load(Ordering::SeqCst), 0);
            assert_eq!(counter.load(Ordering::SeqCst), 0);
        });
    }

    #[test]
    fn loom_try_acquire_with_concurrent_limit_shrink() {
        loom_builder().check(|| {
            let counter = Arc::new(AtomicUsize::new(0));
            let max = Arc::new(AtomicUsize::new(2));

            let c1 = Arc::clone(&counter);
            let m1 = Arc::clone(&max);
            let t1 = loom::thread::spawn(move || {
                let _g = DownloadSlotGuard::try_acquire(&c1, &m1);
            });

            let c2 = Arc::clone(&counter);
            let m2 = Arc::clone(&max);
            let t2 = loom::thread::spawn(move || {
                let _g = DownloadSlotGuard::try_acquire(&c2, &m2);
            });

            let m3 = Arc::clone(&max);
            let t3 = loom::thread::spawn(move || {
                // Mirrors ConcurrencyManager::update_limits (Release store).
                m3.store(1, Ordering::Release);
            });

            t1.join().unwrap();
            t2.join().unwrap();
            t3.join().unwrap();
            assert_eq!(counter.load(Ordering::SeqCst), 0);
        });
    }
}
