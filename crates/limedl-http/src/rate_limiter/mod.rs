use std::time::{Duration, Instant};

#[cfg(not(limedl_loom))]
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[cfg(limedl_loom)]
use loom::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

#[cfg(not(limedl_loom))]
use parking_lot::Mutex;

/// Token-bucket rate limiter for global download speed control.
///
/// Thread-safe: inner state protected by `parking_lot::Mutex` held only
/// for brief arithmetic, never across an await point or blocking call.
/// Cloneable via `Arc` — pass a single instance through the entire app.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    rate: Arc<AtomicU64>,
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug)]
struct Inner {
    rate: u64,
    capacity: u64,
    tokens: f64,
    last_refill: Instant,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            rate: Arc::new(AtomicU64::new(0)),
            inner: Arc::new(Mutex::new(Inner {
                rate: 0,
                capacity: 0,
                tokens: 0.0,
                last_refill: Instant::now(),
            })),
        }
    }
}

impl RateLimiter {
    /// Update the speed limit in bytes/sec (0 = unlimited).
    ///
    /// Refills tokens with the *current* rate before switching, so any
    /// budget accumulated under the old limit is preserved coherently.
    pub fn set_rate(&self, new_rate: u64) {
        let mut inner = lock_inner(&self.inner);
        let elapsed = inner.last_refill.elapsed().as_secs_f64();
        if inner.rate > 0 {
            inner.tokens = (inner.tokens + elapsed * inner.rate as f64).min(inner.capacity as f64);
        }
        inner.last_refill = Instant::now();
        inner.rate = new_rate;
        inner.capacity = if new_rate > 0 {
            (2 * new_rate).max(1)
        } else {
            0
        };
        if new_rate == 0 {
            inner.tokens = 0.0;
        } else {
            inner.tokens = inner.tokens.min(inner.capacity as f64);
        }
        // Store the lock-free fast-path rate while still holding `inner` so
        // concurrent `set_rate` calls cannot leave `self.rate` and `inner.rate`
        // out of sync (which could silently disable limiting on the fast path).
        self.rate.store(new_rate, Ordering::Release);
    }

    /// Async consumer — pauses the current task until `n` bytes of
    /// budget are available, or returns immediately when the limit is 0.
    pub async fn consume(&self, n: usize) {
        if n == 0 || self.rate.load(Ordering::Acquire) == 0 {
            return;
        }
        let n = n as u64;
        loop {
            let maybe_wait = try_consume(&self.inner, n);
            match maybe_wait {
                None => return,
                Some(wait_ns) => tokio::time::sleep(Duration::from_nanos(wait_ns)).await,
            }
        }
    }

    /// Blocking consumer — pauses the current thread until `n` bytes of
    /// budget are available, or returns immediately when the limit is 0.
    ///
    /// Safe to call from `spawn_blocking` because the lock is never held
    /// across the sleep.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn consume_blocking(&self, n: usize) {
        if n == 0 || self.rate.load(Ordering::Acquire) == 0 {
            return;
        }
        let n = n as u64;
        loop {
            let maybe_wait = try_consume(&self.inner, n);
            match maybe_wait {
                None => return,
                Some(wait_ns) => std::thread::sleep(Duration::from_nanos(wait_ns)),
            }
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl RateLimiter {
    /// Set token count directly for testing/benchmarking.
    /// Clamped to [0, capacity] range.
    pub fn set_tokens(&self, tokens: f64) {
        let mut inner = lock_inner(&self.inner);
        let cap = inner.capacity as f64;
        inner.tokens = tokens.clamp(0.0, cap);
    }

    /// Get the number of tokens currently in the bucket.
    pub fn tokens(&self) -> f64 {
        lock_inner(&self.inner).tokens
    }

    /// Get the current capacity (bucket size).
    pub fn capacity(&self) -> u64 {
        lock_inner(&self.inner).capacity
    }
}

/// Tries to consume `n` tokens from the bucket.
///
/// Returns `None` on success (budget granted), or `Some(wait_nanos)` if
/// the caller must sleep and retry.
fn try_consume(inner: &Arc<Mutex<Inner>>, n: u64) -> Option<u64> {
    let mut inner = lock_inner(inner);
    if inner.rate == 0 {
        return None; // unlimited
    }
    let elapsed = inner.last_refill.elapsed().as_secs_f64();
    if elapsed > 0.0 {
        inner.tokens = (inner.tokens + elapsed * inner.rate as f64).min(inner.capacity as f64);
        inner.last_refill = Instant::now();
    }
    if inner.tokens >= n as f64 {
        inner.tokens -= n as f64;
        None
    } else {
        // Preserve existing tokens — they will be refilled during the sleep
        // so that the next attempt has accumulated enough for a successful
        // consume.  Previously this line set `inner.tokens = 0.0`, which
        // discarded partial progress and caused an infinite oscillation
        // when `n > rate × (elapsed since last attempt)`.
        let deficit = n as f64 - inner.tokens;
        // deficit bytes / rate bytes/sec  →  seconds  →  nanos
        let wait_ns = (deficit / inner.rate as f64 * 1_000_000_000.0) as u64;
        Some(wait_ns.max(1)) // at least 1 ns to avoid busy-spin
    }
}

#[cfg(not(limedl_loom))]
fn lock_inner(inner: &Arc<Mutex<Inner>>) -> parking_lot::MutexGuard<'_, Inner> {
    inner.lock()
}

#[cfg(limedl_loom)]
fn lock_inner(inner: &Arc<Mutex<Inner>>) -> loom::sync::MutexGuard<'_, Inner> {
    inner.lock().unwrap()
}

#[cfg(test)]
mod tests;

#[cfg(all(test, limedl_loom))]
mod loom_tests {
    use super::*;

    #[test]
    fn loom_concurrent_set_rate_keeps_atomic_and_inner_in_sync() {
        loom::model(|| {
            let limiter = RateLimiter::default();
            limiter.set_rate(1000);

            let l1 = limiter.clone();
            let t1 = loom::thread::spawn(move || {
                l1.set_rate(0);
            });

            let l2 = limiter.clone();
            let t2 = loom::thread::spawn(move || {
                l2.set_rate(500);
            });

            t1.join().unwrap();
            t2.join().unwrap();

            let atomic_rate = limiter.rate.load(Ordering::SeqCst);
            let inner_rate = lock_inner(&limiter.inner).rate;
            assert_eq!(
                atomic_rate, inner_rate,
                "fast-path atomic rate ({atomic_rate}) diverged from locked inner.rate ({inner_rate})"
            );
        });
    }
}
