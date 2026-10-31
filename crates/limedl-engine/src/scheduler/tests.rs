use crate::download::{DEFAULT_FIXED_THREADS, MAX_TRADITIONAL_THREADS};
use crate::manifest::Manifest;
use crate::types::{
    AppSettings, AutomaticSchedulerSettings, DownloadState, Priority, SchedulerMode,
    SchedulerSettings, SpeedLimitSlot, ThreadMode, TraditionalSchedulerSettings,
};

// ── helpers ───────────────────────────────────────────────────────────

fn manifest(url: &str) -> Manifest {
    Manifest {
        id: String::new(),
        url: url.to_string(),
        final_url: url.to_string(),
        user_agent: String::new(),
        extra_headers: vec![],
        destination_dir: String::new(),
        file_name: String::new(),
        file_name_locked: false,
        destination_path: String::new(),
        temp_path: String::new(),
        total_bytes: None,
        downloaded_bytes: 0,
        supports_ranges: true,
        chunk_size: 4194304,
        connection_count: 0,
        thread_mode: ThreadMode::Adaptive,
        requested_thread_count: None,
        desired_thread_count: None,
        allocated_thread_count: None,
        adaptive_profile_snapshot: None,
        thread_note: None,
        etag: None,
        last_modified: None,
        state: DownloadState::Queued,
        cdn_accelerated: false,
        cdn_node_ip: None,
        checksum_mode: crate::types::ChecksumMode::None,
        checksum: None,
        expected_checksum: None,
        error: None,
        created_at_ms: 0,
        updated_at_ms: 0,
        chunks: vec![],
        mirror_url: None,
        mirror_urls: Vec::new(),
        current_mirror_index: 0,
        priority: Priority::Normal,
    }
}

fn settings_traditional() -> AppSettings {
    AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Traditional,
            traditional: TraditionalSchedulerSettings {
                max_parallel_tasks: 3,
            },
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    }
}

fn settings_automatic() -> AppSettings {
    AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    }
}

// ── effective_allocation_cap ──────────────────────────────────────────

#[test]
fn cap_no_ranges_returns_one() {
    let mut m = manifest("https://example.com/file.bin");
    m.supports_ranges = false;
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        1
    );
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_automatic()),
        1
    );
}

#[test]
fn cap_traditional_uses_requested_threads() {
    let mut m = manifest("https://example.com/file.bin");
    m.requested_thread_count = Some(12);
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        12
    );
}

#[test]
fn cap_traditional_falls_back_to_desired() {
    let mut m = manifest("https://example.com/file.bin");
    m.requested_thread_count = None;
    m.desired_thread_count = Some(6);
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        6
    );
}

#[test]
fn cap_traditional_default_when_none_set() {
    let m = manifest("https://example.com/file.bin");
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        DEFAULT_FIXED_THREADS
    );
}

#[test]
fn cap_traditional_clamps_to_max() {
    let mut m = manifest("https://example.com/file.bin");
    m.requested_thread_count = Some(99);
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        MAX_TRADITIONAL_THREADS
    );
}

#[test]
fn cap_traditional_clamps_to_min() {
    let mut m = manifest("https://example.com/file.bin");
    m.requested_thread_count = Some(0);
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        1
    );
}

#[test]
fn cap_automatic_fixed_uses_requested() {
    let mut m = manifest("https://example.com/file.bin");
    m.thread_mode = ThreadMode::Fixed;
    m.requested_thread_count = Some(5);
    let s = settings_automatic();
    assert_eq!(super::effective_allocation_cap(&m, &s), 5);
}

#[test]
fn cap_automatic_fixed_defaults_to_one() {
    let mut m = manifest("https://example.com/file.bin");
    m.thread_mode = ThreadMode::Fixed;
    m.requested_thread_count = None;
    let s = settings_automatic();
    assert_eq!(super::effective_allocation_cap(&m, &s), 1);
}

#[test]
fn cap_automatic_adaptive_uses_desired() {
    let mut m = manifest("https://example.com/file.bin");
    m.thread_mode = ThreadMode::Adaptive;
    m.desired_thread_count = Some(4);
    let s = settings_automatic();
    assert_eq!(super::effective_allocation_cap(&m, &s), 4);
}

#[test]
fn cap_automatic_adaptive_defaults_to_one() {
    let mut m = manifest("https://example.com/file.bin");
    m.thread_mode = ThreadMode::Adaptive;
    m.desired_thread_count = None;
    let s = settings_automatic();
    assert_eq!(super::effective_allocation_cap(&m, &s), 1);
}

