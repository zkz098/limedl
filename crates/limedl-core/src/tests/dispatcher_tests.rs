//! Task 2: Dispatcher cancel/remove/purge/pause emit `DownloadEvent::Updated` invariant.
//!
//! Contract: `Dispatcher` publishes `DownloadEvent::Updated` on the EventBus after
//! every state-changing operation (pause, cancel, remove, purge) so the frontend
//! receives live state synchronization.
//!
//! Each test injects a ManagedDownload with a specific state into the DownloadManager's
//! in-memory map, then calls the dispatcher operation and asserts an Updated event
//! with the matching task ID arrives on the EventBus.

use std::sync::Arc;

use ntest::timeout;
use parking_lot::Mutex as ParkingMutex;
use tempfile::tempdir;
use tokio::sync::Notify;

use crate::aimd::AimdState;
use crate::backend_registry::BackendRegistry;
use crate::buffer_pool::BufferPool;
use crate::cdn::CdnService;
use crate::dispatcher::Dispatcher;
use crate::event_bus::{DownloadEvent, EventBus};
use crate::download::{DownloadCore, ManagedDownload};
use crate::manager::DownloadManager;
use crate::manifest::{Manifest, CHUNK_SIZE};
use crate::rate_limiter::RateLimiter;
use crate::services::{ConcurrencyManager, DiskIoService, SettingsService};
use crate::types::{
    AppSettings, BtSettings, CdnAccelerationSettings, ChecksumMode, DownloadDefaultsSettings,
    DownloadSnapshot, DownloadState, IoBaselineSettings, MatchType, Priority, ReplacementMode,
    RewriteTarget, SchedulerMode, SchedulerSettings, TaskId, ThreadMode, TraditionalSchedulerSettings,
    UrlRewriteRule, UrlRewriteSettings,
};
use std::sync::atomic::Ordering;

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a DownloadManager with a temp state dir and return it along with the
/// tempdir guard.
///
/// `pub(crate)` so aria2 RPC tests can reuse it for `resolve_gid` coverage
/// without duplicating the whole ManagedDownload fixture.
pub(crate) fn make_manager() -> (tempfile::TempDir, Arc<DownloadManager>) {
    let tmp = tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");
    std::fs::create_dir_all(state_dir.join("logs")).ok();
    let dm = DownloadManager::new_with_components(
        state_dir,
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )
    .expect("DownloadManager::new");
    (tmp, Arc::new(dm))
}

