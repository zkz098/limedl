use std::collections::HashMap;

use parking_lot::Mutex as ParkingMutex;
use tokio::sync::Notify;

use super::*;
use crate::{
    aimd::AimdState,
    error::DownloadError,
    download::{DownloadCore, ManagedDownload},
    manifest::Manifest,
    types::*,
};

// ── Test helpers ──────────────────────────────────────────────────

fn make_managed(id: &str, state: DownloadState, created_at_ms: u64) -> ManagedDownload {
    ManagedDownload {
        core: ParkingMutex::new(DownloadCore {
            snapshot: DownloadSnapshot {
                id: id.to_string(),
                kind: TaskKind::Http,
                state,
                url: format!("https://example.com/{id}"),
                final_url: format!("https://cdn.example.com/{id}"),
                file_name: format!("{id}.bin"),
                destination_path: format!("/tmp/dst/{id}.bin"),
                temp_path: format!("/tmp/temp/{id}.part"),
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
                created_at_ms,
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
                url: format!("https://example.com/{id}"),
                final_url: format!("https://cdn.example.com/{id}"),
                user_agent: String::new(),
                extra_headers: vec![],
                destination_dir: String::new(),
                file_name: format!("{id}.bin"),
                file_name_locked: false,
                destination_path: format!("/tmp/dst/{id}.bin"),
                temp_path: format!("/tmp/temp/{id}.part"),
                total_bytes: None,
                downloaded_bytes: 0,
                supports_ranges: false,
                chunk_size: 4 * 1024 * 1024,
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
                created_at_ms,
                updated_at_ms: 0,
                mirror_url: None,
                mirror_urls: vec![],
                current_mirror_index: 0,
                chunks: vec![],
            },
            durable_bytes: 0,
            speed_tracker: SpeedTracker::default(),
        }),
        runtime: ParkingMutex::new(None),
        aimd: ParkingMutex::new(AimdState::default()),
        stop_notify: Notify::new(),
    }
}

fn make_managed_with_snapshot(
    id: &str,
    state: DownloadState,
    created_at_ms: u64,
    snapshot_mod: impl FnOnce(&mut DownloadSnapshot),
) -> ManagedDownload {
    let dl = make_managed(id, state, created_at_ms);
    {
        let mut core = dl.lock_core();
        snapshot_mod(&mut core.snapshot);
    }
    dl
}

/// Pure eviction logic extracted for testing.
/// Same algorithm as `TaskLifecycle::evict_completed`.
fn evict_completed_test(map: &mut HashMap<String, ManagedDownload>, limit: usize) -> usize {
    if limit == 0 {
        return 0;
    }
    if map.len() <= limit {
        return 0;
    }

    let mut terminal: Vec<(String, u64)> = Vec::new();
    for (id, managed) in map.iter() {
        let core = managed.lock_core();
        if is_terminal(core.snapshot.state) {
            terminal.push((id.clone(), core.snapshot.created_at_ms));
        }
    }

    terminal.sort_by_key(|(_, created)| *created);

    let excess = map.len().saturating_sub(limit);
    let to_remove = terminal.len().min(excess);
    let evicted = to_remove;
    for (id, _) in terminal.iter().take(to_remove) {
        map.remove(id);
    }

    evicted
}

