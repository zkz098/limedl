//! HTTP status and transport error handling.

use super::*;

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
        status.error.as_deref().unwrap_or("").contains("416"),
        "error should mention 416, got: {:?}",
        status.error
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

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

/// A download whose response body stream breaks mid-transfer (e.g. simulated
/// connection reset) must seamlessly recover by resuming from the durable
/// offset rather than failing the task with "error decoding response body".
#[tokio::test]
#[timeout(30_000)]
async fn stream_interruption_resumes_and_completes() -> TestResult {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use axum::{
        Router,
        body::Body,
        extract::State,
        http::{HeaderMap, Method, StatusCode, header},
        response::IntoResponse,
        routing::any,
    };
    use futures_util::stream;

    let test_data = generate_test_content(64 * 1024);
    let total_len = test_data.len();
    let data_arc = Arc::new(test_data);
    let attempts = Arc::new(AtomicUsize::new(0));

    let app_state = (data_arc.clone(), attempts.clone());
    let app = Router::new()
        .route(
            "/interrupted",
            any(
                move |method: Method,
                 State((data, attempts)): State<(Arc<Vec<u8>>, Arc<AtomicUsize>)>,
                 headers: HeaderMap| async move {
                    let mut resp_headers = HeaderMap::new();
                    resp_headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
                    resp_headers.insert(
                        header::CONTENT_DISPOSITION,
                        "attachment; filename=\"flaky.bin\"".parse().unwrap(),
                    );

                    if method == Method::HEAD {
                        resp_headers.insert(
                            header::CONTENT_LENGTH,
                            data.len().to_string().parse().unwrap(),
                        );
                        return (StatusCode::OK, resp_headers).into_response();
                    }

                    let count = attempts.fetch_add(1, Ordering::SeqCst);
                    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok());

                    if count == 0 {
                        // First GET attempt: send 16 KB then abort with a reset error
                        let chunk = bytes::Bytes::copy_from_slice(&data[..16 * 1024]);
                        let err = std::io::Error::new(
                            std::io::ErrorKind::ConnectionReset,
                            "simulated reset",
                        );
                        let s = stream::iter(vec![
                            Ok::<_, std::io::Error>(chunk),
                            Err(err),
                        ]);
                        resp_headers.insert(
                            header::CONTENT_LENGTH,
                            data.len().to_string().parse().unwrap(),
                        );
                        resp_headers.insert(
                            header::CONTENT_RANGE,
                            format!("bytes 0-{}/{total_len}", data.len() - 1)
                                .parse()
                                .unwrap(),
                        );
                        (StatusCode::PARTIAL_CONTENT, resp_headers, Body::from_stream(s))
                            .into_response()
                    } else {
                        // Resumed GET attempt: serve from requested Range
                        let (start, end) = if let Some(r) = range {
                            let r = r.strip_prefix("bytes=").unwrap();
                            let parts: Vec<&str> = r.split('-').collect();
                            let start: usize = parts[0].parse().unwrap();
                            let end: usize = if parts.len() > 1 && !parts[1].is_empty() {
                                parts[1].parse().unwrap()
                            } else {
                                data.len() - 1
                            };
                            (start, end)
                        } else {
                            (0, data.len() - 1)
                        };
                        let slice = bytes::Bytes::copy_from_slice(&data[start..=end]);
                        resp_headers.insert(
                            header::CONTENT_LENGTH,
                            (end - start + 1).to_string().parse().unwrap(),
                        );
                        resp_headers.insert(
                            header::CONTENT_RANGE,
                            format!("bytes {start}-{end}/{total_len}").parse().unwrap(),
                        );
                        (StatusCode::PARTIAL_CONTENT, resp_headers, Body::from(slice))
                            .into_response()
                    }
                },
            ),
        )
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

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
            url: format!("http://{addr}/interrupted"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
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
        "interrupted stream should resume and complete, got {:?} with error={:?}",
        status.state,
        status.error
    );
    assert_eq!(status.downloaded_bytes as usize, total_len);

    let dest_path = std::path::Path::new(&status.destination_path);
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded, *data_arc);

    let _ = manager.remove(&id.to_string()).await;
    server_handle.abort();
    Ok(())
}

