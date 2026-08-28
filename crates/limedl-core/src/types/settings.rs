use serde::{Deserialize, Serialize};

#[cfg(feature = "ts")]
use ts_rs::TS;

use super::bt::BtSettings;
use super::common::{
    default_true, AdaptiveProfile, BackgroundOpacityPreset, ChecksumMode, CloseBehavior, ColorMode,
    DiskType, DoubleClickSettings, LogLevel, ProxyMode, SchedulerMode, SortDirection, SortKey,
    SpeedLimitSlot, ThemeColor, UrlRewriteRule,
};

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    pub mode: ProxyMode,
    pub manual_url: String,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChunkSizeStrategy {
    #[default]
    Adaptive,
    Fixed,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerSettings {
    pub mode: SchedulerMode,
    pub traditional: TraditionalSchedulerSettings,
    pub automatic: AutomaticSchedulerSettings,
    #[serde(default)]
    pub chunk_size_strategy: ChunkSizeStrategy,
    #[serde(default)]
    pub tail_sprint_enabled: bool,
    #[serde(default = "default_warmup")]
    pub connection_warmup_enabled: bool,
}

fn default_warmup() -> bool {
    true
}

impl Default for SchedulerSettings {
    fn default() -> Self {
        Self {
            mode: SchedulerMode::default(),
            traditional: TraditionalSchedulerSettings::default(),
            automatic: AutomaticSchedulerSettings::default(),
            chunk_size_strategy: ChunkSizeStrategy::default(),
            tail_sprint_enabled: false,
            connection_warmup_enabled: true,
        }
    }
}

pub fn default_tracker_list_url() -> String {
    String::from("https://cf.trackerslist.com/best.txt")
}

pub fn default_http_user_agent() -> String {
    String::from(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    )
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDefaultsSettings {
    pub default_download_dir: String,
    pub default_max_retries: u32,
    pub default_checksum: ChecksumMode,
    #[serde(default = "default_http_user_agent")]
    pub default_user_agent: String,
    #[serde(default = "default_true")]
    pub auto_detect_sha256: bool,
}

impl Default for DownloadDefaultsSettings {
    fn default() -> Self {
        Self {
            default_download_dir: String::new(),
            default_max_retries: 5,
            default_checksum: ChecksumMode::Blake3,
            default_user_agent: default_http_user_agent(),
            auto_detect_sha256: true,
        }
    }
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub level: LogLevel,
    #[serde(default)]
    pub file_path: String,

    /// Maximum number of rotated startup log files to keep (e.g., `limedl.1.log`, `limedl.2.log`, ...).
    /// `None` = no count-based cleanup. `Some(0)` = delete all old logs.
    #[serde(default)]
    pub retention_count: Option<u32>,

    /// Maximum age in days of log files to keep. Older files are deleted on startup.
    /// `None` = no age-based cleanup.
    #[serde(default)]
    pub retention_days: Option<u32>,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            level: LogLevel::Info,
            file_path: String::new(),
            retention_count: None,
            retention_days: None,
        }
    }
}

fn default_visible_columns() -> Vec<String> {
    vec![
        "file".into(),
        "size".into(),
        "downloaded".into(),
        "status".into(),
        "progress".into(),
        "speed".into(),
        "eta".into(),
    ]
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    pub sort_key: SortKey,
    #[serde(default)]
    pub sort_direction: SortDirection,
    #[serde(default)]
    pub compact_view: bool,
    #[serde(default = "default_visible_columns")]
    pub visible_columns: Vec<String>,
    /// Behavior when closing the main window: exit or minimize to tray.
    #[serde(default)]
    pub close_behavior: CloseBehavior,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme_color: Default::default(),
            background_opacity: Default::default(),
            color_mode: Default::default(),
            show_detail_info: true,
            show_heatmap: true,
            sort_key: Default::default(),
            sort_direction: Default::default(),
            compact_view: false,
            visible_columns: default_visible_columns(),
            close_behavior: Default::default(),
        }
    }
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    #[serde(default)]
    pub enabled: bool,
}

fn default_buffer_limit_mb() -> u64 {
    1024
}

fn default_game_mode_buffer_mb() -> u64 {
    128
}

fn default_max_parallel_hdd() -> u32 {
    4
}

fn default_game_mode_max_parallel() -> u32 {
    1
}

fn default_hdd_buffer_enabled() -> bool {
    true
}

