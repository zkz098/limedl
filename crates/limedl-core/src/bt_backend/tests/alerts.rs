//! Irontide alert -> info-hash extraction.

use super::*;

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
