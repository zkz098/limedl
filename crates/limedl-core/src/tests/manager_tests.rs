use std::{collections::HashMap, fs, sync::Arc, time::Duration};

use crate::event_bus::EventBus;
use crate::types::IoBaselineSettings;
use axum::{
    Router,
    extract::{OriginalUri, State},
    http::{self, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
};
use http::header;
use ntest::timeout;
use tempfile::tempdir;
use tokio::time::sleep;

use crate::settings::load_settings;
use crate::types::{
    AdaptiveProfile, AppSettings, Aria2RpcSettings, AutomaticSchedulerSettings, BtSettings,
    CdnAccelerationSettings, ChecksumMode, DownloadDefaultsSettings, DownloadSnapshot,
    DownloadState, GitHubMirrorSettings, LogSettings, NotificationSettings, ProxyMode,
    ProxySettings, SchedulerMode, SchedulerSettings, StartDownloadRequest, ThreadMode,
    TraditionalSchedulerSettings,
};
use crate::{DownloadManager, RateLimiter};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone)]
struct TestState {
    files: Arc<HashMap<String, TestFile>>,
    delay_ms: u64,
}

#[derive(Clone)]
struct TestFile {
    bytes: Arc<Vec<u8>>,
    etag: String,
}

fn single_file_state(path: &str, bytes: Arc<Vec<u8>>, etag: &str, delay_ms: u64) -> TestState {
    file_state([(path, bytes, etag)], delay_ms)
}

fn file_state<const N: usize>(files: [(&str, Arc<Vec<u8>>, &str); N], delay_ms: u64) -> TestState {
    TestState {
        files: Arc::new(
            files
                .into_iter()
                .map(|(path, bytes, etag)| {
                    (
                        path.to_string(),
                        TestFile {
                            bytes,
                            etag: etag.to_string(),
                        },
                    )
                })
                .collect(),
        ),
        delay_ms,
    }
}