/// Build a minimal ManagedDownload with the given id and state.
/// The download has no runtime token (not spawned), which is safe for
/// cancel/remove/purge/pause testing because these operations tolerate
/// a missing runtime token.
fn make_download(id: &str, state: DownloadState) -> Arc<ManagedDownload> {
    Arc::new(ManagedDownload {
        core: ParkingMutex::new(DownloadCore {
            snapshot: DownloadSnapshot {
                id: id.to_string(),
                kind: crate::types::TaskKind::Http,
                state,
                url: "https://example.com/file.bin".into(),
                final_url: "https://example.com/file.bin".into(),
                file_name: "file.bin".into(),
                destination_path: "".into(),
                temp_path: "".into(),
                total_bytes: Some(1024),
                downloaded_bytes: 0,
                supports_ranges: false,
                connection_count: 0,
                thread_mode: ThreadMode::Fixed,
                requested_thread_count: Some(1),
                desired_thread_count: Some(1),
                allocated_thread_count: Some(0),
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
                created_at_ms: 1000,
                updated_at_ms: 1000,
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
                url: "https://example.com/file.bin".into(),
                final_url: "https://example.com/file.bin".into(),
                user_agent: "test".into(),
                extra_headers: vec![],
                destination_dir: "".into(),
                file_name: "file.bin".into(),
                file_name_locked: false,
                destination_path: "".into(),
                temp_path: "".into(),
                total_bytes: Some(1024),
                downloaded_bytes: 0,
                supports_ranges: false,
                chunk_size: CHUNK_SIZE,
                connection_count: 0,
                thread_mode: ThreadMode::Fixed,
                requested_thread_count: Some(1),
                desired_thread_count: Some(1),
                allocated_thread_count: Some(0),
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
                created_at_ms: 1000,
                updated_at_ms: 1000,
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

/// Set up a dispatcher with a DM registered as the HTTP backend.
fn make_dispatcher(dm: Arc<DownloadManager>) -> (Arc<EventBus>, Dispatcher) {
    use crate::backend_registry::BackendRegistry;
    let event_bus = Arc::new(EventBus::new(1024));
    let mut registry = BackendRegistry::new();
    registry.register_arc(crate::types::TaskKind::Http, dm.clone());
    let dispatcher = Dispatcher::new(Arc::new(registry), event_bus.clone());
    (event_bus, dispatcher)
}

/// Subscribe to the event bus and return a receiver.
fn subscribe(eb: &EventBus) -> tokio::sync::broadcast::Receiver<DownloadEvent> {
    eb.subscribe()
}

/// Inject a download into the DM's downloads map.
pub(crate) async fn inject_download(dm: &DownloadManager, id: &str, state: DownloadState) {
    let dl = make_download(id, state);
    let id_str = id.to_string();
    dm.downloads.write().await.insert(id_str, dl);
}

// ---------------------------------------------------------------------------
// Test: dispatcher.cancel emits DownloadEvent::Updated
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_cancel_emits_updated() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(1));
    let id_str = task_id.to_string();

    // Inject a Paused download (cancel rejects Completed but Paused is fine)
    inject_download(&dm, &id_str, DownloadState::Paused).await;

    let mut rx = subscribe(&eb);
    let snapshot = dispatcher.cancel(&task_id).await?;
    assert_eq!(snapshot.state, DownloadState::Canceled);

    // Consume the event (should arrive immediately, no timeout needed)
    let event = rx.try_recv()?;
    match event {
        DownloadEvent::Updated { summary } => {
            assert_eq!(summary.id, id_str, "cancel: Updated event id must match task id");
        }
        other => panic!("cancel: expected Updated, got {other:?}"),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: dispatcher.remove emits DownloadEvent::Updated
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_remove_emits_updated() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(2));
    let id_str = task_id.to_string();

    // Inject a Completed download (remove works on any state)
    inject_download(&dm, &id_str, DownloadState::Completed).await;

    let mut rx = subscribe(&eb);
    let snapshot = dispatcher.remove(&task_id).await?;
    assert_eq!(snapshot.state, DownloadState::Completed);

    let event = rx.try_recv()?;
    match event {
        DownloadEvent::Updated { summary } => {
            assert_eq!(summary.id, id_str, "remove: Updated event id must match task id");
        }
        other => panic!("remove: expected Updated, got {other:?}"),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: dispatcher.purge emits DownloadEvent::Updated
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_purge_emits_updated() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(3));
    let id_str = task_id.to_string();

    // Inject a Failed download
    inject_download(&dm, &id_str, DownloadState::Failed).await;

    let mut rx = subscribe(&eb);
    let snapshot = dispatcher.purge(&task_id).await?;
    assert_eq!(snapshot.state, DownloadState::Failed);

    let event = rx.try_recv()?;
    match event {
        DownloadEvent::Updated { summary } => {
            assert_eq!(summary.id, id_str, "purge: Updated event id must match task id");
        }
        other => panic!("purge: expected Updated, got {other:?}"),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: dispatcher.pause emits DownloadEvent::Updated
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_pause_emits_updated() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(4));
    let id_str = task_id.to_string();

    // Inject a Queued download (pause only works on Downloading/Retrying/Queued)
    inject_download(&dm, &id_str, DownloadState::Queued).await;

    let mut rx = subscribe(&eb);
    let snapshot = dispatcher.pause(&task_id).await?;
    assert_eq!(snapshot.state, DownloadState::Paused);

    let event = rx.try_recv()?;
    match event {
        DownloadEvent::Updated { summary } => {
            assert_eq!(summary.id, id_str, "pause: Updated event id must match task id");
        }
        other => panic!("pause: expected Updated, got {other:?}"),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests: BT methods with non-BT TaskId return InvalidRequest
// ---------------------------------------------------------------------------
#[cfg(feature = "bt")]
#[test]
fn bt_set_speed_limit_with_http_taskid_returns_invalid_request() {
    let event_bus = Arc::new(EventBus::new(16));
    let registry = Arc::new(BackendRegistry::new());
    let dispatcher = Dispatcher::new(registry, event_bus);

    let task_id = TaskId::Http(uuid::Uuid::new_v4());
    let result = dispatcher.bt_set_speed_limit(&task_id, Some(1024), Some(512));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("BT task"));
}

#[cfg(feature = "bt")]
#[test]
fn bt_get_peers_with_http_taskid_returns_invalid_request() {
    let event_bus = Arc::new(EventBus::new(16));
    let registry = Arc::new(BackendRegistry::new());
    let dispatcher = Dispatcher::new(registry, event_bus);

    let task_id = TaskId::Http(uuid::Uuid::new_v4());
    let result = dispatcher.bt_get_peers(&task_id);
    assert!(result.is_err());
}

#[cfg(feature = "bt")]
#[test]
fn bt_get_trackers_with_http_taskid_returns_invalid_request() {
    let event_bus = Arc::new(EventBus::new(16));
    let registry = Arc::new(BackendRegistry::new());
    let dispatcher = Dispatcher::new(registry, event_bus);

    let task_id = TaskId::Http(uuid::Uuid::new_v4());
    let result = dispatcher.bt_get_trackers(&task_id);
    assert!(result.is_err());
}

#[cfg(feature = "bt")]
#[test]
fn bt_get_pieces_with_http_taskid_returns_invalid_request() {
    let event_bus = Arc::new(EventBus::new(16));
    let registry = Arc::new(BackendRegistry::new());
    let dispatcher = Dispatcher::new(registry, event_bus);

    let task_id = TaskId::Http(uuid::Uuid::new_v4());
    let result = dispatcher.bt_get_pieces(&task_id);
    assert!(result.is_err());
}

#[cfg(feature = "bt")]
#[test]
fn bt_get_files_with_http_taskid_returns_invalid_request() {
    let event_bus = Arc::new(EventBus::new(16));
    let registry = Arc::new(BackendRegistry::new());
    let dispatcher = Dispatcher::new(registry, event_bus);

    let task_id = TaskId::Http(uuid::Uuid::new_v4());
    let result = dispatcher.bt_get_files(&task_id);
    assert!(result.is_err());
}

#[cfg(feature = "bt")]
#[tokio::test]
async fn bt_update_files_with_http_taskid_returns_invalid_request() {
    let event_bus = Arc::new(EventBus::new(16));
    let registry = Arc::new(BackendRegistry::new());
    let dispatcher = Dispatcher::new(registry, event_bus);

    let task_id = TaskId::Http(uuid::Uuid::new_v4());
    let result = dispatcher.bt_update_files(&task_id, vec![]).await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Test: BT runtime status without BT backend returns Internal error
// ---------------------------------------------------------------------------
#[cfg(feature = "bt")]
#[test]
fn bt_runtime_status_without_bt_backend_returns_internal_error() {
    let event_bus = Arc::new(EventBus::new(16));
    let registry = Arc::new(BackendRegistry::new());
    let dispatcher = Dispatcher::new(registry, event_bus);

    let result = dispatcher.bt_runtime_status();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("BT backend"));
}

// ---------------------------------------------------------------------------
// Dispatcher facade matrix: events, queries, services
// ---------------------------------------------------------------------------

/// A full dispatcher with real services and an empty registry — enough to test
/// the settings/CDN/concurrency/buffer-pool synchronization branches without
/// spinning up a download backend.
fn make_full_dispatcher(
    state_dir: &std::path::Path,
) -> (Dispatcher, Arc<ConcurrencyManager>, Arc<BufferPool>, Arc<CdnService>) {
    let settings_service = Arc::new(
        SettingsService::new(state_dir.join("settings.json")).expect("SettingsService"),
    );
    let pool = Arc::new(BufferPool::new(64, 16, 2, 1));
    let disk_io = Arc::new(DiskIoService::new(pool.clone(), settings_service.clone()));
    let concurrency = Arc::new(ConcurrencyManager::new(1, 1));
    let cdn = Arc::new(CdnService::new());
    let registry = Arc::new(BackendRegistry::new());
    let event_bus = Arc::new(EventBus::new(64));
    let dispatcher = Dispatcher::full(
        registry,
        event_bus,
        settings_service,
        disk_io,
        concurrency.clone(),
        cdn.clone(),
        reqwest::Client::new(),
    );
    (dispatcher, concurrency, pool, cdn)
}

/// `resume` changes state and must publish the resulting snapshot.
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_resume_emits_updated() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(5));
    let id_str = task_id.to_string();
    inject_download(&dm, &id_str, DownloadState::Paused).await;

    // Keep the resumed run off the network: the probe fails against a closed
    // loopback port instead of resolving example.com.
    {
        let dl = dm
            .downloads
            .read()
            .await
            .get(&id_str)
            .cloned()
            .expect("injected download");
        let mut core = dl.lock_core();
        core.manifest.url = "http://127.0.0.1:1/x".into();
        core.manifest.final_url = "http://127.0.0.1:1/x".into();
        core.snapshot.url = "http://127.0.0.1:1/x".into();
    }

    let mut rx = subscribe(&eb);
    let snapshot = dispatcher.resume(&task_id).await?;
    // The scheduler may pick the task up before `resume` returns, so the
    // snapshot is either still queued or already running — never Paused.
    assert!(matches!(
        snapshot.state,
        DownloadState::Queued | DownloadState::Downloading | DownloadState::Retrying
    ));

    let event = rx.try_recv()?;
    match event {
        DownloadEvent::Updated { summary } => {
            assert_eq!(summary.id, id_str, "resume: Updated event id must match task id");
        }
        other => panic!("resume: expected Updated, got {other:?}"),
    }

    Ok(())
}

/// `set_priority` must reach the backend and publish the refreshed snapshot.
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_set_priority_emits_updated() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(6));
    let id_str = task_id.to_string();
    inject_download(&dm, &id_str, DownloadState::Completed).await;

    let mut rx = subscribe(&eb);
    dispatcher.set_priority(&task_id, Priority::High).await?;

    let event = rx.try_recv()?;
    match event {
        DownloadEvent::Updated { summary } => {
            assert_eq!(summary.id, id_str, "set_priority: Updated event id must match task id");
        }
        other => panic!("set_priority: expected Updated, got {other:?}"),
    }
    assert_eq!(dispatcher.status(&task_id).await?.priority, Priority::High);

    Ok(())
}

/// `status`/`list` read through the registry, and `has_active_downloads`
/// aggregates every backend's state.
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_status_list_and_active_aggregation() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (_eb, dispatcher) = make_dispatcher(dm.clone());

    let id = uuid::Uuid::from_u128(7);
    let id_str = id.to_string();
    inject_download(&dm, &id_str, DownloadState::Downloading).await;

    assert!(dispatcher.has_active_downloads().await);
    assert_eq!(dispatcher.status(&TaskId::Http(id)).await?.id, id_str);
    assert_eq!(dispatcher.list().await?.len(), 1);

    // A terminal state drops out of the active aggregation.
    {
        let dl = dm
            .downloads
            .read()
            .await
            .get(&id_str)
            .cloned()
            .expect("injected download");
        let mut core = dl.lock_core();
        core.snapshot.state = DownloadState::Completed;
        core.manifest.state = DownloadState::Completed;
    }
    assert!(!dispatcher.has_active_downloads().await);

    // Unknown task ids are an error, not a panic or a default snapshot.
    assert!(
        dispatcher
            .status(&TaskId::Http(uuid::Uuid::from_u128(99)))
            .await
            .is_err()
    );

    Ok(())
}

