use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

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
        self.rate.store(new_rate, Ordering::Release);
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
    }

    /// Async consumer — pauses the current task until `n` bytes of
    /// budget are available, or returns immediately when the limit is 0.
    pub async fn consume(&self, n: usize) {
        if n == 0 || self.rate.load(Ordering::Relaxed) == 0 {
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
        if n == 0 || self.rate.load(Ordering::Relaxed) == 0 {
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
        let mut inner = self.inner.lock();
        inner.tokens = tokens.clamp(0.0, inner.capacity as f64);
    }

    /// Get the number of tokens currently in the bucket.
    pub fn tokens(&self) -> f64 {
        self.inner.lock().tokens
    }

    /// Get the current capacity (bucket size).
    pub fn capacity(&self) -> u64 {
        self.inner.lock().capacity
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

fn lock_inner(inner: &Arc<Mutex<Inner>>) -> parking_lot::MutexGuard<'_, Inner> {
    inner.lock()
}

#[cfg(test)]
mod tests;
