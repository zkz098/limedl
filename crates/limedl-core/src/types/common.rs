use serde::{Deserialize, Serialize};

#[cfg(feature = "ts")]
use ts_rs::TS;

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableError {
    pub kind: String,
    pub message: String,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumMode {
    None,
    #[default]
    Blake3,
    Sha256,
    #[serde(rename = "sha1")]
    Sha1,
    #[serde(rename = "xxh3_128")]
    Xxh3128,
}

/// Download priority — affects scheduler ordering.
/// Stored as INTEGER in SQLite (0=Low, 1=Normal, 2=High).
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThreadMode {
    Fixed,
    #[default]
    Adaptive,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveProfile {
    Conservative,
    #[default]
    Balanced,
    Aggressive,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    #[default]
    Disabled,
    System,
    Manual,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerMode {
    Traditional,
    #[default]
    Automatic,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ThemeColor {
    Amber,
    Sky,
    #[default]
    Lime,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundOpacityPreset {
    #[default]
    Default,
    Acrylic,
    Frosted,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    Light,
    Dark,
    #[default]
    System,
}

/// Behavior when the user closes the main window.
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiskType {
    #[default]
    Ssd,
    Hdd,
}

/// Action to perform when double-clicking a completed download task.
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    #[default]
    Host,
    Prefix,
    Regex,
    Wildcard,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorEntry {
    pub url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub order: u32,
}
