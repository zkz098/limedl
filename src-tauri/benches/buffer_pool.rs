//! Benchmarks for the buffer pool subsystem.
//!
//! Compares HDD double-buffer vs SSD local buffer write throughput.
//! These are **I/O benchmarks** — data is written to a real disk.
//!
//! ## Usage
//!
//! Set `BENCH_DISK` to the target directory on the disk you want to test:
//!
//! ```powershell
//! $env:BENCH_DISK = "D:\bench"
//! cargo bench --manifest-path src-tauri/Cargo.toml --features test-utils -- buffer_pool
//! ```
//!
//! If `BENCH_DISK` is not set, the system temp directory is used (may not
//! reflect real disk performance depending on tmpfs/RAM-disk behaviour).

mod common;

use std::env;
use std::fs::{self, File};
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use common::BenchHarness;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use limedl_lib::buffer_pool::{BufferPool, DownloadBuffer};

// ── Configuration ────────────────────────────────────────────────────────────
// 100 MB total data, written in 1 MB chunks.
const TOTAL_DATA: u64 = 100 * 1024 * 1024;
const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB
const CHUNK_COUNT: u64 = TOTAL_DATA / CHUNK_SIZE as u64;

/// Resolve the target disk path from `BENCH_DISK` or fall back to temp dir.
fn disk_path() -> String {
    env::var("BENCH_DISK").unwrap_or_else(|_| env::temp_dir().to_string_lossy().into())
}

/// Pre-allocated 1 MB chunk of zeroes shared across all iterations.
fn zero_chunk() -> Bytes {
    Bytes::from(vec![0u8; CHUNK_SIZE])
}

// ── HDD double-buffer benchmark ─────────────────────────────────────────────
//
// Creates a new `BufferPool`, acquires a slot, writes 100 MB via the
// double-buffer path, then flushes everything to disk. Measures end-to-end
// latency including the final `flush_all()`.

fn bench_double_hdd(c: &mut Criterion) {
    let harness = BenchHarness::new(1024); // file size irrelevant — just needs runtime
    let path = disk_path();
    let chunk = zero_chunk();

    let mut group = c.benchmark_group("buffer_pool");
    group.throughput(Throughput::Bytes(TOTAL_DATA));

    group.bench_function("double_hdd", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let pool = Arc::new(BufferPool::new(256, 64, 4, 2));
                    let slot = pool.acquire_slot().await;
                    let file_path = format!("{path}/limedl_bench_hdd_{}.tmp", uuid::Uuid::new_v4());
                    let file = Arc::new(File::create(&file_path).unwrap());
                    let buffer = DownloadBuffer::new(pool.clone(), slot, file.clone());

                    for i in 0..CHUNK_COUNT {
                        buffer
                            .buffer_chunk(i * CHUNK_SIZE as u64, chunk.clone())
                            .await
                            .unwrap();
                    }
                    buffer.flush_all().await.unwrap();

                    // Drop buffer first so the file handle is closed before we
                    // try to remove the file (Windows sharing semantics).
                    drop(buffer);
                    let _ = fs::remove_file(&file_path);
                }
                start.elapsed()
            })
        });
    });

    group.finish();
}

// ── SSD local-buffer benchmark ──────────────────────────────────────────────
//
// Uses a simple local-buffer (write-combining, 8 MB limit) to write the same
// 100 MB payload. The buffer auto-flushes when the local limit is exceeded;
// the final `flush_all()` persists any remaining buffered data.

fn bench_local_ssd(c: &mut Criterion) {
    let harness = BenchHarness::new(1024);
    let path = disk_path();
    let chunk = zero_chunk();

    let mut group = c.benchmark_group("buffer_pool");
    group.throughput(Throughput::Bytes(TOTAL_DATA));

    group.bench_function("local_ssd", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let file_path = format!("{path}/limedl_bench_ssd_{}.tmp", uuid::Uuid::new_v4());
                    let file = Arc::new(File::create(&file_path).unwrap());
                    let buffer = DownloadBuffer::new_local(8 * 1024 * 1024, file.clone());

                    for i in 0..CHUNK_COUNT {
                        buffer
                            .buffer_chunk(i * CHUNK_SIZE as u64, chunk.clone())
                            .await
                            .unwrap();
                    }
                    buffer.flush_all().await.unwrap();

                    drop(buffer);
                    let _ = fs::remove_file(&file_path);
                }
                start.elapsed()
            })
        });
    });

    group.finish();
}

// ── Direct write baseline (no buffer) ──────────────────────────────────────
//
// Writes each 1 MB chunk directly to disk via `spawn_blocking` without any
// in-memory buffering. This is the baseline — comparing `double_hdd` against
// this shows the benefit of the double-buffer optimization on HDD.

fn bench_direct_write(c: &mut Criterion) {
    let harness = BenchHarness::new(1024);
    let path = disk_path();
    let chunk = zero_chunk();

    let mut group = c.benchmark_group("buffer_pool");
    group.throughput(Throughput::Bytes(TOTAL_DATA));

    group.bench_function("direct_write", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let file_path =
                        format!("{path}/limedl_bench_direct_{}.tmp", uuid::Uuid::new_v4());
                    let mut file = File::create(&file_path).unwrap();

                    for i in 0..CHUNK_COUNT {
                        let offset = i * CHUNK_SIZE as u64;
                        let data = chunk.clone();
                        // Pass `file` through `spawn_blocking`: moved in, returned out.
                        // Sequential writes mean the kernel page cache still helps,
                        // but there is zero application-level buffering or pipelining.
                        file = tokio::task::spawn_blocking(move || {
                            use std::io::{Seek, SeekFrom, Write};
                            file.seek(SeekFrom::Start(offset)).expect("seek failed");
                            file.write_all(&data).expect("write_all failed");
                            file
                        })
                        .await
                        .expect("spawn_blocking failed");
                    }

                    drop(file);
                    let _ = fs::remove_file(&file_path);
                }
                start.elapsed()
            })
        });
    });

    group.finish();
}

