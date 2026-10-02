//! JSON-RPC wire types, response factories and aria2 status mapping.

use super::{Deserialize, DownloadState, DownloadSummary, Id20, RpcContext, Serialize, TaskId, TaskKind, Uuid, Value};

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

pub(crate) fn summary_to_aria2_status(summary: &DownloadSummary) -> Value {
    let gid = internal_id_to_gid(&summary.id);
    let total_len = summary
        .total_bytes
        .map_or_else(|| "0".to_string(), |b| b.to_string());
    let speed = summary
        .speed_bytes_per_second
        .map_or_else(|| "0".to_string(), |s| s.to_string());
    let seeders = summary
        .peer_count
        .map_or_else(|| "0".to_string(), |p| p.to_string());
    let uploaded = summary
        .uploaded_bytes
        .map_or_else(|| "0".to_string(), |b| b.to_string());
    let upload_speed = summary
        .upload_speed_bytes_per_second
        .map_or_else(|| "0".to_string(), |s| s.to_string());
    let is_bt = matches!(summary.kind, TaskKind::Bt);

    let bt_block = if is_bt {
        serde_json::json!({
            "infoHash": summary.info_hash.as_deref().unwrap_or(""),
            "uploadLength": uploaded,
            "uploadSpeed": upload_speed,
            "numSeeders": seeders,
        })
    } else {
        serde_json::json!({
            "infoHash": "",
            "uploadLength": "0",
        })
    };

    serde_json::json!({
        "gid": gid,
        "status": state_to_aria2(&summary.state),
        "totalLength": total_len,
        "completedLength": summary.downloaded_bytes.to_string(),
        "downloadSpeed": speed,
        "uploadSpeed": upload_speed,
        "connections": summary.connection_count.to_string(),
        "numSeeders": seeders,
        "dir": summary.destination_path,
        "files": build_file_list(summary),
        "bittorrent": bt_block,
    })
}

pub(crate) fn build_file_list(summary: &DownloadSummary) -> Value {
    let total_len = summary
        .total_bytes
        .map_or_else(|| "0".to_string(), |b| b.to_string());

    Value::Array(vec![serde_json::json!({
        "index": "1",
        "path": summary.file_name,
        "length": total_len,
        "completedLength": summary.downloaded_bytes.to_string(),
        "selected": "true",
        "uris": [{"uri": summary.url, "status": "used"}]
    })])
}
