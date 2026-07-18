//! End-to-end download benchmarks.
//!
//! Measures download throughput under different network conditions using
//! the local `TestServer` and a shared `reqwest` client.  Five scenarios
//! exercise baseline (unlimited), latency-simulated, and bandwidth-limited
//! endpoints.
//!
//! Run: `cargo bench --manifest-path src-tauri/Cargo.toml --features test-utils --bench download_e2e`
//!
//! For a quick smoke test:
//! `cargo bench ... -- --quick --noplot`

mod common;

use criterion::{criterion_group, criterion_main, Criterion, Throughput, black_box};
use common::BenchHarness;

/// File size served by the test server for every scenario.
///
/// Kept at 1 MB so even the slowest scenario (5 MB/s) completes in ~200 ms,
/// keeping iteration times reasonable for criterion.
const FILE_SIZE: u64 = 1_000_000;

fn bench_download_e2e(c: &mut Criterion) {
    let harness = BenchHarness::new(FILE_SIZE);

    let mut group = c.benchmark_group("e2e");
    group.throughput(Throughput::Bytes(FILE_SIZE));
    // Fewer samples than the default (100) because network benchmarks have
    // higher per-iteration variance and are more expensive.
    group.sample_size(10);

    // Shared client enables TCP connection reuse across all scenarios,
    // keeping measurements focused on throughput rather than connect time.
    let client = reqwest::Client::new();

    // ── 1. Baseline: localhost, no delay, unlimited bandwidth ────────────
    {
        let url = harness.server.file_url();
        group.bench_function("baseline", |b| {
            b.iter(|| {
                harness.rt.block_on(async {
                    let resp = client.get(&url).send().await.unwrap();
                    let bytes = resp.bytes().await.unwrap();
                    black_box(bytes.len());
                });
            });
        });
    }

    // ── 2. Low latency: 10 ms initial delay (good network) ──────────────
    {
        let url = harness.server.file_url_slow(10);
        group.bench_function("low_latency", |b| {
            b.iter(|| {
                harness.rt.block_on(async {
                    let resp = client.get(&url).send().await.unwrap();
                    let bytes = resp.bytes().await.unwrap();
                    black_box(bytes.len());
                });
            });
        });
    }

    // ── 3. Medium latency: 50 ms initial delay (typical network) ────────
    {
        let url = harness.server.file_url_slow(50);
        group.bench_function("medium_latency", |b| {
            b.iter(|| {
                harness.rt.block_on(async {
                    let resp = client.get(&url).send().await.unwrap();
                    let bytes = resp.bytes().await.unwrap();
                    black_box(bytes.len());
                });
            });
        });
    }

    // ── 4. Low bandwidth: 10 MB/s cap ────────────────────────────────────
    {
        let url = harness.server.file_url_bandwidth(10_000_000);
        group.bench_function("low_bandwidth", |b| {
            b.iter(|| {
                harness.rt.block_on(async {
                    let resp = client.get(&url).send().await.unwrap();
                    let bytes = resp.bytes().await.unwrap();
                    black_box(bytes.len());
                });
            });
        });
    }

    // ── 5. Very low bandwidth: 5 MB/s cap ────────────────────────────────
    {
        let url = harness.server.file_url_bandwidth(5_000_000);
        group.bench_function("very_low_bandwidth", |b| {
            b.iter(|| {
                harness.rt.block_on(async {
                    let resp = client.get(&url).send().await.unwrap();
                    let bytes = resp.bytes().await.unwrap();
                    black_box(bytes.len());
                });
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_download_e2e);
criterion_main!(benches);