// ── Multi-stream random-write benchmark ─────────────────────────────────
//
// Simulates 4 concurrent download streams writing 1 MB chunks at
// interleaved offsets (worst-case seek pattern for HDD).  This is the
// scenario where the double-buffer's pipelining matters: while one half
// flushes to disk, the other half receives writes from multiple tasks.
//
// Offsets are generated in reverse (99MB → 0MB) and distributed round‑robin
// across streams so no two successive writes target nearby offsets.
// Direct-write pays the full seek penalty; double-buffer absorbs it.

fn bench_multi_stream_random(c: &mut Criterion) {
    let harness = BenchHarness::new(1024);
    let path = disk_path();
    let chunk = zero_chunk();

    // Pre‑compute interleaved offsets: reverse order to maximise seeks.
    let offsets: Vec<u64> = (0..CHUNK_COUNT)
        .rev()
        .map(|i| i * CHUNK_SIZE as u64)
        .collect();

    let mut group = c.benchmark_group("buffer_pool/multi_stream");
    group.throughput(Throughput::Bytes(TOTAL_DATA));

    // ── Multi-stream through double-buffer (HDD) ─────────────────────────
    group.bench_function("double_hdd", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let pool = Arc::new(BufferPool::new(256, 64, 4, 2));
                    let slot = pool.acquire_slot().await;
                    let file_path =
                        format!("{path}/limedl_bench_mshdd_{}.tmp", uuid::Uuid::new_v4());
                    let file = Arc::new(File::create(&file_path).unwrap());
                    let buffer = Arc::new(DownloadBuffer::new(pool.clone(), slot, file.clone()));

                    const STREAMS: usize = 4;
                    let per_stream = offsets.len() / STREAMS;
                    let mut handles = Vec::with_capacity(STREAMS);
                    for s in 0..STREAMS {
                        let buf = buffer.clone();
                        let c = chunk.clone();
                        let slice: Vec<u64> =
                            offsets[s * per_stream..(s + 1) * per_stream].to_vec();
                        handles.push(tokio::spawn(async move {
                            for offset in slice {
                                buf.buffer_chunk(offset, c.clone()).await.unwrap();
                            }
                        }));
                    }
                    for h in handles {
                        h.await.unwrap();
                    }
                    buffer.flush_all().await.unwrap();

                    drop(buffer);
                    let _ = fs::remove_file(&file_path);
                }
                start.elapsed()
            })
        });
    });

    // ── Multi-stream through local buffer (SSD) ──────────────────────────
    group.bench_function("local_ssd", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let file_path =
                        format!("{path}/limedl_bench_msssd_{}.tmp", uuid::Uuid::new_v4());
                    let file = Arc::new(File::create(&file_path).unwrap());
                    let buffer = Arc::new(DownloadBuffer::new_local(8 * 1024 * 1024, file.clone()));

                    const STREAMS: usize = 4;
                    let per_stream = offsets.len() / STREAMS;
                    let mut handles = Vec::with_capacity(STREAMS);
                    for s in 0..STREAMS {
                        let buf = buffer.clone();
                        let c = chunk.clone();
                        let slice: Vec<u64> =
                            offsets[s * per_stream..(s + 1) * per_stream].to_vec();
                        handles.push(tokio::spawn(async move {
                            for offset in slice {
                                buf.buffer_chunk(offset, c.clone()).await.unwrap();
                            }
                        }));
                    }
                    for h in handles {
                        h.await.unwrap();
                    }
                    buffer.flush_all().await.unwrap();

                    drop(buffer);
                    let _ = fs::remove_file(&file_path);
                }
                start.elapsed()
            })
        });
    });

    // ── Multi-stream direct write (baseline) ─────────────────────────────
    group.bench_function("direct_write", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let file_path =
                        format!("{path}/limedl_bench_msdirect_{}.tmp", uuid::Uuid::new_v4());
                    let file = Arc::new(File::create(&file_path).unwrap());

                    const STREAMS: usize = 4;
                    let per_stream = offsets.len() / STREAMS;
                    let mut handles = Vec::with_capacity(STREAMS);
                    for s in 0..STREAMS {
                        let f = file.clone();
                        let c = chunk.clone();
                        let slice: Vec<u64> =
                            offsets[s * per_stream..(s + 1) * per_stream].to_vec();
                        handles.push(tokio::task::spawn_blocking(move || {
                            use std::io::{Seek, SeekFrom, Write};
                            let mut f = f.as_ref().try_clone().expect("try_clone");
                            for offset in slice {
                                f.seek(SeekFrom::Start(offset)).expect("seek");
                                f.write_all(&c).expect("write_all");
                            }
                        }));
                    }
                    for h in handles {
                        h.await.unwrap();
                    }

                    drop(file);
                    let _ = fs::remove_file(&file_path);
                }
                start.elapsed()
            })
        });
    });

    group.finish();
}

// ── Criterion plumbing ──────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_direct_write,
    bench_double_hdd,
    bench_local_ssd,
    bench_multi_stream_random,
);
criterion_main!(benches);
