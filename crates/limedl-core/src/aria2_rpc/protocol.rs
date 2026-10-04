//! JSON-RPC wire types, response factories and aria2 status mapping.

use super::{BtFileStatus, Deserialize, DownloadState, DownloadSummary, Id20, RpcContext, Serialize, TaskId, TaskKind, Uuid, Value};

#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcRequest {
    pub(crate) jsonrpc: String,
    #[serde(default)]
    pub(crate) id: Option<Value>,
    pub(crate) method: String,
    #[serde(default)]
    pub(crate) params: Option<Vec<Value>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcResponse {
    pub(crate) jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcError {
    pub(crate) code: i32,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcNotification {
    pub(crate) jsonrpc: &'static str,
    pub(crate) method: String,
    pub(crate) params: Vec<Value>,
}

pub(crate) const ERR_PARSE: i32 = -32700;
pub(crate) const ERR_INVALID_REQUEST: i32 = -32600;
pub(crate) const ERR_METHOD_NOT_FOUND: i32 = -32601;
pub(crate) const ERR_INVALID_PARAMS: i32 = -32602;
pub(crate) const ERR_INTERNAL: i32 = -32603;

pub(crate) fn make_error(code: i32, message: impl Into<String>) -> JsonRpcError {
    JsonRpcError {
        code,
        message: message.into(),
    }
}

pub(crate) fn error_response(
    id: Option<Value>,
    code: i32,
    message: impl Into<String>,
) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(make_error(code, message)),
    }
}

pub(crate) fn success_response(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

pub fn internal_id_to_gid(internal_id: &str) -> String {
    let hash = xxhash_rust::xxh3::xxh3_64(internal_id.as_bytes());
    format!("{:016x}", hash)
}

pub(crate) async fn resolve_gid(ctx: &RpcContext, gid: &str) -> Option<TaskId> {
    // Check cache first
    {
        let cache = ctx.gid_cache.lock().await;
        if let Some(task_id) = cache.get(gid) {
            return Some(*task_id);
        }
    }

    // Cache miss — scan all backends
    for backend in ctx.registry.iter() {
        if let Ok(list) = backend.list().await {
            for s in &list {
                if internal_id_to_gid(&s.id) == gid {
                    let task_id = match s.kind {
                        TaskKind::Http => TaskId::Http(Uuid::parse_str(&s.id).ok()?),
                        TaskKind::Bt => TaskId::Bt(Id20::from_hex(&s.id).ok()?),
                    };
                    let mut cache = ctx.gid_cache.lock().await;
                    cache.insert(gid.to_string(), task_id);
                    return Some(task_id);
                }
            }
        }
    }

    None
}

pub(crate) fn state_to_aria2(state: &DownloadState) -> &'static str {
    match state {
        DownloadState::Queued => "waiting",
        DownloadState::Downloading => "active",
        DownloadState::Paused => "paused",
        DownloadState::Retrying | DownloadState::Verifying => "active",
        DownloadState::Completed => "complete",
        DownloadState::Failed => "error",
        DownloadState::Canceled => "removed",
    }
}

/// Terminal states, for which aria2 exposes `errorCode`/`errorMessage`.
fn is_terminal_state(state: DownloadState) -> bool {
    matches!(
        state,
        DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
    )
}

fn u64_str(value: Option<u64>) -> String {
    value.map_or_else(|| "0".to_string(), |v| v.to_string())
}

fn f64_str(value: Option<f64>) -> String {
    value.map_or_else(|| "0".to_string(), |v| v.to_string())
}

fn bool_str(value: bool) -> String {
    if value { "true" } else { "false" }.to_string()
}

/// Encode piece completion as aria2's `bitfield`.
///
/// aria2's highest bit corresponds to piece 0 and any overflow bits at the
/// end are zero, so bits are packed MSB-first and the trailing nibble is
/// left-aligned. An empty slice yields an empty string (aria2 omits the key
/// for a download that has not started).
pub(crate) fn bitfield_from_bits(bits: &[bool]) -> String {
    let mut out = String::with_capacity(bits.len().div_ceil(4));
    let mut nibble = 0u8;
    for (index, &bit) in bits.iter().enumerate() {
        nibble = (nibble << 1) | u8::from(bit);
        if index % 4 == 3 {
            out.push(char::from_digit(u32::from(nibble), 16).unwrap_or('0'));
            nibble = 0;
        }
    }
    let remainder = bits.len() % 4;
    if remainder != 0 {
        out.push(char::from_digit(u32::from(nibble << (4 - remainder)), 16).unwrap_or('0'));
    }
    out
}