#[test]
fn cap_automatic_clamps_to_effective_task_cap() {
    let mut m = manifest("https://example.com/file.bin");
    m.thread_mode = ThreadMode::Adaptive;
    m.desired_thread_count = Some(99);
    let s = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_threads_per_task: 3,
                ..AutomaticSchedulerSettings::default()
            },
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    };
    assert_eq!(super::effective_allocation_cap(&m, &s), 3);
}

// ── hostname_from_manifest ────────────────────────────────────────────

#[test]
fn hostname_valid_url_returns_host() {
    let m = manifest("https://cdn.example.com/path/file.zip");
    assert_eq!(
        super::hostname_from_manifest(&m),
        Some("cdn.example.com".to_string())
    );
}

#[test]
fn hostname_domain_with_port() {
    let m = manifest("http://localhost:8080/download/file.iso");
    assert_eq!(
        super::hostname_from_manifest(&m),
        Some("localhost".to_string())
    );
}

#[test]
fn hostname_no_host_returns_none() {
    let m = manifest("file:///C:/path/to/file.txt");
    // file:// URIs may have no host_str
    assert_eq!(super::hostname_from_manifest(&m), None);
}

#[test]
fn hostname_invalid_url_does_not_panic() {
    // Completely unparseable
    let m = manifest("\0invalid url\t\n");
    let result = super::hostname_from_manifest(&m);
    assert!(result.is_none());
}

#[test]
fn hostname_empty_string_does_not_panic() {
    let m = manifest("");
    let result = super::hostname_from_manifest(&m);
    assert!(result.is_none());
}

// ── automatic rebalance: stickiness, monotonic keys and rotation ──────

/// A single active-ranged task with an explicit size, creation time and
/// progress, used by the automatic allocator tests below.
fn rebalance_task(
    id: &str,
    total: u64,
    created_at: u64,
    downloaded: u64,
) -> std::sync::Arc<crate::download::ManagedDownload> {
    let mut m = manifest("https://example.com/file.bin");
    m.id = id.to_string();
    m.state = DownloadState::Downloading;
    m.supports_ranges = true;
    m.thread_mode = ThreadMode::Adaptive;
    m.desired_thread_count = Some(4);
    m.total_bytes = Some(total);
    m.downloaded_bytes = downloaded;
    m.created_at_ms = created_at;
    let s = crate::manifest::snapshot_from_manifest(&m);
    std::sync::Arc::new(crate::download::ManagedDownload {
        core: parking_lot::Mutex::new(crate::download::DownloadCore::new(s, m)),
        runtime: parking_lot::Mutex::new(None),
        aimd: parking_lot::Mutex::new(crate::aimd::AimdState::default()),
        stop_notify: tokio::sync::Notify::new(),
        pieces: parking_lot::Mutex::new(None),
    })
}

fn active_ids(tasks: &[std::sync::Arc<crate::download::ManagedDownload>]) -> Vec<String> {
    tasks
        .iter()
        .filter(|m| m.lock_core().manifest.allocated_thread_count.unwrap_or(0) > 0)
        .map(|m| m.lock_core().manifest.id.clone())
        .collect()
}

fn automatic_settings(budget: usize, min: usize, cap: usize) -> AppSettings {
    AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_parallel_threads: budget,
                max_threads_per_task: cap,
                min_threads_per_task: min,
                adaptive_profile: Default::default(),
            },
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    }
}

fn rebalance_auto(
    candidates: &[std::sync::Arc<crate::download::ManagedDownload>],
    all_downloads: &[std::sync::Arc<crate::download::ManagedDownload>],
    settings: &AppSettings,
    state: &mut super::AutomaticRebalanceState,
) {
    super::rebalance_automatic(
        candidates,
        all_downloads,
        settings,
        state,
        &foldhash::HashMap::default(),
        4,
    );
}

