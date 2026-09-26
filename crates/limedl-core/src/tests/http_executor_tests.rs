use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::tempdir;
use tokio::time::sleep;

use crate::DownloadManager;
use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::test_harness::TestServer;
use crate::types::{AppSettings, ChecksumMode, DownloadState, SchedulerSettings, StartDownloadRequest, ThreadMode};

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
    use rand::Rng;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
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

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "expected Completed, got {:?} with error={:?}",
        status.state,
        status.error
    );
    assert_eq!(status.total_bytes, Some(server.file_size));
    assert_eq!(status.downloaded_bytes, server.file_size);

    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(
        dest_path.exists(),
        "destination file should exist at {}",
        status.destination_path
    );
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded.len() as u64, server.file_size);

    let expected = generate_test_content(server.file_size);
    assert_eq!(
        downloaded, expected,
        "downloaded file content does not match server data"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn single_stream_download_with_blake3_checksum_match() -> TestResult {
    let server = TestServer::new(32 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "expected Completed with matching Blake3 checksum, got {:?} error={:?}",
        status.state,
        status.error
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn single_stream_checksum_mismatch_fails() -> TestResult {
    let server = TestServer::new(16 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let wrong_checksum =
        "0000000000000000000000000000000000000000000000000000000000000000".to_string();

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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Failed,
        "expected Failed on checksum mismatch, got {:?}",
        status.state
    );
    let error_msg = status.error.unwrap_or_default();
    assert!(
        error_msg.contains("Checksum mismatch"),
        "error should contain 'Checksum mismatch', got: {error_msg}"
    );

    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(
        !dest_path.exists(),
        "destination file should not exist on checksum mismatch"
    );

    let _ = manager.remove(&id.to_string()).await;
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

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "expected Completed for multi-stream download, got {:?} with error={:?}",
        status.state,
        status.error
    );
    assert_eq!(status.total_bytes, Some(server.file_size));
    assert_eq!(status.downloaded_bytes, server.file_size);

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded.len() as u64, server.file_size);

    let expected = generate_test_content(server.file_size);
    assert_eq!(
        downloaded, expected,
        "multi-stream downloaded content mismatch"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn multi_stream_blake3_checksum_match() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "multi-stream Blake3 checksum should match, got {:?}",
        status.state
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn multi_stream_sha256_checksum_match() -> TestResult {
    let server = TestServer::new(32 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "multi-stream SHA-256 checksum should match, got {:?}",
        status.state
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

// ==========================================================================
// Edge-case tests
// ==========================================================================

// ── Redirect handling ──────────────────────────────────────────────────────

/// reqwest uses `Policy::limited(10)` by default so redirects are followed
/// transparently.  The downloader should complete successfully at the final
/// destination after following a 301 redirect.
#[tokio::test]
#[timeout(30_000)]
async fn http_301_redirect_follows_and_completes() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_redirect(301),
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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "301 redirect should lead to Completed, got {:?} with error={:?}",
        status.state,
        status.error
    );
    assert_eq!(status.total_bytes, Some(server.file_size));
    assert_eq!(status.downloaded_bytes, server.file_size);

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    let expected = generate_test_content(server.file_size);
    assert_eq!(downloaded, expected, "content after 301 redirect mismatch");

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

/// Same as above but with HTTP 302 redirect (Found).
#[tokio::test]
#[timeout(30_000)]
async fn http_302_redirect_follows_and_completes() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_redirect(302),
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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "302 redirect should lead to Completed, got {:?} with error={:?}",
        status.state,
        status.error
    );
    assert_eq!(status.total_bytes, Some(server.file_size));
    assert_eq!(status.downloaded_bytes, server.file_size);

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    let expected = generate_test_content(server.file_size);
    assert_eq!(downloaded, expected, "content after 302 redirect mismatch");

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

// ── HTTP 416 Range Not Satisfiable ────────────────────────────────────────

/// When the server returns 416 for range requests (but advertises
/// `Accept-Ranges: bytes`), the download should fail because chunk workers
/// cannot retrieve their segments.  The probe succeeds because it does not
/// send a Range header.
///
/// Uses a 16 MiB file to ensure `supports_parallelism` returns true
/// (requires at least `chunk_size * 2 = 8 MiB`).
///
/// TODO: Ideally the executor could fall back to single-stream download on
///       416, similar to how it falls back on 200 OK for range requests.
#[tokio::test]
#[timeout(30_000)]
async fn http_416_range_not_satisfiable_fails() -> TestResult {
    let server = TestServer::new(16 * 1024 * 1024); // 16 MiB — large enough for parallel
    let server = server.await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_range_416(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(4), // Enable multi-stream
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    // The probe (no Range header) succeeds.  Chunk workers get 416 and fail.
    assert_eq!(
        status.state,
        DownloadState::Failed,
        "expected Failed when server returns 416 for range requests, got {:?} with error={:?}",
        status.state,
        status.error
    );
    assert!(
        status
            .error
            .as_deref()
            .unwrap_or("")
            .contains("416"),
        "error should mention 416, got: {:?}",
        status.error
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

// ── Connection refused (fast failure) ─────────────────────────────────────

/// Starting a download to an unreachable local port should fail the download
/// with a connection error.  This verifies the error handling path in the
/// executor and retry logic.
#[tokio::test]
#[timeout(60_000)]
async fn connection_refused_fails_gracefully() -> TestResult {
    // Use an address:port that nothing is listening on.
    let bad_url = "http://127.0.0.1:18763/non-existent-file";

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: bad_url.to_string(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(0), // No retries → fast
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Failed,
        "expected Failed on connection refused, got {:?}",
        status.state
    );
    assert!(
        status.error.is_some(),
        "expected an error message on connection failure"
    );

    // The error may be a transport error (reqwest::Error) or wrapped as Internal.
    // reqwest wraps OS-specific errors generically, so check for common patterns.
    let err_msg = status.error.as_deref().unwrap_or("");
    assert!(
        err_msg.contains("error sending request")
            || err_msg.contains("Connection refused")
            || err_msg.contains("connection refused")
            || err_msg.contains("ECONNREFUSED")
            || err_msg.contains("10061"),
        "error should mention connection failure, got: {err_msg}"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

/// Starting a download to a closed local port (connection refused) should
/// fail gracefully and quickly.  This is deterministic — no network routing
/// ambiguity — unlike a non-routable TEST-NET address which may either
/// timeout or refuse depending on the local network stack.
#[tokio::test]
#[timeout(30_000)]
async fn unreachable_host_fails_gracefully() -> TestResult {
    // Use an address:port that nothing is listening on (connection refused,
    // fast failure).  Port 18763 is an arbitrary high ephemeral port.
    let bad_url = "http://127.0.0.1:18763/non-existent-file";

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: bad_url.to_string(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(0),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Failed,
        "expected Failed on unreachable host (connection refused), got {:?}",
        status.state
    );
    assert!(
        status.error.is_some(),
        "expected an error message on connection failure"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

// ── No Content-Length (chunked transfer encoding) ─────────────────────────

/// When the server does not advertise a Content-Length, the executor falls
/// back to single-stream mode and reads until the stream ends.  The download
/// should complete successfully with the full content.
#[tokio::test]
#[timeout(30_000)]
async fn no_content_length_chunked_download_completes() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_no_length(),
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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "no-length download should complete, got {:?} with error={:?}",
        status.state,
        status.error
    );
    // total_bytes is None because no Content-Length was advertised
    assert_eq!(status.total_bytes, None);
    // downloaded_bytes reflects actual bytes received
    assert_eq!(status.downloaded_bytes, server.file_size);

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded.len() as u64, server.file_size);

    let expected = generate_test_content(server.file_size);
    assert_eq!(downloaded, expected, "no-length download content mismatch");

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

// ── Content-Length mismatch ───────────────────────────────────────────────

/// When the server declares a Content-Length smaller than the actual body,
/// the executor trusts Content-Length and marks the download as Completed
/// with the truncated size.  No error is raised because the bytes received
/// match the declared Content-Length exactly.
///
/// This documents that the executor relies entirely on Content-Length and
/// does not cross-check against received bytes or detect truncation by the
/// server.
#[tokio::test]
#[timeout(30_000)]
async fn wrong_content_length_truncates_completed_download() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_wrong_length(),
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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    // Current behavior: executor trusts Content-Length, so it "completes"
    // even though the server truncated the response.  The file has one
    // fewer byte than the true server content.
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "wrong-length download completed (truncated), got {:?} with error={:?}",
        status.state,
        status.error
    );
    assert_eq!(status.total_bytes, Some(server.file_size - 1));
    assert_eq!(status.downloaded_bytes, server.file_size - 1);

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded.len() as u64, server.file_size - 1);

    // Content matches the first (file_size - 1) bytes of the expected data
    let expected = generate_test_content(server.file_size);
    assert_eq!(
        &downloaded[..],
        &expected[..server.file_size as usize - 1],
        "wrong-length file content should match truncated expected data"
    );
    assert_ne!(
        downloaded.len(),
        expected.len(),
        "downloaded file should be shorter than the true file content"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

// ── Tests / features not covered ──────────────────────────────────────────
//
// 1. gzip / deflate content-encoding
//    reqwest is built with `default-features = false` (gzip/brotli/deflate
//    features NOT enabled).  Responses with `Content-Encoding: gzip` would
//    NOT be decompressed transparently — the downloader would save the raw
//    compressed bytes to disk.  A test for this requires either enabling
//    the `gzip` feature on reqwest or manually decompressing.
//    TODO: Enable reqwest gzip decompression in `configure_client_builder`
//          and add a gzip endpoint + test.
//
// 2. Connection / read timeout
//    The HTTP client has a 15-second read timeout.  Testing it requires a
//    delayed endpoint (>15 s), making the test too slow for routine runs.
//    TODO: Add a timeout test with a custom short-lived client if the
//    timeout configuration becomes user-adjustable.
//
// 3. Proxy support
//    The codebase has proxy support via `AppSettings.proxy` but testing
//    requires a mock proxy server (additional infrastructure).  This is
//    better tested at the `http_client_factory` or integration level.
//    TODO: Add proxy tests when a mock proxy fixture is available.

#[tokio::test]
#[timeout(60_000)]
async fn cancel_download_while_in_progress() -> TestResult {
    let server = TestServer::new(256 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

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
            headers: None,
            mirror_urls: None,
                    priority: None,
        })
        .await?;

    // Poll until the download is actually in progress before cancelling.
    loop {
        let s = manager.status(&id.to_string()).await?;
        if matches!(s.state, DownloadState::Downloading) {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }

    // cancel() returns the final snapshot and removes the download from the manager
    let status = manager.cancel(&id.to_string()).await?;
    assert_eq!(
        status.state,
        DownloadState::Canceled,
        "expected Canceled, got {:?}",
        status.state
    );

    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(
        !dest_path.exists(),
        "destination file should not exist after cancel"
    );

    // The download is already removed by cancel(), so remove() would fail — skip it.
    Ok(())
}

#[tokio::test]
#[timeout(5_000)]
async fn mark_chunk_released_respects_worker_id() -> TestResult {
    use crate::manager::{DownloadCore, ManagedDownload};
    use crate::manifest::{ChunkManifest, Manifest};
    use crate::types::{DownloadSnapshot, Priority, TaskKind};
    use crate::aimd::AimdState;
    use parking_lot::Mutex as ParkingMutex;
    use tokio::sync::Notify;

    // Create a minimal managed download with one chunk that is claimed by worker 42
    let managed = Arc::new(ManagedDownload {
        core: ParkingMutex::new(DownloadCore {
            snapshot: DownloadSnapshot {
                id: "test-wrkr".to_string(),
                kind: TaskKind::Http,
                state: DownloadState::Downloading,
                url: "http://example.com/file".to_string(),
                final_url: "http://example.com/file".to_string(),
                file_name: String::new(),
                destination_path: String::new(),
                temp_path: String::new(),
                total_bytes: Some(8_388_608),
                downloaded_bytes: 0,
                supports_ranges: true,
                connection_count: 0,
                thread_mode: ThreadMode::Fixed,
                requested_thread_count: Some(1),
                desired_thread_count: Some(1),
                allocated_thread_count: None,
                adaptive_profile: None,
                thread_note: None,
                checksum: None,
                expected_checksum: None,
                checksum_mode: ChecksumMode::None,
                etag: None,
                last_modified: None,
                error: None,
                speed_bytes_per_second: None,
                eta_seconds: None,
                uploaded_bytes: None,
                upload_speed_bytes_per_second: None,
                peer_count: None,
                upload_status: None,
                info_hash: None,
                created_at_ms: 0,
                updated_at_ms: 0,
                cdn_accelerated: false,
                cdn_node_ip: None,
                chunks: vec![],
                seed_count: None,
                leech_count: None,
                download_limit_bps: None,
                upload_limit_bps: None,
                mirror_url: None,
                priority: Priority::Normal,
                degraded: false,
                disk_type: None,
                flushing: false,
            },
            manifest: Manifest {
                id: "test-wrkr".to_string(),
                url: "http://example.com/file".to_string(),
                final_url: "http://example.com/file".to_string(),
                user_agent: "test".into(),
                extra_headers: vec![],
                destination_dir: String::new(),
                file_name: String::new(),
                file_name_locked: false,
                destination_path: String::new(),
                temp_path: String::new(),
                total_bytes: Some(8_388_608),
                downloaded_bytes: 0,
                supports_ranges: true,
                chunk_size: 4_194_304,
                connection_count: 0,
                thread_mode: ThreadMode::Fixed,
                requested_thread_count: Some(1),
                desired_thread_count: Some(1),
                allocated_thread_count: None,
                adaptive_profile_snapshot: None,
                thread_note: None,
                etag: None,
                last_modified: None,
                state: DownloadState::Downloading,
                cdn_accelerated: false,
                cdn_node_ip: None,
                priority: Priority::Normal,
                checksum_mode: ChecksumMode::None,
                checksum: None,
                expected_checksum: None,
                error: None,
                created_at_ms: 0,
                updated_at_ms: 0,
                mirror_url: None,
                mirror_urls: vec![],
                current_mirror_index: 0,
                chunks: vec![ChunkManifest {
                    index: 0,
                    start: 0,
                    end: 4_194_303,
                    downloaded: 0,
                    completed: false,
                    claimed_by: Some(42),
                    dirty: false,
                }],
            },
            speed_tracker: Default::default(),
        }),
        runtime: ParkingMutex::new(None),
        aimd: ParkingMutex::new(AimdState::default()),
        stop_notify: Notify::new(),
    });

    let chunk_index = 0usize;
    let original_worker = 42usize;
    let different_worker = 99usize;

    // Verify initial claim
    {
        let core = managed.core.lock();
        assert_eq!(core.manifest.chunks[0].claimed_by, Some(original_worker));
    }

    // Call with different worker_id — claim should NOT be cleared
    crate::http_executor::mark_chunk_released(&managed, chunk_index, different_worker);
    {
        let core = managed.core.lock();
        assert_eq!(
            core.manifest.chunks[0].claimed_by,
            Some(original_worker),
            "claim should not be cleared by a different worker"
        );
    }

    // Call with matching worker_id — claim should BE cleared
    crate::http_executor::mark_chunk_released(&managed, chunk_index, original_worker);
    {
        let core = managed.core.lock();
        assert_eq!(
            core.manifest.chunks[0].claimed_by,
            None,
            "claim should be cleared by the owning worker"
        );
    }

    Ok(())
}

#[tokio::test]
#[timeout(120_000)]
async fn tail_sprint_selective_release_completes() -> TestResult {
    use std::sync::Arc as StdArc;
    use axum::{
        Router,
        extract::{OriginalUri, State},
        http::{HeaderMap, HeaderValue, StatusCode, header},
        response::IntoResponse,
        routing::get,
    };

    // 16 MB file — 4 chunks of 4 MB, so workers spread across chunks
    let file_size: usize = 16 * 1024 * 1024;
    let file_bytes = StdArc::new(generate_test_content(file_size as u64));
    let etag = "\"tail-sprint-test\"";

    // Trailing zone: the last TAIL_ZONE bytes get a stall delay
    const TAIL_ZONE: usize = 2 * 1024 * 1024; // 2 MB
    const STALL_DELAY_MS: u64 = 12_000; // 12 seconds (> 8s stall window)

    #[derive(Clone)]
    struct TailServerState {
        bytes: StdArc<Vec<u8>>,
        etag: String,
    }

    async fn tail_file_head(
        State(state): State<TailServerState>,
        OriginalUri(_uri): OriginalUri,
    ) -> impl IntoResponse {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&state.bytes.len().to_string()).unwrap(),
        );
        headers.insert(header::ETAG, HeaderValue::from_str(&state.etag).unwrap());
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=tail-sprint.bin"),
        );
        (StatusCode::OK, headers).into_response()
    }

    async fn tail_file_get(
        State(state): State<TailServerState>,
        OriginalUri(_uri): OriginalUri,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::ETAG, HeaderValue::from_str(&state.etag).unwrap());
        response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        response_headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=tail-sprint.bin"),
        );

        let requested = headers
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok());

        let mut start: usize = 0;
        let mut end: usize = state.bytes.len() - 1;
        let mut is_range = false;

        if let Some(req) = requested
            && let Some(range) = req.strip_prefix("bytes=")
        {
            let mut pieces = range.split('-');
            if let Some(s) = pieces.next().and_then(|v| v.parse::<usize>().ok()) {
                start = s;
                is_range = true;
            }
            end = pieces
                .next()
                .and_then(|v| if v.is_empty() { None } else { v.parse::<usize>().ok() })
                .unwrap_or(state.bytes.len() - 1);
        }

        if start >= state.bytes.len() {
            return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        }
        end = end.min(state.bytes.len() - 1);

        // Apply stall delay if this request overlaps with the tail zone
        let file_end = state.bytes.len() - 1;
        let tail_start = file_end.saturating_sub(TAIL_ZONE - 1);
        if end >= tail_start {
            sleep(Duration::from_millis(STALL_DELAY_MS)).await;
        }

        let body = state.bytes[start..=end].to_vec();

        if is_range {
            let content_range = format!("bytes {start}-{end}/{}", state.bytes.len());
            response_headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&content_range).unwrap(),
            );
            response_headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&body.len().to_string()).unwrap(),
            );
            (StatusCode::PARTIAL_CONTENT, response_headers, body).into_response()
        } else {
            response_headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&state.bytes.len().to_string()).unwrap(),
            );
            (StatusCode::OK, response_headers, body).into_response()
        }
    }

    let state = TailServerState {
        bytes: file_bytes.clone(),
        etag: etag.to_string(),
    };

    let app = Router::new()
        .route("/tail.bin", get(tail_file_get).head(tail_file_head))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[limedl:test] tail-sprint server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = std::sync::Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        std::sync::Arc::new(RateLimiter::default()),
        std::sync::Arc::new(EventBus::new(1024)),
    )?);

    // Enable tail sprint, disable connection warmup
    manager
        .apply_settings(AppSettings {
            scheduler: SchedulerSettings {
                tail_sprint_enabled: true,
                connection_warmup_enabled: false,
                ..Default::default()
            },
            ..AppSettings::default()
        })
        .await?;

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/tail.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(4),
            max_retries: Some(3),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "tail sprint download should complete successfully"
    );
    assert_eq!(status.total_bytes, Some(file_size as u64));
    assert_eq!(status.downloaded_bytes, file_size as u64);

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded.len(), file_size);
    assert_eq!(&downloaded[..], &file_bytes[..], "downloaded content must match");

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

