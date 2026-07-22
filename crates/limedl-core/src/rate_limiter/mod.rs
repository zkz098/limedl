use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::Mutex;

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
    #[allow(dead_code)]
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
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

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

    // ── concurrent stress tests ────────────────────────────

    #[tokio::test(flavor = "multi_thread")]
    #[ntest::timeout(15_000)]
    async fn concurrent_token_consumption_no_deadlock() {
        let limiter = Arc::new(init_limiter(1_000_000));
        let mut handles = Vec::with_capacity(10);

        for _ in 0..10 {
            let limiter = limiter.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..100 {
                    limiter.consume(1024).await;
                }
            }));
        }

        let all = futures_util::future::join_all(handles);
        tokio::time::timeout(Duration::from_secs(10), all)
            .await
            .expect("deadlock or timeout: not all tasks completed within 10s")
            .into_iter()
            .for_each(|r| r.expect("task panicked"));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ntest::timeout(15_000)]
    async fn concurrent_token_fairness() {
        let limiter = Arc::new(init_limiter(50_000));
        let barrier = Arc::new(tokio::sync::Barrier::new(5));

        let handles: Vec<_> = (0..5)
            .map(|_| {
                let limiter = limiter.clone();
                let barrier = barrier.clone();
                tokio::spawn(async move {
                    barrier.wait().await;
                    let start = tokio::time::Instant::now();
                    for _ in 0..10 {
                        limiter.consume(100).await;
                    }
                    start.elapsed()
                })
            })
            .collect();

        let results: Vec<Duration> = futures_util::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.expect("task panicked"))
            .collect();

        let max = results.iter().max().unwrap().as_nanos();
        let min = results.iter().min().unwrap().as_nanos();

        // Use a generous ratio to tolerate CI runner scheduling variance.
        // The token bucket algorithm is fair by design (tasks wait their turn);
        // this assertion only guards against pathological lock contention.
        assert!(
            max <= min * 1000,
            "fairness violation: fastest task={}ns, slowest task={}ns (ratio={})",
            min,
            max,
            max as f64 / min as f64
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ntest::timeout(15_000)]
    async fn concurrent_refill_while_consuming() {
        let limiter = Arc::new(init_limiter(100_000));
        let consumed = Arc::new(AtomicU64::new(0));
        let start = tokio::time::Instant::now();

        // Consumer tasks
        let mut consumer_handles = Vec::new();
        for _ in 0..10 {
            let limiter = limiter.clone();
            let consumed = consumed.clone();
            consumer_handles.push(tokio::spawn(async move {
                for _ in 0..30 {
                    limiter.consume(1024).await;
                    consumed.fetch_add(1024, Ordering::Relaxed);
                }
            }));
        }

        // Refiller task — periodically triggers refill via set_rate
        let limiter_r = limiter.clone();
        let refiller = tokio::spawn(async move {
            for _ in 0..15 {
                tokio::time::sleep(Duration::from_millis(100)).await;
                limiter_r.set_rate(100_000); // triggers refill + resets last_refill
            }
        });

        let all_consumers = futures_util::future::join_all(consumer_handles);
        tokio::time::timeout(Duration::from_secs(10), async {
            all_consumers.await;
            refiller.await.expect("refiller panicked");
        })
        .await
        .expect("timeout: deadlock or hang");

        let elapsed = start.elapsed().as_secs_f64();
        let total_consumed = consumed.load(Ordering::Relaxed) as f64;
        let remaining = lock_inner(&limiter.inner).tokens;

        // Invariant checks
        assert!(remaining >= 0.0, "negative tokens: {}", remaining);
        assert!(
            remaining <= 200_000.0,
            "tokens exceeded capacity: {}",
            remaining
        );
        assert!(total_consumed > 0.0, "no tokens were consumed");

        // Conservative integrity check: the total tokens accounted for
        // (consumed + remaining) should not exceed what could have been
        // generated: initial_tokens(capacity) + rate * elapsed.
        let upper_bound = 200_000.0 + 100_000.0 * elapsed;
        assert!(
            total_consumed + remaining <= upper_bound + 1000.0,
            "possible token leak: consumed={:.0}, remaining={:.0}, max_possible={:.0}",
            total_consumed,
            remaining,
            upper_bound
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ntest::timeout(30_000)]
    async fn concurrent_speed_limit_enforcement_under_load() {
        const LIMIT: u64 = 1_000_000; // 1 MB/s
        const TOLERANCE: f64 = 0.20; // 20% margin (partial token preservation loosens enforcement)

        let limiter = Arc::new(RateLimiter::default());
        limiter.set_rate(LIMIT);

        let consumed = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicU64::new(0));
        let barrier = Arc::new(tokio::sync::Barrier::new(21)); // 20 consumers + main

        let mut handles = Vec::with_capacity(20);
        for _ in 0..20 {
            let limiter = limiter.clone();
            let consumed = consumed.clone();
            let stop = stop.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await; // sync start
                loop {
                    if stop.load(Ordering::Relaxed) != 0 {
                        break;
                    }
                    limiter.consume(16_384).await;
                    consumed.fetch_add(16_384, Ordering::Relaxed);
                }
            }));
        }

        // Wait for all 20 consumers to be ready, then start the clock
        barrier.wait().await;
        let start = tokio::time::Instant::now();

        // Let them run for the measurement window
        tokio::time::sleep(Duration::from_secs(2)).await;
        stop.store(1, Ordering::Relaxed);
        let elapsed = start.elapsed().as_secs_f64();

        // Wait for all tasks to finish
        for h in handles {
            tokio::time::timeout(Duration::from_secs(5), h)
                .await
                .expect("task did not stop")
                .expect("task panicked");
        }

        let total = consumed.load(Ordering::Relaxed);
        let throughput = total as f64 / elapsed;
        let max_allowed = LIMIT as f64 * (1.0 + TOLERANCE);

        assert!(
            throughput <= max_allowed,
            "throughput {:.0} B/s exceeds limit+tolerance {:.0} B/s (limit={} B/s, total={} B, elapsed={:.2}s)",
            throughput,
            max_allowed,
            LIMIT,
            total,
            elapsed
        );
        assert!(total > 0, "no data consumed");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ntest::timeout(30_000)]
    async fn massive_concurrent_contention() {
        // Very low rate → high contention for scarce tokens
        let limiter = Arc::new(init_limiter(1000));

        let mut handles = Vec::with_capacity(50);
        for _ in 0..50 {
            let limiter = limiter.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..5 {
                    limiter.consume(100).await;
                }
            }));
        }

        let all = futures_util::future::join_all(handles);
        tokio::time::timeout(Duration::from_secs(30), all)
            .await
            .expect("timeout or hang under massive contention")
            .into_iter()
            .for_each(|r| r.expect("task panicked"));
    }
}
