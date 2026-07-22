use std::{
    collections::HashMap,
    fs,
    sync::{
        Arc,
        atomic::{Ordering},
    },
    time::Duration,
};

use crate::error::DownloadError;
use crate::event_bus::EventBus;
use crate::types::IoBaselineSettings;use axum::{
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
use crate::manager::{
    resolve_thread_settings, supports_parallelism, thread_note, cancellation_outcome,
    cancellation_chunk_outcome, record_progress_on_managed, ChunkWorkerOutcome, RunOutcome,
    DEFAULT_FIXED_THREADS, MAX_TRADITIONAL_THREADS,
};
use crate::manifest::CHUNK_SIZE;
use crate::manifest::{ChunkManifest, Manifest};
use crate::manager::{DownloadCore, ManagedDownload};
use crate::aimd::AimdState;
use crate::types::TaskKind;
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
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);
    let id = tokio::time::timeout(
        Duration::from_millis(5_000),
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
// ── supports_parallelism unit tests ─────────────────────────────────

#[test]
fn supports_parallelism_small_file_no_ranges() {
    assert!(!supports_parallelism(Some(CHUNK_SIZE), false, CHUNK_SIZE));
}

#[test]
fn supports_parallelism_small_file_with_ranges() {
    assert!(!supports_parallelism(Some(CHUNK_SIZE), true, CHUNK_SIZE));
}

#[test]
fn supports_parallelism_large_file_with_ranges() {
    assert!(supports_parallelism(Some(CHUNK_SIZE * 2), true, CHUNK_SIZE));
}

#[test]
fn supports_parallelism_large_file_no_ranges() {
    assert!(!supports_parallelism(Some(CHUNK_SIZE * 2), false, CHUNK_SIZE));
}

#[test]
fn supports_parallelism_unknown_size() {
    assert!(!supports_parallelism(None, false, CHUNK_SIZE));
    assert!(!supports_parallelism(None, true, CHUNK_SIZE));
}

#[test]
fn supports_parallelism_zero_bytes() {
    assert!(!supports_parallelism(Some(0), false, CHUNK_SIZE));
    assert!(!supports_parallelism(Some(0), true, CHUNK_SIZE));
}

// ── resolve_thread_settings unit tests ──────────────────────────────

#[test]
fn resolve_traditional_adaptive_mode_default_threads() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Traditional,
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        thread_count: None,
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Fixed);
    assert_eq!(requested, Some(DEFAULT_FIXED_THREADS));
    assert_eq!(desired, Some(DEFAULT_FIXED_THREADS));
    assert_eq!(profile, None);
}

#[test]
fn resolve_traditional_custom_thread_count() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Traditional,
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_count: Some(16),
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Fixed);
    assert_eq!(requested, Some(16));
    assert_eq!(desired, Some(16));
    assert_eq!(profile, None);
}

#[test]
fn resolve_traditional_clamped_to_max() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Traditional,
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_count: Some(100),
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Fixed);
    assert_eq!(requested, Some(MAX_TRADITIONAL_THREADS));
    assert_eq!(desired, Some(MAX_TRADITIONAL_THREADS));
    assert_eq!(profile, None);
}

#[test]
fn resolve_automatic_adaptive_balanced() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                adaptive_profile: AdaptiveProfile::Balanced,
                max_threads_per_task: 8,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        thread_count: None,
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Adaptive);
    assert_eq!(requested, None);
    // Balanced → initial_desired_threads=2, capped at max_threads_per_task=8
    assert_eq!(desired, Some(2));
    assert_eq!(profile, Some(AdaptiveProfile::Balanced));
}

#[test]
fn resolve_automatic_adaptive_conservative() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                adaptive_profile: AdaptiveProfile::Conservative,
                max_threads_per_task: 8,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        ..Default::default()
    };
    let (mode, _, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Adaptive);
    // Conservative → initial_desired_threads=1
    assert_eq!(desired, Some(1));
    assert_eq!(profile, Some(AdaptiveProfile::Conservative));
}

#[test]
fn resolve_automatic_adaptive_aggressive() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                adaptive_profile: AdaptiveProfile::Aggressive,
                max_threads_per_task: 8,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        ..Default::default()
    };
    let (mode, _, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Adaptive);
    // Aggressive → initial_desired_threads=4
    assert_eq!(desired, Some(4));
    assert_eq!(profile, Some(AdaptiveProfile::Aggressive));
}

