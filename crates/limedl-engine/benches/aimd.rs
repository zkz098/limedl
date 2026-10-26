//! Benchmarks for the AIMD congestion controller.
//!
//! These are pure algorithm benchmarks — no network I/O, no test server needed.
//! All AIMD functions are synchronous.
//!
//! Run: `cargo bench -p limedl-engine --bench aimd`

use std::time::{Duration, Instant};

use divan::Bencher;
use limedl_engine::aimd::{AdaptiveProfile, AimdState, initial_desired_threads, reduce_threads};

fn main() {
    divan::main();
}

// ── sample_throughput ───────────────────────────────────────────────────────

/// Measure the cost of calling `AimdState::sample_throughput` in a loop
/// simulating rapid sampling (100 samples at 10 ms intervals).
#[divan::bench]
fn bench_sample_throughput(bencher: Bencher) {
    bencher
        .with_inputs(|| {
            // Pre-seed so the first real sample computes a rate.
            let mut state = AimdState::initial(None, None);
            let t0 = Instant::now();
            state.sample_throughput(0, t0);
            (state, t0)
        })
        .bench_values(|(mut state, t0)| {
            let mut now = t0;
            for i in 0..100 {
                now += Duration::from_millis(10);
                let bytes = std::hint::black_box((i as u64 + 1) * 1024 * 1024);
                let _ =
                    std::hint::black_box(state.sample_throughput(bytes, std::hint::black_box(now)));
            }
        });
}

// ── reduce_threads ──────────────────────────────────────────────────────────

const PROFILES: [AdaptiveProfile; 3] = [
    AdaptiveProfile::Conservative,
    AdaptiveProfile::Balanced,
    AdaptiveProfile::Aggressive,
];

/// Benchmark `reduce_threads(current, profile, min_threads)` for all three
/// profiles at a representative thread count.
#[divan::bench(args = PROFILES)]
fn bench_reduce_threads(profile: AdaptiveProfile) {
    let result = reduce_threads(std::hint::black_box(16), profile, std::hint::black_box(1));
    std::hint::black_box(result);
}

// ── convergence_burst ───────────────────────────────────────────────────────

/// Simulate a throughput burst: feed 200 synthetic samples at increasing byte
/// counts (ramping from 0 to ~200 MB over 2 seconds) and measure how
/// `sample_throughput` + `record_sample` converge.
#[divan::bench]
fn bench_convergence_burst(bencher: Bencher) {
    bencher
        .with_inputs(|| AimdState::initial(None, None))
        .bench_values(|mut state| {
            let base = Instant::now();
            for i in 0..200 {
                // Ramp from 0 to ~200 MB over 200 samples (2 s @ 10 ms each).
                let bytes = std::hint::black_box(i * 1024 * 1024);
                let now = std::hint::black_box(base + Duration::from_millis(i * 10));
                if let Some(tp) = state.sample_throughput(bytes, now) {
                    state.record_sample(std::hint::black_box(tp));
                }
            }
        });
}

// ── initial_desired_threads ──────────────────────────────────────────────────

/// Benchmark `initial_desired_threads` for each profile.
/// Very fast, but useful for regression detection.
#[divan::bench(args = PROFILES)]
fn bench_initial_desired(profile: AdaptiveProfile) {
    let result = initial_desired_threads(profile, 16);
    std::hint::black_box(result);
}
