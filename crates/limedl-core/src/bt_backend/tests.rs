use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use irontide::core::{Id20, Id32, InfoHashes, InfoDictV2, FileTreeNode};

use super::alerts::extract_info_hash;
use super::internal_id_to_gid;
use super::snapshot::{
    build_peer_flags, estimate_eta, map_state, preview_entries_from_meta, v1_file_entries,
    StateHelpers,
};
use super::IrontideBtBackend;
use crate::event_bus::EventBus;
use crate::protocol::DownloadBackend;
use crate::types::{
    AppSettings, DownloadState, DownloadSummary, StartDownloadRequest, TaskId,
    TaskKind,
};

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

// ═══════════════════════════════════════════════════════════════════════
//  Additional extract_info_hash variants
// ═══════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════
//  preview_entries_from_meta — Hybrid and V2
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_preview_entries_from_meta_hybrid() {
    let info = irontide::core::InfoDict {
        name: "hybrid.iso".into(),
        piece_length: 16384,
        pieces: vec![0u8; 20],
        length: Some(100),
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
    // Hybrid uses the v1 info dict for file entries
    let v2 = irontide::core::TorrentMetaV2 {
        info_hashes: InfoHashes::v2_only(Id32::from([0u8; 32])),
        info_bytes: None,
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info: InfoDictV2 {
            name: "hybrid.iso".into(),
            piece_length: 16384,
            meta_version: 2,
            file_tree: FileTreeNode::Directory(BTreeMap::new()),
            ssl_cert: None,
        },
        piece_layers: BTreeMap::new(),
        ssl_cert: None,
    };
    let meta = irontide::core::TorrentMeta::Hybrid(Box::new(v1), Box::new(v2));
    let entries = preview_entries_from_meta(&meta);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "hybrid.iso");
    assert_eq!(entries[0].size, 100);
}

#[test]
fn test_preview_entries_from_meta_v2() {
    // V2 torrents return a placeholder entry because we don't parse V2 file trees
    let meta = irontide::core::TorrentMeta::V2(irontide::core::TorrentMetaV2 {
        info_hashes: InfoHashes::v2_only(Id32::from([0u8; 32])),
        info_bytes: None,
        announce: None,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        info: InfoDictV2 {
            name: "v2-torrent".into(),
            piece_length: 16384,
            meta_version: 2,
            file_tree: FileTreeNode::Directory(BTreeMap::new()),
            ssl_cert: None,
        },
        piece_layers: BTreeMap::new(),
        ssl_cert: None,
    });
    let entries = preview_entries_from_meta(&meta);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "v2-torrent");
    assert_eq!(entries[0].size, 0);
}

// ═══════════════════════════════════════════════════════════════════════
//  v1_file_entries — additional edge cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_v1_file_entries_nested_paths() {
    let files = vec![
        irontide::core::FileEntry {
            length: 200,
            path: vec!["a".into(), "b".into(), "c".into(), "deep.txt".into()],
            attr: None,
            mtime: None,
            symlink_path: None,
        },
    ];
    let info = irontide::core::InfoDict {
        name: "root".into(),
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
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].path, "a/b/c/deep.txt");
    assert_eq!(entries[0].size, 200);
}

