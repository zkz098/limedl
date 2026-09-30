//! End-to-end scheduling behaviour with a live mock server.

use super::*;

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
    let manager = Arc::new(DownloadManager::new_with_components(
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
                tail_sprint_enabled: false,
                connection_warmup_enabled: false,
            },
            download: DownloadDefaultsSettings::default(),
            bt: BtSettings::default(),

            logging: LogSettings::default(),
            aria2_rpc: Aria2RpcSettings::default(),
            cdn_acceleration: CdnAccelerationSettings::default(),
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
            headers: None,
            mirror_urls: None,
            priority: None,
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
            headers: None,
            mirror_urls: None,
            priority: None,
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
    let manager = Arc::new(DownloadManager::new_with_components(
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
                tail_sprint_enabled: false,
                connection_warmup_enabled: false,
            },
            download: DownloadDefaultsSettings::default(),
            bt: BtSettings::default(),

            logging: LogSettings::default(),
            aria2_rpc: Aria2RpcSettings::default(),
            cdn_acceleration: CdnAccelerationSettings::default(),
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
            headers: None,
            mirror_urls: None,
            priority: None,
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
