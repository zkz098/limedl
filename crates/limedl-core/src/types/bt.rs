use serde::{Deserialize, Serialize};

#[cfg(feature = "ts")]
use ts_rs::TS;

use super::common::default_true;
use super::settings::default_tracker_list_url;

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BtUploadStatus {
    #[default]
    Idle,
    Uploading,
    Paused,
    PausedByLimit,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtRuntimeStatus {
    pub connected: bool,
    pub dht_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dht_nodes: Option<usize>,
    pub torrent_count: usize,
    pub peer_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_speed_bytes_per_second: Option<f64>,
    pub uploaded_bytes: u64,
    pub updated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leech_count: Option<u64>,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentFileEntry {
    pub index: usize,
    pub path: String,
    pub size: u64,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtPeerInfo {
    pub address: String,
    pub client: String,
    pub flags: String,
    pub download_speed: f64,
    pub upload_speed: f64,
    pub progress: f64,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtTrackerInfo {
    pub url: String,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtPieceInfo {
    pub index: u64,
    pub completed: bool,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtFileStatus {
    pub index: usize,
    pub path: String,
    pub size: u64,
    pub downloaded_bytes: u64,
    pub included: bool,
}

/// Preallocation strategy for torrent files.
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BtPreallocateMode {
    /// Sparse files (no preallocation).
    #[default]
    None,
    /// Full preallocation.
    Full,
}

/// Protocol encryption (MSE/PE) mode.
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BtEncryptionMode {
    /// Encryption enabled but not required.
    #[default]
    Enabled,
    /// Encryption disabled.
    Disabled,
    /// Require encryption.
    Forced,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BtPortRange {
    pub start: u16,
    pub end: u16,
}

/// Enforcement action taken against peers identified as leechers.
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BtAntiLeechAction {
    /// Session-wide ban of the offending peer IP (ban/unban managed by the loop).
    #[default]
    Ban,
    /// Reduce per-torrent upload (unchoke) slots so fewer leechers are served.
    LimitSlots,
}

/// Seed-mode choking algorithm (engine tuning).
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BtSeedChokingAlgorithm {
    /// Unchoke the peers we upload to fastest.
    #[default]
    FastestUpload,
    /// Round-robin through all interested peers.
    RoundRobin,
    /// Prefer leechers over seeds (anti-leech).
    AntiLeech,
}

/// Top-level unchoke-slot algorithm (engine tuning).
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BtChokingAlgorithm {
    /// Fixed number of unchoke slots.
    #[default]
    FixedSlots,
    /// Rate-based unchoking (auto-adjusts slots).
    RateBased,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtSettings {
    #[serde(default = "default_true")]
    pub dht_enabled: bool,
    #[serde(default)]
    pub tracker_list: String,
    #[serde(default = "default_tracker_list_url")]
    pub tracker_list_url: String,
    pub pause_upload_when_limit_reached: bool,
    pub upload_limit_bytes: u64,
    pub upload_ratio_limit: f64,

    // -- Anti-leech (反吸血) policy loop --
    /// Master switch for the anti-leech background loop.
    #[serde(default)]
    pub anti_leech_enabled: bool,
    /// How offending peers are handled.
    #[serde(default)]
    pub anti_leech_action: BtAntiLeechAction,
    /// Min seconds we must have been unchoking a peer before it can be flagged
    /// as a leecher (warm-up grace; avoids penalising slow-start peers).
    #[serde(default = "default_anti_leech_grace_secs")]
    pub anti_leech_grace_secs: u64,
    /// Min give-back share (own download / own upload) a peer must sustain to
    /// avoid being flagged when it is not choking us. 0 disables the ratio check.
    #[serde(default = "default_anti_leech_ratio")]
    pub anti_leech_ratio: f64,
    /// Ban duration in seconds; a peer is auto-unbanned after this for forgiveness.
    #[serde(default = "default_anti_leech_ban_secs")]
    pub anti_leech_ban_secs: u64,
    /// When action = LimitSlots, max concurrent unchoke slots per torrent that
    /// currently has detected leechers.
    #[serde(default = "default_anti_leech_max_upload_slots")]
    pub anti_leech_max_upload_slots: u32,

    // -- Engine tuning (passed through to the irontide choker/peer manager) --
    /// Seed-mode choking algorithm.
    #[serde(default)]
    pub seed_choking_algorithm: BtSeedChokingAlgorithm,
    /// Top-level unchoke-slot algorithm.
    #[serde(default)]
    pub choking_algorithm: BtChokingAlgorithm,
    /// Maximum upload (unchoke) slots per torrent.
    #[serde(default = "default_max_upload_slots_per_torrent")]
    pub max_upload_slots_per_torrent: u32,
    /// Maximum peer connections per torrent.
    #[serde(default = "default_max_peers_per_torrent")]
    pub max_peers_per_torrent: u32,
    /// Hash-failure involvements before the engine auto-bans a peer (smart ban).
    #[serde(default = "default_smart_ban_max_failures")]
    pub smart_ban_max_failures: u32,
    /// Use parole to isolate the offending peer before striking (smart ban).
    #[serde(default = "default_true")]
    pub smart_ban_parole: bool,
    /// Seconds an evicted peer is blocked from reconnecting before it may rejoin.
    #[serde(default = "default_eviction_ban_duration_secs")]
    pub eviction_ban_duration_secs: u64,
    /// Seconds without receiving piece data before the engine disconnects a
    /// peer (0 = disabled). Helps drop under-contributing peers.
    #[serde(default = "default_data_contribution_timeout_secs")]
    pub data_contribution_timeout_secs: u64,

    // -- IP blocklist (反吸血黑名单) --
    /// Master switch for loading a peer IP blocklist into the session.
    #[serde(default)]
    pub blocklist_enabled: bool,
    /// Path to a blocklist file (eMule `.dat` or P2P plaintext, one CIDR per line).
    #[serde(default)]
    pub blocklist_path: String,
    #[serde(default)]
    pub upnp_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_port_range: Option<BtPortRange>,

    // -- Network & ports --
    /// TCP listen port. None = OS assigns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,
    /// Enable NAT-PMP/PCP port mapping.
    #[serde(default = "default_true")]
    pub enable_natpmp: bool,
    /// Enable IPv6 dual-stack.
    #[serde(default = "default_true")]
    pub enable_ipv6: bool,

    // -- Discovery protocols --
    /// Peer Exchange (BEP 11).
    #[serde(default = "default_true")]
    pub enable_pex: bool,
    /// Local Service Discovery (BEP 14).
    #[serde(default = "default_true")]
    pub enable_lsd: bool,
    /// µTP micro transport protocol (BEP 29).
    #[serde(default = "default_true")]
    pub enable_utp: bool,
    /// Fast Extension (BEP 6).
    #[serde(default = "default_true")]
    pub enable_fast_extension: bool,
    /// Holepunch (BEP 55).
    #[serde(default = "default_true")]
    pub enable_holepunch: bool,
    /// HTTP Web Seed support.
    #[serde(default = "default_true")]
    pub enable_web_seed: bool,
    /// Super seeding mode (BEP 16). Default OFF.
    #[serde(default)]
    pub enable_super_seeding: bool,

    // -- Global rate limits (bytes/sec, 0 = unlimited) --
    #[serde(default)]
    pub global_download_rate_limit: u64,
    #[serde(default)]
    pub global_upload_rate_limit: u64,

    // -- Disk & security --
    /// File preallocation strategy. (irontide only)
    #[serde(default)]
    pub preallocate_mode: BtPreallocateMode,
    /// Protocol encryption mode. (irontide only)
    #[serde(default)]
    pub encryption_mode: BtEncryptionMode,

    // -- Queue strategy --
    /// Max auto-managed active downloads. (irontide only)
    #[serde(default = "default_max_downloads")]
    pub max_downloads: u32,
    /// Max auto-managed active seed tasks. (irontide only)
    #[serde(default = "default_max_seeds")]
    pub max_seeds: u32,
    /// Max total torrents. (irontide only)
    #[serde(default = "default_max_torrents")]
    pub max_torrents: u32,
    /// Hard limit on total active torrents (downloading + seeding + checking). (irontide only)
    #[serde(default = "default_active_limit")]
    pub active_limit: u32,
}

impl Default for BtSettings {
    fn default() -> Self {
        Self {
            dht_enabled: true,
            tracker_list: String::new(),
            tracker_list_url: default_tracker_list_url(),
            pause_upload_when_limit_reached: false,
            upload_limit_bytes: 0,
            upload_ratio_limit: 0.0,
            anti_leech_enabled: false,
            anti_leech_action: BtAntiLeechAction::default(),
            anti_leech_grace_secs: default_anti_leech_grace_secs(),
            anti_leech_ratio: default_anti_leech_ratio(),
            anti_leech_ban_secs: default_anti_leech_ban_secs(),
            anti_leech_max_upload_slots: default_anti_leech_max_upload_slots(),
            seed_choking_algorithm: BtSeedChokingAlgorithm::default(),
            choking_algorithm: BtChokingAlgorithm::default(),
            max_upload_slots_per_torrent: default_max_upload_slots_per_torrent(),
            max_peers_per_torrent: default_max_peers_per_torrent(),
            smart_ban_max_failures: default_smart_ban_max_failures(),
            smart_ban_parole: true,
            eviction_ban_duration_secs: default_eviction_ban_duration_secs(),
            data_contribution_timeout_secs: default_data_contribution_timeout_secs(),
            blocklist_enabled: false,
            blocklist_path: String::new(),
            upnp_enabled: false,
            listen_port_range: None,
            listen_port: None,
            enable_natpmp: true,
            enable_ipv6: true,
            enable_pex: true,
            enable_lsd: true,
            enable_utp: true,
            enable_fast_extension: true,
            enable_holepunch: true,
            enable_web_seed: true,
            enable_super_seeding: false,
            global_download_rate_limit: 0,
            global_upload_rate_limit: 0,
            preallocate_mode: BtPreallocateMode::default(),
            encryption_mode: BtEncryptionMode::default(),
            max_downloads: default_max_downloads(),
            max_seeds: default_max_seeds(),
            max_torrents: default_max_torrents(),
            active_limit: default_active_limit(),
        }
    }
}

pub(crate) fn default_anti_leech_grace_secs() -> u64 {
    300
}
pub(crate) fn default_anti_leech_ratio() -> f64 {
    0.1
}
pub(crate) fn default_anti_leech_ban_secs() -> u64 {
    3600
}
pub(crate) fn default_anti_leech_max_upload_slots() -> u32 {
    4
}

pub(crate) fn default_max_upload_slots_per_torrent() -> u32 {
    4
}
pub(crate) fn default_max_peers_per_torrent() -> u32 {
    128
}
pub(crate) fn default_smart_ban_max_failures() -> u32 {
    3
}
pub(crate) fn default_eviction_ban_duration_secs() -> u64 {
    600
}
pub(crate) fn default_data_contribution_timeout_secs() -> u64 {
    60
}

pub(crate) fn default_max_downloads() -> u32 {
    3
}
pub(crate) fn default_max_seeds() -> u32 {
    5
}
pub(crate) fn default_max_torrents() -> u32 {
    100
}
pub(crate) fn default_active_limit() -> u32 {
    500
}
