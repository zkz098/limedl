//! Completed-task eviction.

use super::*;

#[tokio::test]
#[timeout(30_000)]
async fn evict_completed_removes_oldest_terminal_entries() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // Bypass the settings clamp ([10, 10000]) so we can use a small limit.
    manager
        .settings_service
        .inner()
        .write()
        .max_in_memory_downloads = 2;

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
                    created_at_ms: created_at,
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
                    url: String::new(),
                    final_url: String::new(),
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
                    cdn_node_ip: None,
                    priority: Priority::Normal,
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
                speed_tracker: Default::default(),
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