/// The original bug: the active set was re-derived from remaining bytes every
/// pass, so as the running task made progress the queued tasks overtook it and
/// the active set flipped on every rebalance.  With static keys plus stickiness
/// the active set must survive both progress and repeated passes.
#[test]
fn automatic_active_set_is_sticky_across_progress() {
    let tasks: Vec<_> = (0..12)
        .map(|i| rebalance_task(&format!("t{i:02}"), 100_000_000, i, 0))
        .collect();
    let settings = automatic_settings(4, 1, 4);
    let mut state = super::AutomaticRebalanceState::default();

    let mut first_active: Option<Vec<String>> = None;
    for pass in 0..10 {
        // Simulate a tick of progress on whichever tasks are running: the old
        // ordering used exactly this value as its primary key.
        for task in &tasks {
            let mut core = task.lock_core();
            if core.manifest.allocated_thread_count.unwrap_or(0) > 0 {
                core.manifest.downloaded_bytes += 5_000_000;
            }
        }
        rebalance_auto(&tasks, &tasks, &settings, &mut state);
        let active = active_ids(&tasks);
        if let Some(previous) = &first_active {
            assert_eq!(previous, &active, "active set changed at pass {pass}");
        } else {
            first_active = Some(active);
        }
    }
}

/// Rotation must eventually give every starved task a turn instead of letting
/// the top-ranked tasks hold the budget forever.
#[test]
fn automatic_rotation_gives_every_waiting_task_a_turn() {
    let tasks: Vec<_> = (0..4)
        .map(|i| rebalance_task(&format!("t{i}"), 1000, i, 0))
        .collect();
    let settings = automatic_settings(1, 1, 1);
    let mut state = super::AutomaticRebalanceState::default();

    let mut seen = std::collections::HashSet::new();
    for _ in 0..(4 * super::AUTOMATIC_ROTATION_PASSES + 4) {
        rebalance_auto(&tasks, &tasks, &settings, &mut state);
        if let Some(id) = active_ids(&tasks).into_iter().next() {
            seen.insert(id);
        }
    }

    assert_eq!(
        seen.len(),
        4,
        "every task must get a rotation turn, saw {seen:?}"
    );
}

/// A newly added high-priority task immediately preempts the lowest-priority
/// incumbent, without waiting for starvation rotation.
#[test]
fn automatic_priority_preempts_an_incumbent() {
    let low = rebalance_task("low", 1000, 0, 0);
    let settings = automatic_settings(1, 1, 1);
    let mut state = super::AutomaticRebalanceState::default();

    // Only the normal-priority task exists; it takes the single slot.
    rebalance_auto(
        std::slice::from_ref(&low),
        std::slice::from_ref(&low),
        &settings,
        &mut state,
    );
    assert_eq!(
        active_ids(std::slice::from_ref(&low)),
        vec![String::from("low")]
    );

    // A high-priority task is added: priority is static, so it preempts at once.
    let high = rebalance_task("high", 1000, 1, 0);
    high.lock_core().manifest.priority = Priority::High;
    let tasks = vec![low, high];
    rebalance_auto(&tasks, &tasks, &settings, &mut state);
    assert_eq!(active_ids(&tasks), vec![String::from("high")]);
}

/// Rotation is bounded: with 12 tasks and a 4-thread budget the active set may
/// change at most once per `AUTOMATIC_ROTATION_PASSES` window.
#[test]
fn automatic_rotation_is_bounded() {
    let tasks: Vec<_> = (0..12)
        .map(|i| rebalance_task(&format!("t{i:02}"), 1000, i, 0))
        .collect();
    let settings = automatic_settings(4, 1, 4);
    let mut state = super::AutomaticRebalanceState::default();

    // Warm up so the initial empty -> active transition is not counted.
    rebalance_auto(&tasks, &tasks, &settings, &mut state);
    let mut previous = active_ids(&tasks);
    let mut changes = 0usize;
    for _ in 0..(2 * super::AUTOMATIC_ROTATION_PASSES) {
        rebalance_auto(&tasks, &tasks, &settings, &mut state);
        let active = active_ids(&tasks);
        if active != previous {
            changes += 1;
            previous = active;
        }
    }

    assert!(
        changes <= 2,
        "active set changed {changes} times in {} passes",
        2 * super::AUTOMATIC_ROTATION_PASSES
    );
}

/// A task that changes only its downloaded bytes must not move in the queue.
#[test]
fn automatic_sort_key_ignores_download_progress() {
    let settings = settings_automatic();
    let task_a = rebalance_task("a", 1000, 0, 900);
    let task_b = rebalance_task("b", 1000, 1, 0);
    let task_c = rebalance_task("c", 2000, 2, 1900);

    // Largest total first, then creation order — never remaining bytes.
    let sorted = super::sort_automatic_candidates(
        &[task_a.clone(), task_b.clone(), task_c.clone()],
        &settings,
    );
    let ids: Vec<String> = sorted
        .iter()
        .map(|m| m.lock_core().manifest.id.clone())
        .collect();
    assert_eq!(ids, vec!["c", "a", "b"]);

    // Progress on any task must not change the order.
    task_a.lock_core().manifest.downloaded_bytes = 1000;
    task_c.lock_core().manifest.downloaded_bytes = 0;
    let sorted = super::sort_automatic_candidates(&[task_a, task_b, task_c], &settings);
    let ids: Vec<String> = sorted
        .iter()
        .map(|m| m.lock_core().manifest.id.clone())
        .collect();
    assert_eq!(ids, vec!["c", "a", "b"]);
}

