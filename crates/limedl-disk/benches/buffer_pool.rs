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
//! cargo bench -p limedl-disk --bench buffer_pool
//! ```
//!
//! If `BENCH_DISK` is not set, the system temp directory is used (may not
//! reflect real disk performance depending on tmpfs/RAM-disk behaviour).

use std::env;
use std::fs::{self, File};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use bytes::Bytes;
use divan::Bencher;
use limedl_disk::buffer_pool::{BufferPool, DownloadBuffer, IoWorker};

// ── Configuration ────────────────────────────────────────────────────────────
// 100 MB total data, written in 1 MB chunks.
const TOTAL_DATA: u64 = 100 * 1024 * 1024;
const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB
const CHUNK_COUNT: u64 = TOTAL_DATA / CHUNK_SIZE as u64;
/// Number of concurrent streams in the multi-stream benchmark.
const STREAMS: usize = 4;

fn main() {
    divan::main();
}

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

/// Create the benchmark temp file off the runtime.
async fn create_temp_file(path: &str) -> File {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || File::create(&path).expect("create temp file"))
        .await
        .expect("spawn_blocking failed")
}

/// Remove the benchmark temp file, ignoring the outcome like the call site it
/// replaced (Windows may still hold the handle for a moment).
async fn remove_temp_file(path: &str) {
    let path = path.to_owned();
    let _ = tokio::task::spawn_blocking(move || fs::remove_file(&path)).await;
}

/// Drive `streams` tasks that each write their slice of `offsets` through `buffer`.
async fn drive_streams(
    buffer: &Arc<DownloadBuffer>,
    chunk: &Bytes,
    offsets: &[u64],
    streams: usize,
) {
    let per_stream = offsets.len() / streams;
    let mut handles = Vec::with_capacity(streams);
    for s in 0..streams {
        let buf = Arc::clone(buffer);
        let c = chunk.clone();
        let slice: Vec<u64> = offsets[s * per_stream..(s + 1) * per_stream].to_vec();
        handles.push(tokio::spawn(async move {
            for offset in slice {
                buf.buffer_chunk(offset, c.clone()).await.unwrap();
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
}

/// Runtime for the async benchmark bodies.
fn bench_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("tokio runtime for benchmark")
}

// ── HDD double-buffer benchmark ─────────────────────────────────────────────

#[divan::bench]
fn bench_double_hdd(bencher: Bencher) {
    let rt = bench_runtime();
    let chunk = zero_chunk();

    bencher.bench_local(|| {
        rt.block_on(async {
            let pool = Arc::new(BufferPool::new(256, 64, 4, 2));
            let slot = pool.acquire_slot().await;
            let file_path = unique_temp_name("hdd");
            let file = Arc::new(create_temp_file(&file_path).await);
            let buffer = DownloadBuffer::new(pool.clone(), slot, file.clone());

            for i in 0..CHUNK_COUNT {
                buffer
                    .buffer_chunk(i * CHUNK_SIZE as u64, chunk.clone())
                    .await
                    .unwrap();
            }
            buffer.flush_all().await.unwrap();

            drop(buffer);
            remove_temp_file(&file_path).await;
        });
    });
}

// ── SSD local-buffer benchmark ──────────────────────────────────────────────

#[divan::bench]
fn bench_local_ssd(bencher: Bencher) {
    let rt = bench_runtime();
    let chunk = zero_chunk();
    let worker = IoWorker::new(std::sync::Arc::new(
        limedl_disk::io_scheduler::DiskDeviceManager::new(),
    ));

    bencher.bench_local(|| {
        rt.block_on(async {
            let file_path = unique_temp_name("ssd");
            let file = Arc::new(create_temp_file(&file_path).await);
            let buffer = DownloadBuffer::new_local_pingpong_with_worker(
                4 * 1024 * 1024,
                file.clone(),
                std::path::PathBuf::from(&file_path),
                worker.clone(),
                None,
            );

            for i in 0..CHUNK_COUNT {
                buffer
                    .buffer_chunk(i * CHUNK_SIZE as u64, chunk.clone())
                    .await
                    .unwrap();
            }
            buffer.flush_all().await.unwrap();

            drop(buffer);
            remove_temp_file(&file_path).await;
        });
    });
}

// ── Direct write baseline (no buffer) ──────────────────────────────────────

#[divan::bench]
fn bench_direct_write(bencher: Bencher) {
    let rt = bench_runtime();
    let chunk = zero_chunk();

    bencher.bench_local(|| {
        rt.block_on(async {
            let file_path = unique_temp_name("direct");
            let mut file = create_temp_file(&file_path).await;

            for i in 0..CHUNK_COUNT {
                let offset = i * CHUNK_SIZE as u64;
                let data = chunk.clone();
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
            remove_temp_file(&file_path).await;
        });
    });
}

// ── Multi-stream random-write benchmark ─────────────────────────────────

#[divan::bench]
fn bench_multi_stream_random(bencher: Bencher) {
    let rt = bench_runtime();
    let chunk = zero_chunk();
    let offsets: Vec<u64> = (0..CHUNK_COUNT)
        .rev()
        .map(|i| i * CHUNK_SIZE as u64)
        .collect();

    bencher.bench_local(|| {
        rt.block_on(async {
            let pool = Arc::new(BufferPool::new(256, 64, 4, 2));
            let slot = pool.acquire_slot().await;
            let file_path = unique_temp_name("mshdd");
            let file = Arc::new(create_temp_file(&file_path).await);
            let buffer = Arc::new(DownloadBuffer::new(pool.clone(), slot, file.clone()));

            drive_streams(&buffer, &chunk, &offsets, STREAMS).await;
            buffer.flush_all().await.unwrap();

            drop(buffer);
            remove_temp_file(&file_path).await;
        });
    });
}
