//! Chunk claim/release bookkeeping and cancellation.

use super::*;

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
    use crate::aimd::AimdState;
    use crate::download::{DownloadCore, ManagedDownload};
    use crate::manifest::{ChunkManifest, Manifest};
    use crate::types::{DownloadSnapshot, Priority, TaskKind};
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
                    durable_downloaded: 0,
                    completed: false,
                    claimed_by: Some(42),
                    dirty: false,
                }],
            },
            durable_bytes: 0,
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
            core.manifest.chunks[0].claimed_by, None,
            "claim should be cleared by the owning worker"
        );
    }

    Ok(())
}

#[tokio::test]
#[timeout(120_000)]
async fn tail_sprint_selective_release_completes() -> TestResult {
    use axum::{
        Router,
        extract::{OriginalUri, State},
        http::{HeaderMap, HeaderValue, StatusCode, header},
        response::IntoResponse,
        routing::get,
    };
    use std::sync::Arc as StdArc;

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

        let requested = headers.get(header::RANGE).and_then(|v| v.to_str().ok());

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
                .and_then(|v| {
                    if v.is_empty() {
                        None
                    } else {
                        v.parse::<usize>().ok()
                    }
                })
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
    assert_eq!(
        &downloaded[..],
        &file_bytes[..],
        "downloaded content must match"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}