/// `open_in_explorer` forwards to the backend; with no existing destination it
/// must fail instead of launching anything.
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_open_in_explorer_without_a_location_errors() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (_eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(8));
    inject_download(&dm, &task_id.to_string(), DownloadState::Completed).await;

    assert!(dispatcher.open_in_explorer(&task_id).await.is_err());
    Ok(())
}

/// A minimal dispatcher (no services) must fail closed rather than panic.
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_without_optional_services_reports_errors() -> TestResult {
    let event_bus = Arc::new(EventBus::new(16));
    let dispatcher = Dispatcher::new(Arc::new(BackendRegistry::new()), event_bus);

    assert!(dispatcher.get_settings().await.is_err());
    assert!(dispatcher.get_settings_blocking().is_err());
    assert!(dispatcher.get_io_status().is_err());
    assert!(dispatcher.toggle_game_mode(None).is_err());
    assert!(dispatcher.toggle_overclock_mode(None).is_err());
    assert!(!dispatcher.game_mode());
    assert!(!dispatcher.get_overclock_mode());
    assert!(dispatcher.default_download_dir().await.is_none());
    assert_eq!(
        dispatcher.resolve_mirror_urls("https://example.com/f").await,
        vec!["https://example.com/f".to_string()]
    );
    assert!(dispatcher.detect_disk_type(std::path::Path::new("/tmp")).is_ok());
    assert!(dispatcher.list().await?.is_empty());
    assert!(!dispatcher.has_active_downloads().await);

    Ok(())
}

