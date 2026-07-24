//! Benchmarks for the token-bucket rate limiter.
//!
//! Measures overhead of the async and blocking consume paths, rate
//! switching cost, and concurrent fairness under contention.
//!
//! Run: `cargo bench --manifest-path src-tauri/Cargo.toml --features test-utils`

mod common;

use std::sync::Arc;

use common::BenchHarness;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use limedl_lib::RateLimiter;

// ── helpers ──────────────────────────────────────────────────────────────

// ── 1. Unlimited consume (baseline overhead) ─────────────────────────────

fn bench_consume_unlimited(c: &mut Criterion) {
    let harness = BenchHarness::new(1024);
    let limiter = RateLimiter::default();

    c.bench_function("rate_limiter/consume_unlimited", |b| {
        b.iter(|| {
            harness.rt.block_on(limiter.consume(std::hint::black_box(4096)));
        });
    });
}

// ── 2. Limited consume (100 MB/s, plentiful budget) ──────────────────────

fn bench_consume_limited(c: &mut Criterion) {
    const RATE: u64 = 100_000_000; // 100 MB/s

    let harness = BenchHarness::new(1024);
    let limiter = RateLimiter::default();
    limiter.set_rate(RATE);
    limiter.set_tokens(limiter.capacity() as f64);

    c.bench_function("rate_limiter/consume_limited", |b| {
        b.iter(|| {
            harness.rt.block_on(limiter.consume(std::hint::black_box(8192)));
        });
    });
}

// ── 3. Rate switching (set_rate) ─────────────────────────────────────────

fn bench_set_rate(c: &mut Criterion) {
    let limiter = RateLimiter::default();

    let rates = [1024u64, 1_048_576, 100_000_000, 0];

    let mut group = c.benchmark_group("rate_limiter/set_rate");
    for rate in &rates {
        group.bench_with_input(BenchmarkId::from_parameter(rate), rate, |b, &rate| {
            b.iter(|| {
                limiter.set_rate(std::hint::black_box(rate));
            });
        });
    }
    group.finish();
}

// ── 4. Blocking consume (unlimited + limited) ────────────────────────────

fn bench_consume_blocking(c: &mut Criterion) {
    const RATE: u64 = 100_000_000;

    // Unlimited
    {
        let limiter = RateLimiter::default();
        c.bench_function("rate_limiter/consume_blocking/unlimited", |b| {
            b.iter(|| {
                limiter.consume_blocking(std::hint::black_box(4096));
            });
        });
    }

    // Limited (pre-filled)
    {
        let limiter = RateLimiter::default();
        limiter.set_rate(RATE);
        limiter.set_tokens(limiter.capacity() as f64);
        c.bench_function("rate_limiter/consume_blocking/limited", |b| {
            b.iter(|| {
                limiter.consume_blocking(std::hint::black_box(4096));
            });
        });
    }
}

// ── 5. Multi-consumer concurrent throughput ──────────────────────────────

fn bench_multi_consume(c: &mut Criterion) {
    const RATE: u64 = 50_000_000; // 50 MB/s
    const CONCURRENCY: usize = 4;
    const ITERATIONS: usize = 100;

    let harness = BenchHarness::new(1024);
    let limiter = Arc::new({
        let l = RateLimiter::default();
        l.set_rate(RATE);
        l.set_tokens(l.capacity() as f64);
        l
    });

    c.bench_function("rate_limiter/multi_consume", |b| {
        b.iter(|| {
            harness.rt.block_on(async {
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
    });
}

// ── criterion registration ───────────────────────────────────────────────

criterion_group!(
    benches,
    bench_consume_unlimited,
    bench_consume_limited,
    bench_set_rate,
    bench_consume_blocking,
    bench_multi_consume,
);
criterion_main!(benches);