// ── effective_automatic_task_cap ──────────────────────────────────────

#[test]
fn task_cap_normal() {
    let s = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_threads_per_task: 8,
                ..AutomaticSchedulerSettings::default()
            },
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    };
    assert_eq!(super::effective_automatic_task_cap(&s), 8);
}

#[test]
fn task_cap_zero_clamps_to_one() {
    let s = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_threads_per_task: 0,
                ..AutomaticSchedulerSettings::default()
            },
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    };
    assert_eq!(super::effective_automatic_task_cap(&s), 1);
}

#[test]
fn task_cap_default_is_at_least_one() {
    let s = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    };
    // Default max_threads_per_task is 8, so cap is 8
    assert_eq!(super::effective_automatic_task_cap(&s), 8);
}

// ── AIMD anti-deadlock & anti-cascade tests ───────────────────────────

fn make_managed_task(
    desired: usize,
    allocated: usize,
    profile: crate::types::AdaptiveProfile,
) -> std::sync::Arc<crate::download::ManagedDownload> {
    let mut m = manifest("https://example.com/file.bin");
    m.state = DownloadState::Downloading;
    m.supports_ranges = true;
    m.thread_mode = ThreadMode::Adaptive;
    m.desired_thread_count = Some(desired);
    m.allocated_thread_count = Some(allocated);
    m.adaptive_profile_snapshot = Some(profile);
    let s = crate::manifest::snapshot_from_manifest(&m);
    std::sync::Arc::new(crate::download::ManagedDownload {
        core: parking_lot::Mutex::new(crate::download::DownloadCore::new(s, m)),
        runtime: parking_lot::Mutex::new(None),
        aimd: parking_lot::Mutex::new(crate::aimd::AimdState::default()),
        stop_notify: tokio::sync::Notify::new(),
        pieces: parking_lot::Mutex::new(None),
    })
}

#[test]
fn aimd_deadlock_broken_by_proactive_probing() {
    use std::time::{Duration, Instant};

    let managed = make_managed_task(1, 1, crate::types::AdaptiveProfile::Balanced);
    let settings = settings_automatic();
    let adaptive_cap = 8;
    let min_threads = 1;

    // Cycle 1: establish initial steady throughput (1000.0 B/s)
    {
        let mut aimd = managed.lock_aimd();
        aimd.last_throughput = Some(1000.0);
        aimd.last_sample_at = Some(Instant::now() - Duration::from_secs(2));
        aimd.last_sample_bytes = 0;
        let mut core = managed.lock_core();
        core.manifest.downloaded_bytes = 2000; // 1000 B/s steady
    }
    super::update_one_adaptive(&managed, &settings, adaptive_cap, min_threads);
    {
        let core = managed.lock_core();
        assert_eq!(core.manifest.desired_thread_count, Some(1));
        let aimd = managed.lock_aimd();
        assert_eq!(aimd.stable_cycles, 1);
    }

    // Cycle 2: second steady cycle reaches probe_stable_cycles = 2 -> triggers proactive probe!
    {
        let mut aimd = managed.lock_aimd();
        aimd.last_sample_at = Some(Instant::now() - Duration::from_secs(2));
        aimd.last_sample_bytes = 2000;
        let mut core = managed.lock_core();
        core.manifest.downloaded_bytes = 4000; // 1000 B/s steady (no spontaneous speedup)
    }
    super::update_one_adaptive(&managed, &settings, adaptive_cap, min_threads);
    {
        let core = managed.lock_core();
        assert_eq!(
            core.manifest.desired_thread_count,
            Some(2),
            "should probe up to 2 threads"
        );
        let aimd = managed.lock_aimd();
        assert!(aimd.is_probing_up);
        assert_eq!(aimd.probe_pre_threads, 1);
        assert!(aimd.settling_until.is_some());
    }
}

