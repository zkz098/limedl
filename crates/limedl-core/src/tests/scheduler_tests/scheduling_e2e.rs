//! Adaptive/automatic scheduling against a live mock server.

use super::*;

/// Start two files (10 MB and 5 MB) on separate TestServer instances.
/// The automatic scheduler should allocate more connections to the larger file.
#[tokio::test]
#[timeout(180_000)]
async fn automatic_mode_prioritizes_larger_file() -> TestResult {
    let big_server = TestServer::new(10 * 1024 * 1024).await; // 10 MB
    let small_server = TestServer::new(5 * 1024 * 1024).await; // 5 MB

    let (_tmp, manager) = create_manager().await;
    apply_settings(
        &manager,
        SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_parallel_threads: 3,
                max_threads_per_task: 3,
                min_threads_per_task: 0,
                adaptive_profile: Default::default(),
            },
            ..SchedulerSettings::default()
        },
    )
    .await;

    let out = _tmp.path().join("out").to_string_lossy().to_string();

    let big_id = manager
        .start(req_fixed(&big_server.file_url_range(), &out, "big.bin"))
        .await?;
    let small_id = manager
        .start(req_fixed(&small_server.file_url_range(), &out, "small.bin"))
        .await?;

    // Run the scheduler several times to stabilize allocations (in production
    // the scheduler ticks every 2s; running 3 times with delays simulates this).
    for _ in 0..3 {
        manager.scheduler.update_adaptive_targets(&manager).await?;
        manager.scheduler.rebalance_allocations(&manager).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    let big = manager.status(&big_id.to_string()).await?;
    let small = manager.status(&small_id.to_string()).await?;

    assert!(
        big.connection_count >= small.connection_count,
        "expected big ({} conns) >= small ({} conns)",
        big.connection_count,
        small.connection_count,
    );

    let _ = manager.cancel(&big_id.to_string()).await;
    let _ = manager.cancel(&small_id.to_string()).await;
    Ok(())
}

/// Regression test: `update_adaptive_targets` used to return early whenever
/// `proxy.mode != Disabled`, freezing AIMD (and, silently, overclock mode,
/// which lives behind the same guard) for every proxied download.  The bail-out
/// was a leftover from the removed network-learning feature that the tuner was
/// originally coupled to.
///
/// Also pins that a tuned target is mirrored into the snapshot right away
/// instead of waiting for the next `rebalance_allocations`.
///
/// The proxy is switched on while a transfer is already in flight, so no
/// request ever traverses a real proxy — only the settings gate that used to
/// disable the tuner is exercised.
#[tokio::test]
#[timeout(180_000)]
async fn adaptive_targets_apply_when_proxy_is_enabled() -> TestResult {
    let server = TestServer::new(16 * 1024 * 1024).await; // 16 MB (multi-chunk)

    let (_tmp, manager) = create_manager().await;

    let scheduler = SchedulerSettings {
        mode: SchedulerMode::Automatic,
        automatic: AutomaticSchedulerSettings {
            max_parallel_threads: 4,
            max_threads_per_task: 4,
            min_threads_per_task: 1,
            adaptive_profile: AdaptiveProfile::Balanced,
        },
        ..SchedulerSettings::default()
    };

    // 1 MB/s keeps the transfer in flight while the test pokes the scheduler.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 1_000_000,
            scheduler: scheduler.clone(),
            ..AppSettings::default()
        })
        .await?;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let id = manager
        .start(req_adaptive(&server.file_url_range(), &out, "proxied.bin"))
        .await?;

    // Wait for the transfer to run *and* make progress: the executor's earlier
    // metadata phase re-applies the profile-derived target and may rebuild the
    // AIMD state, so only poke the tuner once bytes are actually flowing.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let status = manager.status(&id.to_string()).await?;
        if status.state == DownloadState::Downloading
            && status.downloaded_bytes > 0
            && status.supports_ranges
        {
            assert_eq!(
                status.desired_thread_count,
                Some(3),
                "expected the untuned Balanced initial target"
            );
            break;
        }
        if tokio::time::Instant::now() > deadline {
            return Err(format!("download never started: {status:?}").into());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Enable a proxy mid-flight: the running workers keep the client they were
    // spawned with, so this only changes what the scheduler reads.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 1_000_000,
            proxy: ProxySettings {
                mode: ProxyMode::System,
                manual_url: String::new(),
            },
            scheduler,
            ..AppSettings::default()
        })
        .await?;

    // Make the next sample look like a steep throughput drop: the tuner must
    // react with a multiplicative decrease (Balanced: 3 → 2).
    let managed = {
        let downloads = manager.downloads.read().await;
        downloads
            .get(&id.to_string())
            .cloned()
            .ok_or("managed download missing")?
    };
    {
        let mut aimd = managed.lock_aimd();
        aimd.last_throughput = Some(1e12);
        aimd.cooldown_until = None;
        aimd.hysteresis_lock_until = None;
    }

    manager.scheduler.update_adaptive_targets(&manager).await?;

    // The tuner must publish the new target itself — without waiting for the
    // `rebalance_allocations` call that follows it in the scheduler loop.
    {
        let core = managed.lock_core();
        assert_eq!(
            core.manifest.desired_thread_count,
            Some(2),
            "AIMD must keep tuning while a proxy is configured (Balanced: 3 → 2), got {:?}",
            core.manifest.desired_thread_count,
        );
    }

    let tuned = manager.status(&id.to_string()).await?;
    assert_eq!(
        tuned.desired_thread_count,
        Some(2),
        "the tuned target must be mirrored into the snapshot immediately, got {:?}",
        tuned.desired_thread_count,
    );

    manager.scheduler.rebalance_allocations(&manager).await?;

    let allocated = manager.status(&id.to_string()).await?;
    assert_eq!(
        allocated.allocated_thread_count,
        Some(2),
        "the allocation must follow the tuned target, got {:?}",
        allocated.allocated_thread_count,
    );

    // Overclock shares the same code path and used to be disabled with it.
    manager.set_overclock_mode(true);
    manager.scheduler.update_adaptive_targets(&manager).await?;

    let overclocked = manager.status(&id.to_string()).await?;
    assert_eq!(
        overclocked.desired_thread_count,
        Some(4),
        "overclock must pin the target at max_threads_per_task, got {:?}",
        overclocked.desired_thread_count,
    );

    manager.set_overclock_mode(false);
    let _ = manager.cancel(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn automatic_scheduler_respects_single_thread_cap_when_min_per_task_is_four() -> TestResult {
    let (_tmp, manager) = create_manager().await;

    apply_settings(
        &manager,
        SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_parallel_threads: 16,
                min_threads_per_task: 4,
                max_threads_per_task: 8,
                ..AutomaticSchedulerSettings::default()
            },
            ..SchedulerSettings::default()
        },
    )
    .await;

    let server = TestServer::new(16 * 1024 * 1024).await;
    let temp = tempdir()?;

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: true,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    // Mark as Queued and supports_ranges: true so rebalance_allocations will allocate threads
    {
        let managed = manager
            .downloads
            .read()
            .await
            .get(&id.to_string())
            .unwrap()
            .clone();
        let mut core = managed.lock_core();
        core.manifest.state = DownloadState::Queued;
        core.manifest.supports_ranges = true;
    }

    manager.scheduler.rebalance_allocations(&manager).await?;

    let status = manager.status(&id.to_string()).await?;
    assert_eq!(
        status.allocated_thread_count,
        Some(1),
        "task with fixed single thread must NOT be allocated 4 threads even when min_threads_per_task=4"
    );

    let _ = manager.cancel(&id.to_string()).await;
    Ok(())
}
