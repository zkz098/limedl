use std::path::Path;

use irontide::core::Id20;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::DownloadError;

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
    #[serde(rename = "xxh3_128")]
    Xxh3128,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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
#[derive(Debug, Clone, Copy, Default, Hash, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    #[default]
    Http,
    Bt,
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

/// Strongly-typed task identifier. Variants hold validated inner types.
///
/// Wire format (serialization) emits `{ kind, id }` struct.
/// Deserialization accepts BOTH:
///   - New: `{ "kind": "http"|"bt", "id": "..." }`
///   - Legacy: `"http:uuid"` or `"bt:hex"` strings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskId {
    Http(Uuid),
    Bt(Id20),
}

impl TaskId {
    pub fn kind(&self) -> TaskKind {
        match self {
            TaskId::Http(_) => TaskKind::Http,
            TaskId::Bt(_) => TaskKind::Bt,
        }
    }

    /// Raw canonical string: UUID hyphenated for Http, lowercase hex for Bt.
    pub fn raw_id(&self) -> String {
        match self {
            TaskId::Http(uuid) => uuid.to_string(),
            TaskId::Bt(info_hash) => info_hash.to_hex(),
        }
    }

    /// Parse from legacy prefixed string: "http:uuid" or "bt:hex".
    /// Returns error if the inner part is invalid.
    pub fn from_legacy_string(s: &str) -> Result<Self, DownloadError> {
        if let Some(hex) = s.strip_prefix("bt:") {
            return Id20::from_hex(hex)
                .map(TaskId::Bt)
                .map_err(|e| DownloadError::InvalidRequest(format!("invalid bt id: {e}")));
        }
        let raw = s.strip_prefix("http:").unwrap_or(s);
        // Try UUID first (HTTP), then Id20 (BT for bare hex strings)
        if let Ok(uuid) = Uuid::parse_str(raw) {
            return Ok(TaskId::Http(uuid));
        }
        if let Ok(info_hash) = Id20::from_hex(raw) {
            return Ok(TaskId::Bt(info_hash));
        }
        Err(DownloadError::InvalidRequest(format!(
            "invalid task id: cannot parse {s:?}"
        )))
    }
}

impl From<Uuid> for TaskId {
    fn from(u: Uuid) -> Self {
        TaskId::Http(u)
    }
}

impl From<Id20> for TaskId {
    fn from(i: Id20) -> Self {
        TaskId::Bt(i)
    }
}

// ── Serialization: emit { kind, id } ──
impl Serialize for TaskId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("TaskId", 2)?;
        st.serialize_field("kind", &self.kind())?;
        st.serialize_field("id", &self.raw_id())?;
        st.end()
    }
}

use serde::de::{self, MapAccess, Visitor};

struct TaskIdVisitor;
impl<'de> Visitor<'de> for TaskIdVisitor {
    type Value = TaskId;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a TaskId object {kind, id} or legacy string \"http:uuid\"/\"bt:hex\"")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<TaskId, E> {
        TaskId::from_legacy_string(v).map_err(|e| de::Error::custom(e))
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<TaskId, M::Error> {
        let mut kind: Option<TaskKind> = None;
        let mut id: Option<String> = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "kind" => kind = Some(map.next_value()?),
                "id" => id = Some(map.next_value()?),
                _ => {
                    let _: de::IgnoredAny = map.next_value()?;
                }
            }
        }
        match (kind, id) {
            (Some(TaskKind::Http), Some(id)) => Uuid::parse_str(&id)
                .map(TaskId::Http)
                .map_err(|e| de::Error::custom(format!("invalid UUID: {e}"))),
            (Some(TaskKind::Bt), Some(id)) => Id20::from_hex(&id)
                .map(TaskId::Bt)
                .map_err(|e| de::Error::custom(format!("invalid info hash: {e}"))),
            (None, _) => Err(de::Error::missing_field("kind")),
            (_, None) => Err(de::Error::missing_field("id")),
        }
    }
}

// ── Deserialization: accept both { kind, id } and legacy "http:..." / "bt:..." ──
impl<'de> Deserialize<'de> for TaskId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(TaskIdVisitor)
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.raw_id())
    }
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadRequest {
    #[serde(default)]
    pub kind: Option<TaskKind>,
    pub url: String,
    pub destination_dir: String,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_mode: Option<ThreadMode>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<ChecksumMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_file_indices: Option<Vec<usize>>,
    #[serde(default)]
    pub start_paused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_urls: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
}

impl StartDownloadRequest {
    /// Classify the download request as HTTP or BT based on URL inspection.
    /// Returns `UnsupportedScheme` error if the URL cannot be classified.
    pub fn classify_kind(&self) -> std::result::Result<TaskKind, DownloadError> {
        if let Some(kind) = self.kind {
            return Ok(kind);
        }

        let source = self.url.trim();
        let lower = source.to_ascii_lowercase();

        if lower.starts_with("magnet:") || lower.ends_with(".torrent") {
            return Ok(TaskKind::Bt);
        }

        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Ok(TaskKind::Http);
        }