/// `save_settings` is the single sync point for runtime limits: concurrency
/// caps, buffer-pool sizing and persisted settings must all move together.
#[tokio::test]
#[timeout(30_000)]
async fn dispatcher_save_settings_syncs_concurrency_and_buffer_pool() -> TestResult {
    let tmp = tempdir()?;
    let (dispatcher, concurrency, pool, _cdn) = make_full_dispatcher(tmp.path());

    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Traditional,
            traditional: TraditionalSchedulerSettings {
                max_parallel_tasks: 7,
            },
            ..Default::default()
        },
        bt: BtSettings {
            max_downloads: 3,
            ..Default::default()
        },
        io_baseline: IoBaselineSettings {
            buffer_limit_mb: 512,
            game_mode_buffer_mb: 64,
            max_parallel_hdd: 5,
            game_mode_max_parallel: 2,
            ..Default::default()
        },
        download: DownloadDefaultsSettings {
            default_download_dir: tmp.path().to_string_lossy().to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let saved = dispatcher.save_settings(&settings).await?;
    assert_eq!(saved.scheduler.traditional.max_parallel_tasks, 7);
    assert_eq!(
        concurrency.max_concurrent_http.load(Ordering::Relaxed),
        7,
        "Traditional mode syncs its task cap into the ConcurrencyManager"
    );
    assert_eq!(concurrency.max_concurrent_bt.load(Ordering::Relaxed), 3);
    assert_eq!(pool.effective_limit(), 512 * 1024 * 1024);
    assert_eq!(pool.effective_max_parallel(), 5);

    // Automatic mode reads a different cap.
    let mut automatic = settings.clone();
    automatic.scheduler.mode = SchedulerMode::Automatic;
    automatic.scheduler.automatic.max_parallel_threads = 11;
    dispatcher.save_settings(&automatic).await?;
    assert_eq!(concurrency.max_concurrent_http.load(Ordering::Relaxed), 11);

    // Read-side helpers reflect the persisted state.
    assert_eq!(
        dispatcher.get_settings().await?.scheduler.automatic.max_parallel_threads,
        11
    );
    assert_eq!(dispatcher.get_settings_blocking()?.bt.max_downloads, 3);
    assert_eq!(
        dispatcher.default_download_dir().await,
        Some(tmp.path().to_string_lossy().to_string())
    );

    // factory_reset returns defaults and re-syncs the runtime limits.
    let defaults = dispatcher.factory_reset().await?;
    assert_eq!(defaults.scheduler.mode, SchedulerMode::Automatic);
    assert_eq!(
        concurrency.max_concurrent_http.load(Ordering::Relaxed),
        defaults.scheduler.automatic.max_parallel_threads
    );

    Ok(())
}