#[test]
fn aimd_probe_rollback_when_no_gain() {
    use std::time::{Duration, Instant};

    let managed = make_managed_task(2, 2, crate::types::AdaptiveProfile::Balanced);
    let settings = settings_automatic();
    let adaptive_cap = 8;
    let min_threads = 1;

    // Simulate probe currently in progress with pre-probe throughput 1000.0 B/s
    {
        let mut aimd = managed.lock_aimd();
        aimd.is_probing_up = true;
        aimd.probe_pre_throughput = Some(1000.0);
        aimd.probe_pre_threads = 1;
        aimd.settling_until = None; // settling completed
        aimd.last_sample_at = Some(Instant::now() - Duration::from_secs(2));
        aimd.last_sample_bytes = 1000;
        let mut core = managed.lock_core();
        core.manifest.downloaded_bytes = 3000; // 1000 B/s, no gain over pre-probe 1000!
    }

    super::update_one_adaptive(&managed, &settings, adaptive_cap, min_threads);

    {
        let core = managed.lock_core();
        assert_eq!(
            core.manifest.desired_thread_count,
            Some(1),
            "should rollback to 1 thread"
        );
        let aimd = managed.lock_aimd();
        assert!(!aimd.is_probing_up);
        assert!(
            aimd.cooldown_until.is_some(),
            "should enter cooldown after failed probe"
        );
    }
}

#[test]
fn aimd_probe_success_retains_increase() {
    use std::time::{Duration, Instant};

    let managed = make_managed_task(2, 2, crate::types::AdaptiveProfile::Balanced);
    let settings = settings_automatic();
    let adaptive_cap = 8;
    let min_threads = 1;

    // Simulate probe currently in progress with pre-probe throughput 1000.0 B/s
    {
        let mut aimd = managed.lock_aimd();
        aimd.is_probing_up = true;
        aimd.probe_pre_throughput = Some(1000.0);
        aimd.probe_pre_threads = 1;
        aimd.settling_until = None; // settling completed
        aimd.last_sample_at = Some(Instant::now() - Duration::from_secs(2));
        aimd.last_sample_bytes = 1000;
        let mut core = managed.lock_core();
        core.manifest.downloaded_bytes = 4600; // 1800 B/s -> 80% gain over 1000!
    }

    super::update_one_adaptive(&managed, &settings, adaptive_cap, min_threads);

    {
        let core = managed.lock_core();
        assert_eq!(
            core.manifest.desired_thread_count,
            Some(2),
            "should retain 2 threads"
        );
        let aimd = managed.lock_aimd();
        assert!(!aimd.is_probing_up);
        assert!(
            aimd.last_throughput
                .is_some_and(|t| (t - 1800.0).abs() < 2.0)
        );
    }
}

#[test]
fn aimd_no_cascade_after_decrease() {
    use std::time::{Duration, Instant};

    let managed = make_managed_task(6, 6, crate::types::AdaptiveProfile::Balanced);
    let settings = settings_automatic();
    let adaptive_cap = 8;
    let min_threads = 1;

    // Trigger initial severe drop: 1000 B/s to 100 B/s
    {
        let mut aimd = managed.lock_aimd();
        aimd.last_throughput = Some(1000.0);
        aimd.last_sample_at = Some(Instant::now() - Duration::from_secs(2));
        aimd.last_sample_bytes = 0;
        let mut core = managed.lock_core();
        core.manifest.downloaded_bytes = 200; // 100 B/s
    }

    super::update_one_adaptive(&managed, &settings, adaptive_cap, min_threads);

    {
        let core = managed.lock_core();
        assert_eq!(
            core.manifest.desired_thread_count,
            Some(3),
            "should decrease from 6 to 3"
        );
        let aimd = managed.lock_aimd();
        assert!(
            aimd.last_throughput.is_none(),
            "baseline must be cleared to prevent cascade"
        );
        assert!(
            aimd.settling_until.is_some(),
            "settling period must be active"
        );
    }

    // Now rebalance takes effect: allocated becomes 3
    {
        let mut core = managed.lock_core();
        core.manifest.allocated_thread_count = Some(3);
        core.manifest.downloaded_bytes = 1200; // 500 B/s under 3 threads
    }

    // During settling / cooldown, no cascading decrease occurs!
    super::update_one_adaptive(&managed, &settings, adaptive_cap, min_threads);
    {
        let core = managed.lock_core();
        assert_eq!(
            core.manifest.desired_thread_count,
            Some(3),
            "must not cascade to 2 or 1"
        );
    }
}