#[test]
fn resolve_automatic_adaptive_capped_by_max_threads() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                adaptive_profile: AdaptiveProfile::Aggressive,
                max_threads_per_task: 2, // cap below initial_desired_threads=4
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        ..Default::default()
    };
    let (mode, _, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Adaptive);
    // Aggressive→4, capped at 2, then max(1) = 2
    assert_eq!(desired, Some(2));
    assert_eq!(profile, Some(AdaptiveProfile::Aggressive));
}

#[test]
fn resolve_automatic_adaptive_zero_max_threads_clamped() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                adaptive_profile: AdaptiveProfile::Balanced,
                max_threads_per_task: 0, // .max(1) ensures at least 1
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        ..Default::default()
    };
    let (mode, _, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Adaptive);
    // Balanced→2, capped at max(0,1)=1
    assert_eq!(desired, Some(1));
    assert_eq!(profile, Some(AdaptiveProfile::Balanced));
}

#[test]
fn resolve_automatic_fixed_default_threads() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_threads_per_task: 6,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Fixed),
        thread_count: None, // defaults to DEFAULT_FIXED_THREADS=8
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Fixed);
    // 8 clamped to max_threads_per_task=6
    assert_eq!(requested, Some(6));
    assert_eq!(desired, Some(6));
    assert_eq!(profile, None);
}

#[test]
fn resolve_automatic_fixed_explicit_thread_count() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_threads_per_task: 10,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Fixed),
        thread_count: Some(4),
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Fixed);
    assert_eq!(requested, Some(4));
    assert_eq!(desired, Some(4));
    assert_eq!(profile, None);
}

#[test]
fn resolve_no_parallelism_returns_fixed_single() {
    let settings = AppSettings::default();
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        thread_count: Some(16), // should be ignored
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, false);
    assert_eq!(mode, ThreadMode::Fixed);
    assert_eq!(requested, Some(1));
    assert_eq!(desired, Some(1));
    assert_eq!(profile, None);
}

// ── thread_note unit tests ──────────────────────────────────────────

#[test]
fn thread_note_no_parallelism() {
    let note = thread_note(false, ThreadMode::Fixed, None);
    assert_eq!(note, Some(String::from("单线程（服务器不支持分段）")));
}

#[test]
fn thread_note_fixed() {
    let note = thread_note(true, ThreadMode::Fixed, None);
    assert_eq!(note, Some(String::from("固定线程")));
}

#[test]
fn thread_note_adaptive_conservative() {
    let note = thread_note(true, ThreadMode::Adaptive, Some(AdaptiveProfile::Conservative));
    assert_eq!(note, Some(String::from("自适应 / 保守")));
}

#[test]
fn thread_note_adaptive_balanced() {
    let note = thread_note(true, ThreadMode::Adaptive, Some(AdaptiveProfile::Balanced));
    assert_eq!(note, Some(String::from("自适应 / 平衡")));
}

#[test]
fn thread_note_adaptive_aggressive() {
    let note = thread_note(true, ThreadMode::Adaptive, Some(AdaptiveProfile::Aggressive));
    assert_eq!(note, Some(String::from("自适应 / 激进")));
}

#[test]
fn thread_note_adaptive_no_profile() {
    let note = thread_note(true, ThreadMode::Adaptive, None);
    assert_eq!(note, None);
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
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);
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

    // Poll until at least one download has threads allocated —
    // on slow CI the 500ms sleep may not be sufficient and both
    // connection_count values could be 0 (vacuously passing).
    let (big_status, small_status) = loop {
        let big = manager.status(&big.to_string()).await?;
        let small = manager.status(&small.to_string()).await?;
        if big.connection_count > 0 || small.connection_count > 0 {
            break (big, small);
        }
        sleep(Duration::from_millis(100)).await;
    };

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
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);
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

    // Poll until AIMD has ramped up to 3+ desired threads.
    // On slow CI runners the fixed 2s sleep may not provide enough
    // progress data for the controller to decide to increase.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        manager.scheduler.update_adaptive_targets(&manager).await?;
        manager.scheduler.rebalance_allocations(&manager).await?;
        let snapshot = manager.status(&id.to_string()).await?;
        if snapshot.desired_thread_count.unwrap_or(0) >= 3 {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!(
                "AIMD never reached 3+ threads (current: {:?})",
                snapshot.desired_thread_count
            );
        }
        sleep(Duration::from_millis(500)).await;
    }
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
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

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
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

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
#[timeout(30_000)]
async fn evict_completed_removes_oldest_terminal_entries() -> TestResult {

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

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
                url: url.to_string(),
                final_url: url.to_string(),
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
                created_at_ms: 0,
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
}

// ── start() validation ────────────────────────────────────────────────