/// Saving with CDN acceleration disabled must tear the accelerator down, not
/// just persist the flag.
#[tokio::test]
#[timeout(30_000)]
async fn dispatcher_save_settings_disabling_cdn_clears_the_accelerator() -> TestResult {
    let tmp = tempdir()?;
    let (dispatcher, _concurrency, _pool, cdn) = make_full_dispatcher(tmp.path());

    let accelerated = AppSettings {
        cdn_acceleration: CdnAccelerationSettings {
            enabled: true,
            active_ip: Some("127.0.0.1".into()),
            active_speed_mbps: Some(12.5),
            ..Default::default()
        },
        ..Default::default()
    };
    cdn.init_from_settings(&accelerated).await;
    assert_eq!(
        cdn.active_ip().await,
        Some("127.0.0.1".parse().unwrap()),
        "fixture must arm the accelerator"
    );

    let mut disabled = accelerated.clone();
    disabled.cdn_acceleration.enabled = false;
    dispatcher.save_settings(&disabled).await?;

    assert!(
        cdn.active_ip().await.is_none(),
        "disabling CDN in settings must clear the live accelerator"
    );
    Ok(())
}

/// URL-rewrite rules feed mirror resolution, and a URL with no matching rule
/// still resolves to itself.
#[tokio::test]
#[timeout(30_000)]
async fn dispatcher_resolve_mirror_urls_applies_enabled_rules() -> TestResult {
    let tmp = tempdir()?;
    let (dispatcher, _concurrency, _pool, _cdn) = make_full_dispatcher(tmp.path());

    let settings = AppSettings {
        url_rewrite: UrlRewriteSettings {
            enabled: true,
            rules: vec![UrlRewriteRule {
                id: "gh".into(),
                name: "GitHub Mirror".into(),
                enabled: true,
                match_type: MatchType::Host,
                pattern: "*.github.com".into(),
                replacement_mode: ReplacementMode::PrefixProxy,
                targets: vec![RewriteTarget {
                    url_template: "https://mirror.example.com".into(),
                    enabled: true,
                    order: 0,
                }],
                encode_url: true,
                fallback_to_original: true,
                order: 0,
            }],
        },
        ..Default::default()
    };
    dispatcher.save_settings(&settings).await?;

    let original = "https://raw.github.com/user/repo/file.zip";
    let resolved = dispatcher.resolve_mirror_urls(original).await;
    assert_eq!(resolved.len(), 2, "mirror plus fallback: {resolved:?}");
    assert!(
        resolved[0].starts_with("https://mirror.example.com/"),
        "first candidate must be the rewritten mirror: {resolved:?}"
    );
    assert_eq!(resolved.last().map(String::as_str), Some(original));

    assert_eq!(
        dispatcher.resolve_mirror_urls("https://example.com/plain").await,
        vec!["https://example.com/plain".to_string()]
    );
    Ok(())
}