// ═══════════════════════════════════════════════════════════════════════
//  internal_id_to_gid
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_internal_id_to_gid_known_hash() {
    let ih = Id20::from([0u8; 20]);
    let gid = internal_id_to_gid(&ih);
    // xxh3_64 of 40 zero hex chars should be deterministic
    assert_eq!(gid.len(), 16);
    assert!(gid.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_internal_id_to_gid_different_hashes_different_gids() {
    let ih1 = Id20::from([1u8; 20]);
    let ih2 = Id20::from([2u8; 20]);
    assert_ne!(internal_id_to_gid(&ih1), internal_id_to_gid(&ih2));
}

#[test]
fn test_internal_id_to_gid_same_hash_same_gid() {
    let ih = Id20::from([42u8; 20]);
    assert_eq!(internal_id_to_gid(&ih), internal_id_to_gid(&ih));
}

// ═══════════════════════════════════════════════════════════════════════
//  StateHelpers::is_terminal
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_state_helpers_is_terminal_completed() {
    assert!(DownloadState::Completed.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_failed() {
    assert!(DownloadState::Failed.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_canceled() {
    assert!(DownloadState::Canceled.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_downloading() {
    assert!(!DownloadState::Downloading.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_paused() {
    assert!(!DownloadState::Paused.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_queued() {
    assert!(!DownloadState::Queued.is_terminal());
}

#[test]
fn test_state_helpers_is_terminal_verifying() {
    assert!(!DownloadState::Verifying.is_terminal());
}

// ═══════════════════════════════════════════════════════════════════════
//  map_state — complex state transition combinations
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_map_state_all_variants_exhaustive() {
    // Verify every irontide TorrentState maps to a DownloadState
    // without panicking or returning unexpected values.
    use irontide::session::TorrentState;
    let cases: Vec<(TorrentState, DownloadState)> = vec![
        (TorrentState::Downloading, DownloadState::Downloading),
        (TorrentState::Seeding, DownloadState::Completed),
        (TorrentState::Complete, DownloadState::Completed),
        (TorrentState::Paused, DownloadState::Paused),
        (TorrentState::Checking, DownloadState::Verifying),
        (TorrentState::FetchingMetadata, DownloadState::Queued),
        (TorrentState::Queued, DownloadState::Queued),
        (TorrentState::Stopped, DownloadState::Canceled),
        (TorrentState::Sharing, DownloadState::Downloading),
    ];
    for (input, expected) in &cases {
        assert_eq!(map_state(input), *expected, "TorrentState::{input:?}");
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Builder config mapping tests — verify BtSettings → irontide builder
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_encryption_mode_mapping_enabled() {
    use crate::types::BtEncryptionMode;
    let builder = irontide::ClientBuilder::new()
        .encryption_mode(match BtEncryptionMode::Enabled {
            BtEncryptionMode::Enabled => irontide::prelude::EncryptionMode::Enabled,
            BtEncryptionMode::Disabled => irontide::prelude::EncryptionMode::Disabled,
            BtEncryptionMode::Forced => irontide::prelude::EncryptionMode::Forced,
        });
    let settings = builder.into_settings();
    assert_eq!(settings.encryption_mode, irontide::prelude::EncryptionMode::Enabled);
}

#[test]
fn test_encryption_mode_mapping_disabled() {
    use crate::types::BtEncryptionMode;
    let builder = irontide::ClientBuilder::new()
        .encryption_mode(match BtEncryptionMode::Disabled {
            BtEncryptionMode::Enabled => irontide::prelude::EncryptionMode::Enabled,
            BtEncryptionMode::Disabled => irontide::prelude::EncryptionMode::Disabled,
            BtEncryptionMode::Forced => irontide::prelude::EncryptionMode::Forced,
        });
    let settings = builder.into_settings();
    assert_eq!(settings.encryption_mode, irontide::prelude::EncryptionMode::Disabled);
}

#[test]
fn test_encryption_mode_mapping_forced() {
    use crate::types::BtEncryptionMode;
    let builder = irontide::ClientBuilder::new()
        .encryption_mode(match BtEncryptionMode::Forced {
            BtEncryptionMode::Enabled => irontide::prelude::EncryptionMode::Enabled,
            BtEncryptionMode::Disabled => irontide::prelude::EncryptionMode::Disabled,
            BtEncryptionMode::Forced => irontide::prelude::EncryptionMode::Forced,
        });
    let settings = builder.into_settings();
    assert_eq!(settings.encryption_mode, irontide::prelude::EncryptionMode::Forced);
}

#[test]
fn test_preallocate_mode_mapping_none() {
    use crate::types::BtPreallocateMode;
    let builder = irontide::ClientBuilder::new()
        .preallocate_mode(match BtPreallocateMode::None {
            BtPreallocateMode::None => irontide::prelude::PreallocateMode::None,
            BtPreallocateMode::Full => irontide::prelude::PreallocateMode::Full,
        });
    let settings = builder.into_settings();
    assert_eq!(settings.preallocate_mode, irontide::prelude::PreallocateMode::None);
}

#[test]
fn test_preallocate_mode_mapping_full() {
    use crate::types::BtPreallocateMode;
    let builder = irontide::ClientBuilder::new()
        .preallocate_mode(match BtPreallocateMode::Full {
            BtPreallocateMode::None => irontide::prelude::PreallocateMode::None,
            BtPreallocateMode::Full => irontide::prelude::PreallocateMode::Full,
        });
    let settings = builder.into_settings();
    assert_eq!(settings.preallocate_mode, irontide::prelude::PreallocateMode::Full);
}

#[test]
fn test_builder_network_feature_defaults() {
    // Verify the default settings used by IrontideBtBackend match irontide defaults
    let settings = irontide::ClientBuilder::new().into_settings();
    // These should match the BtSettings defaults (all true)
    assert!(settings.enable_dht);
    assert!(settings.enable_upnp);
    assert!(settings.enable_natpmp);
    assert!(settings.enable_ipv6);
    assert!(settings.enable_pex);
    assert!(settings.enable_lsd);
    assert!(settings.enable_utp);
    assert!(settings.enable_fast_extension);
    assert!(settings.enable_holepunch);
    assert!(settings.enable_web_seed);
    // Super seeding defaults to false
    assert!(!settings.default_super_seeding);
}

#[test]
fn test_builder_feature_toggle_off() {
    // Verify we can toggle features off (as done in minimal sessions)
    let settings = irontide::ClientBuilder::new()
        .enable_dht(false)
        .enable_lsd(false)
        .enable_upnp(false)
        .enable_natpmp(false)
        .enable_ipv6(false)
        .enable_pex(false)
        .enable_utp(false)
        .into_settings();
    assert!(!settings.enable_dht);
    assert!(!settings.enable_lsd);
    assert!(!settings.enable_upnp);
    assert!(!settings.enable_natpmp);
    assert!(!settings.enable_ipv6);
    assert!(!settings.enable_pex);
    assert!(!settings.enable_utp);
}

#[test]
fn test_builder_queue_limits_match_defaults() {
    // BtSettings defaults: max_downloads=3, max_seeds=5, max_torrents=15, active_limit=10
    let settings = irontide::ClientBuilder::new()
        .active_downloads(3)
        .active_seeds(5)
        .max_torrents(15)
        .active_limit(10)
        .into_settings();
    assert_eq!(settings.active_downloads, 3);
    assert_eq!(settings.active_seeds, 5);
    assert_eq!(settings.max_torrents, 15);
    assert_eq!(settings.active_limit, 10);
}

#[test]
fn test_builder_dht_settings_propagate() {
    // DHT enabled
    let settings = irontide::ClientBuilder::new()
        .enable_dht(true)
        .into_settings();
    assert!(settings.enable_dht);

    // DHT disabled
    let settings = irontide::ClientBuilder::new()
        .enable_dht(false)
        .into_settings();
    assert!(!settings.enable_dht);
}

#[test]
fn test_internal_id_to_gid_consistent_with_aria2_format() {
    // Verify GID is always 16 lowercase hex chars (matching aria2 format)
    let ih = Id20::from([0xABu8; 20]);
    let gid = internal_id_to_gid(&ih);
    assert_eq!(gid.len(), 16);
    assert!(gid.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(gid, gid.to_ascii_lowercase(), "GID must be lowercase");
}

/// Create a minimal irontide session for testing.
async fn make_test_session() -> (tempfile::TempDir, irontide::session::SessionHandle) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dl_dir = tmp.path().join("dl");
    std::fs::create_dir_all(&dl_dir).expect("create dl_dir");
    let resume_dir = tmp.path().join("resume");
    std::fs::create_dir_all(&resume_dir).expect("create resume_dir");

    let irontide_settings = irontide::session::Settings {
        resume_data_dir: Some(resume_dir),
        ..Default::default()
    };
    let session = irontide::ClientBuilder::from_settings(irontide_settings)
        .listen_port(0)
        .enable_dht(false)
        .enable_lsd(false)
        .enable_upnp(false)
        .enable_natpmp(false)
        .enable_ipv6(false)
        .enable_pex(false)
        .enable_utp(false)
        .download_dir(&dl_dir)
        .start()
        .await
        .expect("create irontide session");
    (tmp, session)
}

#[tokio::test]
async fn test_session_create_and_shutdown() {
    let (_tmp, session) = make_test_session().await;

    // Verify empty session
    let stats = session.session_stats().await.unwrap();
    assert_eq!(stats.active_torrents, 0);

    let list = session.list_torrents().await.unwrap();
    assert!(list.is_empty());

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_magnet_add_and_remove() {
    let (_tmp, session) = make_test_session().await;

    // Add a magnet link
    let magnet = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test",
    )
    .unwrap();
    let info_hash = irontide::AddTorrentParams::from_magnet(magnet)
        .add_to(&session)
        .await
        .unwrap();

    assert_eq!(
        info_hash.to_hex(),
        "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
    );

    // Verify it's listed
    let list = session.list_torrents().await.unwrap();
    assert_eq!(list.len(), 1);
    assert!(list.contains(&info_hash));

    // Remove it
    session.remove_torrent(info_hash).await.unwrap();

    let list = session.list_torrents().await.unwrap();
    assert!(list.is_empty());

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_magnet_add_get_stats() {
    let (_tmp, session) = make_test_session().await;

    // Add a magnet link
    let magnet = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:da39a3ee5e6b4b0d3255bfef95601890afd80709&dn=empty",
    )
    .unwrap();
    let info_hash = irontide::AddTorrentParams::from_magnet(magnet)
        .add_to(&session)
        .await
        .unwrap();

    // Stats should be retrievable even for magnet-only torrents
    let stats = session.torrent_stats(info_hash).await.unwrap();
    // Name is empty for magnet links without resolved metadata
    assert!(!stats.has_metadata, "magnet links have no metadata yet");
    assert_eq!(stats.state, irontide::session::TorrentState::FetchingMetadata);

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_pause_resume_magnet() {
    let (_tmp, session) = make_test_session().await;

    // Add a magnet link
    let magnet = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:4a8eeb4c2f4f3ae1e2a8a3d4b5c6d7e8f9a0b1c2&dn=pause-test",
    )
    .unwrap();
    let info_hash = irontide::AddTorrentParams::from_magnet(magnet)
        .add_to(&session)
        .await
        .unwrap();

    // Pause
    session.pause_torrent(info_hash).await.unwrap();
    let stats = session.torrent_stats(info_hash).await.unwrap();
    assert_eq!(stats.state, irontide::session::TorrentState::Paused);

    // Resume
    session.resume_torrent(info_hash).await.unwrap();
    let stats = session.torrent_stats(info_hash).await.unwrap();
    assert_eq!(stats.state, irontide::session::TorrentState::FetchingMetadata);

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_multiple_torrents_listed() {
    let (_tmp, session) = make_test_session().await;

    let magnet1 = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=one",
    )
    .unwrap();
    let info_hash1 = irontide::AddTorrentParams::from_magnet(magnet1)
        .add_to(&session)
        .await
        .unwrap();

    let magnet2 = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:da39a3ee5e6b4b0d3255bfef95601890afd80709&dn=two",
    )
    .unwrap();
    let info_hash2 = irontide::AddTorrentParams::from_magnet(magnet2)
        .add_to(&session)
        .await
        .unwrap();

    let list = session.list_torrents().await.unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.contains(&info_hash1));
    assert!(list.contains(&info_hash2));

    // Get individual stats (names are empty before metadata resolution)
    let stats1 = session.torrent_stats(info_hash1).await.unwrap();
    assert!(!stats1.has_metadata);
    let stats2 = session.torrent_stats(info_hash2).await.unwrap();
    assert!(!stats2.has_metadata);
    // The two torrents are different
    assert_ne!(info_hash1, info_hash2);

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_session_stats_accessible() {
    let (_tmp, session) = make_test_session().await;

    // Add a magnet to have some session activity
    let magnet = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=stats-test",
    )
    .unwrap();
    let _info_hash = irontide::AddTorrentParams::from_magnet(magnet)
        .add_to(&session)
        .await
        .unwrap();

    let session_stats = session.session_stats().await.unwrap();
    assert_eq!(session_stats.active_torrents, 1);

    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_session_remove_with_files_and_readd() {
    let (_tmp, session) = make_test_session().await;

    let magnet = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=remove-me",
    )
    .unwrap();
    let info_hash = irontide::AddTorrentParams::from_magnet(magnet)
        .add_to(&session)
        .await
        .unwrap();

    // Remove with files
    session.remove_torrent_with_files(info_hash).await.unwrap();
    let list = session.list_torrents().await.unwrap();
    assert!(list.is_empty());

    // Re-add the same magnet
    let magnet2 = irontide::core::Magnet::parse(
        "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=readded",
    )
    .unwrap();
    let info_hash2 = irontide::AddTorrentParams::from_magnet(magnet2)
        .add_to(&session)
        .await
        .unwrap();
    assert_eq!(info_hash, info_hash2, "same info hash after re-add");

    let list = session.list_torrents().await.unwrap();
    assert_eq!(list.len(), 1);

    session.shutdown().await.unwrap();
}

// ═══════════════════════════════════════════════════════════════════════════
//  IrontideBtBackend — DownloadBackend trait implementation tests
// ═══════════════════════════════════════════════════════════════════════════
//
// These tests exercise the IrontideBtBackend wrapper struct's DownloadBackend
// trait implementation (start/pause/resume/cancel/remove/purge/status/list/
// shutdown), NOT the raw irontide session (tested above).
//
// Tests that require real network (DHT/tracker) are marked #[ignore].
//
// ── Helpers ────────────────────────────────────────────────────────────

/// Create a minimal IrontideBtBackend with all network features disabled.
async fn make_backend() -> (tempfile::TempDir, IrontideBtBackend) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");
    std::fs::create_dir_all(&state_dir).expect("create state_dir");
    let dl_dir = tmp.path().join("dl");
    std::fs::create_dir_all(&dl_dir).expect("create dl_dir");

    let mut settings = AppSettings::default();
    // Disable all network features so the session starts without I/O.
    settings.bt.dht_enabled = false;
    settings.bt.enable_lsd = false;
    settings.bt.upnp_enabled = false;
    settings.bt.enable_natpmp = false;
    settings.bt.enable_ipv6 = false;
    settings.bt.enable_pex = false;
    settings.bt.enable_utp = false;
    settings.bt.enable_holepunch = false;
    settings.bt.enable_fast_extension = false;
    settings.bt.enable_web_seed = false;
    // Let OS assign an ephemeral port to avoid conflicts in parallel runs.
    settings.bt.listen_port = Some(0);

    let event_bus = Arc::new(EventBus::new(16));
    let active_bt_count = Arc::new(AtomicUsize::new(0));
    let max_concurrent_bt = Arc::new(AtomicUsize::new(5));

    let backend = IrontideBtBackend::new(
        &settings,
        state_dir,
        dl_dir,
        event_bus,
        active_bt_count,
        max_concurrent_bt,
    )
    .await
    .expect("create IrontideBtBackend");

    (tmp, backend)
}

/// Create an IrontideBtBackend with network features enabled (DHT, PEX, uTP).
///
/// Used by #[ignore] network tests that actually need to discover peers.
/// Keep the listen port as OS-assigned (0) to avoid port conflicts.
async fn make_network_backend() -> (tempfile::TempDir, IrontideBtBackend) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");
    std::fs::create_dir_all(&state_dir).expect("create state_dir");
    let dl_dir = tmp.path().join("dl");
    std::fs::create_dir_all(&dl_dir).expect("create dl_dir");

    let mut settings = AppSettings::default();
    // Enable network features needed for actual peer discovery
    settings.bt.dht_enabled = true;
    settings.bt.enable_pex = true;
    settings.bt.enable_utp = true;
    // Let OS assign an ephemeral port to avoid conflicts in parallel runs.
    settings.bt.listen_port = Some(0);

    let event_bus = Arc::new(EventBus::new(16));
    let active_bt_count = Arc::new(AtomicUsize::new(0));
    let max_concurrent_bt = Arc::new(AtomicUsize::new(5));

    let backend = IrontideBtBackend::new(
        &settings,
        state_dir,
        dl_dir,
        event_bus,
        active_bt_count,
        max_concurrent_bt,
    )
    .await
    .expect("create IrontideBtBackend with network");

    (tmp, backend)
}

// ── No-network tests (always run) ──────────────────────────────────────

#[tokio::test]
async fn backend_new_and_shutdown() {
    let (_tmp, backend) = make_backend().await;
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_list_empty_on_start() {
    let (_tmp, backend) = make_backend().await;
    let list: Vec<DownloadSummary> = backend.list().await.expect("list should succeed");
    assert!(list.is_empty(), "expected empty list on fresh backend");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_start_returns_error_for_invalid_magnet() {
    let (_tmp, backend) = make_backend().await;
    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:invalid&dn=test".into(),
        ..Default::default()
    };
    let result: Result<TaskId, _> = DownloadBackend::start(&backend, request).await;
    assert!(result.is_err(), "expected error for invalid magnet link");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_start_returns_error_for_empty_url() {
    let (_tmp, backend) = make_backend().await;
    let request = StartDownloadRequest::default();
    let result: Result<TaskId, _> = DownloadBackend::start(&backend, request).await;
    assert!(result.is_err(), "expected error for empty URL");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_status_returns_not_found_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::status(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_status_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::status(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_pause_returns_not_found_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::pause(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_pause_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::pause(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_resume_returns_not_found_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::resume(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_resume_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::resume(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_cancel_returns_ok_for_unknown_id() {
    // Cancel is tolerant: always succeeds even for unknown info hashes,
    // returning a fallback snapshot with Canceled state.
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::cancel(&backend, &fake_id).await;
    assert!(result.is_ok(), "cancel should succeed even for unknown hash");
    let snapshot = result.unwrap();
    assert_eq!(
        snapshot.state,
        DownloadState::Canceled,
        "should return Canceled state for unknown hash"
    );
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_cancel_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::cancel(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_remove_returns_not_found_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::remove(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_remove_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::remove(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_purge_returns_not_found_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::purge(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_purge_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::purge(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_update_settings_does_not_panic() {
    let (_tmp, backend) = make_backend().await;
    let settings = AppSettings::default();
    let result = DownloadBackend::update_settings(&backend, &settings).await;
    assert!(result.is_ok(), "update_settings should succeed");
    backend.shutdown().await;
}

// ── Network tests (marked #[ignore], not run in CI) ────────────────────

#[ignore]
#[tokio::test]
async fn backend_start_magnet_and_poll_status() {
    let (_tmp, backend) = make_network_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test".into(),
        ..Default::default()
    };
    let task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed for valid magnet");

    // Verify it's listed
    let list = backend.list().await.expect("list should succeed");
    assert_eq!(list.len(), 1, "expected 1 torrent in list");
    assert_eq!(list[0].id, task_id.raw_id(), "task id should match");

    // Get status
    let status = DownloadBackend::status(&backend, &task_id)
        .await
        .expect("status should succeed for started magnet");
    assert_eq!(status.id, task_id.raw_id());
    assert_eq!(status.kind, TaskKind::Bt);

    backend.shutdown().await;
}

#[ignore]
#[tokio::test]
async fn backend_pause_resume_active_download() {
    let (_tmp, backend) = make_network_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test".into(),
        ..Default::default()
    };
    let task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed");

    // Pause
    let paused = DownloadBackend::pause(&backend, &task_id)
        .await
        .expect("pause should succeed");
    assert_eq!(paused.state, DownloadState::Paused, "should be paused");

    // Resume
    let resumed = DownloadBackend::resume(&backend, &task_id)
        .await
        .expect("resume should succeed");
    assert_ne!(
        resumed.state,
        DownloadState::Paused,
        "should no longer be paused after resume"
    );

    backend.shutdown().await;
}

#[ignore]
#[tokio::test]
async fn backend_cancel_active_download() {
    let (_tmp, backend) = make_network_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test".into(),
        ..Default::default()
    };
    let task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed");

    // Cancel
    let canceled = DownloadBackend::cancel(&backend, &task_id)
        .await
        .expect("cancel should succeed");
    assert_eq!(canceled.state, DownloadState::Canceled, "should be canceled");

    backend.shutdown().await;
}

#[ignore]
#[tokio::test]
async fn backend_shutdown_while_active() {
    let (_tmp, backend) = make_network_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test".into(),
        ..Default::default()
    };
    let _task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed");

    // Should not panic
    backend.shutdown().await;
}

// ── open_in_explorer error-path tests (no-network) ────────────────────

#[tokio::test]
async fn backend_open_in_explorer_returns_not_found_for_wrong_variant() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Http(uuid::Uuid::nil());
    let result = DownloadBackend::open_in_explorer(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for wrong TaskId variant");
    backend.shutdown().await;
}

#[tokio::test]
async fn backend_open_in_explorer_returns_error_for_unknown_id() {
    let (_tmp, backend) = make_backend().await;
    let fake_id = TaskId::Bt(Id20::from([0u8; 20]));
    let result = DownloadBackend::open_in_explorer(&backend, &fake_id).await;
    assert!(result.is_err(), "expected error for unknown info hash");
    backend.shutdown().await;
}

// ── Idempotency / edge-case tests (no-network) ────────────────────────

#[tokio::test]
async fn backend_double_shutdown_does_not_panic() {
    let (_tmp, backend) = make_backend().await;
    backend.shutdown().await;
    backend.shutdown().await; // second call must not panic
}

#[tokio::test(flavor = "multi_thread")]
async fn backend_pause_twice_no_panic() {
    let (_tmp, backend) = make_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:1111111111111111111111111111111111111111&dn=pause-twice".into(),
        ..Default::default()
    };
    let task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed for valid magnet");

    // First pause
    let paused = DownloadBackend::pause(&backend, &task_id).await;
    assert!(paused.is_ok(), "first pause should succeed");

    // Second pause should not panic
    let paused2 = DownloadBackend::pause(&backend, &task_id).await;
    assert!(paused2.is_ok(), "second pause should also succeed");

    backend.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn backend_resume_active_no_panic() {
    let (_tmp, backend) = make_backend().await;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:2222222222222222222222222222222222222222&dn=resume-active".into(),
        ..Default::default()
    };
    let task_id = DownloadBackend::start(&backend, request)
        .await
        .expect("start should succeed for valid magnet");

    // Resume while already active (not paused) should not panic
    let resumed = DownloadBackend::resume(&backend, &task_id).await;
    assert!(resumed.is_ok(), "resume-active should not panic");

    backend.shutdown().await;
}