// ==========================================================================
// GitHub release-asset endpoint (self-update download path)
//
// Regression tests for the self-update signature failure:
// `https://api.github.com/.../releases/assets/<id>` only redirects to the real
// artifact when the request carries `Accept: application/octet-stream`; without
// it GitHub returns the asset's JSON metadata, which the updater would save as
// the "installer" and then reject at minisign verification.
// ==========================================================================

#[tokio::test]
#[timeout(30_000)]
async fn github_asset_with_accept_header_downloads_real_bytes() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_github_asset(),
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
            // This is the header the self-update flow must send; it mirrors the
            // upstream updater's `Update::download()` behavior.
            headers: Some(vec!["Accept: application/octet-stream".to_string()]),
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "with Accept: application/octet-stream the download must complete with the real artifact, error={:?}",
        status.error,
    );

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(
        downloaded.len() as u64,
        server.file_size,
        "must download the real binary, not the metadata JSON"
    );
    assert_eq!(
        &downloaded[..],
        &generate_test_content(server.file_size)[..],
        "downloaded bytes must match the real artifact content"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn github_asset_without_accept_header_gets_metadata_json() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_github_asset(),
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
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    // Even though the download "completes", the bytes are the asset metadata
    // JSON, not the installer — this is exactly why a signature check would fail.
    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_ne!(
        downloaded.len() as u64,
        server.file_size,
        "without the Accept header the file must NOT be the real artifact"
    );
    let text = String::from_utf8_lossy(&downloaded);
    assert!(
        text.contains("\"name\"") && text.contains("content_type"),
        "expected JSON metadata, got: {text}"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn anti_hotlink_403_auto_detects_candidate_referer_and_succeeds() -> TestResult {
    use axum::{
        Router,
        extract::State,
        http::{HeaderMap, HeaderValue, StatusCode, header},
        response::IntoResponse,
        routing::get,
    };
    use std::sync::Arc as StdArc;

    let file_size: usize = 1024 * 1024; // 1 MB
    let file_bytes = StdArc::new(generate_test_content(file_size as u64));

    #[derive(Clone)]
    struct HotlinkServerState {
        bytes: StdArc<Vec<u8>>,
    }

    async fn hotlink_head(
        State(state): State<HotlinkServerState>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        // Enforce anti-hotlinking: require Referer header
        if !headers.contains_key(header::REFERER) {
            return StatusCode::FORBIDDEN.into_response();
        }
        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        response_headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&state.bytes.len().to_string()).unwrap(),
        );
        response_headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=protected.bin"),
        );
        (StatusCode::OK, response_headers).into_response()
    }

    async fn hotlink_get(
        State(state): State<HotlinkServerState>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        // Enforce anti-hotlinking: require Referer header
        if !headers.contains_key(header::REFERER) {
            return StatusCode::FORBIDDEN.into_response();
        }
        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        response_headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=protected.bin"),
        );

        let requested = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
        if let Some(req) = requested
            && let Some(range) = req.strip_prefix("bytes=")
        {
            let mut pieces = range.split('-');
            let start = pieces.next().and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
            let end = pieces
                .next()
                .and_then(|v| if v.is_empty() { None } else { v.parse::<usize>().ok() })
                .unwrap_or(state.bytes.len() - 1)
                .min(state.bytes.len() - 1);

            let body = state.bytes[start..=end].to_vec();
            let content_range = format!("bytes {start}-{end}/{}", state.bytes.len());
            response_headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&content_range).unwrap(),
            );
            response_headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&body.len().to_string()).unwrap(),
            );
            (StatusCode::PARTIAL_CONTENT, response_headers, body).into_response()
        } else {
            response_headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&state.bytes.len().to_string()).unwrap(),
            );
            (StatusCode::OK, response_headers, state.bytes.to_vec()).into_response()
        }
    }

    let state = HotlinkServerState {
        bytes: file_bytes.clone(),
    };
    let app = Router::new()
        .route("/protected.bin", get(hotlink_get).head(hotlink_head))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = StdArc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        StdArc::new(RateLimiter::default()),
        StdArc::new(EventBus::new(1024)),
    )?);

    let download_url = format!("http://{address}/protected.bin");
    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: download_url,
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(2),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "download should complete despite 403 hotlink protection, error: {:?}",
        status.error
    );
    assert_eq!(status.downloaded_bytes, file_size as u64);

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded, *file_bytes);

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