#[tokio::test]
#[timeout(30_000)]
async fn loads_legacy_proxy_settings() -> TestResult {
    let temp = tempdir()?;
    let settings_path = temp.path().join("settings.json");
    fs::write(
        &settings_path,
        serde_json::to_vec_pretty(&ProxySettings {
            mode: ProxyMode::System,
            manual_url: String::new(),
        })?,
    )?;

    let settings = load_settings(&settings_path)?;
    assert_eq!(settings.proxy.mode, ProxyMode::System);
    assert_eq!(settings.scheduler.mode, SchedulerMode::Automatic);
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn start_returns_before_http_probe_finishes() -> TestResult {
    let payload = Arc::new(vec![19_u8; 1024 * 1024]);
    let state = single_file_state("/slow.bin", payload, "\"slow-start\"", 800);

    let app = Router::new()
        .route("/slow.bin", get(file_get).head(delayed_file_head))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[limedl:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;
    let id = tokio::time::timeout(
        Duration::from_millis(200),
        manager.start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/slow.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Adaptive),
            thread_count: None,
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        }),
    )
    .await??;

    let initial = manager.status(&id.to_string()).await?;
    assert_eq!(initial.file_name, "slow.bin");
    assert!(matches!(
        initial.state,
        DownloadState::Queued | DownloadState::Downloading
    ));

    for _ in 0..30 {
        let status = manager.status(&id.to_string()).await?;
        if status.file_name == "server-name.bin" {
            let _ = manager.remove(&id.to_string()).await;
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }

    let status = manager.status(&id.to_string()).await?;
    assert_eq!(status.file_name, "server-name.bin");
    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn traditional_mode_limits_running_tasks() -> TestResult {
    let payload = Arc::new(vec![42_u8; 12 * 1024 * 1024]);
    let state = single_file_state("/file.bin", payload, "\"test-etag\"", 180);

    let app = Router::new()
        .route("/file.bin", get(file_get).head(file_head))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[limedl:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;
    manager
        .apply_settings(AppSettings {
            appearance: Default::default(),
            proxy: ProxySettings::default(),
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Traditional,
                traditional: TraditionalSchedulerSettings {
                    max_parallel_tasks: 1,
                },
                automatic: AutomaticSchedulerSettings::default(),
                chunk_size_strategy: Default::default(),
            },
            download: DownloadDefaultsSettings::default(),
            bt: BtSettings::default(),

            logging: LogSettings::default(),
            aria2_rpc: Aria2RpcSettings::default(),
            cdn_acceleration: CdnAccelerationSettings::default(),
            github_mirror: GitHubMirrorSettings::default(),
            global_speed_limit_bps: 0,
            notifications: NotificationSettings::default(),
            io_baseline: IoBaselineSettings::default(),
            autostart: false,
            ..AppSettings::default()
        })
        .await?;

    let first = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/file.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("first.bin")),
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

    let second = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/file.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("second.bin")),
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

    sleep(Duration::from_millis(400)).await;
    let first_status = manager.status(&first.to_string()).await?;
    let second_status = manager.status(&second.to_string()).await?;

    assert!(matches!(
        first_status.state,
        DownloadState::Downloading | DownloadState::Retrying | DownloadState::Completed
    ));
    assert_eq!(second_status.state, DownloadState::Queued);
    let _ = manager.remove(&first.to_string()).await;
    let _ = manager.remove(&second.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn automatic_mode_prioritizes_larger_file() -> TestResult {
    let big_payload = Arc::new(vec![7_u8; 24 * 1024 * 1024]);
    let small_payload = Arc::new(vec![3_u8; 8 * 1024 * 1024]);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let app = Router::new()
        .route("/big.bin", get(file_get).head(file_head))
        .route("/small.bin", get(file_get).head(file_head))
        .with_state(file_state(
            [
                ("/big.bin", big_payload.clone(), "\"big\""),
                ("/small.bin", small_payload, "\"small\""),
            ],
            250,
        ));
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[limedl:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;
    manager
        .apply_settings(AppSettings {
            appearance: Default::default(),
            proxy: ProxySettings::default(),
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Automatic,
                traditional: TraditionalSchedulerSettings::default(),
                automatic: AutomaticSchedulerSettings {
                    max_parallel_threads: 3,
                    max_threads_per_task: 3,
                    min_threads_per_task: 0,
                    adaptive_profile: AdaptiveProfile::Balanced,
                },
                chunk_size_strategy: Default::default(),
            },
            download: DownloadDefaultsSettings::default(),
            bt: BtSettings::default(),

            logging: LogSettings::default(),
            aria2_rpc: Aria2RpcSettings::default(),
            cdn_acceleration: CdnAccelerationSettings::default(),
            github_mirror: GitHubMirrorSettings::default(),
            global_speed_limit_bps: 0,
            notifications: NotificationSettings::default(),
            io_baseline: IoBaselineSettings::default(),
            autostart: false,
            ..AppSettings::default()
        })
        .await?;

    let big = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/big.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("big.bin")),
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(3),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    let small = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/small.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("small.bin")),
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(3),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    sleep(Duration::from_millis(500)).await;
    let big_status = manager.status(&big.to_string()).await?;
    let small_status = manager.status(&small.to_string()).await?;

    assert!(big_status.connection_count >= small_status.connection_count);
    let _ = manager.remove(&big.to_string()).await;
    let _ = manager.remove(&small.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn adaptive_mode_increases_threads_on_stable_transfer() -> TestResult {
    let payload = Arc::new(vec![11_u8; 96 * 1024 * 1024]);
    let state = single_file_state("/file.bin", payload, "\"aimd\"", 500);

    let app = Router::new()
        .route("/file.bin", get(file_get).head(file_head))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[limedl:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;
    manager
        .apply_settings(AppSettings {
            appearance: Default::default(),
            proxy: ProxySettings::default(),
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Automatic,
                traditional: TraditionalSchedulerSettings::default(),
                automatic: AutomaticSchedulerSettings {
                    max_parallel_threads: 4,
                    max_threads_per_task: 4,
                    min_threads_per_task: 0,
                    adaptive_profile: AdaptiveProfile::Balanced,
                },
                chunk_size_strategy: Default::default(),
            },
            download: DownloadDefaultsSettings::default(),
            bt: BtSettings::default(),

            logging: LogSettings::default(),
            aria2_rpc: Aria2RpcSettings::default(),
            cdn_acceleration: CdnAccelerationSettings::default(),
            github_mirror: GitHubMirrorSettings::default(),
            global_speed_limit_bps: 0,
            notifications: NotificationSettings::default(),
            io_baseline: IoBaselineSettings::default(),
            autostart: false,
            ..AppSettings::default()
        })
        .await?;

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/file.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("aimd.bin")),
            user_agent: None,
            thread_mode: Some(ThreadMode::Adaptive),
            thread_count: None,
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    sleep(Duration::from_secs(2)).await;
    manager.scheduler.update_adaptive_targets(&manager).await?;
    manager.scheduler.rebalance_allocations(&manager).await?;
    let snapshot = manager.status(&id.to_string()).await?;
    assert!(matches!(
        snapshot.desired_thread_count,
        Some(thread_count) if thread_count >= 3
    ));
    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn checksum_match_succeeds() -> TestResult {
    let payload = Arc::new(vec![42_u8; 64 * 1024]);
    let state = single_file_state("/file.bin", payload.clone(), "\"chk-good\"", 50);

    let app = Router::new()
        .route("/file.bin", get(file_get).head(file_head))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[limedl:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    // Compute the expected good checksum for the payload
    let expected_good = crate::checksum::hash_slices(ChecksumMode::Blake3, &[&payload]);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/file.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("chk-good.bin")),
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
            expected_checksum: Some(expected_good),
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    // Wait for terminal state
    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "expected Completed with matching checksum, got {:?} error={:?}",
        status.state,
        status.error
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn checksum_mismatch_detected() -> TestResult {
    let payload = Arc::new(vec![42_u8; 64 * 1024]);
    let state = single_file_state("/file.bin", payload, "\"chk-bad\"", 50);

    let app = Router::new()
        .route("/file.bin", get(file_get).head(file_head))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[limedl:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    // Wrong expected checksum — should cause mismatch
    let expected_bad =
        String::from("0000000000000000000000000000000000000000000000000000000000000000");

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/file.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("chk-bad.bin")),
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
            expected_checksum: Some(expected_bad),
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    // Wait for terminal state
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

    // Verify the temp file was NOT renamed to destination
    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(
        !dest_path.exists(),
        "destination file should not exist on checksum mismatch"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

/// Poll until the download reaches a terminal state (Completed, Failed, or Canceled).
async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> DownloadSnapshot {
    loop {
        let status = manager.status(id).await.unwrap();
        if matches!(
            status.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return status;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn file_head(
    State(state): State<TestState>,
    OriginalUri(uri): OriginalUri,
) -> impl IntoResponse {
    build_file_head_response(state, uri).await
}

async fn delayed_file_head(
    State(state): State<TestState>,
    OriginalUri(uri): OriginalUri,
) -> impl IntoResponse {
    sleep(Duration::from_millis(state.delay_ms)).await;
    build_file_head_response(state, uri).await
}

async fn build_file_head_response(state: TestState, uri: axum::http::Uri) -> impl IntoResponse {
    let Some(file) = state.files.get(uri.path()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    let Ok(content_length) = HeaderValue::from_str(&file.bytes.len().to_string()) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    headers.insert(header::CONTENT_LENGTH, content_length);
    let Ok(etag) = HeaderValue::from_str(&file.etag) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    headers.insert(header::ETAG, etag);
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''server-name.bin"),
    );
    (StatusCode::OK, headers).into_response()
}

async fn file_get(
    State(state): State<TestState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> impl IntoResponse {
    sleep(Duration::from_millis(state.delay_ms)).await;
    let Some(file) = state.files.get(uri.path()) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let mut response_headers = HeaderMap::new();
    let Ok(etag) = HeaderValue::from_str(&file.etag) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    response_headers.insert(header::ETAG, etag);
    response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response_headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''server-name.bin"),
    );

    let requested = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    if let Some(requested) = requested {
        let Some(range) = requested.strip_prefix("bytes=") else {
            return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        };
        let mut pieces = range.split('-');
        let Some(start_text) = pieces.next() else {
            return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        };
        let Ok(start) = start_text.parse::<usize>() else {
            return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        };
        let end = pieces
            .next()
            .and_then(|value| {
                if value.is_empty() {
                    None
                } else {
                    value.parse::<usize>().ok()
                }
            })
            .unwrap_or(file.bytes.len() - 1);
        if start >= file.bytes.len() {
            return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        }
        let end = end.min(file.bytes.len() - 1);
        let body = file.bytes[start..=end].to_vec();
        let Ok(content_range) =
            HeaderValue::from_str(&format!("bytes {start}-{end}/{}", file.bytes.len()))
        else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        response_headers.insert(header::CONTENT_RANGE, content_range);
        let Ok(content_length) = HeaderValue::from_str(&body.len().to_string()) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        response_headers.insert(header::CONTENT_LENGTH, content_length);
        return (StatusCode::PARTIAL_CONTENT, response_headers, body).into_response();
    }

    let Ok(content_length) = HeaderValue::from_str(&file.bytes.len().to_string()) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    response_headers.insert(header::CONTENT_LENGTH, content_length);
    (
        StatusCode::OK,
        response_headers,
        file.bytes.as_ref().clone(),
    )
        .into_response()
}

#[tokio::test]
#[timeout(10000)]
async fn evict_completed_removes_oldest_terminal_entries() -> TestResult {
    use crate::aimd::AimdState;
    use crate::manager::{DownloadCore, ManagedDownload};
    use crate::manifest::Manifest;
    use crate::types::TaskKind;
    use parking_lot::Mutex as ParkingMutex;
    use tokio::sync::Notify;

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    // Bypass the settings clamp ([10, 10000]) so we can use a small limit.
    manager.settings.write().await.max_in_memory_downloads = 2;

    let make_dl = |id: &str, state: DownloadState, created_at: u64| -> Arc<ManagedDownload> {
        Arc::new(ManagedDownload {
            core: ParkingMutex::new(DownloadCore {
                snapshot: DownloadSnapshot {
                    id: id.to_string(),
                    kind: TaskKind::Http,
                    state,
                    url: String::new(),
                    final_url: String::new(),
                    file_name: String::new(),
                    destination_path: String::new(),
                    temp_path: String::new(),
                    total_bytes: None,
                    downloaded_bytes: 0,
                    supports_ranges: false,
                    connection_count: 0,
                    thread_mode: ThreadMode::Adaptive,
                    requested_thread_count: None,
                    desired_thread_count: None,
                    allocated_thread_count: None,
                    adaptive_profile: None,
                    thread_note: None,
                    checksum: None,
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
                    created_at_ms: created_at,
                    updated_at_ms: 0,
                    cdn_accelerated: false,
                    chunks: vec![],
                    seed_count: None,
                    leech_count: None,
                    download_limit_bps: None,
                    upload_limit_bps: None,
                    mirror_url: None,
                    degraded: false,
                    disk_type: None,
                    flushing: false,
                },
                manifest: Manifest {
                    id: id.to_string(),
                    url: String::new(),
                    final_url: String::new(),
                    user_agent: "test".into(),
                    destination_dir: String::new(),
                    file_name: String::new(),
                    file_name_locked: false,
                    destination_path: String::new(),
                    temp_path: String::new(),
                    total_bytes: None,
                    downloaded_bytes: 0,
                    supports_ranges: false,
                    chunk_size: 4_194_304,
                    connection_count: 0,
                    thread_mode: ThreadMode::Adaptive,
                    requested_thread_count: None,
                    desired_thread_count: None,
                    allocated_thread_count: None,
                    adaptive_profile_snapshot: None,
                    thread_note: None,
                    etag: None,
                    last_modified: None,
                    state,
                    cdn_accelerated: false,
                    checksum_mode: ChecksumMode::None,
                    checksum: None,
                    expected_checksum: None,
                    error: None,
                    created_at_ms: created_at,
                    updated_at_ms: 0,
                    mirror_url: None,
                    mirror_urls: vec![],
                    current_mirror_index: 0,
                    chunks: vec![],
                },
            }),
            runtime: ParkingMutex::new(None),
            aimd: ParkingMutex::new(AimdState::default()),
            stop_notify: Notify::new(),
        })
    };

    // Insert 4 entries: 2 terminal (oldest first), 1 active, 1 terminal.
    {
        let mut map = manager.downloads.write().await;
        map.insert(
            "completed-old".into(),
            make_dl("completed-old", DownloadState::Completed, 100),
        );
        map.insert(
            "downloading".into(),
            make_dl("downloading", DownloadState::Downloading, 200),
        );
        map.insert(
            "completed-new".into(),
            make_dl("completed-new", DownloadState::Completed, 300),
        );
        map.insert(
            "failed".into(),
            make_dl("failed", DownloadState::Failed, 400),
        );
    }

    assert_eq!(manager.downloads.read().await.len(), 4);

    let evicted = manager.task_lifecycle.evict_completed(&manager).await;
    // limit=2, excess=2, terminal=[completed-old, completed-new, failed]
    // Should evict the 2 oldest terminal entries: completed-old and completed-new
    assert_eq!(evicted, 2, "should have evicted 2 terminal entries");

    let remaining = manager.downloads.read().await;
    assert_eq!(remaining.len(), 2, "should have 2 entries remaining");
    assert!(
        remaining.contains_key("downloading"),
        "active download must remain"
    );
    assert!(
        remaining.contains_key("failed"),
        "newest terminal entry must remain"
    );
    assert!(
        !remaining.contains_key("completed-old"),
        "oldest terminal entry must be evicted"
    );
    assert!(
        !remaining.contains_key("completed-new"),
        "second-oldest terminal entry must be evicted"
    );

    Ok(())
}
