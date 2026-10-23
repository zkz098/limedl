use serde::{Deserialize, Serialize};

use super::bt::BtSettings;
use super::common::{
    AdaptiveProfile, BackgroundOpacityPreset, ChecksumMode, CloseBehavior, ColorMode, DiskType,
    DoubleClickSettings, LogLevel, ProxyMode, SchedulerMode, SortDirection, SortKey,
    SpeedLimitSlot, ThemeColor, UrlRewriteRule, default_true,
};
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueSortStrategy {
    #[default]
    PriorityFirst,
    ShortestJobFirst,
    Fifo,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChunkSizeStrategy {
    #[default]
    Adaptive,
    Fixed,
}

fn default_scheduler_tick_ms() -> u64 {
    2000
}

fn default_max_connections_per_host() -> usize {
    16
}

fn default_aimd_degrade_threshold() -> f64 {
    0.16
}

fn default_aimd_increase_threshold() -> f64 {
    0.04
}

fn default_aimd_severe_drop_threshold() -> f64 {
    0.50
}

fn default_aimd_cooldown_secs() -> u64 {
    3
}

fn default_aimd_samples_needed() -> u32 {
    1
}

fn default_aimd_probe_stable_cycles() -> u32 {
    2
}

fn default_aimd_decrease_factor() -> f64 {
    0.50
}

fn default_aimd_step_ratio() -> f64 {
    0.25
}

fn default_aimd_oscillation_threshold() -> u32 {
    3
}

fn default_aimd_hysteresis_multiplier() -> u32 {
    4
}

fn default_tail_sprint_stall_window_secs() -> u64 {
    8
}

fn default_tail_sprint_min_split_size_mb() -> u64 {
    1
}

fn default_work_steal_min_split_size_mb() -> u64 {
    2
}

fn default_connect_timeout_secs() -> u64 {
    30
}

fn default_read_timeout_secs() -> u64 {
    30
}

fn default_pool_max_idle_per_host() -> usize {
    20
}

fn default_tcp_keepalive_secs() -> u64 {
    60
}

fn default_retry_backoff_base_ms() -> u64 {
    250
}

fn default_retry_backoff_max_ms() -> u64 {
    4000
}

fn default_retry_backoff_jitter_percent() -> u64 {
    25
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpertSchedulerSettings {
    // 1. Scheduler Loop & Host Limits
    #[serde(default = "default_scheduler_tick_ms")]
    pub scheduler_tick_ms: u64,
    #[serde(default = "default_max_connections_per_host")]
    pub max_connections_per_host: usize,

    // 2. AIMD Dynamic Tuning
    #[serde(default = "default_aimd_degrade_threshold")]
    pub aimd_degrade_threshold: f64,
    #[serde(default = "default_aimd_increase_threshold")]
    pub aimd_increase_threshold: f64,
    #[serde(default = "default_aimd_severe_drop_threshold")]
    pub aimd_severe_drop_threshold: f64,
    #[serde(default = "default_aimd_cooldown_secs")]
    pub aimd_cooldown_secs: u64,
    #[serde(default = "default_aimd_samples_needed")]
    pub aimd_samples_needed: u32,
    #[serde(default = "default_aimd_probe_stable_cycles")]
    pub aimd_probe_stable_cycles: u32,
    #[serde(default = "default_aimd_decrease_factor")]
    pub aimd_decrease_factor: f64,
    #[serde(default = "default_aimd_step_ratio")]
    pub aimd_step_ratio: f64,
    #[serde(default = "default_aimd_oscillation_threshold")]
    pub aimd_oscillation_threshold: u32,
    #[serde(default = "default_aimd_hysteresis_multiplier")]
    pub aimd_hysteresis_multiplier: u32,

    // 3. Chunking, Tail Sprint & Work Stealing
    #[serde(default)]
    pub custom_chunk_size_mb: u64,
    #[serde(default = "default_tail_sprint_stall_window_secs")]
    pub tail_sprint_stall_window_secs: u64,
    #[serde(default = "default_tail_sprint_min_split_size_mb")]
    pub tail_sprint_min_split_size_mb: u64,
    #[serde(default = "default_work_steal_min_split_size_mb")]
    pub work_steal_min_split_size_mb: u64,

    // 4. Transport & Connection Pool
    #[serde(default = "default_connect_timeout_secs")]
    pub connect_timeout_secs: u64,
    #[serde(default = "default_read_timeout_secs")]
    pub read_timeout_secs: u64,
    #[serde(default = "default_pool_max_idle_per_host")]
    pub pool_max_idle_per_host: usize,
    #[serde(default = "default_tcp_keepalive_secs")]
    pub tcp_keepalive_secs: u64,

    // 5. Retry Backoff
    #[serde(default = "default_retry_backoff_base_ms")]
    pub retry_backoff_base_ms: u64,
    #[serde(default = "default_retry_backoff_max_ms")]
    pub retry_backoff_max_ms: u64,
    #[serde(default = "default_retry_backoff_jitter_percent")]
    pub retry_backoff_jitter_percent: u64,

    // 6. Queue Candidate Sorting
    #[serde(default)]
    pub queue_sort_strategy: QueueSortStrategy,
}

impl Default for ExpertSchedulerSettings {
    fn default() -> Self {
        Self {
            scheduler_tick_ms: default_scheduler_tick_ms(),
            max_connections_per_host: default_max_connections_per_host(),
            aimd_degrade_threshold: default_aimd_degrade_threshold(),
            aimd_increase_threshold: default_aimd_increase_threshold(),
            aimd_severe_drop_threshold: default_aimd_severe_drop_threshold(),
            aimd_cooldown_secs: default_aimd_cooldown_secs(),
            aimd_samples_needed: default_aimd_samples_needed(),
            aimd_probe_stable_cycles: default_aimd_probe_stable_cycles(),
            aimd_decrease_factor: default_aimd_decrease_factor(),
            aimd_step_ratio: default_aimd_step_ratio(),
            aimd_oscillation_threshold: default_aimd_oscillation_threshold(),
            aimd_hysteresis_multiplier: default_aimd_hysteresis_multiplier(),
            custom_chunk_size_mb: 0,
            tail_sprint_stall_window_secs: default_tail_sprint_stall_window_secs(),
            tail_sprint_min_split_size_mb: default_tail_sprint_min_split_size_mb(),
            work_steal_min_split_size_mb: default_work_steal_min_split_size_mb(),
            connect_timeout_secs: default_connect_timeout_secs(),
            read_timeout_secs: default_read_timeout_secs(),
            pool_max_idle_per_host: default_pool_max_idle_per_host(),
            tcp_keepalive_secs: default_tcp_keepalive_secs(),
            retry_backoff_base_ms: default_retry_backoff_base_ms(),
            retry_backoff_max_ms: default_retry_backoff_max_ms(),
            retry_backoff_jitter_percent: default_retry_backoff_jitter_percent(),
            queue_sort_strategy: QueueSortStrategy::default(),
        }
    }
}

impl ExpertSchedulerSettings {
    pub fn preset_lan_ultra() -> Self {
        Self {
            scheduler_tick_ms: 500,
            max_connections_per_host: 32,
            aimd_step_ratio: 0.5,
            aimd_cooldown_secs: 1,
            custom_chunk_size_mb: 64,
            connect_timeout_secs: 10,
            read_timeout_secs: 60,
            pool_max_idle_per_host: 50,
            ..Default::default()
        }
    }

    pub fn preset_stealth() -> Self {
        Self {
            scheduler_tick_ms: 3000,
            max_connections_per_host: 2,
            aimd_degrade_threshold: 0.12,
            aimd_cooldown_secs: 5,
            connect_timeout_secs: 15,
            read_timeout_secs: 30,
            pool_max_idle_per_host: 4,
            retry_backoff_base_ms: 1000,
            retry_backoff_max_ms: 15000,
            ..Default::default()
        }
    }

    pub fn preset_lossy_network() -> Self {
        Self {
            aimd_degrade_threshold: 0.30,
            aimd_decrease_factor: 0.75,
            aimd_samples_needed: 3,
            aimd_severe_drop_threshold: 0.30,
            aimd_cooldown_secs: 4,
            tail_sprint_stall_window_secs: 15,
            retry_backoff_jitter_percent: 40,
            ..Default::default()
        }
    }
}

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
    #[serde(default)]
    pub expert_mode: bool,
    #[serde(default)]
    pub expert: ExpertSchedulerSettings,
    #[serde(default = "default_http3_alt_svc_enabled")]
    pub http3_alt_svc_enabled: bool,
}

fn default_warmup() -> bool {
    true
}

fn default_http3_alt_svc_enabled() -> bool {
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
            expert_mode: false,
            expert: ExpertSchedulerSettings::default(),
            http3_alt_svc_enabled: true,
        }
    }
}

