#[cfg(feature = "bt")]
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::bt::BtUploadStatus;
use super::common::{AdaptiveProfile, ChecksumMode, DiskType, Priority, ThreadMode};
use crate::error::DownloadError;
#[derive(Debug, Clone, Copy, Default, Hash, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    #[default]
    Http,
    #[cfg(feature = "bt")]
    Bt,
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

/// Strongly-typed task identifier. Variants hold validated inner types.
///
/// The wire string form is `"http:uuid"` / `"bt:hex"` (see
/// [`TaskId::from_wire_string`]); the desktop UI and the Aria2 RPC layer both
/// speak it. `TaskId` is deliberately **not** `Serialize`/`Deserialize`: nothing
/// stores it directly (summaries carry a `String` id, the Aria2 GID map is
/// in-memory), so the old WebSocket-era wire-format impls were removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskId {
    Http(Uuid),
    #[cfg(feature = "bt")]
    Bt(InfoHash),
}

/// A 20-byte BitTorrent v1 info-hash (SHA-1 of the bencoded `info` dictionary).
///
/// This is the foundation layer's own type on purpose. The BitTorrent engine's
/// equivalent (`irontide::core::Id20`) used to be named here directly, which put
/// the whole BT engine — 100+ crates of DHT/peer-wire/`aws-lc` — underneath
/// `limedl-types`, i.e. underneath *every* limedl crate. The REST of the engine
/// only ever needs the 20 bytes and the hex spelling, so that is all this type
/// keeps; `bt_backend` converts at the one boundary that actually talks to the
/// engine (see `bt_backend::info_hash` / `bt_backend::irontide_id`).
///
/// The orphan rule is why the conversion cannot be `impl From<Id20> for InfoHash`
/// here: this crate must not name `Id20` at all.
#[cfg(feature = "bt")]
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InfoHash([u8; 20]);

#[cfg(feature = "bt")]
impl InfoHash {
    /// All zeros.
    pub const ZERO: Self = Self([0u8; 20]);

    pub const fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Parse the 40-character lowercase-or-uppercase hex form used on the wire
    /// (`"bt:<hex>"`) and in every user-facing id.
    pub fn from_hex(s: &str) -> Result<Self, DownloadError> {
        let raw = s.as_bytes();
        if raw.len() != 40 {
            return Err(invalid_info_hash(s));
        }
        let mut bytes = [0u8; 20];
        // `raw.len()` is 40, so `as_chunks` has no remainder to handle.
        let pairs = raw.as_chunks::<2>().0;
        for (slot, [hi_digit, lo_digit]) in bytes.iter_mut().zip(pairs) {
            let hi = hex_digit(*hi_digit).ok_or_else(|| invalid_info_hash(s))?;
            let lo = hex_digit(*lo_digit).ok_or_else(|| invalid_info_hash(s))?;
            *slot = (hi << 4) | lo;
        }
        Ok(Self(bytes))
    }

    /// Lowercase hex, 40 characters — the canonical spelling (`Id20::to_hex`
    /// produced the same string, so persisted ids keep working).
    pub fn to_hex(&self) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(40);
        for byte in self.0 {
            out.push(DIGITS[usize::from(byte >> 4)] as char);
            out.push(DIGITS[usize::from(byte & 0x0f)] as char);
        }
        out
    }
}

#[cfg(feature = "bt")]
fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(feature = "bt")]
fn invalid_info_hash(input: &str) -> DownloadError {
    DownloadError::InvalidRequest(format!(
        "invalid bt info-hash: {input:?} is not 40 hex characters"
    ))
}

#[cfg(feature = "bt")]
impl From<[u8; 20]> for InfoHash {
    fn from(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }
}

#[cfg(feature = "bt")]
impl std::fmt::Debug for InfoHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InfoHash({})", self.to_hex())
    }
}

#[cfg(feature = "bt")]
impl std::fmt::Display for InfoHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl TaskId {
    pub fn kind(&self) -> TaskKind {
        match self {
            TaskId::Http(_) => TaskKind::Http,
            #[cfg(feature = "bt")]
            TaskId::Bt(_) => TaskKind::Bt,
        }
    }

    /// The HTTP id, or `None` for a BitTorrent task.
    ///
    /// Callers that only serve HTTP must use this instead of
    /// `let TaskId::Http(id) = task_id else { … }`: with the `bt` feature off the
    /// enum has a single variant, that destructuring becomes irrefutable, and
    /// rustc's `irrefutable_let_patterns` lint fires (fatal under the CI gate).
    /// Destructuring an `Option` stays refutable in every configuration.
    pub fn as_http(&self) -> Option<Uuid> {
        match self {
            TaskId::Http(uuid) => Some(*uuid),
            #[cfg(feature = "bt")]
            TaskId::Bt(_) => None,
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
                return InfoHash::from_hex(hex).map(TaskId::Bt);
            }
        }
        let raw = s.strip_prefix("http:").unwrap_or(s);
        if let Ok(uuid) = Uuid::parse_str(raw) {
            return Ok(TaskId::Http(uuid));
        }
        #[cfg(feature = "bt")]
        {
            if let Ok(info_hash) = InfoHash::from_hex(raw) {
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
impl From<InfoHash> for TaskId {
    fn from(i: InfoHash) -> Self {
        TaskId::Bt(i)
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.raw_id())
    }
}
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

/// Result of an on-demand integrity verification check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityVerificationResult {
    pub task_id: String,
    pub mode: ChecksumMode,
    pub computed: String,
    pub expected: Option<String>,
    pub matches: Option<bool>,
    pub target_path: String,
}