#[tokio::test]
#[timeout(30_000)]
async fn start_rejects_unsupported_scheme() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let result = manager
        .start(StartDownloadRequest {
            kind: None,
            url: "ftp://example.com/file.bin".into(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some("test.bin".into()),
            user_agent: None,
            thread_mode: None,
            thread_count: None,
            max_retries: None,
            checksum: None,
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await;

    assert!(matches!(result, Err(DownloadError::UnsupportedScheme)));
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn start_rejects_empty_destination_dir() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let result = manager
        .start(StartDownloadRequest {
            kind: None,
            url: "http://example.com/file.bin".into(),
            destination_dir: String::new(),
            file_name: Some("test.bin".into()),
            user_agent: None,
            thread_mode: None,
            thread_count: None,
            max_retries: None,
            checksum: None,
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await;

    assert!(matches!(result, Err(DownloadError::InvalidResponse(ref msg)) if msg.contains("not set")));
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn start_rejects_relative_destination_dir() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let result = manager
        .start(StartDownloadRequest {
            kind: None,
            url: "http://example.com/file.bin".into(),
            destination_dir: "relative/out".into(),
            file_name: Some("test.bin".into()),
            user_agent: None,
            thread_mode: None,
            thread_count: None,
            max_retries: None,
            checksum: None,
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await;

    assert!(matches!(result, Err(DownloadError::InvalidResponse(ref msg)) if msg.contains("absolute path")));
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn start_rejects_checksum_mode_mismatch() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let result = manager
        .start(StartDownloadRequest {
            kind: None,
            url: "http://example.com/file.bin".into(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some("test.bin".into()),
            user_agent: None,
            thread_mode: None,
            thread_count: None,
            max_retries: None,
            checksum: Some(ChecksumMode::None),
            expected_checksum: Some("abc123".into()),
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await;

    assert!(matches!(result, Err(DownloadError::InvalidRequest(ref msg)) if msg.contains("checksum_mode")));
    Ok(())
}

// ── pause() state guards ─────────────────────────────────────────────

#[tokio::test]
#[timeout(30_000)]
async fn pause_on_paused_task_returns_snapshot_unchanged() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "paused-task";
    let dl = make_managed(id, DownloadState::Paused, "http://example.com/file.bin");
    manager.downloads.write().await.insert(id.to_string(), dl.clone());

    let snapshot_before = manager.status(id).await?;
    let result = manager.pause(id).await?;
    // pause() on paused state returns the clone atomically, without modifying state
    assert_eq!(result.state, DownloadState::Paused);
    let snapshot_after = manager.status(id).await?;
    assert_eq!(snapshot_after.state, DownloadState::Paused);
    // Task still in the list (pause does not remove it)
    assert!(manager.downloads.read().await.contains_key(id));
    // Snapshot returned by pause() should match the snapshot from status()
    assert_eq!(result.state, snapshot_before.state);

    let _ = manager.remove(id).await;
    Ok(())
}

// ── cancel() state guards ────────────────────────────────────────────

#[tokio::test]
#[timeout(30_000)]
async fn cancel_on_completed_task_skips_file_cleanup_and_removes() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "completed-task";
    let dl = make_managed(id, DownloadState::Completed, "http://example.com/file.bin");
    manager.downloads.write().await.insert(id.to_string(), dl.clone());

    // cancel() on completed task skips the cancellation path, but still removes from list
    let snapshot = manager.cancel(id).await?;
    assert_eq!(snapshot.state, DownloadState::Completed);
    // Task should be removed from active list
    assert!(
        !manager.downloads.read().await.contains_key(id),
        "completed task should be removed from downloads after cancel"
    );

    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn cancel_on_already_canceled_task_still_removes() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "already-canceled";
    let dl = make_managed(id, DownloadState::Canceled, "http://example.com/file.bin");
    manager.downloads.write().await.insert(id.to_string(), dl.clone());

    let result = manager.cancel(id).await;
    assert!(result.is_ok(), "cancel on canceled task should return Ok: {:?}", result.err());
    // Task should be removed from active list
    assert!(
        !manager.downloads.read().await.contains_key(id),
        "canceled task should be removed from downloads after second cancel"
    );

    // Second cancel on already-removed task should return NotFound
    let second = manager.cancel(id).await;
    assert!(matches!(second, Err(DownloadError::NotFound)));

    Ok(())
}

// ── resume() state validation ────────────────────────────────────────

#[tokio::test]
#[timeout(30_000)]
async fn resume_canceled_task_returns_canceled_error() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "canceled-task";
    let dl = make_managed(id, DownloadState::Canceled, "http://example.com/file.bin");
    manager.downloads.write().await.insert(id.to_string(), dl.clone());

    let result = manager.resume(id).await;
    assert!(matches!(result, Err(DownloadError::Canceled)));

    let _ = manager.remove(id).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn resume_completed_task_returns_not_resumable_error() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "completed-task";
    let dl = make_managed(id, DownloadState::Completed, "http://example.com/file.bin");
    manager.downloads.write().await.insert(id.to_string(), dl.clone());

    let result = manager.resume(id).await;
    assert!(matches!(result, Err(DownloadError::NotResumable)));

    let _ = manager.remove(id).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn resume_running_task_returns_already_running_error() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "downloading-task";
    let dl = make_managed(id, DownloadState::Downloading, "http://example.com/file.bin");
    manager.downloads.write().await.insert(id.to_string(), dl.clone());

    let result = manager.resume(id).await;
    assert!(matches!(result, Err(DownloadError::AlreadyRunning)));

    let _ = manager.remove(id).await;
    Ok(())
}

// ── get_summary() / find_active_by_url() ──────────────────────────────

#[tokio::test]
#[timeout(30_000)]
async fn get_summary_nonexistent_id_returns_none() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let summary = manager.get_summary("nonexistent").await;
    assert!(summary.is_none());
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn find_active_by_url_no_match_returns_none() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let result = manager.find_active_by_url("http://example.com/nonexistent").await;
    assert!(result.is_none());
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn find_active_by_url_match_returns_some() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "active-dl";
    let url = "http://example.com/active.bin";
    let dl = make_managed(id, DownloadState::Downloading, url);
    manager.downloads.write().await.insert(id.to_string(), dl.clone());

    let found = manager.find_active_by_url(url).await;
    assert_eq!(found, Some(id.to_string()));

    let _ = manager.remove(id).await;
    Ok(())
}

// ── try_acquire_http() / try_acquire_bt() at capacity ─────────────────

#[tokio::test]
#[timeout(30_000)]
async fn try_acquire_http_at_capacity_returns_error() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // Set max to 0 to simulate at capacity
    manager.limits.max_concurrent_http.store(0, Ordering::Release);

    let result = manager.try_acquire_http();
    assert!(matches!(result, Err(DownloadError::TooManyConcurrentDownloads)));
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn try_acquire_bt_at_capacity_returns_error() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // Set max to 0 to simulate at capacity
    manager.limits.max_concurrent_bt.store(0, Ordering::Release);

    let result = manager.try_acquire_bt();
    assert!(matches!(result, Err(DownloadError::TooManyConcurrentDownloads)));
    Ok(())
}

// ── apply_settings() client rebuild path ────────────────────────────────

#[tokio::test]
#[timeout(30_000)]
async fn apply_settings_proxy_change_triggers_client_rebuild() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let mut settings = manager.settings().await?;
    // Switch proxy from default (Disabled) to System — triggers client rebuild
    settings.proxy.mode = ProxyMode::System;
    let result = manager.apply_settings(settings).await;
    assert!(result.is_ok(), "apply_settings with proxy change should succeed: {:?}", result.err());

    // Verify the proxy mode stuck
    let updated = manager.settings().await?;
    assert_eq!(updated.proxy.mode, ProxyMode::System);
    Ok(())
}

// ── game_mode() / overclock_mode() getters/setters ──────────────────────

#[tokio::test]
#[timeout(30_000)]
async fn game_mode_setter_and_getter() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    assert!(!manager.game_mode(), "game_mode should default to false");

    manager.set_game_mode(true);
    assert!(manager.game_mode(), "game_mode should be true after set");

    manager.set_game_mode(false);
    assert!(!manager.game_mode(), "game_mode should be false after unset");
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn overclock_mode_setter_and_getter() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    assert!(!manager.overclock_mode(), "overclock_mode should default to false");

    manager.set_overclock_mode(true);
    assert!(manager.overclock_mode(), "overclock_mode should be true after set");

    manager.set_overclock_mode(false);
    assert!(!manager.overclock_mode(), "overclock_mode should be false after unset");
    Ok(())
}

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
    let m = make_managed(id, DownloadState::Downloading, "https://example.com/file.bin");
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
        completed: false,
        claimed_by: None,
        dirty: false,
    }];
    drop(core);
    m
}

