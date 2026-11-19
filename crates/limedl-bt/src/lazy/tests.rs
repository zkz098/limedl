use super::*;
use crate::protocol::DownloadBackend;

/// Settings that start a session without any network I/O.
fn offline_settings() -> AppSettings {
    let mut settings = AppSettings::default();
    settings.bt.dht_enabled = false;
    settings.bt.upnp_enabled = false;
    settings.bt.enable_ipv6 = false;
    settings.bt.enable_utp = false;
    settings.bt.listen_port = Some(0);
    settings
}

fn make_backend(settings: &AppSettings) -> (tempfile::TempDir, Arc<LazyBtBackend>) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");
    let output_dir = tmp.path().join("out");
    std::fs::create_dir_all(&state_dir).expect("create state dir");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let backend = Arc::new(LazyBtBackend::new(
        settings,
        state_dir,
        output_dir,
        Arc::new(EventBus::new(64)),
        Arc::new(Database::open_in_memory().expect("in-memory db")),
        SlotArc::new(SlotAtomicUsize::new(0)),
        SlotArc::new(SlotAtomicUsize::new(5)),
    ));
    (tmp, backend)
}

fn sample_summary(id: &str, state: DownloadState) -> DownloadSummary {
    DownloadSummary {
        id: id.to_string(),
        kind: crate::types::TaskKind::Bt,
        state,
        url: id.to_string(),
        file_name: "sample".into(),
        destination_path: "/tmp/sample".into(),
        total_bytes: Some(2048),
        downloaded_bytes: 1024,
        connection_count: 7,
        thread_mode: crate::types::ThreadMode::Fixed,
        requested_thread_count: None,
        desired_thread_count: None,
        allocated_thread_count: None,
        adaptive_profile: None,
        thread_note: None,
        speed_bytes_per_second: Some(4096.0),
        eta_seconds: Some(42),
        uploaded_bytes: Some(0),
        upload_speed_bytes_per_second: Some(12.0),
        peer_count: Some(7),
        upload_status: Some(BtUploadStatus::Uploading),
        info_hash: Some(id.to_string()),
        expected_checksum: None,
        error: None,
        cdn_accelerated: false,
        cdn_node_ip: None,
        created_at_ms: 123,
        priority: crate::types::Priority::Normal,
        seed_count: Some(1),
        leech_count: Some(2),
        download_limit_bps: None,
        upload_limit_bps: None,
        chunks: Vec::new(),
        mirror_url: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn read_only_calls_do_not_start_the_engine() {
    let (_tmp, backend) = make_backend(&offline_settings());

    assert!(!backend.is_ready(), "engine must not start on construction");
    assert!(backend.startup_error().is_none());

    let list = DownloadBackend::list(backend.as_ref())
        .await
        .expect("list must not fail before warm-up");
    assert!(list.is_empty());
    assert!(backend.get_peers(Id20::new([7u8; 20])).unwrap().is_empty());

    let status = backend.runtime_status();
    assert!(!status.connected, "not-ready status must be disconnected");
    assert_eq!(status.torrent_count, 0);
    assert!(
        !backend.is_ready(),
        "read-only calls must not start the engine"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn list_serves_the_index_with_volatile_metrics_zeroed() {
    let (_tmp, backend) = make_backend(&offline_settings());
    let id = "aa".repeat(20);
    backend
        .db
        .replace_bt_tasks(&[sample_summary(&id, DownloadState::Paused)])
        .expect("seed the index");

    let list = DownloadBackend::list(backend.as_ref())
        .await
        .expect("list must fall back to the index");

    assert_eq!(
        list.len(),
        1,
        "indexed task must be served while the engine is down"
    );
    let row = &list[0];
    assert_eq!(row.id, id);
    assert_eq!(row.state, DownloadState::Paused);
    assert_eq!(row.downloaded_bytes, 1024, "persisted progress is kept");
    assert_eq!(row.speed_bytes_per_second, None);
    assert_eq!(row.upload_speed_bytes_per_second, None);
    assert_eq!(row.eta_seconds, None);
    assert_eq!(row.connection_count, 0);
    assert_eq!(row.peer_count, Some(0));
    assert!(
        !backend.is_ready(),
        "serving the index must not start the engine"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn has_unfinished_tasks_tracks_index_and_resume_files() {
    let (_tmp, backend) = make_backend(&offline_settings());
    assert!(
        !backend.has_unfinished_tasks(),
        "empty index and no resume files means no work"
    );

    backend
        .db
        .replace_bt_tasks(&[sample_summary(
            "bb".repeat(20).as_str(),
            DownloadState::Paused,
        )])
        .expect("seed the index");
    assert!(
        !backend.has_unfinished_tasks(),
        "a paused task alone must not start the engine"
    );

    backend
        .db
        .replace_bt_tasks(&[sample_summary(
            "cc".repeat(20).as_str(),
            DownloadState::Downloading,
        )])
        .expect("seed the index");
    assert!(backend.has_unfinished_tasks());

    // An empty index with a leftover resume file must still trigger a start.
    backend.db.clear_bt_tasks().expect("clear index");
    let resume_dir = backend.state_dir.join("resume").join("torrents");
    std::fs::create_dir_all(&resume_dir).expect("create resume dir");
    std::fs::write(resume_dir.join("some.json"), b"{}").expect("write resume file");
    assert!(backend.has_unfinished_tasks());

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_before_warm_up_never_starts_the_engine() {
    let (_tmp, backend) = make_backend(&offline_settings());

    backend.shutdown().await;

    assert!(!backend.is_ready());
    // Later work is rejected instead of resurrecting the engine.
    let err = match backend.engine().await {
        Ok(_) => panic!("engine must stay down after shutdown"),
        Err(e) => e,
    };
    assert!(err.to_string().contains("shutting down"), "got: {err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn warm_up_starts_the_engine_in_the_background() {
    let mut settings = offline_settings();
    settings.bt.dht_enabled = false;
    let (_tmp, backend) = make_backend(&settings);

    backend.spawn_startup();

    assert!(
        backend.wait_ready().await,
        "warm-up should bring the engine up"
    );
    assert!(backend.is_ready());
    assert!(
        !backend.runtime_status().dht_enabled,
        "launch settings must be used"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn settings_stored_before_warm_up_are_used_at_startup() {
    let settings = offline_settings();
    let (_tmp, backend) = make_backend(&AppSettings::default());

    // The engine would start with DHT enabled; disable it beforehand.
    backend.apply_settings(&settings);
    assert!(!backend.bt_settings().dht_enabled);

    backend.spawn_startup();
    assert!(backend.wait_ready().await);
    assert!(!backend.runtime_status().dht_enabled);

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn engine_restarts_after_an_idle_shutdown() {
    let settings = offline_settings();
    let (_tmp, backend) = make_backend(&settings);

    assert!(backend.engine().await.is_ok());
    assert!(backend.is_ready());

    // While within the idle settle window, stop_idle_engine must keep the engine up.
    backend.stop_idle_engine().await;
    assert!(
        backend.is_ready(),
        "active settle window must keep engine up"
    );

    // Simulate elapsed IDLE_SETTLE window before the idle supervisor unloads it.
    backend.last_activity_ms.store(0, Ordering::Release);
    backend.stop_idle_engine().await;
    assert!(!backend.is_ready(), "idle engine must be unloaded");

    // On-demand work must build a fresh session, not fail on a spent cell.
    assert!(backend.engine().await.is_ok());
    assert!(backend.is_ready());

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn spawn_startup_restarts_after_an_idle_shutdown() {
    let (_tmp, backend) = make_backend(&offline_settings());

    backend.spawn_startup();
    assert!(backend.wait_for_warmup().await);
    backend.last_activity_ms.store(0, Ordering::Release);
    backend.stop_idle_engine().await;
    assert!(!backend.is_ready());

    // The finished warm-up handle must not make this a no-op (this is the
    // "leave lightweight mode" path).
    backend.spawn_startup();
    assert!(backend.wait_for_warmup().await);
    assert!(backend.is_ready());

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn wait_for_warmup_does_not_start_the_engine() {
    let (_tmp, backend) = make_backend(&offline_settings());

    assert!(
        !backend.wait_for_warmup().await,
        "no warm-up was requested, so nothing should start"
    );
    assert!(!backend.is_ready());

    backend.spawn_startup();
    assert!(backend.wait_for_warmup().await);

    backend.shutdown().await;
}

#[test]
fn test_take_legacy_resume_files_covers_both_layouts() {
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path();
    let resume_dir = state_dir.join("resume");
    let torrents_dir = resume_dir.join("torrents");
    std::fs::create_dir_all(&torrents_dir).unwrap();

    let top_level = resume_dir.join("legacy_task.resume");
    let nested = torrents_dir.join("deadbeef.resume");
    let json_file = resume_dir.join("session.json");
    let torrent_file = resume_dir.join("deadbeef.torrent");
    std::fs::write(&top_level, b"legacy data").unwrap();
    std::fs::write(&nested, b"legacy data").unwrap();
    std::fs::write(&json_file, b"{}").unwrap();
    std::fs::write(&torrent_file, b"torrent").unwrap();

    assert_eq!(take_legacy_resume_files(state_dir), 2);
    assert!(!top_level.exists(), "top-level .resume file must be pruned");
    assert!(!nested.exists(), "torrents/ .resume file must be pruned");
    assert!(json_file.exists(), "valid .json resume file must be kept");
    assert!(torrent_file.exists(), "valid .torrent file must be kept");

    assert_eq!(take_legacy_resume_files(state_dir), 0);
}
