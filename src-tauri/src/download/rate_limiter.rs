use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Token-bucket rate limiter for global download speed control.
///
/// Thread-safe: inner state protected by `std::sync::Mutex` held only
/// for brief arithmetic, never across an await point or blocking call.
/// Cloneable via `Arc` — pass a single instance through the entire app.
#[derive(Debug, Clone)]
pub struct RateLimiter {
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
    }

    /// Async consumer — pauses the current task until `n` bytes of
    /// budget are available, or returns immediately when the limit is 0.
    pub async fn consume(&self, n: usize) {
        if n == 0 {
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
    pub fn consume_blocking(&self, n: usize) {
        if n == 0 {
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
        let deficit = n as f64 - inner.tokens;
        inner.tokens = 0.0;
        // deficit bytes / rate bytes/sec  →  seconds  →  nanos
        let wait_ns = (deficit / inner.rate as f64 * 1_000_000_000.0) as u64;
        Some(wait_ns.max(1)) // at least 1 ns to avoid busy-spin
    }
}

fn lock_inner(inner: &Arc<Mutex<Inner>>) -> std::sync::MutexGuard<'_, Inner> {
    match inner.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("rate limiter lock poisoned, recovering with inner state");
            poisoned.into_inner()
        }
    }
}
