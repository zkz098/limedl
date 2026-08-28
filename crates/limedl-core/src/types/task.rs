#[cfg(feature = "bt")]
use std::path::Path;

#[cfg(feature = "bt")]
use irontide::core::Id20;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "ts")]
use ts_rs::TS;

use super::bt::BtUploadStatus;
use super::common::{AdaptiveProfile, ChecksumMode, DiskType, Priority, ThreadMode};
use crate::error::DownloadError;

#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Copy, Default, Hash, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    #[default]
    Http,
    #[cfg(feature = "bt")]
    Bt,
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

/// Strongly-typed task identifier. Variants hold validated inner types.
///
/// Wire format (serialization) emits `{ kind, id }` struct.
/// Deserialization accepts BOTH:
///   - New: `{ "kind": "http"|"bt", "id": "..." }`
///   - Legacy: `"http:uuid"` or `"bt:hex"` strings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskId {
    Http(Uuid),
    #[cfg(feature = "bt")]
    Bt(Id20),
}

impl TaskId {
    pub fn kind(&self) -> TaskKind {
        match self {
            TaskId::Http(_) => TaskKind::Http,
            #[cfg(feature = "bt")]
            TaskId::Bt(_) => TaskKind::Bt,
        }
    }

    /// Raw canonical string: UUID hyphenated for Http, lowercase hex for Bt.
    pub fn raw_id(&self) -> String {
        match self {
            TaskId::Http(uuid) => uuid.to_string(),
            #[cfg(feature = "bt")]
            TaskId::Bt(info_hash) => info_hash.to_hex(),
        }
    }

    /// Parse a task id from its wire-string form: `"http:uuid"` or `"bt:hex"`.
    /// Returns an error if the inner part is invalid.
    pub fn from_wire_string(s: &str) -> Result<Self, DownloadError> {
        #[cfg(feature = "bt")]
        {
            if let Some(hex) = s.strip_prefix("bt:") {
                return Id20::from_hex(hex)
                    .map(TaskId::Bt)
                    .map_err(|e| DownloadError::InvalidRequest(format!("invalid bt id: {e}")));
            }
        }
        let raw = s.strip_prefix("http:").unwrap_or(s);
        if let Ok(uuid) = Uuid::parse_str(raw) {
            return Ok(TaskId::Http(uuid));
        }
        #[cfg(feature = "bt")]
        {
            if let Ok(info_hash) = Id20::from_hex(raw) {
                return Ok(TaskId::Bt(info_hash));
            }
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

#[cfg(feature = "bt")]
impl From<Id20> for TaskId {
    fn from(i: Id20) -> Self {
        TaskId::Bt(i)
    }
}

impl Serialize for TaskId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("TaskId", 2)?;
        st.serialize_field("kind", &self.kind())?;
        st.serialize_field("id", &self.raw_id())?;
        st.end()
    }
}

struct TaskIdVisitor;
impl<'de> Visitor<'de> for TaskIdVisitor {
    type Value = TaskId;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a TaskId object {kind, id} or legacy string \"http:uuid\"/\"bt:hex\"")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<TaskId, E> {
        TaskId::from_wire_string(v).map_err(|e| de::Error::custom(e))
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
            #[cfg(feature = "bt")]
            (Some(TaskKind::Bt), Some(id)) => Id20::from_hex(&id)
                .map(TaskId::Bt)
                .map_err(|e| de::Error::custom(format!("invalid info hash: {e}"))),
            (None, _) => Err(de::Error::missing_field("kind")),
            (_, None) => Err(de::Error::missing_field("id")),
        }
    }
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<Vec<String>>,
    #[serde(default)]
    pub start_paused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_urls: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
}

impl StartDownloadRequest {
    /// Classify the download request as HTTP or BT based on URL inspection.
    pub fn classify_kind(&self) -> std::result::Result<TaskKind, DownloadError> {
        if let Some(kind) = self.kind {
            return Ok(kind);
        }

        let source = self.url.trim();
        let lower = source.to_ascii_lowercase();

        #[cfg(feature = "bt")]
        {
            if lower.starts_with("magnet:") || lower.ends_with(".torrent") {
                return Ok(TaskKind::Bt);
            }
        }

        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Ok(TaskKind::Http);
        }

        #[cfg(feature = "bt")]
        {
            let path = Path::new(source);
            if path
                .extension()
                .and_then(|v| v.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("torrent"))
            {
                return Ok(TaskKind::Bt);
            }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_checksum: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdn_node_ip: Option<String>,
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
    pub expected_checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub cdn_accelerated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdn_node_ip: Option<String>,
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
            expected_checksum: value.expected_checksum.clone(),
            error: value.error.clone(),
            cdn_accelerated: value.cdn_accelerated,
            cdn_node_ip: value.cdn_node_ip.clone(),
            created_at_ms: value.created_at_ms,
            priority: value.priority,
            seed_count: value.seed_count,
            leech_count: value.leech_count,
            download_limit_bps: value.download_limit_bps,
            upload_limit_bps: value.upload_limit_bps,
            chunks: Vec::new(),
            mirror_url: value.mirror_url.clone(),
        }
    }
}

/// Lightweight incremental progress update sent every ~300ms during active downloads.
/// Contains only high-frequency fields. Static/low-frequency fields stay in `DownloadSummary`.
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
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
