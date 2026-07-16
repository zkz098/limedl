use std::net::SocketAddr;
use std::str::FromStr;

use irontide::core::Id20;

use super::alerts::extract_info_hash;
use super::snapshot::{
    build_peer_flags, estimate_eta, map_state, preview_entries_from_meta, v1_file_entries,
};
use super::super::types::DownloadState;

// ── map_state ──────────────────────────────────────────────────────────

#[test]
fn test_map_state_downloading() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Downloading),
        DownloadState::Downloading
    );
}

#[test]
fn test_map_state_seeding() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Seeding),
        DownloadState::Completed
    );
}

#[test]
fn test_map_state_complete() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Complete),
        DownloadState::Completed
    );
}

#[test]
fn test_map_state_paused() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Paused),
        DownloadState::Paused
    );
}

#[test]
fn test_map_state_checking() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Checking),
        DownloadState::Verifying
    );
}

#[test]
fn test_map_state_fetching_metadata() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::FetchingMetadata),
        DownloadState::Queued
    );
}

#[test]
fn test_map_state_queued() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Queued),
        DownloadState::Queued
    );
}

#[test]
fn test_map_state_stopped() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Stopped),
        DownloadState::Canceled
    );
}

#[test]
fn test_map_state_sharing() {
    assert_eq!(
        map_state(&irontide::session::TorrentState::Sharing),
        DownloadState::Downloading
    );
}

// ── build_peer_flags ───────────────────────────────────────────────────

fn make_peer() -> irontide::session::PeerInfo {
    irontide::session::PeerInfo {
        addr: SocketAddr::from_str("127.0.0.1:6881").unwrap(),
        client: String::new(),
        peer_choking: false,
        peer_interested: false,
        am_choking: false,
        am_interested: false,
        download_rate: 0,
        upload_rate: 0,
        num_pieces: 0,
        source: irontide::session::PeerSource::Tracker,
        supports_fast: false,
        upload_only: false,
        snubbed: false,
        connected_duration_secs: 0,
        num_pending_requests: 0,
        num_incoming_requests: 0,
        is_optimistic: false,
        is_encrypted: false,
        uses_utp: false,
        uses_holepunch: false,
        in_flight_requests: 0,
        target_pipeline_depth: 0,
        relevance: 0.0,
        connection_kind: irontide::session::PeerConnectionKind::Tcp,
        progress: 0.0,
        country_code: None,
    }
}

#[test]
fn test_build_peer_flags_empty() {
    let peer = make_peer();
    assert_eq!(build_peer_flags(&peer), "");
}

#[test]
fn test_build_peer_flags_all() {
    let mut peer = make_peer();
    peer.is_encrypted = true;
    peer.uses_utp = true;
    peer.supports_fast = true;
    peer.upload_only = true;
    peer.snubbed = true;
    peer.am_choking = true;
    peer.peer_interested = true;
    assert_eq!(build_peer_flags(&peer), "EuFUScI");
}

#[test]
fn test_build_peer_flags_encrypted() {
    let mut peer = make_peer();
    peer.is_encrypted = true;
    assert_eq!(build_peer_flags(&peer), "E");
}

#[test]
fn test_build_peer_flags_utp() {
    let mut peer = make_peer();
    peer.uses_utp = true;
    assert_eq!(build_peer_flags(&peer), "u");
}

#[test]
fn test_build_peer_flags_fast() {
    let mut peer = make_peer();
    peer.supports_fast = true;
    assert_eq!(build_peer_flags(&peer), "F");
}

#[test]
fn test_build_peer_flags_upload_only() {
    let mut peer = make_peer();
    peer.upload_only = true;
    assert_eq!(build_peer_flags(&peer), "U");
}

#[test]
fn test_build_peer_flags_snubbed() {
    let mut peer = make_peer();
    peer.snubbed = true;
    assert_eq!(build_peer_flags(&peer), "S");
}

#[test]
fn test_build_peer_flags_am_choking() {
    let mut peer = make_peer();
    peer.am_choking = true;
    assert_eq!(build_peer_flags(&peer), "c");
}

#[test]
fn test_build_peer_flags_interested() {
    let mut peer = make_peer();
    peer.peer_interested = true;
    assert_eq!(build_peer_flags(&peer), "I");
}

#[test]
fn test_build_peer_flags_combination() {
    let mut peer = make_peer();
    peer.is_encrypted = true;
    peer.supports_fast = true;
    peer.am_choking = true;
    // E + F + c
    assert_eq!(build_peer_flags(&peer), "EFc");
}

// ── estimate_eta ───────────────────────────────────────────────────────

#[test]
fn test_estimate_eta_normal() {
    assert_eq!(estimate_eta(1000, 500, Some(100.0)), Some(5));
}

