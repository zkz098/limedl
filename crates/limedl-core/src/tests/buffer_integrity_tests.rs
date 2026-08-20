//! Integrity tests for the SSD / HDD write-combining buffers under fault
//! conditions (write error / blocked flush / buffer pressure).
//!
//! The real-world bug report surfaced as an error log like
//! `background SSD ping-pong buffer flush failed (IoWorker)`. That log comes from
//! `DownloadBuffer::buffer_chunk_pingpong_impl`: when a background flush batch
//! fails, the buffer sets its internal `error_flag`, which 1) makes subsequent
//! `buffer_chunk` calls return `Err` so the chunk worker falls back to direct
//! I/O, and 2) makes `flush_all` return an error so the engine must end the task
//! in `Failed` rather than silently completing with a corrupt or short file.
//!
//! These tests prove that guarantee at two levels:
//! - Buffer level: a background flush failure is surfaced (`has_degraded()`,
//!   `flush_all` errors) and never corrupts previously written bytes — written
//!   data stays at the correct offsets, only the failed batch's region is absent.
//! - Pipeline level: a real download using the SSD write-combining buffer ends
//!   in `Failed` (never a silent `Completed`) when a background flush is injected
//!   to fail mid-transfer.
//!
//! Fault injection is deterministic and targeted by file pointer
//! (`crate::buffer_pool::fault`), so parallel tests writing to other files are
//! never disturbed.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use ntest::timeout;
use tempfile::TempDir;
use tokio::time::sleep;