/// Pure snapshot-building logic extracted for testing.
/// Same algorithm as `TaskLifecycle::build_snapshot`.
fn build_snapshot_test(managed: &ManagedDownload) -> DownloadSnapshot {
    let core = managed.lock_core();
    let mut snapshot = core.snapshot.clone();
    let chunks: Vec<ChunkInfo> = core
        .manifest
        .chunks
        .iter()
        .map(|c| ChunkInfo {
            index: c.index,
            start: c.start,
            end: c.end,
            downloaded: c.downloaded,
            completed: c.completed,
            claimed_by: c.claimed_by,
        })
        .collect();
    snapshot.chunks = chunks;

    let speed = if is_terminal(snapshot.state) {
        None
    } else if core.speed_tracker.is_initialized() {
        core.speed_tracker.current_speed(std::time::Instant::now())
    } else {
        let elapsed = (snapshot
            .updated_at_ms
            .saturating_sub(snapshot.created_at_ms))
        .max(1) as f64
            / 1000.0;

        let average_speed = if snapshot.downloaded_bytes == 0 {
            None
        } else {
            Some(snapshot.downloaded_bytes as f64 / elapsed)
        };

        managed.aimd.lock().last_throughput.or(average_speed)
    };
    drop(core);

    let eta = match (snapshot.total_bytes, speed) {
        (Some(total), Some(speed)) if speed > 0.0 && total >= snapshot.downloaded_bytes => {
            Some(((total - snapshot.downloaded_bytes) as f64 / speed).ceil() as u64)
        }
        _ => None,
    };

    snapshot.speed_bytes_per_second = if is_terminal(snapshot.state) {
        None
    } else {
        speed
    };
    snapshot.eta_seconds = if is_terminal(snapshot.state) {
        None
    } else {
        eta
    };
    snapshot
}

// ── is_terminal ──────────────────────────────────────────────────

#[test]
fn is_terminal_completed() {
    assert!(is_terminal(DownloadState::Completed));
}

#[test]
fn is_terminal_failed() {
    assert!(is_terminal(DownloadState::Failed));
}

#[test]
fn is_terminal_canceled() {
    assert!(is_terminal(DownloadState::Canceled));
}

#[test]
fn is_terminal_non_terminal_states() {
    assert!(!is_terminal(DownloadState::Queued));
    assert!(!is_terminal(DownloadState::Downloading));
    assert!(!is_terminal(DownloadState::Paused));
    assert!(!is_terminal(DownloadState::Retrying));
    assert!(!is_terminal(DownloadState::Verifying));
}

// ── is_network_error ─────────────────────────────────────────────

#[test]
fn is_network_error_http_builder_not_network() {
    // A reqwest builder error (invalid URL) is NOT connect/timeout/body
    let err = reqwest::Client::builder()
        .build()
        .unwrap()
        .get("http://")
        .build()
        .unwrap_err();
    let dl_err = DownloadError::Http(err);
    assert!(!is_network_error(&dl_err));
}

#[test]
fn is_network_error_http_decode_is_true() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        use axum::{Router, routing::get};
        let app = Router::new().route(
            "/bad-gzip",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_ENCODING, "gzip")],
                    "not gzip content",
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let err = reqwest::Client::new()
            .get(format!("http://{addr}/bad-gzip"))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap_err();

        assert!(err.is_decode());
        let dl_err = DownloadError::Http(err);
        assert!(is_network_error(&dl_err));

        handle.abort();
    });
}

#[test]
fn is_network_error_io_is_false() {
    let dl_err = DownloadError::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionRefused,
        "connection refused",
    ));
    assert!(!is_network_error(&dl_err));
}