// ── record_progress_on_managed ───────────────────────────────────────

#[tokio::test]
#[timeout(30_000)]
async fn record_progress_normal_update() {
    let managed = make_managed_with_chunk("dl1", 0, 0, 4_194_304, 0, Some(8_388_608));

    record_progress_on_managed(&managed, Some(0), 1000);

    let core = managed.core.lock();
    assert_eq!(core.snapshot.downloaded_bytes, 1000);
    assert_eq!(core.manifest.downloaded_bytes, 1000);
    assert_eq!(core.manifest.chunks[0].downloaded, 1000);
    assert!(!core.manifest.chunks[0].completed);
    assert!(core.manifest.chunks[0].dirty);
    assert!(core.snapshot.updated_at_ms > 0);
}

#[tokio::test]
#[timeout(30_000)]
async fn record_progress_chunk_index_beyond_range() {
    let managed = make_managed_with_chunk("dl2", 500, 0, 4_194_304, 100, Some(8_388_608));

    // Use a chunk index that doesn't exist
    record_progress_on_managed(&managed, Some(999), 2000);

    let core = managed.core.lock();
    // Snapshot and manifest still update
    assert_eq!(core.snapshot.downloaded_bytes, 2500);
    assert_eq!(core.manifest.downloaded_bytes, 2500);
    // Chunk remains unchanged
    assert_eq!(core.manifest.chunks[0].downloaded, 100);
    assert!(!core.manifest.chunks[0].completed);
}

