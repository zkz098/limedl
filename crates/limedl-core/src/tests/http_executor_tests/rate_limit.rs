//! HTTP 429 downgrade behaviour.

use super::*;

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
    use std::sync::Arc as StdArc;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