        let path = Path::new(source);
        if path
            .extension()
            .and_then(|v| v.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("torrent"))
        {
            return Ok(TaskKind::Bt);
        }

        Err(DownloadError::UnsupportedScheme)
    }
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkInfo {
    pub index: usize,
    pub start: u64,
    pub end: u64,
    pub downloaded: u64,
    pub completed: bool,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<usize>,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub supports_ranges: bool,
    pub connection_count: usize,
    pub thread_mode: ThreadMode,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_thread_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub desired_thread_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocated_thread_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive_profile: Option<AdaptiveProfile>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_note: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    pub checksum_mode: ChecksumMode,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_bytes_per_second: Option<f64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub uploaded_bytes: Option<u64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_speed_bytes_per_second: Option<f64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_status: Option<BtUploadStatus>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_hash: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default)]
    pub priority: Priority,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_url: Option<String>,
    #[serde(default)]
    pub degraded: bool,
    /// Disk type for this download (set after detection).
    /// None if not yet detected (defaults to SSD behavior).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_type: Option<DiskType>,
    /// Whether the buffer is currently being flushed to disk.
    #[serde(default)]
    pub flushing: bool,
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
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
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub connection_count: usize,
    pub thread_mode: ThreadMode,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_thread_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub desired_thread_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocated_thread_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive_profile: Option<AdaptiveProfile>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_note: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_bytes_per_second: Option<f64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub uploaded_bytes: Option<u64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_speed_bytes_per_second: Option<f64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_status: Option<BtUploadStatus>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_hash: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub cdn_accelerated: bool,
    pub created_at_ms: u64,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leech_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_limit_bps: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_limit_bps: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chunks: Vec<ChunkInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_url: Option<String>,
}

/// Lightweight incremental progress update sent every ~300ms during active downloads.
/// Contains only high-frequency fields. Static/low-frequency fields stay in `DownloadSummary`.
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub id: String,
    pub state: DownloadState,
    pub downloaded_bytes: u64,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_bytes_per_second: Option<f64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u64>,
    pub connection_count: usize,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocated_thread_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub uploaded_bytes: Option<u64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_speed_bytes_per_second: Option<f64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_count: Option<usize>,
#[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_status: Option<BtUploadStatus>,
    #[serde(default)]
    pub degraded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_type: Option<DiskType>,
    #[serde(default)]
    pub flushing: bool,
}

impl From<&DownloadSnapshot> for DownloadProgress {
    fn from(snapshot: &DownloadSnapshot) -> Self {
        Self {
            id: snapshot.id.clone(),
            state: snapshot.state,
            downloaded_bytes: snapshot.downloaded_bytes,
            total_bytes: snapshot.total_bytes,
            speed_bytes_per_second: snapshot.speed_bytes_per_second,
            eta_seconds: snapshot.eta_seconds,
            connection_count: snapshot.connection_count,
            allocated_thread_count: snapshot.allocated_thread_count,
            error: snapshot.error.clone(),
            uploaded_bytes: snapshot.uploaded_bytes,
            upload_speed_bytes_per_second: snapshot.upload_speed_bytes_per_second,
            peer_count: snapshot.peer_count,
            upload_status: snapshot.upload_status,
            degraded: snapshot.degraded,
            disk_type: snapshot.disk_type,
            flushing: snapshot.flushing,
        }
    }
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
            priority: value.priority,
            seed_count: value.seed_count,
            leech_count: value.leech_count,
            download_limit_bps: value.download_limit_bps,
            upload_limit_bps: value.upload_limit_bps,
            chunks: value.chunks.clone(),
            mirror_url: value.mirror_url.clone(),
        }
    }
}

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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerSettings {
    pub mode: SchedulerMode,
    pub traditional: TraditionalSchedulerSettings,
    pub automatic: AutomaticSchedulerSettings,
    #[serde(default)]
    pub chunk_size_strategy: ChunkSizeStrategy,
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

fn default_true() -> bool {
    true
}

fn default_max_downloads() -> u32 {
    3
}
fn default_max_seeds() -> u32 {
    5
}
fn default_max_torrents() -> u32 {
    100
}
fn default_active_limit() -> u32 {
    500
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
        }
    }
}

