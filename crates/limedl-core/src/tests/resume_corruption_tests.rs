//! Pause/resume E2E corruption tests.
//!
//! The reported bug (a download that *finishes* but whose SHA-256 differs from
//! the source) is most likely to surface in stateful paths where file contents
//! are assembled across sessions: resume after pause, chunk re-claiming, and
//! partial-chunk tail writes. These tests pause a download at staggered points,
//! resume it, let it finish, and then re-read the assembled file from disk and
//! require its SHA-256 to equal the source.
//!
//! Resume is a prime suspect because the engine resets its incremental hasher on
//! pause and re-derives incomplete-chunk offsets from the manifest on resume — if
//! that offset arithmetic overlaps or gaps, the final file is corrupt even though
//! the download reports `Completed`.
//!
//! Two pausing strategies are used so the tests are deterministic rather than
//! racing a fast localhost download:
//! - `pause_on_any_progress`: pause the instant any bytes are observed, tearing
//!   down chunk workers mid-flight (multi-threaded range download).
//! - `pause_at_fraction`: use the bandwidth-throttled endpoint so progress is
//!   gradual and a specific fraction (e.g. the tail near completion) is reliably
//!   catchable.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::TempDir;
use tokio::time::sleep;

use crate::DownloadManager;
use crate::checksum::hash_slices;
use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::test_harness::TestServer;
use crate::types::{ChecksumMode, DownloadState, StartDownloadRequest};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> crate::types::DownloadSnapshot {
    let start = std::time::Instant::now();
    loop {
        let status = manager.status(id).await.unwrap();
        if matches!(
            status.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return status;
        }
        assert!(
            start.elapsed() < Duration::from_secs(90),
            "download {id} did not reach terminal state"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

/// Pause as soon as any download progress is observed, then resume. With a fast
/// localhost server this tears chunk workers down in the middle of their work.
async fn pause_on_any_progress(manager: &Arc<DownloadManager>, id: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        let snap = manager.status(id).await.unwrap();
        if snap.state == DownloadState::Completed || snap.state == DownloadState::Failed {
            panic!("download reached terminal before any progress was observed");
        }
        if snap.downloaded_bytes > 0 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for initial progress"
        );
        sleep(Duration::from_millis(5)).await;
    }
    do_pause_and_resume(manager, id).await;
}

/// Wait until at least `ratio * size` bytes are downloaded (using a
/// bandwidth-throttled server so progress is gradual), then pause and resume.
async fn pause_at_fraction(
    manager: &Arc<DownloadManager>,
    id: &str,
    size: u64,
    ratio: f64,
) {
    let target = (size as f64 * ratio) as u64;
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        let snap = manager.status(id).await.unwrap();
        if snap.state == DownloadState::Completed || snap.state == DownloadState::Failed {
            panic!("download reached terminal before pause threshold ({ratio})");
        }
        if snap.downloaded_bytes >= target {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {ratio} progress (got {})",
            snap.downloaded_bytes
        );
        sleep(Duration::from_millis(10)).await;
    }
    do_pause_and_resume(manager, id).await;
}

async fn do_pause_and_resume(manager: &Arc<DownloadManager>, id: &str) {
    let paused = manager.pause(id).await.expect("pause failed");
    assert!(
        matches!(paused.state, DownloadState::Paused),
        "expected Paused, got {:?}",
        paused.state
    );
    let resumed = manager.resume(id).await.expect("resume failed");
    assert!(
        matches!(
            resumed.state,
            DownloadState::Queued | DownloadState::Downloading | DownloadState::Retrying
        ),
        "expected active after resume, got {:?}",
        resumed.state
    );
}

async fn start_download(
    manager: &Arc<DownloadManager>,
    url: &str,
    dest_dir: &Path,
    threads: usize,
) -> String {
    let request = StartDownloadRequest {
        kind: None,
        url: url.to_string(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test.bin".into()),
        thread_mode: None,
        thread_count: Some(threads),
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
    manager.start(request).await.expect("start failed").to_string()
}

/// Verify a resumed, completed download is byte-identical to the source.
fn assert_download_matches_source(
    snap: &crate::types::DownloadSnapshot,
    server: &TestServer,
) -> TestResult {
    let bytes = std::fs::read(PathBuf::from(&snap.destination_path))?;
    assert_eq!(
        bytes.len() as u64,
        server.file_size,
        "resumed download length {} != source length {}",
        bytes.len(),
        server.file_size
    );
    assert_eq!(
        hash_slices(ChecksumMode::Sha256, &[&bytes]),
        server.sha256_hash,
        "resumed download produced a file that differs from source"
    );
    Ok(())
}

async fn run_multi_thread_resume() -> TestResult {
    const SIZE: u64 = 17 * 1024 * 1024;
    const THREADS: usize = 8;

    let server = TestServer::new(SIZE).await;
    let temp = TempDir::new()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let dest_dir = temp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = start_download(&manager, &server.file_url_range(), &dest_dir, THREADS).await;
    pause_on_any_progress(&manager, &id).await;
    let snap = wait_for_terminal(&manager, &id).await;
    assert!(
        matches!(snap.state, DownloadState::Completed),
        "resumed download ended in {:?}: {:?}",
        snap.state,
        snap.error
    );
    assert_download_matches_source(&snap, &server)
}

/// Pause a multi-threaded range download the instant any bytes arrive, resume, and
/// require the final on-disk bytes to match — exercises tearing down chunk workers
/// mid-flight and re-claiming the same chunks on resume.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn resume_mid_chunk_multithread_preserves_bytes() -> TestResult {
    run_multi_thread_resume().await
}

/// Pause mid-transfer using a bandwidth-throttled server so the fraction is
/// reliably observed, then resume and verify byte-identity. Exercises re-claiming
/// partially-written chunks without overlap or gaps.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn resume_after_mid_pause_preserves_bytes() -> TestResult {
    run_fraction_resume(0.55).await
}

/// Pause near completion (tail-chunk re-claim), then resume and verify byte-identity.
/// An off-by-one in the leftover offset is most likely to corrupt the final bytes here.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn resume_after_late_pause_preserves_bytes() -> TestResult {
    run_fraction_resume(0.9).await
}

async fn run_fraction_resume(ratio: f64) -> TestResult {
    const SIZE: u64 = 17 * 1024 * 1024;
    // ~2 MB/s throttle: 17 MiB takes ~8.5 s, making any fraction reliably catchable.
    const BPS: u64 = 2 * 1024 * 1024;

    let server = TestServer::new(SIZE).await;
    let temp = TempDir::new()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let dest_dir = temp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // Bandwidth endpoint is single-stream; pause/resume of partial data still
    // exercises the resume offset/merge path that the bug report implicates.
    let id = start_download(&manager, &server.file_url_bandwidth(BPS), &dest_dir, 1).await;
    pause_at_fraction(&manager, &id, SIZE, ratio).await;
    let snap = wait_for_terminal(&manager, &id).await;
    assert!(
        matches!(snap.state, DownloadState::Completed),
        "resumed download ended in {:?}: {:?}",
        snap.state,
        snap.error
    );
    assert_download_matches_source(&snap, &server)
}
