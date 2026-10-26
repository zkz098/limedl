//! Benchmarks for the token-bucket rate limiter.
//!
//! Measures overhead of the async and blocking consume paths, rate
//! switching cost, and concurrent fairness under contention.
//!
//! Run: `cargo bench -p limedl-http --features test-utils --bench rate_limiter`

use std::sync::Arc;

use divan::Bencher;
use limedl_http::rate_limiter::RateLimiter;

fn main() {
    divan::main();
}

fn bench_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("tokio runtime for benchmark")
}

// ── 1. Unlimited consume (baseline overhead) ─────────────────────────────

#[divan::bench]
fn bench_consume_unlimited(bencher: Bencher) {
    let rt = bench_runtime();
    let limiter = RateLimiter::default();

    bencher.bench_local(|| {
        rt.block_on(limiter.consume(std::hint::black_box(4096)));
    });
}

// ── 2. Limited consume (100 MB/s, plentiful budget) ──────────────────────

#[divan::bench]
fn bench_consume_limited(bencher: Bencher) {
    const RATE: u64 = 100_000_000; // 100 MB/s

    let rt = bench_runtime();
    let limiter = RateLimiter::default();
    limiter.set_rate(RATE);
    limiter.set_tokens(limiter.capacity() as f64);

    bencher.bench_local(|| {
        rt.block_on(limiter.consume(std::hint::black_box(8192)));
    });
}

// ── 3. Rate switching (set_rate) ─────────────────────────────────────────

const RATES: [u64; 4] = [1024, 1_048_576, 100_000_000, 0];

#[divan::bench(args = RATES)]
fn bench_set_rate(rate: u64) {
    let limiter = RateLimiter::default();
    limiter.set_rate(std::hint::black_box(rate));
}

// ── 4. Blocking consume (unlimited + limited) ────────────────────────────

#[divan::bench]
fn bench_consume_blocking_unlimited() {
    let limiter = RateLimiter::default();
    limiter.consume_blocking(std::hint::black_box(4096));
}

#[divan::bench]
fn bench_consume_blocking_limited() {
    const RATE: u64 = 100_000_000;
    let limiter = RateLimiter::default();
    limiter.set_rate(RATE);
    limiter.set_tokens(limiter.capacity() as f64);
    limiter.consume_blocking(std::hint::black_box(4096));
}

// ── 5. Multi-consumer concurrent throughput ──────────────────────────────

#[divan::bench]
fn bench_multi_consume(bencher: Bencher) {
    const RATE: u64 = 50_000_000; // 50 MB/s
    const CONCURRENCY: usize = 4;
    const ITERATIONS: usize = 100;

    let rt = bench_runtime();
    let limiter = Arc::new({
        let l = RateLimiter::default();
        l.set_rate(RATE);
        l.set_tokens(l.capacity() as f64);
        l
    });

    bencher.bench_local(|| {
        rt.block_on(async {
            let mut handles = Vec::with_capacity(CONCURRENCY);
            for _ in 0..CONCURRENCY {
                let limiter = limiter.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..ITERATIONS {
                        limiter.consume(std::hint::black_box(4096)).await;
                    }
                }));
            }
            for handle in handles {
                handle.await.expect("multi-consume task panicked");
            }
        });
    });
}