#[test]
fn test_expert_queue_sort_strategies() {
    use crate::types::QueueSortStrategy;

    let make_task = |id: &str, priority: Priority, total: u64, downloaded: u64, created_at: u64| {
        let mut m = manifest("https://example.com/test.bin");
        m.id = id.to_string();
        m.priority = priority;
        m.total_bytes = Some(total);
        m.downloaded_bytes = downloaded;
        m.created_at_ms = created_at;
        let s = crate::manifest::snapshot_from_manifest(&m);
        std::sync::Arc::new(crate::download::ManagedDownload {
            core: parking_lot::Mutex::new(crate::download::DownloadCore::new(s, m)),
            runtime: parking_lot::Mutex::new(None),
            aimd: parking_lot::Mutex::new(crate::aimd::AimdState::default()),
            stop_notify: tokio::sync::Notify::new(),
            pieces: parking_lot::Mutex::new(None),
        })
    };

    // Task A: High priority, 100MB remaining, created at 300ms
    let task_a = make_task("task_a", Priority::High, 100 * 1024 * 1024, 0, 300);
    // Task B: Normal priority, 10MB remaining, created at 200ms
    let task_b = make_task(
        "task_b",
        Priority::Normal,
        20 * 1024 * 1024,
        10 * 1024 * 1024,
        200,
    );
    // Task C: Low priority, 5MB remaining, created at 100ms
    let task_c = make_task(
        "task_c",
        Priority::Low,
        10 * 1024 * 1024,
        5 * 1024 * 1024,
        100,
    );

    // 1. Priority First (default)
    {
        let candidates = vec![task_b.clone(), task_a.clone(), task_c.clone()];
        let mut settings = settings_automatic();
        settings.scheduler.expert_mode = true;
        settings.scheduler.expert.queue_sort_strategy = QueueSortStrategy::PriorityFirst;
        let sorted = super::sort_automatic_candidates(&candidates, &settings);
        assert_eq!(
            sorted[0].lock_core().manifest.id,
            "task_a",
            "High priority first"
        );
        assert_eq!(
            sorted[1].lock_core().manifest.id,
            "task_b",
            "Normal priority second"
        );
        assert_eq!(
            sorted[2].lock_core().manifest.id,
            "task_c",
            "Low priority third"
        );
    }

    // 2. Shortest Job First
    {
        let candidates = vec![task_a.clone(), task_b.clone(), task_c.clone()];
        let mut settings = settings_automatic();
        settings.scheduler.expert_mode = true;
        settings.scheduler.expert.queue_sort_strategy = QueueSortStrategy::ShortestJobFirst;
        let sorted = super::sort_automatic_candidates(&candidates, &settings);
        assert_eq!(
            sorted[0].lock_core().manifest.id,
            "task_c",
            "5MB remaining first"
        );
        assert_eq!(
            sorted[1].lock_core().manifest.id,
            "task_b",
            "10MB remaining second"
        );
        assert_eq!(
            sorted[2].lock_core().manifest.id,
            "task_a",
            "100MB remaining third"
        );
    }

    // 3. FIFO
    {
        let candidates = vec![task_a.clone(), task_b.clone(), task_c.clone()];
        let mut settings = settings_automatic();
        settings.scheduler.expert_mode = true;
        settings.scheduler.expert.queue_sort_strategy = QueueSortStrategy::Fifo;
        let sorted = super::sort_automatic_candidates(&candidates, &settings);
        assert_eq!(
            sorted[0].lock_core().manifest.id,
            "task_c",
            "Created at 100ms first"
        );
        assert_eq!(
            sorted[1].lock_core().manifest.id,
            "task_b",
            "Created at 200ms second"
        );
        assert_eq!(
            sorted[2].lock_core().manifest.id,
            "task_a",
            "Created at 300ms third"
        );
    }
}

#[test]
fn test_expert_host_cap() {
    let mut settings = settings_automatic();
    assert_eq!(
        super::effective_max_connections_per_host(&settings),
        super::MAX_CONNECTIONS_PER_HOST
    );

    settings.scheduler.expert_mode = true;
    settings.scheduler.expert.max_connections_per_host = 32;
    assert_eq!(super::effective_max_connections_per_host(&settings), 32);

    settings.scheduler.expert.max_connections_per_host = 2;
    assert_eq!(super::effective_max_connections_per_host(&settings), 2);

    let mut tracker = super::HostCapTracker::new(2);
    let m = manifest("https://example.com/file.bin");
    assert_eq!(tracker.grant(&m, 4), 2, "must be capped at max_per_host");
    assert_eq!(
        tracker.grant(&m, 4),
        0,
        "no capacity remaining for this host"
    );
}

