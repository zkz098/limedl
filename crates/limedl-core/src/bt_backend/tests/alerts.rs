//! Irontide alert -> info-hash extraction and alert-bridge event mapping.

use super::*;

use crate::event_bus::DownloadEvent;

#[test]
fn test_extract_info_hash_torrent_added() {
    let ih = Id20::from([1u8; 20]);
    let kind = irontide::session::AlertKind::TorrentAdded {
        info_hash: ih,
        name: "test".into(),
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_torrent_finished() {
    let ih = Id20::from([2u8; 20]);
    let kind = irontide::session::AlertKind::TorrentFinished { info_hash: ih };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_torrent_paused() {
    let ih = Id20::from([3u8; 20]);
    let kind = irontide::session::AlertKind::TorrentPaused { info_hash: ih };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_state_changed() {
    let ih = Id20::from([4u8; 20]);
    let kind = irontide::session::AlertKind::StateChanged {
        info_hash: ih,
        prev_state: irontide::session::TorrentState::Downloading,
        new_state: irontide::session::TorrentState::Seeding,
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_tracker_reply() {
    let ih = Id20::from([5u8; 20]);
    let kind = irontide::session::AlertKind::TrackerReply {
        info_hash: ih,
        url: "http://tracker.example.com/announce".into(),
        num_peers: 10,
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_session_stats_update() {
    // SessionStatsUpdate is a tuple variant without info_hash
    let stats = irontide::session::SessionStats {
        active_torrents: 0,
        total_downloaded: 0,
        total_uploaded: 0,
        dht_nodes: 0,
        external_address: None,
        incoming_peer_connections: 0,
    };
    let kind = irontide::session::AlertKind::SessionStatsUpdate(stats);
    assert_eq!(extract_info_hash(&kind), None);
}

#[test]
fn test_extract_info_hash_settings_changed() {
    // SettingsChanged is a unit variant with no fields at all
    let kind = irontide::session::AlertKind::SettingsChanged;
    assert_eq!(extract_info_hash(&kind), None);
}

#[test]
fn test_extract_info_hash_listen_succeeded() {
    // ListenSucceeded has port but no info_hash
    let kind = irontide::session::AlertKind::ListenSucceeded { port: 6881 };
    assert_eq!(extract_info_hash(&kind), None);
}

#[test]
fn test_extract_info_hash_dht_bootstrap() {
    // DhtBootstrapComplete is unit, no info_hash
    let kind = irontide::session::AlertKind::DhtBootstrapComplete;
    assert_eq!(extract_info_hash(&kind), None);
}

#[test]
fn test_extract_info_hash_peer_blocked() {
    // PeerBlocked has addr but no info_hash
    let kind = irontide::session::AlertKind::PeerBlocked {
        addr: SocketAddr::from_str("10.0.0.1:6881").unwrap(),
    };
    assert_eq!(extract_info_hash(&kind), None);
}

#[test]
fn test_extract_info_hash_torrent_resumed() {
    let ih = Id20::from([10u8; 20]);
    let kind = irontide::session::AlertKind::TorrentResumed { info_hash: ih };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_torrent_removed() {
    let ih = Id20::from([11u8; 20]);
    let kind = irontide::session::AlertKind::TorrentRemoved { info_hash: ih };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_metadata_received() {
    let ih = Id20::from([12u8; 20]);
    let kind = irontide::session::AlertKind::MetadataReceived {
        info_hash: ih,
        name: "ubuntu.iso".into(),
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_metadata_failed() {
    let ih = Id20::from([13u8; 20]);
    let kind = irontide::session::AlertKind::MetadataFailed { info_hash: ih };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_torrent_checked() {
    let ih = Id20::from([14u8; 20]);
    let kind = irontide::session::AlertKind::TorrentChecked {
        info_hash: ih,
        pieces_have: 42,
        pieces_total: 100,
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_checking_progress() {
    let ih = Id20::from([15u8; 20]);
    let kind = irontide::session::AlertKind::CheckingProgress {
        info_hash: ih,
        progress: 0.5,
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_piece_finished() {
    let ih = Id20::from([16u8; 20]);
    let kind = irontide::session::AlertKind::PieceFinished {
        info_hash: ih,
        piece: 5,
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_block_finished() {
    let ih = Id20::from([17u8; 20]);
    let kind = irontide::session::AlertKind::BlockFinished {
        info_hash: ih,
        piece: 3,
        offset: 16384,
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_hash_failed() {
    let ih = Id20::from([18u8; 20]);
    let kind = irontide::session::AlertKind::HashFailed {
        info_hash: ih,
        piece: 7,
        contributors: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))],
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_peer_banned() {
    let ih = Id20::from([19u8; 20]);
    let kind = irontide::session::AlertKind::PeerBanned {
        info_hash: ih,
        addr: SocketAddr::from_str("10.0.0.2:6881").unwrap(),
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_scrape_reply() {
    let ih = Id20::from([20u8; 20]);
    let kind = irontide::session::AlertKind::ScrapeReply {
        info_hash: ih,
        url: "http://tracker.example.com/scrape".into(),
        complete: 10,
        incomplete: 3,
        downloaded: 50,
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_scrape_error() {
    let ih = Id20::from([21u8; 20]);
    let kind = irontide::session::AlertKind::ScrapeError {
        info_hash: ih,
        url: "http://tracker.example.com/scrape".into(),
        message: "scrape failed".into(),
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_dht_get_peers() {
    let ih = Id20::from([22u8; 20]);
    let kind = irontide::session::AlertKind::DhtGetPeers {
        info_hash: ih,
        num_peers: 15,
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_file_completed() {
    let ih = Id20::from([23u8; 20]);
    let kind = irontide::session::AlertKind::FileCompleted {
        info_hash: ih,
        file_index: 0,
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_file_renamed() {
    let ih = Id20::from([24u8; 20]);
    let kind = irontide::session::AlertKind::FileRenamed {
        info_hash: ih,
        index: 1,
        new_path: PathBuf::from("/new/path/file.txt"),
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_resume_data_saved() {
    let ih = Id20::from([25u8; 20]);
    let kind = irontide::session::AlertKind::ResumeDataSaved { info_hash: ih };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_torrent_error() {
    let ih = Id20::from([26u8; 20]);
    let kind = irontide::session::AlertKind::TorrentError {
        info_hash: ih,
        message: "disk full".into(),
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_performance_warning() {
    let ih = Id20::from([27u8; 20]);
    let kind = irontide::session::AlertKind::PerformanceWarning {
        info_hash: ih,
        message: "slow disk".into(),
    };
    assert_eq!(extract_info_hash(&kind), Some(&ih));
}

#[test]
fn test_extract_info_hash_listen_failed() {
    // ListenFailed has port + message but no info_hash → None
    let kind = irontide::session::AlertKind::ListenFailed {
        port: 6881,
        message: "port in use".into(),
    };
    assert_eq!(extract_info_hash(&kind), None);
}

#[test]
fn test_extract_info_hash_dht_node_violation() {
    // DhtNodeIdViolation has node_id + addr but no info_hash → None
    let kind = irontide::session::AlertKind::DhtNodeIdViolation {
        node_id: Id20::from([99u8; 20]),
        addr: SocketAddr::from_str("10.0.0.3:6881").unwrap(),
    };
    assert_eq!(extract_info_hash(&kind), None);
}

#[test]
fn test_extract_info_hash_disk_stats_update() {
    // DiskStatsUpdate is a tuple variant without info_hash → None
    let stats = irontide::session::DiskStats {
        read_bytes: 0,
        write_bytes: 0,
        cache_hits: 0,
        cache_misses: 0,
        write_buffer_bytes: 0,
        queued_jobs: 0,
        read_cache_bytes: 0,
        pool_entries: 0,
        prefetch_count: 0,
        eviction_count: 0,
        skeleton_count: 0,
    };
    let kind = irontide::session::AlertKind::DiskStatsUpdate(stats);
    assert_eq!(extract_info_hash(&kind), None);
}

// ── handle_alert: alert bridge → frontend event mapping ───────────────

/// Drain every event currently buffered on `rx`.
fn drain_events(rx: &mut tokio::sync::broadcast::Receiver<DownloadEvent>) -> Vec<DownloadEvent> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

/// The Aria2 notification names among `events`, in order.
fn aria2_events(events: &[DownloadEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|event| match event {
            DownloadEvent::Aria2Notification { event_name, .. } => Some(event_name.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn alert_bridge_torrent_added_registers_task_and_notifies() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let info_hash = Id20::from([1u8; 20]);

    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::TorrentAdded {
            info_hash,
            name: "test".into(),
        },
    )
    .await;

    assert!(
        backend.task_map.contains_key(&info_hash),
        "TorrentAdded must register the task"
    );
    let events = drain_events(&mut rx);
    assert_eq!(aria2_events(&events), vec!["aria2.onDownloadStart"]);
    match &events[0] {
        DownloadEvent::Aria2Notification { gid, .. } => {
            assert_eq!(gid, &internal_id_to_gid(&info_hash), "gid comes from the info hash");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    // A second TorrentAdded for a task that is already tracked must not
    // overwrite the existing entry.
    let sentinel = Id20::from([0xFF; 20]);
    backend.task_map.insert(info_hash, sentinel);
    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::TorrentAdded {
            info_hash,
            name: "test".into(),
        },
    )
    .await;
    assert_eq!(
        *backend.task_map.get(&info_hash).unwrap(),
        sentinel,
        "an existing task must be left alone"
    );
    drain_events(&mut rx);

    backend.shutdown().await;
}

#[tokio::test]
async fn alert_bridge_torrent_removed_drops_task() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let info_hash = Id20::from([2u8; 20]);
    backend.task_map.insert(info_hash, info_hash);

    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::TorrentRemoved { info_hash },
    )
    .await;

    assert!(
        !backend.task_map.contains_key(&info_hash),
        "TorrentRemoved must drop the task"
    );
    assert!(drain_events(&mut rx).is_empty(), "removal publishes nothing");

    backend.shutdown().await;
}

#[tokio::test]
async fn alert_bridge_pause_and_resume_notifications() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let info_hash = Id20::from([3u8; 20]);

    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::TorrentPaused { info_hash },
    )
    .await;
    assert_eq!(
        aria2_events(&drain_events(&mut rx)),
        vec!["aria2.onDownloadPause"]
    );

    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::TorrentResumed { info_hash },
    )
    .await;
    assert_eq!(
        aria2_events(&drain_events(&mut rx)),
        vec!["aria2.onDownloadStart"],
        "a resume is reported as a start"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn alert_bridge_torrent_finished_emits_complete_progress_and_updated() {
    let (tmp, backend) = make_backend().await;
    let path = write_torrent_fixture(tmp.path());
    let info_hash = backend
        .start(StartDownloadRequest {
            url: path.to_string_lossy().into_owned(),
            ..Default::default()
        })
        .await
        .expect("start fixture");
    let mut rx = backend.event_bus.subscribe();

    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::TorrentFinished { info_hash },
    )
    .await;

    let events = drain_events(&mut rx);
    assert_eq!(
        aria2_events(&events),
        vec!["aria2.onDownloadComplete", "aria2.onBtDownloadComplete"]
    );
    let task_id = info_hash.to_hex();
    assert!(
        events.iter().any(|event| matches!(
            event,
            DownloadEvent::Progress { id, progress_json }
                if id == &task_id && progress_json["state"] == "completed"
        )),
        "finished torrents get a final progress tick: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            DownloadEvent::Updated { id, summary_json }
                if id == &task_id && summary_json["state"] == "completed"
        )),
        "finished torrents get a full summary update: {events:?}"
    );

    backend.shutdown().await;
}

#[tokio::test]
async fn alert_bridge_torrent_finished_without_stats_still_updates() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let info_hash = Id20::from([0xAB; 20]);

    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::TorrentFinished { info_hash },
    )
    .await;

    let events = drain_events(&mut rx);
    assert_eq!(
        aria2_events(&events),
        vec!["aria2.onDownloadComplete", "aria2.onBtDownloadComplete"]
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, DownloadEvent::Progress { .. })),
        "no stats means no progress tick: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            DownloadEvent::Updated { id, summary_json }
                if id == &info_hash.to_hex() && summary_json["state"] == "completed"
        )),
        "the summary update must still be emitted: {events:?}"
    );

    backend.shutdown().await;
}

#[tokio::test]
async fn alert_bridge_torrent_error_notifies_and_updates() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let info_hash = Id20::from([4u8; 20]);

    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::TorrentError {
            info_hash,
            message: "disk full".into(),
        },
    )
    .await;

    let events = drain_events(&mut rx);
    assert_eq!(aria2_events(&events), vec!["aria2.onDownloadError"]);
    assert!(
        events.iter().any(|event| matches!(
            event,
            DownloadEvent::Updated { summary_json, .. }
                if summary_json["state"] == "error" && summary_json["error"] == "disk full"
        )),
        "the error message must reach the frontend: {events:?}"
    );

    backend.shutdown().await;
}

#[tokio::test]
async fn alert_bridge_tracker_reply_only_updates_with_peers() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let info_hash = Id20::from([5u8; 20]);

    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::TrackerReply {
            info_hash,
            url: "http://tracker.example.com/announce".into(),
            num_peers: 3,
        },
    )
    .await;
    let events = drain_events(&mut rx);
    assert_eq!(events.len(), 1, "a useful tracker reply updates the row");
    match &events[0] {
        DownloadEvent::Updated { id, summary_json } => {
            assert_eq!(id, &info_hash.to_hex());
            assert_eq!(summary_json["peers"], 3);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    // A reply with no peers carries no information worth a repaint.
    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::TrackerReply {
            info_hash,
            url: "http://tracker.example.com/announce".into(),
            num_peers: 0,
        },
    )
    .await;
    assert!(
        drain_events(&mut rx).is_empty(),
        "zero peers must not publish anything"
    );

    backend.shutdown().await;
}

#[tokio::test]
async fn alert_bridge_ignores_alerts_without_an_info_hash() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();

    handle_alert(
        &backend.session,
        &backend.event_bus,
        &backend.task_map,
        &irontide::session::AlertKind::SettingsChanged,
    )
    .await;

    assert!(drain_events(&mut rx).is_empty());
    assert!(backend.task_map.is_empty());

    backend.shutdown().await;
}

#[tokio::test]
async fn alert_bridge_log_only_alerts_publish_nothing() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let info_hash = Id20::from([6u8; 20]);
    let addr = SocketAddr::from_str("127.0.0.1:6881").unwrap();

    let log_only = [
        irontide::session::AlertKind::MetadataReceived {
            info_hash,
            name: "ubuntu.iso".into(),
        },
        irontide::session::AlertKind::StateChanged {
            info_hash,
            prev_state: irontide::session::TorrentState::Downloading,
            new_state: irontide::session::TorrentState::Seeding,
        },
        irontide::session::AlertKind::TorrentChecked {
            info_hash,
            pieces_have: 1,
            pieces_total: 1,
        },
        irontide::session::AlertKind::FileCompleted {
            info_hash,
            file_index: 0,
        },
        irontide::session::AlertKind::HashFailed {
            info_hash,
            piece: 0,
            contributors: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))],
        },
        irontide::session::AlertKind::TrackerWarning {
            info_hash,
            url: "http://tracker.example.com/announce".into(),
            message: "offline".into(),
        },
        irontide::session::AlertKind::TrackerError {
            info_hash,
            url: "http://tracker.example.com/announce".into(),
            message: "timeout".into(),
        },
        irontide::session::AlertKind::PeerConnected { info_hash, addr },
        irontide::session::AlertKind::PeerDisconnected {
            info_hash,
            addr,
            reason: Some("peer closed".into()),
        },
        irontide::session::AlertKind::StorageMoved {
            info_hash,
            new_path: PathBuf::from("/moved"),
        },
        irontide::session::AlertKind::FileError {
            info_hash,
            path: PathBuf::from("/moved/file.bin"),
            message: "gone".into(),
        },
    ];

    for kind in &log_only {
        handle_alert(&backend.session, &backend.event_bus, &backend.task_map, kind).await;
    }

    assert!(
        drain_events(&mut rx).is_empty(),
        "log-only alerts must not repaint the frontend"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn alert_bridge_periodic_tick_emits_progress_for_active_torrents() {
    let (tmp, backend) = make_backend().await;
    let path = write_torrent_fixture(tmp.path());
    let info_hash = backend
        .start(StartDownloadRequest {
            url: path.to_string_lossy().into_owned(),
            ..Default::default()
        })
        .await
        .expect("start fixture");
    let mut rx = backend.event_bus.subscribe();

    emit_progress_for_all_torrents(&backend.session, &backend.event_bus, &backend.task_map).await;

    let events = drain_events(&mut rx);
    assert_eq!(events.len(), 1, "one torrent, one progress tick");
    match &events[0] {
        DownloadEvent::Progress { id, progress_json } => {
            assert_eq!(id, &info_hash.to_hex());
            assert_eq!(progress_json["totalBytes"], 30, "fixture payload size");
            assert_eq!(progress_json["downloadedBytes"], 0);
            assert_eq!(progress_json["uploadStatus"], "idle");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    backend.shutdown().await;
}

#[tokio::test]
async fn alert_bridge_periodic_tick_skips_unknown_torrents() {
    let (_tmp, backend) = make_backend().await;
    let mut rx = backend.event_bus.subscribe();
    let unknown = Id20::from([0xCD; 20]);
    backend.task_map.insert(unknown, unknown);

    emit_progress_for_all_torrents(&backend.session, &backend.event_bus, &backend.task_map).await;

    assert!(
        drain_events(&mut rx).is_empty(),
        "a task the engine does not know cannot produce a progress tick"
    );

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn alert_bridge_setup_replaces_the_previous_loop_and_forwards_real_alerts() {
    let (_tmp, backend) = make_backend().await;
    let backend = Arc::new(backend);
    let mut rx = backend.event_bus.subscribe();

    backend.setup_alert_bridge().await;
    let first = backend.alert_task.lock().as_ref().map(|handle| handle.id());
    assert!(first.is_some(), "setup stores a join handle");

    // The second setup must abort the first loop and install a new one.
    backend.setup_alert_bridge().await;
    let second = backend.alert_task.lock().as_ref().map(|handle| handle.id());
    assert!(second.is_some(), "replacement loop is stored");
    assert_ne!(first, second, "setup must replace the previous task");

    // The live loop must forward a real `TorrentAdded`. Subscribing happens in
    // `setup_alert_bridge`, so nothing emitted after it returns can be lost.
    let info_hash = backend
        .start(StartDownloadRequest {
            url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=live"
                .into(),
            ..Default::default()
        })
        .await
        .expect("start magnet");
    let expected_gid = internal_id_to_gid(&info_hash);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await.expect("event bus stays open") {
                DownloadEvent::Aria2Notification { event_name, gid }
                    if event_name == "aria2.onDownloadStart" && gid == expected_gid =>
                {
                    break;
                }
                _ => continue,
            }
        }
    })
    .await
    .expect("TorrentAdded must reach the live bridge");

    backend.shutdown().await;
}
