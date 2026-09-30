//! GitHub asset mirrors, anti-hotlink and anti-abuse responses.

use super::*;

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
            let start = pieces
                .next()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let end = pieces
                .next()
                .and_then(|v| {
                    if v.is_empty() {
                        None
                    } else {
                        v.parse::<usize>().ok()
                    }
                })
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
    assert!(
        seen <= 3,
        "no Referer fan-out expected, saw {seen} requests"
    );

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
