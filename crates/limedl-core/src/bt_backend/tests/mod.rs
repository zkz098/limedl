use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use irontide::core::{FileTreeNode, Id20, Id32, InfoDictV2, InfoHashes};

use super::IrontideBtBackend;
use super::alerts::extract_info_hash;
use super::anti_leech::peer_is_leecher;
use super::internal_id_to_gid;
use super::queries::sanitize_peer_client;
use super::snapshot::{
    StateHelpers, build_peer_flags, estimate_eta, map_state, preview_entries_from_meta,
    v1_file_entries,
};
use crate::event_bus::EventBus;
use crate::protocol::DownloadBackend;
use crate::types::{
    AppSettings, DownloadState, DownloadSummary, StartDownloadRequest, TaskId, TaskKind,
};

// ── map_state ──────────────────────────────────────────────────────────

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

// ── peer_is_leecher (anti-leech) ───────────────────────────────────────

const GRACE: u64 = 300;
const RATIO: f64 = 0.1;

/// Build a peer unchoked for `unchoke_secs`, with the given chokes/rates/progress.
fn make_leech_candidate(
    peer_choking: bool,
    am_choking: bool,
    download_rate: u64,
    upload_rate: u64,
    progress: f32,
) -> irontide::session::PeerInfo {
    let mut peer = make_peer();
    peer.peer_choking = peer_choking;
    peer.am_choking = am_choking;
    peer.download_rate = download_rate;
    peer.upload_rate = upload_rate;
    peer.progress = progress;
    peer
}

// ── estimate_eta ───────────────────────────────────────────────────────

// ── extract_info_hash ──────────────────────────────────────────────────

// ── v1_file_entries / preview_entries_from_meta ────────────────────────

// ═══════════════════════════════════════════════════════════════════════
//  Additional extract_info_hash variants
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
//  preview_entries_from_meta — Hybrid and V2
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
//  v1_file_entries — additional edge cases
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
//  internal_id_to_gid
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
//  StateHelpers::is_terminal
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
//  map_state — complex state transition combinations
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
//  Builder config mapping tests — verify BtSettings → irontide builder
// ═══════════════════════════════════════════════════════════════════════

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

// ── Network tests (marked #[ignore], not run in CI) ────────────────────

// ── open_in_explorer error-path tests (no-network) ────────────────────

// ── Idempotency / edge-case tests (no-network) ────────────────────────

mod alerts;
mod anti_leech;
mod backend_api;
mod eta;
mod id_gid;
mod peer_flags;
mod session;
mod settings_builder;
mod state_mapping;
mod torrent_meta;