#[test]
fn test_estimate_eta_zero_speed() {
    assert_eq!(estimate_eta(1000, 500, Some(0.0)), None);
}

#[test]
fn test_estimate_eta_completed() {
    assert_eq!(estimate_eta(1000, 1000, Some(100.0)), None);
}

#[test]
fn test_estimate_eta_over_downloaded() {
    assert_eq!(estimate_eta(1000, 1500, Some(100.0)), None);
}

#[test]
fn test_estimate_eta_none_speed() {
    assert_eq!(estimate_eta(1000, 500, None), None);
}

#[test]
fn test_estimate_eta_small_speed() {
    // 1 byte remaining at 0.5 B/s => ceil(1.0 / 0.5) = 2
    assert_eq!(estimate_eta(1000, 999, Some(0.5)), Some(2));
}

#[test]
fn test_estimate_eta_exact_division() {
    // 100 bytes remaining at 50 B/s => 2 seconds
    assert_eq!(estimate_eta(200, 100, Some(50.0)), Some(2));
}

// ── extract_info_hash ──────────────────────────────────────────────────

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

// ── v1_file_entries / preview_entries_from_meta ────────────────────────

#[test]
fn test_v1_file_entries_single_file() {
    let info = irontide::core::InfoDict {
        name: "ubuntu.iso".into(),
        piece_length: 262144,
        pieces: vec![0u8; 20],
        length: Some(1_000_000_000),
        files: None,
        private: None,
        source: None,
        ssl_cert: None,
        similar: vec![],
        collections: vec![],
    };
    let v1 = irontide::core::TorrentMetaV1 {
        info_hash: Id20::from([0u8; 20]),
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info,
        url_list: vec![],
        httpseeds: vec![],
        info_bytes: None,
        ssl_cert: None,
    };
    let entries = v1_file_entries(&v1);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "ubuntu.iso");
    assert_eq!(entries[0].size, 1_000_000_000);
}

#[test]
fn test_v1_file_entries_multi_file() {
    let files = vec![
        irontide::core::FileEntry {
            length: 500,
            path: vec!["dir".into(), "file1.txt".into()],
            attr: None,
            mtime: None,
            symlink_path: None,
        },
        irontide::core::FileEntry {
            length: 1200,
            path: vec!["file2.txt".into()],
            attr: None,
            mtime: None,
            symlink_path: None,
        },
    ];
    let info = irontide::core::InfoDict {
        name: "mydir".into(),
        piece_length: 16384,
        pieces: vec![0u8; 20],
        length: None,
        files: Some(files),
        private: None,
        source: None,
        ssl_cert: None,
        similar: vec![],
        collections: vec![],
    };
    let v1 = irontide::core::TorrentMetaV1 {
        info_hash: Id20::from([0u8; 20]),
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info,
        url_list: vec![],
        httpseeds: vec![],
        info_bytes: None,
        ssl_cert: None,
    };
    let entries = v1_file_entries(&v1);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "dir/file1.txt");
    assert_eq!(entries[0].size, 500);
    assert_eq!(entries[1].index, 1);
    assert_eq!(entries[1].path, "file2.txt");
    assert_eq!(entries[1].size, 1200);
}

#[test]
fn test_v1_file_entries_empty_file_list() {
    // A torrent with no files (unusual but code handles it)
    let info = irontide::core::InfoDict {
        name: "empty".into(),
        piece_length: 16384,
        pieces: vec![0u8; 20],
        length: Some(0),
        files: None,
        private: None,
        source: None,
        ssl_cert: None,
        similar: vec![],
        collections: vec![],
    };
    let v1 = irontide::core::TorrentMetaV1 {
        info_hash: Id20::from([0u8; 20]),
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info,
        url_list: vec![],
        httpseeds: vec![],
        info_bytes: None,
        ssl_cert: None,
    };
    let entries = v1_file_entries(&v1);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "empty");
    assert_eq!(entries[0].size, 0);
}

#[test]
fn test_preview_entries_from_meta_v1() {
    // Delegates to v1_file_entries, so a single smoke test suffices
    let info = irontide::core::InfoDict {
        name: "test.iso".into(),
        piece_length: 16384,
        pieces: vec![0u8; 20],
        length: Some(42),
        files: None,
        private: None,
        source: None,
        ssl_cert: None,
        similar: vec![],
        collections: vec![],
    };
    let v1 = irontide::core::TorrentMetaV1 {
        info_hash: Id20::from([0u8; 20]),
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info,
        url_list: vec![],
        httpseeds: vec![],
        info_bytes: None,
        ssl_cert: None,
    };
    let meta = irontide::core::TorrentMeta::V1(v1);
    let entries = preview_entries_from_meta(&meta);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "test.iso");
    assert_eq!(entries[0].size, 42);
}
