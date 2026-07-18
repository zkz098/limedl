use std::{collections::HashMap, sync::Arc};

use crate::download::event_bus::EventBus;
use crate::download::types::IoBaselineSettings;
use axum::{
    Router,
    extract::{OriginalUri, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
};
use ntest::timeout;
use tempfile::tempdir;

use super::super::settings::load_settings;
use super::super::types::{
    AdaptiveProfile, Aria2RpcSettings, AutomaticSchedulerSettings, BtSettings,
    CdnAccelerationSettings, DownloadDefaultsSettings, GitHubMirrorSettings, LogSettings,
    NotificationSettings, ProxyMode, ProxySettings, SchedulerMode, SchedulerSettings,
    TraditionalSchedulerSettings,
};
use super::*;

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
            eprintln!("[flareget:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager =
        DownloadManager::new(
            temp.path().join("state"),
            Arc::new(RateLimiter::default()),
            Arc::new(EventBus::new(256)),
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

    let initial = manager.status(&id).await?;
    assert_eq!(initial.file_name, "slow.bin");
    assert!(matches!(
        initial.state,
        DownloadState::Queued | DownloadState::Downloading
    ));

    for _ in 0..30 {
        let status = manager.status(&id).await?;
        if status.file_name == "server-name.bin" {
            let _ = manager.remove(&id).await;
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }

    let status = manager.status(&id).await?;
    assert_eq!(status.file_name, "server-name.bin");
    let _ = manager.remove(&id).await;
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
            eprintln!("[flareget:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager =
        DownloadManager::new(
            temp.path().join("state"),
            Arc::new(RateLimiter::default()),
            Arc::new(EventBus::new(256)),
        )?;
    manager
        .update_settings(AppSettings {
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
    let first_status = manager.status(&first).await?;
    let second_status = manager.status(&second).await?;

    assert!(matches!(
        first_status.state,
        DownloadState::Downloading | DownloadState::Retrying | DownloadState::Completed
    ));
    assert_eq!(second_status.state, DownloadState::Queued);
    let _ = manager.remove(&first).await;
    let _ = manager.remove(&second).await;
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
            eprintln!("[flareget:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager =
        DownloadManager::new(
            temp.path().join("state"),
            Arc::new(RateLimiter::default()),
            Arc::new(EventBus::new(256)),
        )?;
    manager
        .update_settings(AppSettings {
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
    let big_status = manager.status(&big).await?;
    let small_status = manager.status(&small).await?;

    assert!(big_status.connection_count >= small_status.connection_count);
    let _ = manager.remove(&big).await;
    let _ = manager.remove(&small).await;
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
            eprintln!("[flareget:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager =
        DownloadManager::new(
            temp.path().join("state"),
            Arc::new(RateLimiter::default()),
            Arc::new(EventBus::new(256)),
        )?;
    manager
        .update_settings(AppSettings {
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
    manager.update_adaptive_targets().await?;
    manager.rebalance_allocations().await?;
    let snapshot = manager.status(&id).await?;
    assert!(matches!(
        snapshot.desired_thread_count,
        Some(thread_count) if thread_count >= 3
    ));
    let _ = manager.remove(&id).await;
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
            eprintln!("[flareget:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(256)),
    )?;

    // Compute the expected good checksum for the payload
    let expected_good = super::super::checksum::hash_slices(
        ChecksumMode::Blake3,
        &[&payload],
    );

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
    let status = wait_for_terminal(&manager, &id).await;
    assert_eq!(status.state, DownloadState::Completed, "expected Completed with matching checksum, got {:?} error={:?}", status.state, status.error);

    let _ = manager.remove(&id).await;
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
            eprintln!("[flareget:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(256)),
    )?;

    // Wrong expected checksum — should cause mismatch
    let expected_bad = String::from("0000000000000000000000000000000000000000000000000000000000000000");

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
    let status = wait_for_terminal(&manager, &id).await;
    assert_eq!(status.state, DownloadState::Failed, "expected Failed on checksum mismatch, got {:?}", status.state);
    let error_msg = status.error.unwrap_or_default();
    assert!(error_msg.contains("Checksum mismatch"), "error should contain 'Checksum mismatch', got: {error_msg}");

    // Verify the temp file was NOT renamed to destination
    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(!dest_path.exists(), "destination file should not exist on checksum mismatch");

    let _ = manager.remove(&id).await;
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