#[test]
fn test_expert_aimd_decrease_factor() {
    use std::time::{Duration, Instant};

    let managed = make_managed_task(10, 10, crate::types::AdaptiveProfile::Balanced);
    let mut settings = settings_automatic();
    settings.scheduler.expert_mode = true;
    settings.scheduler.expert.aimd_decrease_factor = 0.85; // LAN Ultra preset: 15% reduction
    settings.scheduler.expert.aimd_degrade_threshold = 0.90;
    settings.scheduler.expert.aimd_samples_needed = 1;
    settings.scheduler.expert.aimd_cooldown_secs = 1;

    {
        let mut aimd = managed.lock_aimd();
        aimd.last_throughput = Some(1000.0);
        aimd.last_sample_at = Some(Instant::now() - Duration::from_secs(2));
        aimd.last_sample_bytes = 0;
        let mut core = managed.lock_core();
        // Degrade to 850 B/s (below 90% threshold 900 B/s, but not severe drop)
        core.manifest.downloaded_bytes = 1700;
    }

    super::update_one_adaptive(&managed, &settings, 16, 1);

    let core = managed.lock_core();
    // 10 * 0.85 = 8.5 -> ceil = 9
    assert_eq!(core.manifest.desired_thread_count, Some(9));
}

#[test]
fn test_rebalance_automatic_caps_hdd_parallel_tasks() {
    use crate::types::DiskType;
    use foldhash::HashMap;

    // 6 tasks, all on HDD, max_parallel_hdd = 3, total thread budget = 32
    let tasks: Vec<_> = (0..6)
        .map(|i| rebalance_task(&format!("hdd_{i}"), 1000, i, 0))
        .collect();
    let mut disk_types = HashMap::default();
    for t in &tasks {
        disk_types.insert(t.lock_core().manifest.id.clone(), DiskType::Hdd);
    }

    let settings = automatic_settings(32, 2, 8);
    let mut state = super::AutomaticRebalanceState::default();

    super::rebalance_automatic(&tasks, &tasks, &settings, &mut state, &disk_types, 3);

    let active = active_ids(&tasks);
    assert_eq!(
        active.len(),
        3,
        "only 3 HDD tasks should be active when max_parallel_hdd = 3, got: {active:?}"
    );

    // Verify the other 3 are queued
    for task in &tasks[3..] {
        let core = task.lock_core();
        assert_eq!(core.manifest.state, DownloadState::Queued);
        assert_eq!(core.manifest.allocated_thread_count, Some(0));
    }
}

#[test]
fn test_rebalance_automatic_allows_ssd_when_hdd_quota_full() {
    use crate::types::DiskType;
    use foldhash::HashMap;

    // 4 HDD tasks + 2 SSD tasks, max_parallel_hdd = 2, total thread budget = 32
    let hdd_tasks: Vec<_> = (0..4)
        .map(|i| rebalance_task(&format!("hdd_{i}"), 1000, i, 0))
        .collect();
    let ssd_tasks: Vec<_> = (0..2)
        .map(|i| rebalance_task(&format!("ssd_{i}"), 1000, i + 10, 0))
        .collect();

    let mut disk_types = HashMap::default();
    for t in &hdd_tasks {
        disk_types.insert(t.lock_core().manifest.id.clone(), DiskType::Hdd);
    }
    for t in &ssd_tasks {
        disk_types.insert(t.lock_core().manifest.id.clone(), DiskType::Ssd);
    }

    let mut all_tasks = Vec::new();
    all_tasks.extend(hdd_tasks.clone());
    all_tasks.extend(ssd_tasks.clone());

    let settings = automatic_settings(32, 2, 8);
    let mut state = super::AutomaticRebalanceState::default();

    super::rebalance_automatic(
        &all_tasks,
        &all_tasks,
        &settings,
        &mut state,
        &disk_types,
        2,
    );

    let active = active_ids(&all_tasks);
    // 2 HDD tasks + 2 SSD tasks = 4 active
    assert_eq!(
        active.len(),
        4,
        "expected 2 HDD + 2 SSD active, got: {active:?}"
    );
    assert!(active.contains(&"hdd_0".to_string()));
    assert!(active.contains(&"hdd_1".to_string()));
    assert!(active.contains(&"ssd_0".to_string()));
    assert!(active.contains(&"ssd_1".to_string()));
    assert!(!active.contains(&"hdd_2".to_string()));
    assert!(!active.contains(&"hdd_3".to_string()));
}

