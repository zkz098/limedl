use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::tempdir;
use tokio::time::sleep;

use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::test_harness::TestServer;
use crate::types::{
    ChecksumMode, DownloadState, StartDownloadRequest, ThreadMode,
};
use crate::DownloadManager;


type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> crate::types::DownloadSnapshot {
    loop {
        let status = manager.status(id).await.unwrap();
        if matches!(
            status.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return status;
        }
        sleep(Duration::from_millis(100)).await;
    }
}

fn generate_test_content(size: u64) -> Vec<u8> {
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rand::RngCore;
    let mut rng = StdRng::seed_from_u64(42);
    let mut data = vec![0u8; size as usize];
    rng.fill_bytes(&mut data);
    data
}

// ==========================================================================
// Single-stream download tests (using /file - no Accept-Ranges)
// ==========================================================================

#[tokio::test]
#[timeout(30_000)]
async fn single_stream_download_completes_successfully() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id).await;
    assert_eq!(
        status.state, DownloadState::Completed,
        "expected Completed, got {:?} with error={:?}",
        status.state, status.error
    );
    assert_eq!(status.total_bytes, Some(server.file_size));
    assert_eq!(status.downloaded_bytes, server.file_size);

    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(dest_path.exists(), "destination file should exist at {}", status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded.len() as u64, server.file_size);

    let expected = generate_test_content(server.file_size);
    assert_eq!(downloaded, expected, "downloaded file content does not match server data");

    let _ = manager.remove(&id).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn single_stream_download_with_blake3_checksum_match() -> TestResult {
    let server = TestServer::new(32 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
            expected_checksum: Some(server.blake3_hash.clone()),
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id).await;
    assert_eq!(
        status.state, DownloadState::Completed,
        "expected Completed with matching Blake3 checksum, got {:?} error={:?}",
        status.state, status.error
    );

    let _ = manager.remove(&id).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn single_stream_checksum_mismatch_fails() -> TestResult {
    let server = TestServer::new(16 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let wrong_checksum = "0000000000000000000000000000000000000000000000000000000000000000".to_string();

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
            expected_checksum: Some(wrong_checksum),
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id).await;
    assert_eq!(
        status.state, DownloadState::Failed,
        "expected Failed on checksum mismatch, got {:?}",
        status.state
    );
    let error_msg = status.error.unwrap_or_default();
    assert!(
        error_msg.contains("Checksum mismatch"),
        "error should contain 'Checksum mismatch', got: {error_msg}"
    );

    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(!dest_path.exists(), "destination file should not exist on checksum mismatch");

    let _ = manager.remove(&id).await;
    Ok(())
}

// ==========================================================================
// Multi-stream (range-based) download tests (using /file/range)
// ==========================================================================

#[tokio::test]
#[timeout(30_000)]
async fn multi_stream_download_completes_successfully() -> TestResult {
    let server = TestServer::new(128 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_range(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(4),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id).await;
    assert_eq!(
        status.state, DownloadState::Completed,
        "expected Completed for multi-stream download, got {:?} with error={:?}",
        status.state, status.error
    );
    assert_eq!(status.total_bytes, Some(server.file_size));
    assert_eq!(status.downloaded_bytes, server.file_size);

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded.len() as u64, server.file_size);

    let expected = generate_test_content(server.file_size);
    assert_eq!(downloaded, expected, "multi-stream downloaded content mismatch");

    let _ = manager.remove(&id).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn multi_stream_blake3_checksum_match() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_range(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(4),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
            expected_checksum: Some(server.blake3_hash.clone()),
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id).await;
    assert_eq!(status.state, DownloadState::Completed,
        "multi-stream Blake3 checksum should match, got {:?}",
        status.state);

    let _ = manager.remove(&id).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn multi_stream_sha256_checksum_match() -> TestResult {
    let server = TestServer::new(32 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_range(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(4),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Sha256),
            expected_checksum: Some(server.sha256_hash.clone()),
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id).await;
    assert_eq!(status.state, DownloadState::Completed,
        "multi-stream SHA-256 checksum should match, got {:?}",
        status.state);

    let _ = manager.remove(&id).await;
    Ok(())
}

// ==========================================================================
// Cancellation test
// ==========================================================================

#[tokio::test]
#[timeout(60_000)]
async fn cancel_download_while_in_progress() -> TestResult {
    let server = TestServer::new(256 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let slow_url = server.file_url_bandwidth(8 * 1024);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: slow_url,
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    sleep(Duration::from_millis(800)).await;

    // cancel() returns the final snapshot and removes the download from the manager
    let status = manager.cancel(&id).await?;
    assert_eq!(status.state, DownloadState::Canceled,
        "expected Canceled, got {:?}", status.state);

    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(!dest_path.exists(), "destination file should not exist after cancel");

    // The download is already removed by cancel(), so remove() would fail — skip it.
    Ok(())
}
