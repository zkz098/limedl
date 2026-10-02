use serde::{Deserialize, Serialize};

/// High-performance DashMap using foldhash's fast RandomState instead of std's SipHash.
pub type FastDashMap<K, V> = dashmap::DashMap<K, V, foldhash::fast::RandomState>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableError {
    pub kind: String,
    pub message: String,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Hash algorithm used to verify a downloaded file.
///
/// The supported set is deliberately narrow: BLAKE3 (the default), SHA-256 and
/// SHA-512. Weak or non-cryptographic digests (SHA-1, XXH3-128) are not
/// accepted — a checksum mismatch costs a re-download
/// of the affected chunks, so a collision-prone digest defeats the purpose.
///
/// [`ChecksumMode::None`] is not an algorithm: it means "no verification".
pub enum ChecksumMode {
    None,
    #[default]
    Blake3,
    Sha256,
    Sha512,
}

/// Download priority — affects scheduler ordering.
/// Stored as INTEGER in SQLite (0=Low, 1=Normal, 2=High).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Priority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
}

impl From<u8> for Priority {
    fn from(v: u8) -> Self {
        match v {
            0 => Priority::Low,
            2 => Priority::High,
            _ => Priority::Normal,
        }
    }
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    Name,
    Size,
    Progress,
    Speed,
    #[default]
    AddedAt,
    State,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
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
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ThemeColor {
    Amber,
    Sky,
    #[default]
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

/// Behavior when the user closes the main window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CloseBehavior {
    /// Exit the application completely.
    Exit,
    /// Minimize to system tray (keep running in background).
    #[default]
    MinimizeToTray,
}

/// Disk type for I/O optimization decisions.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiskType {
    #[default]
    Ssd,
    Hdd,
    /// Remote or otherwise unresolvable location: a UNC/SMB share, a mapped
    /// network drive, a network mount on Linux/macOS, or a WSL path whose
    /// distro could not be resolved.
    ///
    /// Scheduled like [`DiskType::Ssd`] (write combining, parallel channels) —
    /// not because the bytes land on flash, but because the local seek-penalty
    /// heuristics say nothing here and the transport, not the platter, is the
    /// bottleneck. It exists so the mode is *visible* instead of being reported
    /// as SSD: a share backed by spindles can be pinned with
    /// `io_baseline.disk_type_overrides` now that the device queue honours it.
    Network,
}

/// Action to perform when double-clicking a completed download task.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DoubleClickOnCompleted {
    /// Do nothing.
    #[default]
    None,
    /// Open the downloaded file directly (OS default handler).
    OpenFile,
    /// Open file explorer and select the downloaded file.
    OpenInExplorer,
    /// Open the download directory in file explorer.
    OpenDownloadDir,
}

/// Action to perform when double-clicking an uncompleted download task.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DoubleClickOnUncompleted {
    /// Do nothing.
    #[default]
    None,
    /// Toggle between pause and resume.
    TogglePauseResume,
}

/// Settings for double-click behavior on download tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoubleClickSettings {
    /// Action when double-clicking a completed task.
    #[serde(default)]
    pub on_completed: DoubleClickOnCompleted,
    /// Action when double-clicking an uncompleted task.
    #[serde(default)]
    pub on_uncompleted: DoubleClickOnUncompleted,
}

impl Default for DoubleClickSettings {
    fn default() -> Self {
        Self {
            on_completed: DoubleClickOnCompleted::None,
            on_uncompleted: DoubleClickOnUncompleted::None,
        }
    }
}

/// A time-of-day speed limit slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedLimitSlot {
    /// Start hour (0-23, inclusive)
    pub start_hour: u8,
    /// End hour (0-23, exclusive — e.g. 18 means "until 18:00")
    pub end_hour: u8,
    /// Speed limit in bytes per second (0 = unlimited)
    pub limit_bps: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    #[default]
    Host,
    Prefix,
    Regex,
    Wildcard,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplacementMode {
    #[default]
    PrefixProxy,
    Template,
}

pub(crate) fn default_true() -> bool {
    true
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteTarget {
    #[serde(default)]
    pub url_template: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub order: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlRewriteRule {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub match_type: MatchType,
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub replacement_mode: ReplacementMode,
    #[serde(default)]
    pub targets: Vec<RewriteTarget>,
    #[serde(default)]
    pub encode_url: bool,
    #[serde(default = "default_true")]
    pub fallback_to_original: bool,
    #[serde(default)]
    pub order: u32,
}