use crate::DownloadManager;
use crate::buffer_pool::{BufferPool, DownloadBuffer, IoWorker};
use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::test_harness::TestServer;
use crate::types::{
    AppSettings, ChecksumMode, DiskType, DownloadState, IoBaselineSettings, SchedulerSettings,
    StartDownloadRequest,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deterministic non-zero byte pattern so an unwritten (preallocated, zeroed)
/// region is always distinguishable from a written one.
fn pattern_at(i: u64) -> u8 {
    (i % 250 + 1) as u8
}

fn pattern_chunk(start: u64, len: u64) -> Vec<u8> {
    (start..start + len).map(pattern_at).collect()
}

/// Assert that every non-zero byte in `file` sits at exactly its expected
/// offset (no shifted/corrupt data anywhere) and return the count of written
/// bytes.
fn assert_no_wrong_bytes(file: &[u8]) -> u64 {
    let mut written = 0;
    for (i, &b) in file.iter().enumerate() {
        if b != 0 {
            assert_eq!(b, pattern_at(i as u64), "byte corruption at offset {i}");
            written += 1;
        }
    }
    written
}

fn preallocated_file(dir: &std::path::Path, total: u64) -> std::io::Result<Arc<std::fs::File>> {
    let path = dir.join("test.bin");
    let file = std::fs::File::create(&path)?;
    file.set_len(total)?;
    Ok(Arc::new(file))
}

async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> crate::types::DownloadSnapshot {
    let start = Instant::now();
    loop {
        let status = manager.status(id).await.unwrap();
        if matches!(
            status.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return status;
        }
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "download {id} did not reach terminal state"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

// ==========================================================================
// Buffer-level: SSD write-combining (local ping-pong)
// ==========================================================================

/// Baseline: without a fault, feeding the whole file through the SSD ping-pong
/// buffer yields a byte-identical file.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn ssd_pingpong_no_fault_byte_identical() -> TestResult {
    const TOTAL: u64 = 512 * 1024;
    const HALF: u64 = 64 * 1024;
    const CHUNK: u64 = 32 * 1024;

    let dir = tempfile::tempdir()?;
    let file = preallocated_file(dir.path(), TOTAL)?;
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(HALF, file.clone(), worker);

    let mut offset = 0u64;
    while offset < TOTAL {
        let data = Bytes::from(pattern_chunk(offset, CHUNK));
        buf.buffer_chunk(offset, data).await?;
        offset += CHUNK;
    }
    buf.flush_all().await?;

    let on_disk = std::fs::read(dir.path().join("test.bin"))?;
    assert_eq!(on_disk.len() as u64, TOTAL);
    for (i, &b) in on_disk.iter().enumerate() {
        assert_eq!(b, pattern_at(i as u64), "SSD buffer wrote wrong byte at {i}");
    }
    Ok(())
}

/// SSD ping-pong: when a background flush batch fails, the buffer must surface
/// the failure (`has_degraded`, `flush_all` errors) and never corrupt
/// previously written bytes — written data stays at correct offsets and only
/// the failed batch's region is absent.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn ssd_pingpong_flush_failure_surfaces_error_without_corruption() -> TestResult {
    const TOTAL: u64 = 512 * 1024;
    const HALF: u64 = 64 * 1024;
    const CHUNK: u64 = 32 * 1024;

    let dir = tempfile::tempdir()?;
    let file = preallocated_file(dir.path(), TOTAL)?;
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_local_pingpong_with_worker(HALF, file.clone(), worker);
    let ptr = Arc::as_ptr(&file) as usize;

    // Fail the 3rd background batch — a middle flush, with correct data before
    // it and more data buffered after it.
    // Serialise with other fault-injecting tests (shared fault globals).
    let _inj = crate::buffer_pool::fault::injection_lock().await;
    let _guard = crate::buffer_pool::fault::arm(ptr, 2);

    let mut offset = 0u64;
    while offset < TOTAL / 2 {
        let data = Bytes::from(pattern_chunk(offset, CHUNK));
        // After the background failure is detected, buffer_chunk returns Err —
        // exactly the signal the chunk worker uses to fall back to direct I/O.
        if buf.buffer_chunk(offset, data).await.is_err() {
            break;
        }
        offset += CHUNK;
    }

    assert!(
        buf.flush_all().await.is_err(),
        "flush_all must surface the background flush failure (integrity preserved)"
    );
    // flush_all awaits the in-flight background flush, so the failure has fully
    // propagated by now and the buffer must be marked degraded.
    assert!(
        buf.has_degraded(),
        "background flush failure must set the buffer's error flag"
    );

    let on_disk = std::fs::read(dir.path().join("test.bin"))?;
    let written = assert_no_wrong_bytes(&on_disk);
    // A failed batch means some buffered bytes were lost — but this is surfaced
    // as an error, never silently presented as success.
    assert!(
        written < TOTAL,
        "a failed batch must leave some bytes absent (surfaced, not silent)"
    );
    // And bytes written before the failure must have landed intact.
    assert!(
        written > 0,
        "expected correctly-written bytes to survive the failure"
    );
    Ok(())
}

// ==========================================================================
// Buffer-level: HDD double-buffer (pool-backed)
// ==========================================================================

/// Baseline: without a fault, the HDD double-buffer writes a byte-identical file.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn hdd_double_buffer_no_fault_byte_identical() -> TestResult {
    const TOTAL: u64 = 512 * 1024;
    const CHUNK: u64 = 32 * 1024;

    let dir = tempfile::tempdir()?;
    let file = preallocated_file(dir.path(), TOTAL)?;
    let pool = Arc::new(BufferPool::new(0, 0, 1, 1)); // min 64 KiB half
    let slot = pool.acquire_slot().await;
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_with_worker(pool, slot, file.clone(), worker);

    let mut offset = 0u64;
    while offset < TOTAL {
        let data = Bytes::from(pattern_chunk(offset, CHUNK));
        buf.buffer_chunk(offset, data).await?;
        offset += CHUNK;
    }
    buf.flush_all().await?;

    let on_disk = std::fs::read(dir.path().join("test.bin"))?;
    assert_eq!(on_disk.len() as u64, TOTAL);
    for (i, &b) in on_disk.iter().enumerate() {
        assert_eq!(b, pattern_at(i as u64), "HDD buffer wrote wrong byte at {i}");
    }
    Ok(())
}

