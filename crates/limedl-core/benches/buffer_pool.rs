//! Benchmarks for the buffer pool subsystem.
//!
//! Compares HDD double-buffer vs SSD local buffer write throughput.
//! These are **I/O benchmarks** 鈥?data is written to a real disk.
//!
//! ## Usage
//!
//! Set `BENCH_DISK` to the target directory on the disk you want to test:
//!
//! ```powershell
//! $env:BENCH_DISK = "D:\bench"
//! cargo bench --manifest-path crates/limedl-core/Cargo.toml --features test-utils -- buffer_pool
//! ```
//!
//! If `BENCH_DISK` is not set, the system temp directory is used (may not
//! reflect real disk performance depending on tmpfs/RAM-disk behaviour).

mod common;

use std::env;
use std::fs::{self, File};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use bytes::Bytes;
use common::BenchHarness;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use limedl_core::buffer_pool::{BufferPool, DownloadBuffer, IoWorker};

// 鈹€鈹€ Configuration 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
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

/// Monotonically increasing counter for unique temp file names.
static FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_name(prefix: &str) -> String {
    let id = FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = disk_path();
    format!("{path}/limedl_bench_{prefix}_{id}.tmp")
}

// 鈹€鈹€ HDD double-buffer benchmark 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
//
// Creates a new `BufferPool`, acquires a slot, writes 100 MB via the
// double-buffer path, then flushes everything to disk. Measures end-to-end
// latency including the final `flush_all()`.

fn bench_double_hdd(c: &mut Criterion) {
    let harness = BenchHarness::new(1024); // file size irrelevant 鈥?just needs runtime
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
                    let file_path = unique_temp_name("hdd");
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

// 鈹€鈹€ SSD local-buffer benchmark 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
//
// Uses a local ping-pong (double-buffer) to write the same 100 MB payload.
// This is the production SSD write path. The buffer flushes a half when full;
// the final `flush_all()` persists any remaining buffered data.

fn bench_local_ssd(c: &mut Criterion) {
    let harness = BenchHarness::new(1024);
    let chunk = zero_chunk();

    let mut group = c.benchmark_group("buffer_pool");
    group.throughput(Throughput::Bytes(TOTAL_DATA));

    // Dedicated I/O worker thread (production SSD ping-pong path).
    let worker = IoWorker::spawn_pool(1);

    group.bench_function("ssd_pingpong", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let file_path = unique_temp_name("ssd");
                    let file = Arc::new(File::create(&file_path).unwrap());
                    let buffer = DownloadBuffer::new_local_pingpong_with_worker(
                        4 * 1024 * 1024,
                        file.clone(),
                        worker.clone(),
                    );

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

// 鈹€鈹€ Direct write baseline (no buffer) 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
//
// Writes each 1 MB chunk directly to disk via `spawn_blocking` without any
// in-memory buffering. This is the baseline 鈥?comparing `double_hdd` against
// this shows the benefit of the double-buffer optimization on HDD.

fn bench_direct_write(c: &mut Criterion) {
    let harness = BenchHarness::new(1024);
    let chunk = zero_chunk();

    let mut group = c.benchmark_group("buffer_pool");
    group.throughput(Throughput::Bytes(TOTAL_DATA));

    group.bench_function("direct_write", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let file_path = unique_temp_name("direct");
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

// 鈹€鈹€ Multi-stream random-write benchmark 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
//
// Simulates 4 concurrent download streams writing 1 MB chunks at
// interleaved offsets (worst-case seek pattern for HDD).  This is the
// scenario where the double-buffer's pipelining matters: while one half
// flushes to disk, the other half receives writes from multiple tasks.
//
// Offsets are generated in reverse (99MB 鈫?0MB) and distributed round鈥憆obin
// across streams so no two successive writes target nearby offsets.
// Direct-write pays the full seek penalty; double-buffer absorbs it.

fn bench_multi_stream_random(c: &mut Criterion) {
    let harness = BenchHarness::new(1024);
    let chunk = zero_chunk();

    // Pre鈥慶ompute interleaved offsets: reverse order to maximise seeks.
    let offsets: Vec<u64> = (0..CHUNK_COUNT)
        .rev()
        .map(|i| i * CHUNK_SIZE as u64)
        .collect();

    let mut group = c.benchmark_group("buffer_pool/multi_stream");
    group.throughput(Throughput::Bytes(TOTAL_DATA));

    // 鈹€鈹€ Multi-stream through double-buffer (HDD) 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
    group.bench_function("double_hdd", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let pool = Arc::new(BufferPool::new(256, 64, 4, 2));
                    let slot = pool.acquire_slot().await;
                    let file_path = unique_temp_name("mshdd");
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

    // Multi-stream through SSD ping-pong buffer.
    let worker = IoWorker::spawn_pool(1);
    group.bench_function("ssd_pingpong", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let file_path = unique_temp_name("msssd");
                    let file = Arc::new(File::create(&file_path).unwrap());
                    let buffer = Arc::new(DownloadBuffer::new_local_pingpong_with_worker(
                        4 * 1024 * 1024,
                        file.clone(),
                        worker.clone(),
                    ));

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

    // 鈹€鈹€ Multi-stream direct write (baseline) 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
    group.bench_function("direct_write", |b| {
        b.iter_custom(|iters| {
            harness.rt.block_on(async {
                let start = Instant::now();
                for _ in 0..iters {
                    let file_path = unique_temp_name("msdirect");
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

// 鈹€鈹€ Criterion plumbing 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

criterion_group!(
    benches,
    bench_direct_write,
    bench_double_hdd,
    bench_local_ssd,
    bench_multi_stream_random,
);
criterion_main!(benches);