/// TUNA-style anti-abuse 403: HEAD succeeds (the edge exempts it) but the real
/// GET is rejected with an HTML "uncommon characteristics" page. The download
/// must fail with an actionable hint and must NOT fan out Referer probes.
#[tokio::test]
#[timeout(30_000)]
async fn anti_abuse_403_surfaces_actionable_error_without_referer_probes() -> TestResult {
    use axum::{
        Router,
        extract::State,
        http::{HeaderMap, HeaderValue, StatusCode, header},
        response::IntoResponse,
        routing::get,
    };
    use std::sync::Arc as StdArc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ANTI_ABUSE_BODY: &str = "<html><body><h1>Sorry, you've been denied access to this page</h1>\
        <ul><li>The software that you are using is with uncommon characteristics;</li>\
        <li>您访问使用的软件带有非常用软件的特征；</li></ul></body></html>";

    #[derive(Clone)]
    struct AntiAbuseState {
        requests: StdArc<AtomicUsize>,
    }

    async fn abuse_head(State(state): State<AntiAbuseState>) -> impl IntoResponse {
        state.requests.fetch_add(1, Ordering::SeqCst);
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("1024"));
        headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=blocked.bin"),
        );
        (StatusCode::OK, headers).into_response()
    }

    async fn abuse_get(State(state): State<AntiAbuseState>) -> impl IntoResponse {
        state.requests.fetch_add(1, Ordering::SeqCst);
        (
            StatusCode::FORBIDDEN,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            ANTI_ABUSE_BODY,
        )
            .into_response()
    }

    let requests = StdArc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/blocked.bin", get(abuse_get).head(abuse_head))
        .with_state(AntiAbuseState {
            requests: requests.clone(),
        });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = StdArc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        StdArc::new(RateLimiter::default()),
        StdArc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/blocked.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(2),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(status.state, DownloadState::Failed);
    let error = status.error.unwrap_or_default();
    assert!(
        error.contains("anti-abuse"),
        "403 from an anti-abuse page must carry an actionable hint, got: {error}"
    );
    assert!(
        error.contains("User-Agent"),
        "hint should mention the User-Agent workaround, got: {error}"
    );
    // HEAD probe + one GET; candidate-Referer probing must be skipped.
    let seen = requests.load(Ordering::SeqCst);
    assert!(seen <= 3, "no Referer fan-out expected, saw {seen} requests");

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

