//! End-to-end download benchmarks.
//!
//! Measures download throughput under different network conditions using
//! the local `TestServer` and a shared `reqwest` client.
//!
//! Run: `cargo bench -p limedl-core --features test-utils --bench download_e2e`

use divan::Bencher;
use limedl_testkit::BenchHarness;

/// File size served by the test server for every scenario.
const FILE_SIZE: u64 = 1_000_000;

fn main() {
    divan::main();
}

// ── 1. Baseline: localhost, no delay, unlimited bandwidth ────────────

#[divan::bench]
fn bench_baseline(bencher: Bencher) {
    let harness = BenchHarness::new(FILE_SIZE);
    let client = reqwest::Client::new();
    let url = harness.server.file_url();

    bencher.bench_local(|| {
        harness.rt.block_on(async {
            let resp = client.get(&url).send().await.unwrap();
            let bytes = resp.bytes().await.unwrap();
            std::hint::black_box(bytes.len());
        });
    });
}

// ── 2. Low latency: 10 ms initial delay (good network) ──────────────

#[divan::bench]
fn bench_low_latency(bencher: Bencher) {
    let harness = BenchHarness::new(FILE_SIZE);
    let client = reqwest::Client::new();
    let url = harness.server.file_url_slow(10);

    bencher.bench_local(|| {
        harness.rt.block_on(async {
            let resp = client.get(&url).send().await.unwrap();
            let bytes = resp.bytes().await.unwrap();
            std::hint::black_box(bytes.len());
        });
    });
}

// ── 3. Medium latency: 50 ms initial delay (typical network) ────────

#[divan::bench]
fn bench_medium_latency(bencher: Bencher) {
    let harness = BenchHarness::new(FILE_SIZE);
    let client = reqwest::Client::new();
    let url = harness.server.file_url_slow(50);

    bencher.bench_local(|| {
        harness.rt.block_on(async {
            let resp = client.get(&url).send().await.unwrap();
            let bytes = resp.bytes().await.unwrap();
            std::hint::black_box(bytes.len());
        });
    });
}

// ── 4. Low bandwidth: 10 MB/s cap ────────────────────────────────────

#[divan::bench]
fn bench_low_bandwidth(bencher: Bencher) {
    let harness = BenchHarness::new(FILE_SIZE);
    let client = reqwest::Client::new();
    let url = harness.server.file_url_bandwidth(10_000_000);

    bencher.bench_local(|| {
        harness.rt.block_on(async {
            let resp = client.get(&url).send().await.unwrap();
            let bytes = resp.bytes().await.unwrap();
            std::hint::black_box(bytes.len());
        });
    });
}

// ── 5. Very low bandwidth: 5 MB/s cap ────────────────────────────────

#[divan::bench]
fn bench_very_low_bandwidth(bencher: Bencher) {
    let harness = BenchHarness::new(FILE_SIZE);
    let client = reqwest::Client::new();
    let url = harness.server.file_url_bandwidth(5_000_000);

    bencher.bench_local(|| {
        harness.rt.block_on(async {
            let resp = client.get(&url).send().await.unwrap();
            let bytes = resp.bytes().await.unwrap();
            std::hint::black_box(bytes.len());
        });
    });
}
