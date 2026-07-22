use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::tempdir;
use tokio::time::sleep;

use crate::DownloadManager;
use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::test_harness::TestServer;
use crate::types::{ChecksumMode, DownloadState, StartDownloadRequest, ThreadMode};

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
    use rand::RngCore;
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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

    let manager = Arc::new(DownloadManager::new(
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
            mirror_urls: None,
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
