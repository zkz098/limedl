use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableError {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumMode {
    None,
    #[default]
    Blake3,
    Sha256,
    #[serde(rename = "xxh3_128")]
    Xxh3128,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    Queued,
    Downloading,
    Paused,
    Retrying,
    Verifying,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BtUploadStatus {
    #[default]
    Idle,
    Uploading,
    Paused,
    PausedByLimit,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    #[default]
    Http,
    Bt,
    Metalink,
    Sftp,
}

/// Typed task identifier replacing fragile string-prefix routing (`is_bt_task_id` / `is_sftp_task_id`).
///
/// All three variants hold the **external** (wire-format) string so that `as_str()` returns
/// exactly what the frontend sent.  Use `parse()` to construct from a raw download id and
/// `http_inner()` to strip the `"http:"` prefix before routing to the HTTP manager.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TaskId {
    Http(String),
    Bt(String),
    Sftp(String),
}

impl TaskId {
    /// Construct a `TaskId` by inspecting the string prefix.
    ///
    /// - Starts with `"bt:"`   → `Bt`
    /// - Starts with `"sftp:"` → `Sftp`
    /// - Everything else       → `Http`
    pub fn parse(id: &str) -> Self {
        if id.starts_with("bt:") {
            TaskId::Bt(id.to_string())
        } else if id.starts_with("sftp:") {
            TaskId::Sftp(id.to_string())
        } else {
            TaskId::Http(id.to_string())
        }
    }

    /// The external (wire-format) string that this `TaskId` was constructed from.
    pub fn as_str(&self) -> &str {
        match self {
            TaskId::Http(id) | TaskId::Bt(id) | TaskId::Sftp(id) => id.as_str(),
        }
    }

    /// Strip the `"http:"` prefix for routing to the HTTP download manager.
    ///
    /// # Panics
    /// Panics if called on a non-`Http` variant.
    pub fn http_inner(&self) -> &str {
        match self {
            TaskId::Http(id) => id.strip_prefix("http:").unwrap_or(id),
            _ => panic!("TaskId::http_inner called on {:?}", self),
        }
    }

    /// Produce an external (prefixed) HTTP task id from a raw internal UUID string.
    pub fn make_http(uuid: String) -> String {
        format!("http:{uuid}")
    }
}

impl From<&str> for TaskId {
    fn from(id: &str) -> Self {
        TaskId::parse(id)
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThreadMode {
    Fixed,
    #[default]
    Adaptive,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveProfile {
    Conservative,
    #[default]
    Balanced,
    Aggressive,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    #[default]
    Disabled,
    System,
    Manual,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerMode {
    Traditional,
    #[default]
    Automatic,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceLearningMode {
    #[default]
    Fixed,
    Mobile,
    SemiMobile,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadRequest {
    #[serde(default)]
    pub kind: Option<TaskKind>,
    pub url: String,
    pub destination_dir: String,
    pub file_name: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    pub thread_mode: Option<ThreadMode>,
    pub thread_count: Option<usize>,
    pub max_retries: Option<u32>,
    pub checksum: Option<ChecksumMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_limit_bps: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_limit_bps: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_file_indices: Option<Vec<usize>>,
    #[serde(default)]
    pub start_paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkInfo {
    pub index: usize,
    pub start: u64,
    pub end: u64,
    pub downloaded: u64,
    pub completed: bool,
    pub claimed_by: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSnapshot {
    pub id: String,
    #[serde(default)]
    pub kind: TaskKind,
    pub state: DownloadState,
    pub url: String,
    pub final_url: String,
    pub file_name: String,
    pub destination_path: String,
    pub temp_path: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub supports_ranges: bool,
    pub connection_count: usize,
    pub thread_mode: ThreadMode,
    pub requested_thread_count: Option<usize>,
    pub desired_thread_count: Option<usize>,
    pub allocated_thread_count: Option<usize>,
    pub adaptive_profile: Option<AdaptiveProfile>,
    pub thread_note: Option<String>,
    pub checksum: Option<String>,
    pub checksum_mode: ChecksumMode,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub error: Option<String>,
    pub speed_bytes_per_second: Option<f64>,
    pub eta_seconds: Option<u64>,
    pub uploaded_bytes: Option<u64>,
    pub upload_speed_bytes_per_second: Option<f64>,
    pub peer_count: Option<usize>,
    pub upload_status: Option<BtUploadStatus>,
    pub info_hash: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default)]
    pub cdn_accelerated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chunks: Vec<ChunkInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leech_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_limit_bps: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_limit_bps: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSummary {
    pub id: String,
    #[serde(default)]
    pub kind: TaskKind,
    pub state: DownloadState,
    pub url: String,
    pub file_name: String,
    pub destination_path: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub connection_count: usize,
    pub thread_mode: ThreadMode,
    pub requested_thread_count: Option<usize>,
    pub desired_thread_count: Option<usize>,
    pub allocated_thread_count: Option<usize>,
    pub adaptive_profile: Option<AdaptiveProfile>,
    pub thread_note: Option<String>,
    pub speed_bytes_per_second: Option<f64>,
    pub eta_seconds: Option<u64>,
    pub uploaded_bytes: Option<u64>,
    pub upload_speed_bytes_per_second: Option<f64>,
    pub peer_count: Option<usize>,
    pub upload_status: Option<BtUploadStatus>,
    pub info_hash: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub cdn_accelerated: bool,
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leech_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_limit_bps: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_limit_bps: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtRuntimeStatus {
    pub connected: bool,
    pub dht_enabled: bool,
    pub dht_nodes: Option<usize>,
    pub torrent_count: usize,
    pub peer_count: usize,
    pub upload_speed_bytes_per_second: Option<f64>,
    pub uploaded_bytes: u64,
    pub updated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leech_count: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentFileEntry {
    pub index: usize,
    pub path: String,
    pub size: u64,
}

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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtTrackerInfo {
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BtPieceInfo {
    pub index: u64,
    pub completed: bool,
}

impl From<&DownloadSnapshot> for DownloadSummary {
    fn from(value: &DownloadSnapshot) -> Self {
        Self {
            id: value.id.clone(),
            kind: value.kind,
            state: value.state,
            url: value.url.clone(),
            file_name: value.file_name.clone(),
            destination_path: value.destination_path.clone(),
            total_bytes: value.total_bytes,
            downloaded_bytes: value.downloaded_bytes,
            connection_count: value.connection_count,
            thread_mode: value.thread_mode,
            requested_thread_count: value.requested_thread_count,
            desired_thread_count: value.desired_thread_count,
            allocated_thread_count: value.allocated_thread_count,
            adaptive_profile: value.adaptive_profile,
            thread_note: value.thread_note.clone(),
            speed_bytes_per_second: value.speed_bytes_per_second,
            eta_seconds: value.eta_seconds,
            uploaded_bytes: value.uploaded_bytes,
            upload_speed_bytes_per_second: value.upload_speed_bytes_per_second,
            peer_count: value.peer_count,
            upload_status: value.upload_status,
            info_hash: value.info_hash.clone(),
            error: value.error.clone(),
            cdn_accelerated: value.cdn_accelerated,
            created_at_ms: value.created_at_ms,
            seed_count: value.seed_count,
            leech_count: value.leech_count,
            download_limit_bps: value.download_limit_bps,
            upload_limit_bps: value.upload_limit_bps,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    pub mode: ProxyMode,
    pub manual_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraditionalSchedulerSettings {
    pub max_parallel_tasks: usize,
}

impl Default for TraditionalSchedulerSettings {
    fn default() -> Self {
        Self {
            max_parallel_tasks: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomaticSchedulerSettings {
    pub max_parallel_threads: usize,
    pub max_threads_per_task: usize,
    #[serde(default = "default_min_threads")]
    pub min_threads_per_task: usize,
    pub adaptive_profile: AdaptiveProfile,
}

fn default_min_threads() -> usize {
    0
}

impl Default for AutomaticSchedulerSettings {
    fn default() -> Self {
        Self {
            max_parallel_threads: 16,
            max_threads_per_task: 8,
            min_threads_per_task: 0,
            adaptive_profile: AdaptiveProfile::Balanced,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerSettings {
    pub mode: SchedulerMode,
    pub traditional: TraditionalSchedulerSettings,
    pub automatic: AutomaticSchedulerSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDefaultsSettings {
    pub default_download_dir: String,
    pub default_max_retries: u32,
    pub default_checksum: ChecksumMode,
    #[serde(default = "default_http_user_agent")]
    pub default_user_agent: String,
    #[serde(default)]
    pub enable_metalink: bool,
    #[serde(default)]
    pub enable_sftp: bool,
}

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
        }
    }
}

fn default_true() -> bool {
    true
}

pub fn default_tracker_list_url() -> String {
    String::from("https://cf.trackerslist.com/best.txt")
}

pub fn default_http_user_agent() -> String {
    String::from(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    )
}

impl Default for DownloadDefaultsSettings {
    fn default() -> Self {
        Self {
            default_download_dir: String::new(),
            default_max_retries: 5,
            default_checksum: ChecksumMode::Blake3,
            default_user_agent: default_http_user_agent(),
            enable_metalink: false,
            enable_sftp: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkLearningMetrics {
    pub estimated_bandwidth_bps: f64,
    pub stability_score: f64,
    pub penalty_rate: f64,
    pub recommended_initial_threads: usize,
    pub recommended_max_threads_per_task_cap: usize,
    pub sample_count: u32,
    pub last_observed_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSceneProfile {
    pub id: String,
    pub name: String,
    pub learning_enabled: bool,
    pub learned_metrics: Option<NetworkLearningMetrics>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkLearningSettings {
    pub device_mode: DeviceLearningMode,
    pub current_scene_id: String,
    pub scenes: Vec<NetworkSceneProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub level: LogLevel,
    #[serde(default)]
    pub file_path: String,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            level: LogLevel::Info,
            file_path: String::new(),
        }
    }
}

impl Default for NetworkLearningSettings {
    fn default() -> Self {
        Self {
            device_mode: DeviceLearningMode::Fixed,
            current_scene_id: String::from("default"),
            scenes: vec![NetworkSceneProfile {
                id: String::from("default"),
                name: String::from("默认场景"),
                learning_enabled: true,
                learned_metrics: None,
                updated_at_ms: 0,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ThemeColor {
    #[default]
    Default,
    Amber,
    Sky,
    Lime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundOpacityPreset {
    #[default]
    Default,
    Acrylic,
    Frosted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    #[serde(default)]
    pub theme_color: ThemeColor,
    #[serde(default)]
    pub background_opacity: BackgroundOpacityPreset,
    #[serde(default)]
    pub color_mode: ColorMode,
    #[serde(default = "default_true")]
    pub show_detail_info: bool,
    #[serde(default = "default_true")]
    pub show_heatmap: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub appearance: AppearanceSettings,
    #[serde(default)]
    pub proxy: ProxySettings,
    #[serde(default)]
    pub scheduler: SchedulerSettings,
    #[serde(default)]
    pub download: DownloadDefaultsSettings,
    #[serde(default)]
    pub bt: BtSettings,
    #[serde(default)]
    pub network_learning: NetworkLearningSettings,
    #[serde(default)]
    pub logging: LogSettings,
    #[serde(default)]
    pub aria2_rpc: Aria2RpcSettings,
    #[serde(default)]
    pub cdn_acceleration: CdnAccelerationSettings,
    #[serde(default)]
    pub global_speed_limit_bps: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2RpcSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_aria2_port")]
    pub port: u16,
    #[serde(default)]
    pub secret: Option<String>,
}

fn default_aria2_port() -> u16 {
    6800
}

impl Default for Aria2RpcSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 6800,
            secret: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdnAccelerationSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub active_ip: Option<String>,
    #[serde(default)]
    pub active_speed_mbps: Option<f64>,
    #[serde(default)]
    pub last_test_at_ms: Option<u64>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdn_settings_round_trip() {
        let original = CdnAccelerationSettings {
            enabled: true,
            active_ip: Some("192.168.1.100".into()),
            active_speed_mbps: Some(45.5),
            last_test_at_ms: Some(1700000000000),
            last_error: Some("timeout".into()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CdnAccelerationSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_cdn_settings_defaults() {
        let json = "{}";
        let settings: CdnAccelerationSettings = serde_json::from_str(json).unwrap();
        assert!(!settings.enabled);
        assert!(settings.active_ip.is_none());
        assert!(settings.active_speed_mbps.is_none());
        assert!(settings.last_test_at_ms.is_none());
        assert!(settings.last_error.is_none());
    }

    #[test]
    fn test_app_settings_backward_compat() {
        let json = r#"{
            "appearance": {"themeColor": "default", "backgroundOpacity": "default", "colorMode": "system", "showDetailInfo": true, "showHeatmap": true},
            "proxy": {"mode": "system", "manualUrl": ""},
            "scheduler": {"mode": "traditional", "traditional": {"maxParallelTasks": 3}, "automatic": {"maxParallelThreads": 8, "maxThreadsPerTask": 4, "minThreadsPerTask": 1, "adaptiveProfile": "conservative"}},
            "download": {"defaultDownloadDir": "", "defaultMaxRetries": 3, "defaultChecksum": "blake3", "defaultUserAgent": "", "enableMetalink": false, "enableSftp": false},
            "bt": {"pauseUploadWhenLimitReached": false, "uploadLimitBytes": 0, "uploadRatioLimit": 0, "dhtEnabled": true, "trackerList": "", "trackerListUrl": ""},
            "networkLearning": {"deviceMode": "fixed", "currentSceneId": "", "scenes": []},
            "logging": {"enabled": false, "level": "info", "filePath": ""},
            "aria2Rpc": {"enabled": false, "port": 6800, "secret": null}
        }"#;
        let settings: AppSettings = serde_json::from_str(json).unwrap();
        assert!(!settings.cdn_acceleration.enabled);
    }

    #[test]
    fn test_settings_round_trip_with_cdn() {
        let original = AppSettings {
            cdn_acceleration: CdnAccelerationSettings {
                enabled: true,
                active_ip: Some("10.0.0.1".into()),
                active_speed_mbps: Some(88.3),
                last_test_at_ms: Some(1700000000000),
                last_error: None,
            },
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(original.cdn_acceleration, deserialized.cdn_acceleration);
        assert!(deserialized.cdn_acceleration.enabled);
        assert_eq!(
            deserialized.cdn_acceleration.active_ip.as_deref(),
            Some("10.0.0.1")
        );
        assert_eq!(deserialized.cdn_acceleration.active_speed_mbps, Some(88.3));
    }
}
