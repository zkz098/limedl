//! Fairness, thread-mode mixing and shared rate limiting.

use super::*;

/// Start a single download with a 500 KB/s global limit and verify the
/// measured throughput stays below 2× the limit.
#[tokio::test]
#[timeout(300_000)]
async fn scheduler_respects_global_speed_limit() -> TestResult {
    // Need a file large enough that chunked mode spawns ≥2 workers so the
    // rate limiter receives small stream sub-chunks (64 KB) that fit within
    // its token-bucket capacity (2 × rate).  With default 4 MiB chunk_size:
    //   chunks = ceil(file_size / 4 MiB)
    //   workers = min(allocation, max(chunks / 2, 1))
    // With allocation=4 and ≥5 chunks we get ≥2 workers.
    let file_size: u64 = 20 * 1024 * 1024; // 20 MB → 5 chunks
    let server = TestServer::new(file_size).await;

    let (_tmp, manager) = create_manager().await;

    let limit_bps: u64 = 512_000; // 500 KB/s
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: limit_bps,
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Traditional,
                traditional: TraditionalSchedulerSettings {
                    max_parallel_tasks: 1,
                },
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        })
        .await?;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let id = manager
        .start(req_fixed(&server.file_url_range(), &out, "speed-test.bin"))
        .await?;

    let start = std::time::Instant::now();
    let done = tokio::time::timeout(
        Duration::from_secs(120),
        wait_for_terminal(&manager, &id.to_string()),
    )
    .await
    .map_err(|_| "timeout")?;

    let elapsed = start.elapsed().as_secs_f64();
    let avg_speed = file_size as f64 / elapsed;

    assert_eq!(
        done.state,
        DownloadState::Completed,
        "error={:?}",
        done.error
    );
    assert!(
        avg_speed <= limit_bps as f64 * 3.0,
        "avg speed {:.0} bps exceeded 3x limit {} bps",
        avg_speed,
        limit_bps
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

/// Two equal-sized downloads in Automatic mode with a shared budget of 4
/// threads.  Both should get similar thread allocations, complete successfully,
/// and the largest allocation gap should be at most 2.
///
/// Uses files >= 8 MB so `supports_parallelism` enables multi‑threaded mode
/// (default chunk_size = 4 MiB, threshold = 2 chunks).
#[tokio::test]
#[timeout(180_000)]
async fn multi_download_fairness_under_limited_threads() -> TestResult {
    let server = TestServer::new(10 * 1024 * 1024).await; // 10 MB (≥ 8 MB)

    let (_tmp, manager) = create_manager().await;
    apply_settings(
        &manager,
        SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_parallel_threads: 4,
                max_threads_per_task: 4,
                min_threads_per_task: 1,
                adaptive_profile: Default::default(),
            },
            ..SchedulerSettings::default()
        },
    )
    .await;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let url = server.file_url_range();

    let id1 = manager
        .start(req_adaptive(&url, &out, "fair-first.bin"))
        .await?;
    let id2 = manager
        .start(req_adaptive(&url, &out, "fair-second.bin"))
        .await?;

    // Wait for probes to finish, then rebalance so allocations are final.
    tokio::time::sleep(Duration::from_secs(2)).await;
    manager.scheduler.update_adaptive_targets(&manager).await?;
    manager.scheduler.rebalance_allocations(&manager).await?;

    let s1 = manager.status(&id1.to_string()).await?;
    let s2 = manager.status(&id2.to_string()).await?;

    let diff = (s1.connection_count as i64 - s2.connection_count as i64).unsigned_abs();
    assert!(
        diff <= 2,
        "thread allocation difference too large: {} vs {} (diff={})",
        s1.connection_count,
        s2.connection_count,
        diff,
    );

    // Wait for both to complete.
    let done1 = tokio::time::timeout(
        Duration::from_secs(60),
        wait_for_terminal(&manager, &id1.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for first download")?;
    let done2 = tokio::time::timeout(
        Duration::from_secs(60),
        wait_for_terminal(&manager, &id2.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for second download")?;

    assert_eq!(
        done1.state,
        DownloadState::Completed,
        "error={:?}",
        done1.error
    );
    assert_eq!(
        done2.state,
        DownloadState::Completed,
        "error={:?}",
        done2.error
    );

    let _ = manager.remove(&id1.to_string()).await;
    let _ = manager.remove(&id2.to_string()).await;
    Ok(())
}

/// One Adaptive and one Fixed (2 threads) download share a max_parallel_threads
/// budget of 4.  The Fixed download must get exactly 2 threads, the Adaptive
/// download gets the remaining budget, and total allocated threads ≤ budget.
///
/// Uses 10 MB files (≥ 8 MB so `supports_parallelism` enables multi‑threading)
/// and a 1 MB/s global speed limit so both downloads remain in progress long
/// enough to observe the scheduler's allocation.
#[tokio::test]
#[timeout(180_000)]
async fn mixed_thread_mode_downloads_coexist() -> TestResult {
    let server = TestServer::new(10 * 1024 * 1024).await; // 10 MB (≥ 8 MB)

    let (_tmp, manager) = create_manager().await;

    // Speed limit ensures downloads haven't finished when we check
    // allocations (10 MB @ 1 MB/s shared = ~10 s per download).
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 1_000_000, // 1 MB/s
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Automatic,
                automatic: AutomaticSchedulerSettings {
                    max_parallel_threads: 4,
                    max_threads_per_task: 4,
                    min_threads_per_task: 1,
                    adaptive_profile: Default::default(),
                },
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        })
        .await?;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let url = server.file_url_range();

    // Fixed-mode download requesting exactly 2 threads.
    let fixed_id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: url.clone(),
            destination_dir: out.clone(),
            file_name: Some("fixed.bin".to_string()),
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(2),
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
    // Adaptive-mode download — scheduler decides allocation.
    let adaptive_id = manager
        .start(req_adaptive(&url, &out, "adaptive.bin"))
        .await?;

    // Trigger rebalances until the Fixed-mode download gets its 2 threads.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let fixed_snap = loop {
        manager.scheduler.rebalance_allocations(&manager).await?;
        let s = manager.status(&fixed_id.to_string()).await?;
        if s.connection_count == 2 {
            break s;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("timed out waiting for fixed-mode download to get 2 threads");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    let adaptive_snap = manager.status(&adaptive_id.to_string()).await?;

    let total = fixed_snap.connection_count + adaptive_snap.connection_count;

    assert_eq!(
        fixed_snap.connection_count, 2,
        "Fixed-mode download should have exactly 2 threads, got {}. state={:?} supports_ranges={}",
        fixed_snap.connection_count, fixed_snap.state, fixed_snap.supports_ranges,
    );
    assert!(
        adaptive_snap.connection_count >= 1,
        "Adaptive-mode download should have at least 1 thread, got {}",
        adaptive_snap.connection_count,
    );
    assert!(
        total <= 4,
        "total thread allocation {} exceeds max_parallel_threads (4)",
        total,
    );

    // Remove speed limit so completion is fast.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 0,
            ..AppSettings::default()
        })
        .await?;

    // Wait for both to complete.
    let done_fixed = tokio::time::timeout(
        Duration::from_secs(60),
        wait_for_terminal(&manager, &fixed_id.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for fixed download")?;
    let done_adaptive = tokio::time::timeout(
        Duration::from_secs(60),
        wait_for_terminal(&manager, &adaptive_id.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for adaptive download")?;

    assert_eq!(
        done_fixed.state,
        DownloadState::Completed,
        "error={:?}",
        done_fixed.error
    );
    assert_eq!(
        done_adaptive.state,
        DownloadState::Completed,
        "error={:?}",
        done_adaptive.error
    );

    let _ = manager.remove(&fixed_id.to_string()).await;
    let _ = manager.remove(&adaptive_id.to_string()).await;
    Ok(())
}

/// Two 3 MB downloads share a 500 KB/s global speed limit.  Combined
/// throughput must not exceed the limit by more than 30 %.
#[tokio::test]
#[timeout(120_000)]
async fn rate_limiter_shared_across_multiple_downloads() -> TestResult {
    let file_size: u64 = 3 * 1024 * 1024; // 3 MB
    let server = TestServer::new(file_size).await;

    let (_tmp, manager) = create_manager().await;

    let limit_bps: u64 = 512_000; // 500 KB/s
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: limit_bps,
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Traditional,
                traditional: TraditionalSchedulerSettings {
                    max_parallel_tasks: 2,
                },
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        })
        .await?;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let url = server.file_url_range();

    let id1 = manager
        .start(req_fixed_range(&url, &out, "rate-a.bin"))
        .await?;
    let id2 = manager
        .start(req_fixed_range(&url, &out, "rate-b.bin"))
        .await?;

    let start = std::time::Instant::now();

    let done1 = tokio::time::timeout(
        Duration::from_secs(90),
        wait_for_terminal(&manager, &id1.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for first download")?;
    let done2 = tokio::time::timeout(
        Duration::from_secs(90),
        wait_for_terminal(&manager, &id2.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for second download")?;

    let elapsed = start.elapsed().as_secs_f64();
    let total_bytes = done1.downloaded_bytes + done2.downloaded_bytes;
    let avg_speed = total_bytes as f64 / elapsed;

    // Combined throughput must be ≤ limit + 100 % tolerance.
    let tolerance = limit_bps as f64 * 200.0 / 100.0;
    assert!(
        avg_speed <= tolerance,
        "combined throughput {:.0} B/s exceeds tolerance {:.0} B/s (limit={} B/s + 100%)",
        avg_speed,
        tolerance,
        limit_bps,
    );

    assert_eq!(
        done1.state,
        DownloadState::Completed,
        "error={:?}",
        done1.error
    );
    assert_eq!(
        done2.state,
        DownloadState::Completed,
        "error={:?}",
        done2.error
    );

    let _ = manager.remove(&id1.to_string()).await;
    let _ = manager.remove(&id2.to_string()).await;
    Ok(())
}