#[test]
fn test_rebalance_traditional_caps_hdd_parallel_tasks() {
    use crate::types::DiskType;
    use foldhash::HashMap;

    // Traditional mode: max_parallel_tasks = 5, max_parallel_hdd = 2
    let mut settings = settings_traditional();
    settings.scheduler.traditional.max_parallel_tasks = 5;

    let hdd_tasks: Vec<_> = (0..4)
        .map(|i| rebalance_task(&format!("hdd_{i}"), 1000, i, 0))
        .collect();
    let ssd_tasks: Vec<_> = (0..2)
        .map(|i| rebalance_task(&format!("ssd_{i}"), 1000, i + 10, 0))
        .collect();

    let mut disk_types = HashMap::default();
    for t in &hdd_tasks {
        disk_types.insert(t.lock_core().manifest.id.clone(), DiskType::Hdd);
    }
    for t in &ssd_tasks {
        disk_types.insert(t.lock_core().manifest.id.clone(), DiskType::Ssd);
    }

    let mut all_tasks = Vec::new();
    all_tasks.extend(hdd_tasks.clone());
    all_tasks.extend(ssd_tasks.clone());

    super::rebalance_traditional(&mut all_tasks, &settings, &disk_types, 2);

    let active = active_ids(&all_tasks);
    // 2 HDD + 2 SSD = 4 active (< 5 max_parallel_tasks)
    assert_eq!(
        active.len(),
        4,
        "expected 2 HDD + 2 SSD active, got: {active:?}"
    );
    assert!(active.contains(&"hdd_0".to_string()));
    assert!(active.contains(&"hdd_1".to_string()));
    assert!(active.contains(&"ssd_0".to_string()));
    assert!(active.contains(&"ssd_1".to_string()));
}

#[test]
fn scheduler_idle_flag_initially_active() {
    let scheduler = super::Scheduler::new();
    assert!(
        scheduler
            .has_active_tasks
            .load(std::sync::atomic::Ordering::Acquire)
    );
}

// ── speed limit schedule ─────────────────────────────────────────────

fn slot(start_hour: u8, end_hour: u8, limit_bps: u64) -> SpeedLimitSlot {
    SpeedLimitSlot {
        start_hour,
        end_hour,
        limit_bps,
    }
}

#[test]
fn speed_limit_schedule_empty_has_no_limit() {
    assert_eq!(super::matching_speed_limit(&[], 0), None);
    assert_eq!(super::matching_speed_limit(&[], 12), None);
}

#[test]
fn speed_limit_schedule_uses_half_open_ranges() {
    let schedule = [slot(8, 18, 1_000_000)];

    assert_eq!(super::matching_speed_limit(&schedule, 7), None);
    assert_eq!(super::matching_speed_limit(&schedule, 8), Some(1_000_000));
    assert_eq!(super::matching_speed_limit(&schedule, 17), Some(1_000_000));
    assert_eq!(super::matching_speed_limit(&schedule, 18), None);
}

#[test]
fn speed_limit_schedule_wraps_midnight() {
    let schedule = [slot(22, 6, 500_000)];

    assert_eq!(super::matching_speed_limit(&schedule, 21), None);
    assert_eq!(super::matching_speed_limit(&schedule, 22), Some(500_000));
    assert_eq!(super::matching_speed_limit(&schedule, 23), Some(500_000));
    assert_eq!(super::matching_speed_limit(&schedule, 0), Some(500_000));
    assert_eq!(super::matching_speed_limit(&schedule, 5), Some(500_000));
    assert_eq!(super::matching_speed_limit(&schedule, 6), None);
}

#[test]
fn speed_limit_schedule_first_match_wins() {
    let schedule = [slot(8, 12, 100), slot(10, 14, 200), slot(14, 18, 300)];

    assert_eq!(super::matching_speed_limit(&schedule, 9), Some(100));
    assert_eq!(super::matching_speed_limit(&schedule, 12), Some(200));
    assert_eq!(super::matching_speed_limit(&schedule, 15), Some(300));
}

#[test]
fn speed_limit_schedule_zero_limit_is_meaningful() {
    let schedule = [slot(0, 12, 0)];
    assert_eq!(super::matching_speed_limit(&schedule, 6), Some(0));
}