/// Tracker list fetching normalizes the remote text and surfaces transport
/// failures instead of returning a partial list.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn dispatcher_fetch_tracker_list_normalizes_remote_text() -> TestResult {
    use axum::{Router, routing::get};

    let app = Router::new().route(
        "/trackers.txt",
        get(|| async {
            "udp://tracker.a.example:80/announce\n\n# comment\nftp://ignored.example/x\nhttps://tracker.b.example/announce\n"
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    // Minimal dispatcher: no pre-built HTTP client, so the method builds one.
    let event_bus = Arc::new(EventBus::new(16));
    let dispatcher = Dispatcher::new(Arc::new(BackendRegistry::new()), event_bus);

    let list = dispatcher
        .fetch_tracker_list(&format!("http://{addr}/trackers.txt"))
        .await?;
    assert_eq!(
        list,
        vec![
            "https://tracker.b.example/announce".to_string(),
            "udp://tracker.a.example:80/announce".to_string(),
        ],
        "normalized, sorted, unsupported schemes dropped"
    );

    assert!(
        dispatcher
            .fetch_tracker_list("not a url")
            .await
            .is_err(),
        "an unparsable tracker-list URL must be an error"
    );
    Ok(())
}

/// `probe_checksum` without an explicit file name must derive it from the URL.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn dispatcher_probe_checksum_derives_the_name_from_the_url() -> TestResult {
    let server = crate::test_harness::TestServer::new(64 * 1024).await;
    let event_bus = Arc::new(EventBus::new(16));
    let dispatcher = Dispatcher::new(Arc::new(BackendRegistry::new()), event_bus);

    // The harness serves both `/file` and `/file.sha256`.
    let probed = dispatcher.probe_checksum(&server.file_url(), None).await?;
    assert_eq!(probed.as_deref(), Some(server.sha256_hash.as_str()));
    Ok(())
}

/// Once a BT backend is registered, the BT facade methods must reach it — even
/// while the engine is warming up, they answer with empty data instead of the
/// "BT backend not registered" error.
#[cfg(feature = "bt")]
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn dispatcher_bt_facade_reaches_a_registered_backend() -> TestResult {
    use crate::bt_backend::LazyBtBackend;
    use crate::types::TaskKind;

    let tmp = tempdir()?;
    let event_bus = Arc::new(EventBus::new(64));
    let backend = Arc::new(LazyBtBackend::new(
        &AppSettings::default(),
        tmp.path().join("bt_state"),
        tmp.path().join("bt_out"),
        event_bus.clone(),
        Arc::new(crate::database::Database::open_in_memory()?),
        Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        Arc::new(std::sync::atomic::AtomicUsize::new(2)),
    ));
    let mut registry = BackendRegistry::new();
    registry.register_arc(TaskKind::Bt, backend);
    let dispatcher = Dispatcher::new(Arc::new(registry), event_bus);

    let status = dispatcher.bt_runtime_status()?;
    assert!(!status.connected, "engine is not started in this fixture");

    let info_hash = irontide::core::Id20::from_hex(&"11".repeat(20)).expect("valid info hash");
    let task_id = TaskId::Bt(info_hash);

    assert!(dispatcher.bt_get_peers(&task_id)?.is_empty());
    assert!(dispatcher.bt_get_trackers(&task_id)?.is_empty());
    assert!(dispatcher.bt_get_pieces(&task_id)?.is_empty());
    assert!(dispatcher.bt_get_files(&task_id)?.is_empty());
    dispatcher.bt_set_speed_limit(&task_id, Some(1024), Some(512))?;

    Ok(())
}