/// I/O baseline settings for HDD/SSD intelligent buffer optimization.
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IoBaselineSettings {
    /// Total memory buffer pool limit in MiB for HDD downloads.
    /// Default: 1024 MiB (1 GiB). User can adjust.
    #[serde(default = "default_buffer_limit_mb")]
    pub buffer_limit_mb: u64,
    /// Buffer size in MiB to use when game/performance mode is active.
    /// Default: 128 MiB.
    #[serde(default = "default_game_mode_buffer_mb")]
    pub game_mode_buffer_mb: u64,
    /// Whether game/performance mode is currently active (runtime-only, never persisted).
    #[cfg_attr(feature = "ts", ts(type = "boolean"))]
    #[serde(default, skip)]
    pub game_mode: bool,
    /// Maximum number of parallel HDD download buffers (slots).
    /// Default: 4. User can adjust.
    #[serde(default = "default_max_parallel_hdd")]
    pub max_parallel_hdd: u32,
    /// Reduced max-parallel when game mode is active.
    /// Default: 1.
    #[serde(default = "default_game_mode_max_parallel")]
    pub game_mode_max_parallel: u32,
    /// User-specified disk type overrides keyed by directory path.
    /// e.g. {"D:\\downloads": "hdd"} forces that directory to be treated as HDD.
    #[cfg_attr(feature = "ts", ts(type = "Record<string, DiskType>"))]
    #[serde(default)]
    pub disk_type_overrides: foldhash::HashMap<String, DiskType>,
    /// Whether HDD double-buffer optimization is enabled.
    /// When disabled on HDD, uses a small 4 MiB write-combining buffer instead of the pool.
    /// Default: true.
    #[serde(default = "default_hdd_buffer_enabled")]
    pub hdd_buffer_enabled: bool,
    /// SSD write-combining buffer size per download in MiB.
    /// 0 = auto (use chunk size). Max 4096 MiB (4 GiB).
    /// Default: 0.
    #[serde(default)]
    pub ssd_write_combine_mb: u64,
}

impl Default for IoBaselineSettings {
    fn default() -> Self {
        Self {
            buffer_limit_mb: default_buffer_limit_mb(),
            game_mode_buffer_mb: default_game_mode_buffer_mb(),
            game_mode: false,
            max_parallel_hdd: default_max_parallel_hdd(),
            game_mode_max_parallel: default_game_mode_max_parallel(),
            disk_type_overrides: foldhash::HashMap::default(),
            hdd_buffer_enabled: default_hdd_buffer_enabled(),
            ssd_write_combine_mb: 0,
        }
    }
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlRewriteSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub rules: Vec<UrlRewriteRule>,
}

fn default_max_in_memory_downloads() -> usize {
    200
}

fn default_aria2_port() -> u16 {
    6800
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2RpcSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_aria2_port")]
    pub port: u16,
    #[serde(default)]
    pub secret: Option<String>,
    /// Allowed CORS origins for the Aria2 RPC HTTP endpoint.
    /// If empty, defaults to ["http://localhost", "http://127.0.0.1"].
    /// If empty AND allow_any_origin is true, allows all origins (insecure).
    #[serde(default)]
    pub cors_allowed_origins: Vec<String>,
}

impl Default for Aria2RpcSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 6800,
            secret: None,
            cors_allowed_origins: Vec::new(),
        }
    }
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdnAccelerationSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub custom_test_url: Option<String>,
    #[serde(default)]
    pub custom_cidrs: Option<String>,
    #[serde(default)]
    pub active_ip: Option<String>,
    #[serde(default)]
    pub active_speed_mbps: Option<f64>,
    #[serde(default)]
    pub last_test_at_ms: Option<u64>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubMirrorSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mirrors: Vec<super::common::MirrorEntry>,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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
    pub logging: LogSettings,
    #[serde(default)]
    pub aria2_rpc: Aria2RpcSettings,
    #[serde(default)]
    pub cdn_acceleration: CdnAccelerationSettings,
    #[serde(default)]
    pub github_mirror: GitHubMirrorSettings,
    #[serde(default)]
    pub url_rewrite: UrlRewriteSettings,
    #[serde(default)]
    pub global_speed_limit_bps: u64,
    #[serde(default)]
    pub speed_limit_schedule: Vec<SpeedLimitSlot>,
    #[serde(default)]
    pub notifications: NotificationSettings,
    #[serde(default)]
    pub io_baseline: IoBaselineSettings,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub setup_completed: bool,
    #[serde(default)]
    pub last_setup_step: Option<u32>,
    /// Double-click action configuration for download tasks.
    #[serde(default)]
    pub double_click: DoubleClickSettings,
    /// Maximum number of completed/failed/canceled downloads kept in memory.
    /// Older terminal-state entries are evicted when this limit is exceeded.
    #[serde(default = "default_max_in_memory_downloads")]
    pub max_in_memory_downloads: usize,
}