/// Built-in default User-Agent: a browser-shaped UA with a *recent* Chrome
/// major version.
///
/// Keep the major version fresh. Some mirror edges (e.g. TUNA
/// `mirrors.tuna.tsinghua.edu.cn`) classify a browser-shaped UA whose version
/// is stale or not yet released as spoofed software and answer `403 Forbidden`
/// ("software with uncommon characteristics"). Bump this at least every few
/// releases; verified passing Chrome/154 in 2026-09.
///
/// Note: since Chrome 96 the real UA string freezes minor/build to `.0.0.0`,
/// so only the major needs updating.
///
/// Existing users keep whatever `default_user_agent` is stored in their
/// `settings.json`; a code change here only affects fresh settings.
pub fn default_http_user_agent() -> String {
    String::from(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36",
    )
}
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
    /// User interface language: e.g. "zh-CN", "zh-TW", "en-US", or empty for system default.
    #[serde(default)]
    pub language: String,
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
            language: String::new(),
        }
    }
}
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
    ///
    /// A key covers its whole subtree: the lookup normalizes both sides and
    /// matches on component boundaries, so the most specific (longest) key wins.
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

/// Loopback-only by default. A headless daemon passes `0.0.0.0` explicitly
/// (usually via `limedl-server --rpc-listen`), which then requires
/// authentication — see `Aria2RpcServer::serve`.
fn default_aria2_listen_address() -> String {
    "127.0.0.1".to_string()
}