/// HDD double-buffer: a background flush failure is surfaced and never corrupts
/// previously written bytes.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn hdd_double_buffer_flush_failure_surfaces_error_without_corruption() -> TestResult {
    const TOTAL: u64 = 512 * 1024;
    const CHUNK: u64 = 32 * 1024;

    let dir = tempfile::tempdir()?;
    let file = preallocated_file(dir.path(), TOTAL)?;
    let pool = Arc::new(BufferPool::new(0, 0, 1, 1)); // min 64 KiB half
    let slot = pool.acquire_slot().await;
    let worker = IoWorker::spawn();
    let buf = DownloadBuffer::new_with_worker(pool, slot, file.clone(), worker);
    let ptr = Arc::as_ptr(&file) as usize;

    let _inj = crate::buffer_pool::fault::injection_lock().await;
    let _guard = crate::buffer_pool::fault::arm(ptr, 2);

    let mut offset = 0u64;
    while offset < TOTAL / 2 {
        let data = Bytes::from(pattern_chunk(offset, CHUNK));
        if buf.buffer_chunk(offset, data).await.is_err() {
            break;
        }
        offset += CHUNK;
    }

    assert!(buf.flush_all().await.is_err());
    assert!(buf.has_degraded());

    let on_disk = std::fs::read(dir.path().join("test.bin"))?;
    let written = assert_no_wrong_bytes(&on_disk);
    assert!(written < TOTAL, "failed batch must leave bytes absent (surfaced)");
    assert!(written > 0, "expected correctly-written bytes to survive");
    Ok(())
}

// ==========================================================================
// Pipeline: SSD write-combining buffer under an injected flush failure
// ==========================================================================

/// Full-pipeline E2E reproduction of the reported "SSD double-buffer flush
/// failed" scenario: a real multi-threaded download using the SSD write-combining
/// buffer has a background flush injected to fail mid-transfer. The download
/// must end in `Failed` — never a silent `Completed` with a corrupt file.
#[tokio::test(flavor = "multi_thread")]
#[timeout(180_000)]
async fn pipeline_ssd_buffer_flush_failure_ends_failed() -> TestResult {
    const SIZE: u64 = 96 * 1024 * 1024;

    let server = TestServer::new(SIZE).await;
    let temp = TempDir::new()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let dest_dir = temp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // Force the SSD write-combining buffer with a small half (2 MiB limit ->
    // ~1 MiB half) so many background flips occur during the transfer, giving a
    // wide window to hit one mid-transfer.
    let mut overrides = foldhash::HashMap::default();
    overrides.insert(dest_dir.to_string_lossy().to_string(), DiskType::Ssd);
    manager
        .apply_settings(AppSettings {
            scheduler: SchedulerSettings {
                connection_warmup_enabled: false,
                tail_sprint_enabled: false,
                ..Default::default()
            },
            io_baseline: IoBaselineSettings {
                disk_type_overrides: overrides,
                ssd_write_combine_mb: 2,
                ..Default::default()
            },
            ..Default::default()
        })
        .await?;

    let request = StartDownloadRequest {
        kind: None,
        url: server.file_url_range(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test.bin".into()),
        thread_mode: None,
        thread_count: Some(4),
        max_retries: Some(2),
        checksum: Some(ChecksumMode::Sha256),
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        user_agent: None,
        priority: None,
    };
    // Serialise with other fault-injecting tests (shared fault globals). Acquire
    // the lock BEFORE starting the download so the fault state can't be touched
    // while our download (and its buffer registration / batch writes) is live.
    let _inj = crate::buffer_pool::fault::injection_lock().await;
    let id = manager.start(request).await?;
    let id_str = id.to_string();

    // Resolve OUR download's temp file by id, then arm a fault against a middle
    // background batch once the buffer exists.
    let deadline = Instant::now() + Duration::from_secs(30);
    let ptr = loop {
        if let Some(p) = crate::buffer_pool::fault::file_ptr_for(&id_str) {
            break p;
        }
        let st = manager.status(&id_str).await?;
        if matches!(
            st.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            panic!("download reached terminal before the buffer was created");
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for download buffer registration"
        );
        sleep(Duration::from_millis(5)).await;
    };
    let _guard = crate::buffer_pool::fault::arm(ptr, 3);

    let snap = wait_for_terminal(&manager, &id_str).await;
    assert!(
        matches!(snap.state, DownloadState::Failed),
        "SSD buffer flush failure must end in Failed (integrity preserved), got {:?}: {:?}",
        snap.state,
        snap.error
    );
    Ok(())
}