/// A plain 403 (empty body, no WAF markers) must keep the generic message and
/// must not be misreported as an anti-abuse block.
#[tokio::test]
#[timeout(30_000)]
async fn plain_403_keeps_generic_error_message() -> TestResult {
    use axum::{
        Router,
        extract::State,
        http::{HeaderMap, HeaderValue, StatusCode, header},
        response::IntoResponse,
        routing::get,
    };
    use std::sync::Arc as StdArc;

    #[derive(Clone)]
    struct PlainForbiddenState;

    async fn plain_head() -> impl IntoResponse {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("1024"));
        headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=denied.bin"),
        );
        (StatusCode::OK, headers).into_response()
    }

    async fn plain_get(State(_state): State<PlainForbiddenState>) -> impl IntoResponse {
        StatusCode::FORBIDDEN.into_response()
    }

    let app = Router::new()
        .route("/denied.bin", get(plain_get).head(plain_head))
        .with_state(PlainForbiddenState);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = StdArc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        StdArc::new(RateLimiter::default()),
        StdArc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/denied.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(2),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(status.state, DownloadState::Failed);
    let error = status.error.unwrap_or_default();
    assert!(
        error.contains("403"),
        "plain 403 should keep the status message, got: {error}"
    );
    assert!(
        !error.contains("anti-abuse"),
        "plain 403 must not be misreported, got: {error}"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn too_many_requests_429_downgrades_to_single_thread_and_succeeds() -> TestResult {
    use axum::{
        Router,
        extract::State,
        http::{HeaderMap, HeaderValue, StatusCode, header},
        response::IntoResponse,
        routing::get,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc as StdArc;

    // 32 MB file (8 chunks of 4 MB, so chunk_count/2 = 4 workers)
    let file_size: usize = 32 * 1024 * 1024;
    let file_bytes = StdArc::new(generate_test_content(file_size as u64));

    struct ConnGuard(StdArc<AtomicUsize>);
    impl Drop for ConnGuard {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[derive(Clone)]
    struct RateLimitServerState {
        bytes: StdArc<Vec<u8>>,
        active_connections: StdArc<AtomicUsize>,
    }

    async fn rl_head(State(state): State<RateLimitServerState>) -> impl IntoResponse {
        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        response_headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&state.bytes.len().to_string()).unwrap(),
        );
        response_headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=ratelimit.bin"),
        );
        (StatusCode::OK, response_headers).into_response()
    }

    async fn rl_get(
        State(state): State<RateLimitServerState>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        let count = state.active_connections.fetch_add(1, Ordering::SeqCst);
        if count >= 1 {
            // More than 1 concurrent connection -> reject with 429
            state.active_connections.fetch_sub(1, Ordering::SeqCst);
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }

        let _guard = ConnGuard(state.active_connections.clone());

        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        response_headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=ratelimit.bin"),
        );

        let requested = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
        let (start, end, is_range) = if let Some(req) = requested
            && let Some(range) = req.strip_prefix("bytes=")
        {
            let mut pieces = range.split('-');
            let start = pieces.next().and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
            let end = pieces
                .next()
                .and_then(|v| if v.is_empty() { None } else { v.parse::<usize>().ok() })
                .unwrap_or(state.bytes.len() - 1)
                .min(state.bytes.len() - 1);
            (start, end, true)
        } else {
            (0, state.bytes.len() - 1, false)
        };

        // Artificial slight latency so multiple workers overlap if running concurrently
        tokio::time::sleep(Duration::from_millis(50)).await;

        let body = state.bytes[start..=end].to_vec();

        if is_range {
            let content_range = format!("bytes {start}-{end}/{}", state.bytes.len());
            response_headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&content_range).unwrap(),
            );
            response_headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&body.len().to_string()).unwrap(),
            );
            (StatusCode::PARTIAL_CONTENT, response_headers, body).into_response()
        } else {
            response_headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&state.bytes.len().to_string()).unwrap(),
            );
            (StatusCode::OK, response_headers, body).into_response()
        }
    }

    let active_connections = StdArc::new(AtomicUsize::new(0));
    let state = RateLimitServerState {
        bytes: file_bytes.clone(),
        active_connections,
    };
    let app = Router::new()
        .route("/ratelimit.bin", get(rl_get).head(rl_head))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = StdArc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        StdArc::new(RateLimiter::default()),
        StdArc::new(EventBus::new(1024)),
    )?);

    let download_url = format!("http://{address}/ratelimit.bin");
    // Request multi-threaded download (4 threads)
    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: download_url,
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(4),
            max_retries: Some(3),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "download must complete successfully after 429 downgrade, error: {:?}",
        status.error
    );
    assert_eq!(status.downloaded_bytes, file_size as u64);
    assert_eq!(
        status.thread_note.as_deref(),
        Some("单线程（429 限流降级）"),
        "thread_note should reflect 429 downgrade"
    );

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded, *file_bytes);

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}