fn default_max_in_memory_downloads() -> usize {
    200
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_limits: Option<DownloadLimits>,
    /// Maximum number of completed/failed/canceled downloads kept in memory.
    /// Older terminal-state entries are evicted when this limit is exceeded.
    #[serde(default = "default_max_in_memory_downloads")]
    pub max_in_memory_downloads: usize,
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

fn default_aria2_port() -> u16 {
    6800
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorEntry {
    pub url: String,
    #[serde(default = "default_mirror_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub order: u32,
}

fn default_mirror_enabled() -> bool {
    true
}

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubMirrorSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mirrors: Vec<MirrorEntry>,
}

/// Limits on concurrent active downloads.
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadLimits {
    pub max_concurrent_http: usize,
    pub max_concurrent_bt: usize,
}

impl Default for DownloadLimits {
    fn default() -> Self {
        Self {
            max_concurrent_http: 5,
            max_concurrent_bt: 3,
        }
    }
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
            "appearance": {"themeColor": "lime", "backgroundOpacity": "default", "colorMode": "system", "showDetailInfo": true, "showHeatmap": true, "sortKey": "added_at", "sortDirection": "desc", "compactView": false, "visibleColumns": ["file", "size", "downloaded", "status", "progress", "speed", "eta"]},
            "proxy": {"mode": "system", "manualUrl": ""},
            "scheduler": {"mode": "traditional", "traditional": {"maxParallelTasks": 3}, "automatic": {"maxParallelThreads": 8, "maxThreadsPerTask": 4, "minThreadsPerTask": 1, "adaptiveProfile": "conservative"}},
            "download": {"defaultDownloadDir": "", "defaultMaxRetries": 3, "defaultChecksum": "blake3", "defaultUserAgent": ""},
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

    // ── TaskId serde round-trip ────────────────────────────────────────

    #[test]
    fn task_id_http_round_trip() {
        let uuid = uuid::Uuid::new_v4();
        let original = TaskId::Http(uuid);
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, original);
        // Verify JSON structure
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["kind"], "http");
        assert_eq!(parsed["id"], uuid.to_string());
    }

    #[test]
    fn task_id_bt_round_trip() {
        let hash = irontide::core::Id20::from([0xab; 20]);
        let original = TaskId::Bt(hash);
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, original);
        // Verify JSON structure
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["kind"], "bt");
        assert_eq!(parsed["id"], "abababababababababababababababababababab");
    }

    #[test]
    fn task_id_legacy_bt_string() {
        let json = "\"bt:abcdef0123456789abcdef0123456789abcdef01\"";
        let deserialized: TaskId = serde_json::from_str(json).unwrap();
        assert!(matches!(deserialized, TaskId::Bt(_)));
        assert_eq!(deserialized.raw_id(), "abcdef0123456789abcdef0123456789abcdef01");
    }

    #[test]
    fn task_id_legacy_http_string() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let json = format!("\"http:{uuid_str}\"");
        let deserialized: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, TaskId::Http(uuid::Uuid::parse_str(uuid_str).unwrap()));
    }

    #[test]
    fn task_id_legacy_bare_uuid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let json = format!("\"{uuid_str}\"");
        let deserialized: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, TaskId::Http(uuid::Uuid::parse_str(uuid_str).unwrap()));
    }

    #[test]
    fn task_id_malformed_uuid_returns_error() {
        let json = "\"http:not-a-uuid\"";
        let result: Result<TaskId, _> = serde_json::from_str(json);
        assert!(result.is_err(), "malformed UUID should fail");
    }

    #[test]
    fn task_id_missing_kind_field_returns_error() {
        let json = r#"{"id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let result: Result<TaskId, _> = serde_json::from_str(json);
        assert!(result.is_err(), "missing kind should fail");
    }

    #[test]
    fn task_id_missing_id_field_returns_error() {
        let json = r#"{"kind": "http"}"#;
        let result: Result<TaskId, _> = serde_json::from_str(json);
        assert!(result.is_err(), "missing id should fail");
    }

    // ── classify_kind ──────────────────────────────────────────────────

    #[test]
    fn classify_kind_explicit_kind_wins() {
        let req = StartDownloadRequest {
            kind: Some(TaskKind::Bt),
            url: "https://example.com/file.zip".into(),
            ..StartDownloadRequest::default()
        };
        assert_eq!(req.classify_kind().unwrap(), TaskKind::Bt);
    }

    #[test]
    fn classify_kind_magnet_link() {
        let req = StartDownloadRequest {
            kind: None,
            url: "magnet:?xt=urn:btih:abcdef0123456789abcdef0123456789abcdef01".into(),
            ..StartDownloadRequest::default()
        };
        assert_eq!(req.classify_kind().unwrap(), TaskKind::Bt);
    }

    #[test]
    fn classify_kind_torrent_url_suffix() {
        let req = StartDownloadRequest {
            kind: None,
            url: "https://example.com/file.torrent".into(),
            ..StartDownloadRequest::default()
        };
        assert_eq!(req.classify_kind().unwrap(), TaskKind::Bt);
    }

    #[test]
    fn classify_kind_torrent_extension_checks_path() {
        let req = StartDownloadRequest {
            kind: None,
            url: "/some/path/debian-12.torrent".into(),
            ..StartDownloadRequest::default()
        };
        assert_eq!(req.classify_kind().unwrap(), TaskKind::Bt);
    }

    #[test]
    fn classify_kind_http_url() {
        let req = StartDownloadRequest {
            kind: None,
            url: "https://cdn.example.com/file.zip".into(),
            ..StartDownloadRequest::default()
        };
        assert_eq!(req.classify_kind().unwrap(), TaskKind::Http);
    }

    #[test]
    fn classify_kind_unknown_scheme_returns_error() {
        let req = StartDownloadRequest {
            kind: None,
            url: "ftp://example.com/file.zip".into(),
            ..StartDownloadRequest::default()
        };
        let result = req.classify_kind();
        assert!(matches!(result, Err(DownloadError::UnsupportedScheme)));
    }
}
