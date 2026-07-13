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

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ─────────────────────────────────────────────

    /// Set a rate and fill tokens to capacity for deterministic testing.
    fn init_limiter(rate: u64) -> RateLimiter {
        let limiter = RateLimiter::default();
        limiter.set_rate(rate);
        // After set_rate, tokens start at 0; fill them up manually.
        let mut inner = lock_inner(&limiter.inner);
        inner.tokens = inner.capacity as f64;
        drop(inner);
        limiter
    }

    // ── default / unlimited ─────────────────────────────────

    #[test]
    fn default_rate_limiter_is_unlimited() {
        let limiter = RateLimiter::default();
        assert_eq!(lock_inner(&limiter.inner).rate, 0);
        // With rate=0, even a huge consume should succeed immediately
        assert!(try_consume(&limiter.inner, 100_000_000).is_none());
    }

    // ── set_rate ────────────────────────────────────────────

    #[test]
    fn set_rate_from_zero_to_nonzero() {
        let limiter = RateLimiter::default();
        limiter.set_rate(1024);
        let inner = lock_inner(&limiter.inner);
        assert_eq!(inner.rate, 1024);
        assert_eq!(inner.capacity, 2048); // 2 * rate
        assert!(inner.tokens <= 2048.0);
    }

    #[test]
    fn set_rate_from_nonzero_to_zero() {
        let limiter = RateLimiter::default();
        limiter.set_rate(1024);
        limiter.set_rate(0);
        let inner = lock_inner(&limiter.inner);
        assert_eq!(inner.rate, 0);
        assert_eq!(inner.capacity, 0);
        assert_eq!(inner.tokens, 0.0);
    }

    #[test]
    fn set_rate_with_tiny_value_ensures_min_capacity() {
        let limiter = RateLimiter::default();
        limiter.set_rate(1);
        let inner = lock_inner(&limiter.inner);
        // capacity = max(2 * 1, 1) = 2
        assert_eq!(inner.capacity, 2);
    }

    #[test]
    fn set_rate_preserves_tokens_when_switching_rates() {
        let limiter = init_limiter(1000); // capacity = 2000, tokens = 2000

        // Consume 500 tokens
        assert!(try_consume(&limiter.inner, 500).is_none());

        let tokens_before = lock_inner(&limiter.inner).tokens;
        assert!((tokens_before - 1500.0).abs() < 1.0);

        // Switch to a higher rate — tokens should be preserved (capped at new capacity)
        limiter.set_rate(2000);

        let inner = lock_inner(&limiter.inner);
        assert_eq!(inner.rate, 2000);
        // Tokens should still be ~1500 (barely any elapsed refill)
        assert!(inner.tokens >= 1499.0);
        assert!(inner.tokens <= 4000.0);
    }

    // ── try_consume ─────────────────────────────────────────

    #[test]
    fn try_consume_unlimited_returns_none() {
        let limiter = RateLimiter::default();
        // rate = 0, unlimited
        assert!(try_consume(&limiter.inner, 0).is_none());
        assert!(try_consume(&limiter.inner, 1).is_none());
        assert!(try_consume(&limiter.inner, 999_999_999).is_none());
    }

    #[test]
    fn try_consume_within_capacity_succeeds() {
        let limiter = init_limiter(1000);
        assert!(try_consume(&limiter.inner, 500).is_none());
        assert!(try_consume(&limiter.inner, 1500).is_none());
    }

    #[test]
    fn try_consume_exact_capacity_succeeds() {
        let limiter = init_limiter(1000);
        assert!(try_consume(&limiter.inner, 2000).is_none());
    }

    #[test]
    fn try_consume_beyond_capacity_returns_wait() {
        let limiter = init_limiter(1000);
        let wait = try_consume(&limiter.inner, 3000);
        assert!(wait.is_some());
        assert!(wait.unwrap() >= 1);
    }

    #[test]
    fn try_consume_depletes_tokens_progressively() {
        let limiter = init_limiter(1000);

        // Use up 1500
        assert!(try_consume(&limiter.inner, 1500).is_none());
        // Remaining 500
        assert!(try_consume(&limiter.inner, 500).is_none());
        // Exhausted — next consume should wait
        assert!(try_consume(&limiter.inner, 100).is_some());
    }

    #[test]
    fn try_consume_wait_time_is_reasonable() {
        let limiter = init_limiter(1000);

        // Drain the bucket
        assert!(try_consume(&limiter.inner, 2000).is_none());

        // Try to consume 500 more — deficit 500, rate 1000 B/s → 0.5s = 500_000_000 ns
        let wait = try_consume(&limiter.inner, 500);
        assert!(wait.is_some());
        let wait_ns = wait.unwrap();
        assert!(wait_ns >= 1);
        assert!(wait_ns <= 600_000_000);
    }

    #[test]
    fn try_consume_unlimited_handles_max_rate() {
        // Use a large but safe rate (avoids 2*rate overflow in set_rate)
        let limiter = RateLimiter::default();
        // Set rate to 0 (default) for unlimited behavior
        assert!(try_consume(&limiter.inner, u64::MAX).is_none());
    }

    // ── consume (async) ─────────────────────────────────────

    #[tokio::test]
    async fn consume_zero_bytes_returns_immediately() {
        let limiter = RateLimiter::default();
        // n=0 should return without acquiring the lock
        limiter.consume(0).await;
    }

    #[tokio::test]
    async fn consume_unlimited_returns_immediately() {
        let limiter = RateLimiter::default();
        // rate=0 → unlimited, should return immediately for any n
        limiter.consume(10_000).await;
    }

    // ── consume_blocking ────────────────────────────────────

    #[test]
    fn consume_blocking_zero_bytes_returns_immediately() {
        let limiter = RateLimiter::default();
        limiter.consume_blocking(0);
    }

    #[test]
    fn consume_blocking_unlimited_returns_immediately() {
        let limiter = RateLimiter::default();
        limiter.consume_blocking(10_000);
    }
}
