//! Benchmarks for the AIMD congestion controller.
//!
//! These are pure algorithm benchmarks — no network I/O, no TestServer needed.
//! All AIMD functions are synchronous. The `common` module is imported but its
//! `BenchHarness` is not instantiated here (it would add unnecessary overhead).

mod common;

use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, black_box};
use limedl_lib::aimd::{AimdState, AdaptiveProfile, initial_desired_threads, reduce_threads};

// ── sample_throughput ───────────────────────────────────────────────────────

/// Measure the cost of calling `AimdState::sample_throughput` in a loop
/// simulating rapid sampling (100 samples at 10 ms intervals).
fn bench_sample_throughput(c: &mut Criterion) {
    c.bench_function("aimd_sample_throughput", |b| {
        b.iter_batched(
            || {
                // Pre-seed so the first real sample computes a rate.
                let mut state = AimdState::initial(None, None);
                let t0 = Instant::now();
                state.sample_throughput(0, t0);
                (state, t0)
            },
            |(mut state, t0)| {
                let mut now = t0;
                for i in 0..100 {
                    now += Duration::from_millis(10);
                    let bytes = black_box((i as u64 + 1) * 1024 * 1024);
                    let _ = black_box(state.sample_throughput(bytes, black_box(now)));
                }
            },
            BatchSize::SmallInput,
        );
    });
}

// ── reduce_threads ──────────────────────────────────────────────────────────

/// Benchmark `reduce_threads(current, profile, min_threads)` for all three
/// profiles at a representative thread count.
fn bench_reduce_threads(c: &mut Criterion) {
    let mut group = c.benchmark_group("aimd_reduce_threads");
    let profiles: &[(AdaptiveProfile, &str)] = &[
        (AdaptiveProfile::Conservative, "conservative"),
        (AdaptiveProfile::Balanced, "balanced"),
        (AdaptiveProfile::Aggressive, "aggressive"),
    ];

    for (profile, name) in profiles {
        group.bench_with_input(
            BenchmarkId::new("reduce_threads", *name),
            profile,
            |b, profile| {
                b.iter(|| {
                    let result = reduce_threads(black_box(16), *profile, black_box(1));
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

// ── convergence_burst ───────────────────────────────────────────────────────

/// Simulate a throughput burst: feed 200 synthetic samples at increasing byte
/// counts (ramping from 0 to ~200 MB over 2 seconds) and measure how
/// `sample_throughput` + `record_sample` converge.
fn bench_convergence_burst(c: &mut Criterion) {
    c.bench_function("aimd_convergence_burst", |b| {
        b.iter_batched(
            || AimdState::initial(None, None),
            |mut state| {
                let base = Instant::now();
                for i in 0..200 {
                    // Ramp from 0 to ~200 MB over 200 samples (2 s @ 10 ms each).
                    let bytes = black_box((i as u64) * 1024 * 1024);
                    let now = black_box(base + Duration::from_millis(i * 10));
                    if let Some(tp) = state.sample_throughput(bytes, now) {
                        state.record_sample(black_box(tp));
                    }
                }
            },
            BatchSize::SmallInput,
        );
    });
}

// ── initial_desired_threads ──────────────────────────────────────────────────

/// Benchmark `initial_desired_threads` for each profile.
/// Very fast, but useful for regression detection.
fn bench_initial_desired(c: &mut Criterion) {
    let mut group = c.benchmark_group("aimd_initial_desired_threads");
    let profiles: &[(AdaptiveProfile, &str)] = &[
        (AdaptiveProfile::Conservative, "conservative"),
        (AdaptiveProfile::Balanced, "balanced"),
        (AdaptiveProfile::Aggressive, "aggressive"),
    ];

    for (profile, name) in profiles {
        group.bench_with_input(
            BenchmarkId::new("initial_desired_threads", *name),
            profile,
            |b, profile| {
                b.iter(|| {
                    let result = initial_desired_threads(*profile);
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

// ── criterion plumbing ──────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_sample_throughput,
    bench_reduce_threads,
    bench_convergence_burst,
    bench_initial_desired,
);
criterion_main!(benches);