#[test]
fn is_network_error_permission_denied_is_false() {
    let dl_err = DownloadError::PermissionDenied {
        path: "/data/file".into(),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    assert!(!is_network_error(&dl_err));
}

#[test]
fn is_network_error_all_non_http_variants_false() {
    let variants: Vec<DownloadError> = vec![
        DownloadError::UnsupportedScheme,
        DownloadError::NotFound,
        DownloadError::AlreadyRunning,
        DownloadError::NotResumable,
        DownloadError::Canceled,
        DownloadError::MissingFileName,
        DownloadError::Interrupted,
        DownloadError::InvalidResponse("test".into()),
        DownloadError::InvalidRequest("test".into()),
        DownloadError::InvalidProxy("test".into()),
        DownloadError::Torrent("test".into()),
        DownloadError::TorrentNetwork("test".into()),
        DownloadError::TorrentInvalidData("test".into()),
        DownloadError::TorrentIo("test".into()),
        DownloadError::InsufficientDiskSpace {
            available: 0,
            required: 100,
        },
        DownloadError::DatabaseInit("test".into()),
        DownloadError::Internal("test".into()),
        DownloadError::TooManyConcurrentDownloads,
    ];
    for (i, variant) in variants.into_iter().enumerate() {
        assert!(
            !is_network_error(&variant),
            "variant index {i} should not be a network error"
        );
    }
}

// ── build_snapshot ───────────────────────────────────────────────

#[test]
fn build_snapshot_normal_progress() {
    let dl = make_managed_with_snapshot("snap-1", DownloadState::Downloading, 1000, |s| {
        s.downloaded_bytes = 5_000_000;
        s.total_bytes = Some(10_000_000);
        s.updated_at_ms = 2000; // elapsed = 1.0s
    });

    let snap = build_snapshot_test(&dl);

    assert_eq!(snap.id, "snap-1");
    assert_eq!(snap.state, DownloadState::Downloading);
    // Average speed: 5_000_000 / 1.0 = 5_000_000 B/s
    assert!(
        snap.speed_bytes_per_second.is_some(),
        "speed should be Some for active download"
    );
    let speed = snap.speed_bytes_per_second.unwrap();
    assert!(
        (speed - 5_000_000.0).abs() < 1.0,
        "expected ~5MB/s, got {speed}"
    );

    // ETA: (10_000_000 - 5_000_000) / 5_000_000 = 1 second
    assert_eq!(snap.eta_seconds, Some(1));
}

#[test]
fn build_snapshot_zero_downloaded_speed_none() {
    let dl = make_managed_with_snapshot("snap-2", DownloadState::Downloading, 1000, |s| {
        s.downloaded_bytes = 0;
        s.total_bytes = Some(10_000_000);
        s.updated_at_ms = 5000; // 4s elapsed, but 0 bytes → speed None
    });

    let snap = build_snapshot_test(&dl);

    assert_eq!(snap.speed_bytes_per_second, None);
    assert_eq!(snap.eta_seconds, None);
}

#[test]
fn build_snapshot_terminal_state_speed_eta_none() {
    for &state in &[
        DownloadState::Completed,
        DownloadState::Failed,
        DownloadState::Canceled,
    ] {
        let dl = make_managed_with_snapshot("snap-term", state, 1000, |s| {
            s.downloaded_bytes = 8_000_000;
            s.total_bytes = Some(10_000_000);
            s.updated_at_ms = 3000; // elapsed = 2.0s, avg speed = 4MB/s
        });

        let snap = build_snapshot_test(&dl);

        assert!(
            snap.speed_bytes_per_second.is_none(),
            "speed should be None for terminal state {state:?}"
        );
        assert!(
            snap.eta_seconds.is_none(),
            "ETA should be None for terminal state {state:?}"
        );
    }
}

#[test]
fn build_snapshot_persistent_bytes_added() {
    let dl = make_managed_with_snapshot("snap-resume", DownloadState::Downloading, 1000, |s| {
        s.downloaded_bytes = 5_000_000;
        s.total_bytes = Some(10_000_000);
        s.updated_at_ms = 2000; // elapsed = 1.0s
    });

    // Update AIMD last_throughput for realistic scenario
    {
        let mut aimd = dl.aimd.lock();
        aimd.last_throughput = Some(6_000_000.0);
    }

    let snap = build_snapshot_test(&dl);

    // speed should use AIMD last_throughput over average
    let speed = snap.speed_bytes_per_second.unwrap();
    assert!(
        (speed - 6_000_000.0).abs() < 1.0,
        "expected AIMD speed 6MB/s, got {speed}"
    );

    // ETA based on AIMD speed
    assert_eq!(snap.eta_seconds, Some(1)); // (10M - 5M) / 6M = 0.83 → ceil → 1
}

#[test]
fn build_snapshot_eta_with_unknown_total_is_none() {
    let dl = make_managed_with_snapshot("snap-eta-none", DownloadState::Downloading, 1000, |s| {
        s.downloaded_bytes = 5_000_000;
        s.total_bytes = None; // unknown total
        s.updated_at_ms = 2000;
    });

    let snap = build_snapshot_test(&dl);

    assert!(snap.speed_bytes_per_second.is_some()); // still has speed
    assert!(snap.eta_seconds.is_none()); // no ETA without total
}

#[test]
fn build_snapshot_total_less_than_downloaded_no_negative_eta() {
    let dl = make_managed_with_snapshot("snap-over", DownloadState::Downloading, 1000, |s| {
        s.downloaded_bytes = 12_000_000;
        s.total_bytes = Some(10_000_000); // server said 10MB, but we got 12MB
        s.updated_at_ms = 2000;
    });

    let snap = build_snapshot_test(&dl);

    // total < downloaded → ETA should be None (can't compute meaningful ETA)
    assert!(snap.eta_seconds.is_none());
}

#[test]
fn build_snapshot_speed_zero_eta_none() {
    let dl = make_managed_with_snapshot("snap-zero", DownloadState::Downloading, 1000, |s| {
        s.downloaded_bytes = 5_000_000;
        s.total_bytes = Some(10_000_000);
        s.updated_at_ms = 2000;
    });

    // Force AIMD to return zero speed for this case
    {
        let mut aimd = dl.aimd.lock();
        aimd.last_throughput = Some(0.0);
    }

    let snap = build_snapshot_test(&dl);

    // speed=0 → ETA should be None (division by zero guard)
    assert!(snap.eta_seconds.is_none());
}

#[test]
fn build_snapshot_exact_completion_no_eta() {
    let dl = make_managed_with_snapshot("snap-done", DownloadState::Completed, 1000, |s| {
        s.downloaded_bytes = 10_000_000;
        s.total_bytes = Some(10_000_000);
        s.updated_at_ms = 5000;
    });

    let snap = build_snapshot_test(&dl);

    // Terminal state → no speed, no ETA even if download is complete
    assert!(snap.speed_bytes_per_second.is_none());
    assert!(snap.eta_seconds.is_none());
}

#[test]
fn build_snapshot_non_terminal_retaining_state() {
    // Non-terminal states should preserve speed/ETA
    let non_terminal = [
        DownloadState::Queued,
        DownloadState::Downloading,
        DownloadState::Paused,
        DownloadState::Retrying,
        DownloadState::Verifying,
    ];

    for &state in &non_terminal {
        let dl = make_managed_with_snapshot("snap-active", state, 1000, |s| {
            s.downloaded_bytes = 5_000_000;
            s.total_bytes = Some(10_000_000);
            s.updated_at_ms = 2000;
        });

        let snap = build_snapshot_test(&dl);

        assert!(
            snap.speed_bytes_per_second.is_some(),
            "speed should be Some for non-terminal state {state:?}"
        );
        assert!(
            snap.eta_seconds.is_some(),
            "ETA should be Some for non-terminal state {state:?}"
        );
    }
}

// ── evict_completed ──────────────────────────────────────────────

#[test]
fn evict_completed_limit_zero_keeps_everything() {
    // With limit==0 the actual code returns 0 early (keep everything).
    // This matches the semantics: max_in_memory_downloads=0 means "no limit".
    let mut map: HashMap<String, ManagedDownload> = HashMap::new();
    map.insert("a".into(), make_managed("a", DownloadState::Completed, 100));
    map.insert("b".into(), make_managed("b", DownloadState::Failed, 200));
    map.insert("c".into(), make_managed("c", DownloadState::Canceled, 300));
    map.insert(
        "d".into(),
        make_managed("d", DownloadState::Downloading, 400),
    );

    let evicted = evict_completed_test(&mut map, 0);

    assert_eq!(evicted, 0);
    assert_eq!(map.len(), 4);
}

#[test]
fn evict_completed_limit_gt_total_keeps_everything() {
    let mut map: HashMap<String, ManagedDownload> = HashMap::new();
    map.insert("a".into(), make_managed("a", DownloadState::Completed, 100));
    map.insert("b".into(), make_managed("b", DownloadState::Failed, 200));

    let evicted = evict_completed_test(&mut map, 10);

    assert_eq!(evicted, 0);
    assert_eq!(map.len(), 2);
}

#[test]
fn evict_completed_no_terminal_no_op() {
    let mut map: HashMap<String, ManagedDownload> = HashMap::new();
    map.insert(
        "a".into(),
        make_managed("a", DownloadState::Downloading, 100),
    );
    map.insert("b".into(), make_managed("b", DownloadState::Paused, 200));

    let evicted = evict_completed_test(&mut map, 1);

    assert_eq!(evicted, 0);
    assert_eq!(map.len(), 2);
}

#[test]
fn evict_completed_mixed_only_terminal_evicted() {
    let mut map: HashMap<String, ManagedDownload> = HashMap::new();
    map.insert(
        "active".into(),
        make_managed("active", DownloadState::Downloading, 100),
    );
    map.insert(
        "done1".into(),
        make_managed("done1", DownloadState::Completed, 200),
    );
    map.insert(
        "done2".into(),
        make_managed("done2", DownloadState::Completed, 300),
    );
    map.insert(
        "failed".into(),
        make_managed("failed", DownloadState::Failed, 400),
    );
    map.insert(
        "paused".into(),
        make_managed("paused", DownloadState::Paused, 500),
    );

    // limit=3, 5 total → excess=2 → evict 2 oldest terminal
    let evicted = evict_completed_test(&mut map, 3);

    assert_eq!(evicted, 2);
    assert_eq!(map.len(), 3);
    assert!(map.contains_key("active"), "active should survive");
    assert!(map.contains_key("paused"), "paused should survive");
    // Oldest terminal (done1, done2) removed, newest terminal (failed) stays
    assert!(
        !map.contains_key("done1"),
        "oldest completed 'done1' evicted"
    );
    assert!(!map.contains_key("done2"), "second oldest 'done2' evicted");
    assert!(map.contains_key("failed"), "newest terminal 'failed' stays");
}

#[test]
fn evict_completed_eviction_order_oldest_first() {
    let mut map: HashMap<String, ManagedDownload> = HashMap::new();
    // Insert in non-chronological order to verify sort
    map.insert("c".into(), make_managed("c", DownloadState::Completed, 300));
    map.insert("a".into(), make_managed("a", DownloadState::Completed, 100));
    map.insert("b".into(), make_managed("b", DownloadState::Completed, 200));

    // limit=1, 3 total → excess=2 → evict 2 oldest
    let evicted = evict_completed_test(&mut map, 1);

    assert_eq!(evicted, 2);
    assert_eq!(map.len(), 1);
    // Oldest two (a=100, b=200) should be evicted, newest (c=300) stays
    assert!(!map.contains_key("a"));
    assert!(!map.contains_key("b"));
    assert!(map.contains_key("c"), "newest entry 'c' should survive");
}

#[test]
fn evict_completed_limit_exactly_total_keeps_all() {
    let mut map: HashMap<String, ManagedDownload> = HashMap::new();
    map.insert("a".into(), make_managed("a", DownloadState::Completed, 100));
    map.insert("b".into(), make_managed("b", DownloadState::Completed, 200));
    map.insert("c".into(), make_managed("c", DownloadState::Failed, 300));

    let evicted = evict_completed_test(&mut map, 3);

    assert_eq!(evicted, 0);
    assert_eq!(map.len(), 3);
}

#[test]
fn evict_completed_running_state_not_evicted() {
    let mut map: HashMap<String, ManagedDownload> = HashMap::new();
    map.insert(
        "running".into(),
        make_managed("running", DownloadState::Downloading, 50),
    );
    map.insert(
        "done".into(),
        make_managed("done", DownloadState::Completed, 100),
    );

    // limit=1, excess=1, but only 'done' is terminal — evict 'done'
    let evicted = evict_completed_test(&mut map, 1);

    assert_eq!(evicted, 1);
    assert_eq!(map.len(), 1);
    assert!(map.contains_key("running"), "running download survives");
    assert!(!map.contains_key("done"), "completed download evicted");
}

#[test]
fn evict_completed_empty_map_no_op() {
    let mut map: HashMap<String, ManagedDownload> = HashMap::new();
    let evicted = evict_completed_test(&mut map, 100);
    assert_eq!(evicted, 0);
    assert!(map.is_empty());
}

#[test]
fn evict_completed_fewer_terminal_than_excess_evicts_all_terminal() {
    let mut map: HashMap<String, ManagedDownload> = HashMap::new();
    // 6 total, limit=1, excess=5, but only 2 terminal → evict both terminal
    map.insert(
        "dl1".into(),
        make_managed("dl1", DownloadState::Downloading, 100),
    );
    map.insert(
        "dl2".into(),
        make_managed("dl2", DownloadState::Downloading, 200),
    );
    map.insert(
        "dl3".into(),
        make_managed("dl3", DownloadState::Paused, 300),
    );
    map.insert(
        "dl4".into(),
        make_managed("dl4", DownloadState::Queued, 400),
    );
    map.insert(
        "done1".into(),
        make_managed("done1", DownloadState::Completed, 500),
    );
    map.insert(
        "done2".into(),
        make_managed("done2", DownloadState::Failed, 600),
    );

    let evicted = evict_completed_test(&mut map, 1);

    assert_eq!(evicted, 2);
    assert_eq!(map.len(), 4);
    // All active survive
    assert!(map.contains_key("dl1"));
    assert!(map.contains_key("dl2"));
    assert!(map.contains_key("dl3"));
    assert!(map.contains_key("dl4"));
    // Both terminal evicted
    assert!(!map.contains_key("done1"));
    assert!(!map.contains_key("done2"));
}

// ── is_terminal + is_network_error combined with eviction ────────

#[test]
fn evict_completed_canceled_is_terminal() {
    let mut map: HashMap<String, ManagedDownload> = HashMap::new();
    map.insert("a".into(), make_managed("a", DownloadState::Canceled, 100));
    map.insert("b".into(), make_managed("b", DownloadState::Completed, 200));

    let evicted = evict_completed_test(&mut map, 1);

    assert_eq!(evicted, 1);
    assert_eq!(map.len(), 1);
    // Canceled is older → evicted
    assert!(!map.contains_key("a"));
    assert!(map.contains_key("b"));
}

#[test]
fn build_snapshot_speed_tracker_overrides_average_speed_for_old_task() {
    // A task created 10 hours ago (36_000_000 ms ago).
    // Under the old bug, speed was: downloaded_bytes / (now - created_at)
    // With 10MB downloaded over 10 hours, old formula gave ~277 B/s.
    // With SpeedTracker receiving 10MB right now, speed reflects the actual ~10MB/s rate.
    let dl = Arc::new(make_managed_with_snapshot(
        "snap-speed-tracker",
        DownloadState::Downloading,
        1000,
        |s| {
            s.total_bytes = Some(50_000_000);
            s.updated_at_ms = 36_001_000;
        },
    ));

    crate::download::record_progress_on_managed(&dl, None, 10_000_000);

    let snap = build_snapshot_test(&dl);

    let speed = snap.speed_bytes_per_second.expect("speed should be Some");
    assert!(
        speed > 1_000_000.0,
        "speed should reflect recent throughput, got {speed} B/s (old bug would be ~277 B/s)"
    );
}
