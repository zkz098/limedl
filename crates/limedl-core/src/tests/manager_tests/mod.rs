use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use crate::error::DownloadError;
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

use crate::aimd::AimdState;
use crate::download::{
    ChunkWorkerOutcome, DEFAULT_FIXED_THREADS, MAX_TRADITIONAL_THREADS, RunOutcome,
    cancellation_chunk_outcome, cancellation_outcome, record_progress_on_managed,
    resolve_thread_settings, supports_parallelism, thread_note,
};
use crate::download::{DownloadCore, ManagedDownload};
use crate::manifest::CHUNK_SIZE;
use crate::manifest::{ChunkManifest, Manifest};
use crate::types::TaskKind;
use crate::types::{
    AdaptiveProfile, AppSettings, Aria2RpcSettings, AutomaticSchedulerSettings, BtSettings,
    CdnAccelerationSettings, ChecksumMode, DownloadDefaultsSettings, DownloadSnapshot,
    DownloadState, LogSettings, NotificationSettings, Priority, ProxyMode, ProxySettings,
    SchedulerMode, SchedulerSettings, StartDownloadRequest, ThreadMode,
    TraditionalSchedulerSettings,
};
use crate::{DownloadManager, RateLimiter};
use parking_lot::Mutex as ParkingMutex;
use tokio::sync::Notify;

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

// ── supports_parallelism unit tests ─────────────────────────────────

// ── resolve_thread_settings unit tests ──────────────────────────────

// ── thread_note unit tests ──────────────────────────────────────────

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

// ── Helper to construct a ManagedDownload for state-guard tests ─────

fn make_managed(id: &str, state: DownloadState, url: &str) -> Arc<ManagedDownload> {
    Arc::new(ManagedDownload {
        core: ParkingMutex::new(DownloadCore {
            snapshot: DownloadSnapshot {
                id: id.to_string(),
                kind: TaskKind::Http,
                state,
                url: url.to_string(),
                final_url: url.to_string(),
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
                id: id.to_string(),
                url: url.to_string(),
                final_url: url.to_string(),
                user_agent: "test".into(),
                extra_headers: vec![],
                destination_dir: String::new(),
                file_name: String::new(),
                file_name_locked: false,
                destination_path: String::new(),
                temp_path: String::new(),
                total_bytes: None,
                downloaded_bytes: 0,
                supports_ranges: false,
                connection_count: 0,
                chunk_size: CHUNK_SIZE,
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
                chunks: vec![],
            },
            durable_bytes: 0,
            speed_tracker: Default::default(),
        }),
        runtime: ParkingMutex::new(None),
        aimd: ParkingMutex::new(AimdState::default()),
        stop_notify: Notify::new(),
    })
}

// ── start() validation ────────────────────────────────────────────────

// ── pause() state guards ─────────────────────────────────────────────

// ── cancel() state guards ────────────────────────────────────────────

// ── resume() state validation ────────────────────────────────────────

// ── get_summary() / find_active_by_url() ──────────────────────────────

// ── try_acquire_http() / try_acquire_bt() at capacity ─────────────────

// ── apply_settings() client rebuild path ────────────────────────────────

// ── game_mode() / overclock_mode() getters/setters ──────────────────────

// ── Helper for record_progress tests ──────────────────────────────────

/// Build a ManagedDownload with a single chunk at the given offset/size.
fn make_managed_with_chunk(
    id: &str,
    downloaded_bytes: u64,
    chunk_start: u64,
    chunk_end: u64,
    chunk_downloaded: u64,
    total: Option<u64>,
) -> Arc<ManagedDownload> {
    let m = make_managed(
        id,
        DownloadState::Downloading,
        "https://example.com/file.bin",
    );
    let mut core = m.core.lock();
    core.snapshot.downloaded_bytes = downloaded_bytes;
    core.snapshot.total_bytes = total;
    core.manifest.downloaded_bytes = downloaded_bytes;
    core.manifest.total_bytes = total;
    core.manifest.chunks = vec![ChunkManifest {
        index: 0,
        start: chunk_start,
        end: chunk_end,
        downloaded: chunk_downloaded,
        durable_downloaded: chunk_downloaded,
        completed: false,
        claimed_by: None,
        dirty: false,
    }];
    drop(core);
    m
}

// ── record_progress_on_managed ───────────────────────────────────────

// ── cancellation_outcome ─────────────────────────────────────────────

// ── cancellation_chunk_outcome ───────────────────────────────────────

mod checksum;
mod eviction;
mod lifecycle;
mod progress;
mod queries;
mod scheduling_e2e;
mod start_validation;
mod startup;
mod thread_resolution;