/// How the Aria2 RPC endpoint authenticates its clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aria2AuthMode {
    /// One shared secret for every client (the historical behaviour).
    #[default]
    Single,
    /// One independent token per client, each stored as an Argon2id hash.
    /// A client is authenticated by matching its token against every stored
    /// hash; the plaintext token is never persisted.
    PerClient,
}

/// One client allowed to call the Aria2 RPC endpoint in `PerClient` mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2Client {
    /// Stable identity so the UI can keep a row's hash across name edits.
    #[serde(default)]
    pub id: String,
    /// Human label shown in the UI; not used for authentication.
    #[serde(default)]
    pub name: String,
    /// Argon2id PHC string of the client's token. Never the plaintext token.
    pub token_hash: String,
    /// Unix milliseconds when the token was generated (display only).
    #[serde(default)]
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2RpcSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_aria2_port")]
    pub port: u16,
    /// Interface to bind the JSON-RPC listener to. Defaults to `127.0.0.1`.
    /// Set to `0.0.0.0` (or a specific LAN address) to reach it from another
    /// machine; a non-loopback address is only accepted together with
    /// authentication, otherwise the server refuses to start.
    #[serde(default = "default_aria2_listen_address")]
    pub listen_address: String,
    #[serde(default)]
    pub secret: Option<String>,
    /// Which authentication scheme is active. Defaults to the shared-secret
    /// mode so existing settings keep working after an upgrade.
    #[serde(default)]
    pub auth_mode: Aria2AuthMode,
    /// Per-client tokens, only consulted when `auth_mode == PerClient`.
    #[serde(default)]
    pub clients: Vec<Aria2Client>,
    /// Allowed CORS origins for the Aria2 RPC HTTP endpoint.
    /// If empty, defaults to ["http://localhost", "http://127.0.0.1"].
    #[serde(default)]
    pub cors_allowed_origins: Vec<String>,
    /// Allow every origin (`Access-Control-Allow-Origin: *`). Needed when the
    /// AriaNg page is served from a host other than the RPC server. Insecure:
    /// any page in the browser can reach the endpoint, so it still requires a
    /// token. When set, credentials are not allowed (browsers reject `*` with
    /// `Access-Control-Allow-Credentials`).
    #[serde(default)]
    pub allow_any_origin: bool,
    /// Let `aria2.shutdown` / `aria2.forceShutdown` terminate the process.
    /// The desktop client leaves this off (it is a managed subsystem and the
    /// UI owns exit); the headless `limedl-server` daemon turns it on.
    #[serde(default)]
    pub exit_on_shutdown: bool,
}

impl Default for Aria2RpcSettings {
    fn default() -> Self {
        Self {
            // Off by default. The endpoint is unauthenticated whenever `secret`
            // is empty, and while it only binds loopback that still lets any
            // local process (or a page served from localhost, which the default
            // CORS policy allows) drive downloads and read paths. Opting in is a
            // deliberate user action, so the safe value is the default; an
            // existing `settings.json` that says `"enabled": true` is honoured.
            enabled: false,
            port: 6800,
            listen_address: default_aria2_listen_address(),
            secret: None,
            auth_mode: Aria2AuthMode::Single,
            clients: Vec::new(),
            cors_allowed_origins: Vec::new(),
            allow_any_origin: false,
            exit_on_shutdown: false,
        }
    }
}
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerSettings {
    /// Hold an OS power lock while downloads are running so the machine does not
    /// suspend (and the transfer does not stall) mid-download.
    ///
    /// Defaults to on: a download that silently stops because the machine went
    /// to sleep is the failure users report, not the battery they saved. The
    /// lock is only ever taken while a task is actively running, and is released
    /// as soon as the last one finishes; the display is still allowed to turn
    /// off. Set to `false` to keep the stock OS power policy untouched.
    ///
    /// The setting is persisted on every platform, but only the desktop clients
    /// act on it (`limedl-native`); the headless server leaves power management
    /// to its host.
    #[serde(default = "default_true")]
    pub prevent_sleep: bool,
}

impl Default for PowerSettings {
    fn default() -> Self {
        Self {
            prevent_sleep: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub appearance: AppearanceSettings,
    /// Sleep prevention while downloading (see [`PowerSettings`]).
    #[serde(default)]
    pub power: PowerSettings,
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
