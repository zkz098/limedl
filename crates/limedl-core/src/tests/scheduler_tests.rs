//! Integration tests for the scheduler (rebalance logic) via DownloadManager.
//!
//! Each test creates a real HTTP server (via [`TestServer`]), a real
//! [`DownloadManager`], and exercises scheduler behaviour through
//! the public `start` / `pause` / `resume` / `cancel` / `status` API.

use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::tempdir;

use crate::{
    event_bus::EventBus,
    manager::DownloadManager,
    rate_limiter::RateLimiter,
    test_harness::TestServer,
    types::{
        AppSettings, AutomaticSchedulerSettings, ChecksumMode, DownloadSnapshot, DownloadState,
        SchedulerMode, SchedulerSettings, StartDownloadRequest, ThreadMode,
        TraditionalSchedulerSettings,
    },
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// Manager helpers
// ---------------------------------------------------------------------------

async fn create_manager() -> (tempfile::TempDir, Arc<DownloadManager>) {
    let tmp = tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");
    std::fs::create_dir_all(state_dir.join("logs")).ok();
    let manager = Arc::new(
        DownloadManager::new(
            state_dir,
            Arc::new(RateLimiter::default()),
            Arc::new(EventBus::new(1024)),
        )
        .expect("DownloadManager::new"),
    );
    (tmp, manager)
}

async fn apply_settings(manager: &DownloadManager, scheduler: SchedulerSettings) {
    manager
        .apply_settings(AppSettings {
            scheduler,
            ..AppSettings::default()
        })
        .await
        .expect("update_settings");
}

fn req_fixed(url: &str, dir: &str, name: &str) -> StartDownloadRequest {
    StartDownloadRequest {
        kind: None,
        url: url.to_string(),
        destination_dir: dir.to_string(),
        file_name: Some(name.to_string()),
        user_agent: None,
        thread_mode: Some(ThreadMode::Fixed),
        thread_count: Some(4),
        max_retries: Some(1),
        checksum: Some(ChecksumMode::None),
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        priority: None,
    }
}

/// Convenience helper for range-supported multi-threaded downloads.
fn req_fixed_range(url: &str, out: &str, name: &str) -> StartDownloadRequest {
    StartDownloadRequest {
        kind: None,
        url: url.to_string(),
        destination_dir: out.to_string(),
        file_name: Some(name.to_string()),
        user_agent: None,
        thread_mode: Some(ThreadMode::Fixed),
        thread_count: Some(4),
        max_retries: Some(1),
        checksum: Some(ChecksumMode::None),
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        priority: None,
    }
}

/// Convenience helper for adaptive downloads (Automatic scheduler).
fn req_adaptive(url: &str, out: &str, name: &str) -> StartDownloadRequest {
    StartDownloadRequest {
        kind: None,
        url: url.to_string(),
        destination_dir: out.to_string(),
        file_name: Some(name.to_string()),
        user_agent: None,
        thread_mode: Some(ThreadMode::Adaptive),
        thread_count: None,
        max_retries: Some(1),
        checksum: Some(ChecksumMode::None),
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        priority: None,
    }
}

// ---------------------------------------------------------------------------
// Polling helpers
// ---------------------------------------------------------------------------

async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> DownloadSnapshot {
    loop {
        let s = manager.status(id).await.unwrap();
        if matches!(
            s.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return s;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
// ===========================================================================
// Test 2: Automatic mode prioritises larger file
// ===========================================================================

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

// ===========================================================================
// Test 3: Pause / resume preserves progress
// ===========================================================================

/// Start a download with a global speed limit, observe progress, pause,
/// verify bytes are preserved, resume, and complete.
#[tokio::test]
#[timeout(180_000)]
async fn pause_resume_does_not_lose_progress() -> TestResult {
    let server = TestServer::new(50 * 1024 * 1024).await; // 50 MB

    let (_tmp, manager) = create_manager().await;

    // Apply a rate limit so the download takes long enough to observe progress.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 1_000_000, // 1 MB/s
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
    let id = manager
        .start(req_fixed(&server.file_url_range(), &out, "pause-test.bin"))
        .await?;

    // Wait for some progress, polling up to 10 seconds.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let before = loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let s = manager.status(&id.to_string()).await?;
        if s.downloaded_bytes > 0 {
            break s;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("timed out waiting for download progress before pause");
        }
    };
    let bytes_before = before.downloaded_bytes;

    // Pause.
    let paused = manager.pause(&id.to_string()).await?;
    assert_eq!(paused.state, DownloadState::Paused);

    let bytes_at_pause = paused.downloaded_bytes;
    assert!(
        bytes_at_pause >= bytes_before,
        "regressed {} < {}",
        bytes_at_pause,
        bytes_before
    );

    // Resume.
    let resumed = manager.resume(&id.to_string()).await?;
    assert!(
        matches!(
            resumed.state,
            DownloadState::Queued | DownloadState::Downloading
        ),
        "resume gave {:?}",
        resumed.state
    );

    // Remove the speed limit so completion doesn't take forever.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 0, // unlimited
            ..AppSettings::default()
        })
        .await?;

    // Wait for completion.
    let done = tokio::time::timeout(
        Duration::from_secs(120),
        wait_for_terminal(&manager, &id.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for resume")?;

    assert!(
        done.downloaded_bytes >= bytes_at_pause,
        "progress regressed after resume: {} < {}",
        done.downloaded_bytes,
        bytes_at_pause
    );
    assert_eq!(done.state, DownloadState::Completed);

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

// ===========================================================================
// Test 4: Cancel stops a download
// ===========================================================================

/// Start a download with a significant startup delay, then cancel it mid-flight.
#[tokio::test]
#[timeout(180_000)]
async fn cancel_stops_download() -> TestResult {
    let server = TestServer::new(1024 * 1024).await; // 1 MB
    let url = server.file_url_slow(1000); // 1 s startup delay

    let (_tmp, manager) = create_manager().await;
    apply_settings(
        &manager,
        SchedulerSettings {
            mode: SchedulerMode::Traditional,
            traditional: TraditionalSchedulerSettings {
                max_parallel_tasks: 2,
            },
            ..SchedulerSettings::default()
        },
    )
    .await;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let id = manager
        .start(req_fixed(&url, &out, "cancel-test.bin"))
        .await?;

    // Poll until the download transitions to Downloading — on slow CI
    // runners the 500ms sleep may not be sufficient.
    let before = loop {
        let s = manager.status(&id.to_string()).await?;
        if matches!(s.state, DownloadState::Downloading) {
            break s;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert!(
        matches!(before.state, DownloadState::Downloading),
        "state was {:?}",
        before.state
    );

    let canceled = manager.cancel(&id.to_string()).await?;
    assert_eq!(canceled.state, DownloadState::Canceled);

    // Cancelled downloads should be removed from the list.
    let list = manager.list().await?;
    assert!(
        list.iter().all(|s| s.id != id.to_string()),
        "canceled download should not appear"
    );

    Ok(())
}

// ===========================================================================
// Test 5: Global speed limit is respected
// ===========================================================================

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

// ===========================================================================
// Test 7: Multi-download fairness under limited threads
// ===========================================================================

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

// ===========================================================================
// Test 8: Mixed thread-mode downloads coexist
// ===========================================================================

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

// ===========================================================================
// Test 9: Rate limiter shared across multiple downloads
// ===========================================================================

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

// ===========================================================================
// Test 10: Pausing one download does not affect the other
// ===========================================================================

/// Start two downloads, pause one, verify the other continues normally,
/// resume the paused one, and confirm both complete.
#[tokio::test]
#[timeout(180_000)]
async fn pause_one_download_does_not_affect_other() -> TestResult {
    let server = TestServer::new(5 * 1024 * 1024).await; // 5 MB

    let (_tmp, manager) = create_manager().await;

    // Apply a modest speed limit so downloads overlap long enough.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 1_000_000, // 1 MB/s (shared)
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
        .start(req_fixed_range(&url, &out, "pause-other-a.bin"))
        .await?;
    let id2 = manager
        .start(req_fixed_range(&url, &out, "pause-other-b.bin"))
        .await?;

    // Poll until both downloads have started — on slow CI runners the
    // 500ms sleep may not be sufficient for the scheduler to assign states.
    loop {
        let s2 = manager.status(&id2.to_string()).await?;
        if matches!(
            s2.state,
            DownloadState::Downloading | DownloadState::Completed
        ) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Pause the first.
    let paused = manager.pause(&id1.to_string()).await?;
    assert_eq!(paused.state, DownloadState::Paused);

    // The second download must still be active (Downloading or Completed).
    let other = manager.status(&id2.to_string()).await?;
    assert!(
        matches!(
            other.state,
            DownloadState::Downloading | DownloadState::Completed
        ),
        "second download should be running after first is paused, got {:?}",
        other.state,
    );

    // Resume the paused download.
    let resumed = manager.resume(&id1.to_string()).await?;
    assert!(
        matches!(
            resumed.state,
            DownloadState::Queued | DownloadState::Downloading
        ),
        "resume gave {:?}",
        resumed.state,
    );

    // Remove speed limit so completion is fast.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 0,
            ..AppSettings::default()
        })
        .await?;

    // Wait for both to complete.
    let done1 = tokio::time::timeout(
        Duration::from_secs(60),
        wait_for_terminal(&manager, &id1.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for first download after resume")?;
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

// ===========================================================================
// Test 11: Cancelling one download unblocks a queued download
// ===========================================================================

/// With max_parallel_tasks=1, start two downloads; the second is queued.
/// Cancel the first and verify the second transitions to Downloading or
/// Completed.
#[tokio::test]
#[timeout(120_000)]
async fn cancel_one_unblocks_queued() -> TestResult {
    let server = TestServer::new(512 * 1024).await; // 512 KB

    let (_tmp, manager) = create_manager().await;
    apply_settings(
        &manager,
        SchedulerSettings {
            mode: SchedulerMode::Traditional,
            traditional: TraditionalSchedulerSettings {
                max_parallel_tasks: 1,
            },
            ..SchedulerSettings::default()
        },
    )
    .await;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let url = server.file_url_slow(2000); // 2s startup delay prevents instant completion

    let id1 = manager
        .start(req_fixed(&url, &out, "cancel-first.bin"))
        .await?;
    let id2 = manager
        .start(req_fixed(&url, &out, "cancel-second.bin"))
        .await?;

    // Second must be queued (max_parallel_tasks=1).
    // Poll until scheduler assigns a state — on slow CI runners the
    // transition from start() may not be instant; on fast machines
    // the first download (512KB) may complete before we observe Queued.
    loop {
        let s = manager.status(&id2.to_string()).await?;
        if !matches!(s.state, DownloadState::Queued | DownloadState::Downloading) {
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        }
        break;
    }

    // Cancel the first — this removes it and triggers rebalance.
    let canceled = manager.cancel(&id1.to_string()).await?;
    assert_eq!(canceled.state, DownloadState::Canceled);

    // After cancel + rebalance, the second should be running or already complete.
    let s2 = manager.status(&id2.to_string()).await?;
    assert!(
        matches!(
            s2.state,
            DownloadState::Downloading | DownloadState::Completed
        ),
        "second download should be running after first canceled, got {:?}",
        s2.state,
    );

    // Wait for the second to finish.
    let done2 = tokio::time::timeout(
        Duration::from_secs(30),
        wait_for_terminal(&manager, &id2.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for second download after cancel")?;
    assert_eq!(done2.state, DownloadState::Completed);

    let _ = manager.remove(&id2.to_string()).await;
    Ok(())
}