#[tokio::test]
#[timeout(30_000)]
async fn record_progress_chunk_exceeds_bounds_marks_completed() {
    // Chunk covers 0..1_000_000, currently at 999_500
    let managed = make_managed_with_chunk("dl3", 999_500, 0, 1_000_000, 999_500, Some(10_000_000));

    // Add 1000 bytes — pushes chunk downloaded (1_000_500) past its size (1_000_000)
    record_progress_on_managed(&managed, Some(0), 1000);

    let core = managed.core.lock();
    assert_eq!(core.manifest.chunks[0].downloaded, 1_000_500);
    assert!(core.manifest.chunks[0].completed, "chunk should be marked completed when downloaded > size");
    assert!(core.manifest.chunks[0].claimed_by.is_none(), "claimed_by should be cleared");
}

#[tokio::test]
#[timeout(30_000)]
async fn record_progress_overflow_protection() {
    let managed = make_managed_with_chunk("dl4", u64::MAX, 0, 4_194_304, 0, Some(8_388_608));

    record_progress_on_managed(&managed, Some(0), 5000);

    let core = managed.core.lock();
    // saturating_add should keep value at u64::MAX
    assert_eq!(core.snapshot.downloaded_bytes, u64::MAX);
    assert_eq!(core.manifest.downloaded_bytes, u64::MAX);
}

#[tokio::test]
#[timeout(30_000)]
async fn record_progress_none_chunk_index_still_updates_snapshot() {
    let managed = make_managed_with_chunk("dl5", 100, 0, 4_194_304, 50, Some(8_388_608));

    record_progress_on_managed(&managed, None, 777);

    let core = managed.core.lock();
    assert_eq!(core.snapshot.downloaded_bytes, 877);
    assert_eq!(core.manifest.downloaded_bytes, 877);
    // Chunk should be untouched
    assert_eq!(core.manifest.chunks[0].downloaded, 50);
}

// ── cancellation_outcome ─────────────────────────────────────────────

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_outcome_when_canceled() {
    let managed = make_managed("canceled", DownloadState::Canceled, "https://example.com/f");
    let outcome = cancellation_outcome(&managed);
    assert!(matches!(outcome, RunOutcome::Canceled));
}

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_outcome_when_downloading() {
    let managed = make_managed("dl", DownloadState::Downloading, "https://example.com/f");
    let outcome = cancellation_outcome(&managed);
    assert!(matches!(outcome, RunOutcome::Paused));
}

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_outcome_when_paused() {
    let managed = make_managed("paused", DownloadState::Paused, "https://example.com/f");
    let outcome = cancellation_outcome(&managed);
    assert!(matches!(outcome, RunOutcome::Paused));
}

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_outcome_when_completed() {
    let managed = make_managed("done", DownloadState::Completed, "https://example.com/f");
    let outcome = cancellation_outcome(&managed);
    assert!(matches!(outcome, RunOutcome::Paused));
}

// ── cancellation_chunk_outcome ───────────────────────────────────────

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_chunk_outcome_when_canceled() {
    let managed = make_managed("canceled", DownloadState::Canceled, "https://example.com/f");
    let outcome = cancellation_chunk_outcome(&managed);
    assert!(matches!(outcome, ChunkWorkerOutcome::Canceled));
}

#[tokio::test]
#[timeout(30_000)]
async fn cancellation_chunk_outcome_when_downloading() {
    let managed = make_managed("dl", DownloadState::Downloading, "https://example.com/f");
    let outcome = cancellation_chunk_outcome(&managed);
    assert!(matches!(outcome, ChunkWorkerOutcome::Paused));
}