pub(crate) fn summary_to_aria2_status(summary: &DownloadSummary) -> Value {
    let gid = internal_id_to_gid(&summary.id);
    let is_bt = matches!(summary.kind, TaskKind::Bt);

    // `seeder` describes the *local* endpoint: it is a seeder once it holds
    // the whole payload. A task with an unknown total length cannot claim it.
    let seeder = is_bt
        && summary
            .total_bytes
            .is_some_and(|total| total > 0 && summary.downloaded_bytes >= total);

    let mut map = serde_json::Map::new();
    map.insert("gid".to_string(), Value::String(gid));
    map.insert(
        "status".to_string(),
        Value::String(state_to_aria2(&summary.state).to_string()),
    );
    map.insert("totalLength".to_string(), Value::String(u64_str(summary.total_bytes)));
    map.insert(
        "completedLength".to_string(),
        Value::String(summary.downloaded_bytes.to_string()),
    );
    map.insert("uploadLength".to_string(), Value::String(u64_str(summary.uploaded_bytes)));
    map.insert(
        "downloadSpeed".to_string(),
        Value::String(f64_str(summary.speed_bytes_per_second)),
    );
    map.insert(
        "uploadSpeed".to_string(),
        Value::String(f64_str(summary.upload_speed_bytes_per_second)),
    );
    map.insert(
        "connections".to_string(),
        Value::String(summary.connection_count.to_string()),
    );
    if is_bt {
        // aria2 keeps `infoHash`, `numSeeders` and `seeder` at the top level;
        // the `bittorrent` object carries torrent metadata instead.
        map.insert(
            "infoHash".to_string(),
            Value::String(summary.info_hash.clone().unwrap_or_default()),
        );
        map.insert(
            "numSeeders".to_string(),
            Value::String(summary.peer_count.map_or_else(|| "0".to_string(), |p| p.to_string())),
        );
        map.insert("seeder".to_string(), Value::String(bool_str(seeder)));
    }
    if is_terminal_state(summary.state) {
        // aria2 defines code 0 as "no error" and 1 as the generic unknown
        // error. limedl does not persist aria2 exit-status codes, so report a
        // real failure as 1 rather than guessing a specific, misleading code;
        // the human-readable reason stays in `errorMessage`.
        map.insert(
            "errorCode".to_string(),
            Value::String(if summary.error.is_some() { "1".to_string() } else { "0".to_string() }),
        );
        map.insert(
            "errorMessage".to_string(),
            Value::String(summary.error.clone().unwrap_or_default()),
        );
    }
    map.insert("dir".to_string(), Value::String(summary.destination_path.clone()));
    map.insert("files".to_string(), build_file_list(summary));
    // `bitfield`/`numPieces`/`pieceLength` and the BitTorrent `bittorrent`
    // object need a backend/manifest lookup, so the caller adds them for
    // tellStatus/tellActive. The list-wide methods stay cheap and omit them.
    Value::Object(map)
}

pub(crate) fn build_file_list(summary: &DownloadSummary) -> Value {
    Value::Array(vec![serde_json::json!({
        "index": "1",
        // aria2 reports the absolute save path, not just the file name.
        "path": summary.destination_path,
        "length": u64_str(summary.total_bytes),
        "completedLength": summary.downloaded_bytes.to_string(),
        "selected": "true",
        "uris": [{"uri": summary.url, "status": "used"}]
    })])
}

/// Convert the BT engine's file statuses into aria2's `files` array.
///
/// aria2 file indices are **1-based strings**; the engine's
/// [`BtFileStatus::index`] is 0-based. A magnet whose metadata has not arrived
/// has no files yet — callers pass an empty slice and get an empty array
/// instead of a synthetic entry.
pub(crate) fn bt_files_to_aria2(files: &[BtFileStatus]) -> Value {
    Value::Array(
        files
            .iter()
            .map(|file| {
                serde_json::json!({
                    "index": (file.index + 1).to_string(),
                    "path": file.path,
                    "length": file.size.to_string(),
                    "completedLength": file.downloaded_bytes.to_string(),
                    "selected": if file.included { "true" } else { "false" },
                    "uris": [],
                })
            })
            .collect(),
    )
}

/// Keep only the requested status fields, matching aria2's `keys` parameter.
pub(crate) fn filter_status_keys(value: Value, keys: &[String]) -> Value {
    match value {
        Value::Object(mut map) => {
            map.retain(|key, _| keys.iter().any(|wanted| wanted == key));
            Value::Object(map)
        }
        other => other,
    }
}
