use std::path::Path;

use serde_json::Value;

use super::parse::{aria2_bool, find_header_value, parse_aria2_limit, parse_select_file};
use crate::context::RpcContext;
use crate::jsonrpc::{ERR_INTERNAL, ERR_INVALID_PARAMS, JsonRpcError, make_error};
use crate::protocol::resolve_gid;
use crate::query::{extract_gid, get_all_summaries};
use crate::types::TaskId;

/// Read a task's effective User-Agent and extra headers from the in-memory
/// download map (all persisted downloads are loaded there at startup).
pub async fn task_request_headers(ctx: &RpcContext, id: &str) -> (String, Vec<String>) {
    let Some(dm) = ctx.http() else {
        return (String::new(), Vec::new());
    };
    let managed = dm.downloads.read().await.get(id).cloned();
    match managed {
        Some(managed) => {
            let core = managed.lock_core();
            (
                core.manifest.user_agent.clone(),
                core.manifest.extra_headers.clone(),
            )
        }
        None => (String::new(), Vec::new()),
    }
}

/// `aria2.changeOption` — apply options to a single existing task.
///
/// Only options the engine can actually change at runtime are accepted;
/// anything else fails with a clear error instead of pretending to apply.
/// Supported today:
///
/// - `pause` (both protocols)
/// - `select-file` (BitTorrent, 1-based aria2 indices)
/// - `max-download-limit` / `max-upload-limit` (BitTorrent)
///
pub async fn handle_change_option(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let options = params
        .get(1)
        .and_then(|v| v.as_object())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing options object"))?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    for (key, value) in options {
        match key.as_str() {
            "select-file" => {
                if !matches!(task_id, TaskId::Bt(_)) {
                    return Err(make_error(
                        ERR_INVALID_PARAMS,
                        "select-file is only supported for BitTorrent tasks",
                    ));
                }
                let selected = parse_select_file(value)?;
                ctx.dispatcher
                    .bt_update_files(&task_id, selected)
                    .await
                    .map_err(|e| make_error(ERR_INTERNAL, format!("select-file failed: {e}")))?;
            }
            "pause" => {
                if aria2_bool(value) {
                    ctx.dispatcher
                        .pause(&task_id)
                        .await
                        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
                } else {
                    ctx.dispatcher
                        .resume(&task_id)
                        .await
                        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
                }
            }
            "max-download-limit" | "max-upload-limit" => {
                if !matches!(task_id, TaskId::Bt(_)) {
                    return Err(make_error(
                        ERR_INVALID_PARAMS,
                        format!("{key} is only supported for BitTorrent tasks"),
                    ));
                }
                let limit = parse_aria2_limit(value)?;
                // Preserve the other direction: the API sets both at once.
                let snapshot = ctx
                    .dispatcher
                    .status(&task_id)
                    .await
                    .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
                let (download, upload) = if key == "max-download-limit" {
                    (limit, snapshot.upload_limit_bps)
                } else {
                    (snapshot.download_limit_bps, limit)
                };
                ctx.dispatcher
                    .bt_set_speed_limit(&task_id, download, upload)
                    .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
            }
            other => {
                return Err(make_error(
                    ERR_INVALID_PARAMS,
                    format!("option '{other}' cannot be changed at runtime"),
                ));
            }
        }
    }

    Ok(Value::String("OK".to_string()))
}

pub async fn handle_get_option(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let raw_id = task_id.raw_id();
    let summary = get_all_summaries(ctx)
        .await?
        .into_iter()
        .find(|s| s.id == raw_id)
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    // Extract parent directory from destination_path
    let dir = Path::new(&summary.destination_path)
        .parent()
        .and_then(Path::to_str)
        .unwrap_or(&summary.destination_path)
        .to_string();

    // Convert priority to aria2 position string if available
    let position = (summary.priority as u8).to_string();

    // Effective per-task request metadata (User-Agent, Referer, custom headers).
    let (user_agent, extra_headers) = task_request_headers(ctx, &raw_id).await;
    let referer = find_header_value(&extra_headers, "referer").unwrap_or_default();
    let header_values: Vec<Value> = extra_headers.iter().cloned().map(Value::String).collect();

    let mut map = serde_json::Map::new();
    map.insert("dir".to_string(), Value::String(dir));
    map.insert("out".to_string(), Value::String(summary.file_name));
    map.insert(
        "split".to_string(),
        Value::String(
            summary
                .requested_thread_count
                .map_or_else(|| "5".to_string(), |n| n.to_string()),
        ),
    );
    map.insert(
        "max-connection-per-server".to_string(),
        Value::String(summary.connection_count.to_string()),
    );
    map.insert(
        "piece-length".to_string(),
        Value::String("1048576".to_string()),
    );
    map.insert(
        "allow-overwrite".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "allow-piece-length-change".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "always-resume".to_string(),
        Value::String("true".to_string()),
    );
    map.insert("async-dns".to_string(), Value::String("true".to_string()));
    map.insert(
        "auto-file-renaming".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "auto-save-interval".to_string(),
        Value::String("60".to_string()),
    );
    map.insert(
        "conditional-get".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "connect-timeout".to_string(),
        Value::String("60".to_string()),
    );
    map.insert(
        "content-disposition-default-utf8".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("continue".to_string(), Value::String("true".to_string()));
    map.insert("dry-run".to_string(), Value::String("false".to_string()));
    map.insert(
        "enable-http-keep-alive".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "enable-http-pipelining".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "enable-mmap".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "enable-peer-exchange".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "file-allocation".to_string(),
        Value::String("none".to_string()),
    );
    map.insert(
        "follow-metalink".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "follow-torrent".to_string(),
        Value::String("true".to_string()),
    );
    map.insert("force-save".to_string(), Value::String("false".to_string()));
    map.insert("ftp-passwd".to_string(), Value::String("".to_string()));
    map.insert("ftp-user".to_string(), Value::String("".to_string()));
    map.insert("gid".to_string(), Value::String(gid));
    map.insert(
        "hash-check-only".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "http-accept-gzip".to_string(),
        // limedl negotiates gzip/br/zstd automatically on the plain single-stream
        // GET (range requests are forced to identity), so the option is
        // effectively always on. Reported truthfully as `true`.
        Value::String("true".to_string()),
    );
    map.insert(
        "http-auth-challenge".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "http-no-cache".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "lowest-speed-limit".to_string(),
        Value::String("0".to_string()),
    );
    map.insert(
        "max-file-not-found".to_string(),
        Value::String("0".to_string()),
    );
    map.insert(
        "max-resume-failure-tries".to_string(),
        Value::String("0".to_string()),
    );
    map.insert("max-tries".to_string(), Value::String("5".to_string()));
    map.insert(
        "max-upload-limit".to_string(),
        Value::String("0".to_string()),
    );
    map.insert(
        "metalink-base-uri".to_string(),
        Value::String("".to_string()),
    );
    map.insert(
        "metalink-enable-unique-protocol".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "metalink-language".to_string(),
        Value::String("".to_string()),
    );
    map.insert(
        "metalink-location".to_string(),
        Value::String("".to_string()),
    );
    map.insert("metalink-os".to_string(), Value::String("".to_string()));
    map.insert(
        "metalink-preferred-protocol".to_string(),
        Value::String("http".to_string()),
    );
    map.insert(
        "metalink-version".to_string(),
        Value::String("".to_string()),
    );
    map.insert(
        "min-split-size".to_string(),
        Value::String("20M".to_string()),
    );
    map.insert(
        "no-file-allocation-limit".to_string(),
        Value::String("5M".to_string()),
    );
    map.insert("no-netrc".to_string(), Value::String("false".to_string()));
    map.insert(
        "parameterized-uri".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("pause".to_string(), Value::String("false".to_string()));
    map.insert(
        "pause-metadata".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("proxy-method".to_string(), Value::String("get".to_string()));
    map.insert(
        "realtime-chunk-checksum".to_string(),
        Value::String("true".to_string()),
    );
    map.insert("referer".to_string(), Value::String(referer));
    map.insert(
        "remote-time".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "remove-control-file".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("retry-wait".to_string(), Value::String("0".to_string()));
    map.insert("reuse-uri".to_string(), Value::String("true".to_string()));
    map.insert(
        "rpc-save-upload-metadata".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "save-cookies".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "save-not-found".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "save-session-interval".to_string(),
        Value::String("0".to_string()),
    );
    map.insert("seed-ratio".to_string(), Value::String("1.0".to_string()));
    map.insert("seed-time".to_string(), Value::String("0".to_string()));
    map.insert("server-stat-of".to_string(), Value::String("".to_string()));
    map.insert(
        "server-stat-timeout".to_string(),
        Value::String("86400".to_string()),
    );
    map.insert(
        "show-console-readout".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "socket-recv-buffer-size".to_string(),
        Value::String("0".to_string()),
    );
    map.insert("stderr".to_string(), Value::String("false".to_string()));
    map.insert("stop".to_string(), Value::String("0".to_string()));
    map.insert(
        "stop-with-process".to_string(),
        Value::String("0".to_string()),
    );
    map.insert(
        "stream-piece-selector".to_string(),
        Value::String("default".to_string()),
    );
    map.insert(
        "summary-interval".to_string(),
        Value::String("60".to_string()),
    );
    map.insert("timeout".to_string(), Value::String("60".to_string()));
    map.insert(
        "uri-selector".to_string(),
        Value::String("feedback".to_string()),
    );
    map.insert("use-head".to_string(), Value::String("false".to_string()));
    map.insert("user-agent".to_string(), Value::String(user_agent));
    map.insert("header".to_string(), Value::Array(header_values));
    map.insert("position".to_string(), Value::String(position));
    // AriaNg's task-option set (its `aria2TaskAvailableOptions`) — the ones the
    // engine cannot surface are reported as safe defaults so its task dialog
    // renders completely.
    map.insert(
        "max-download-limit".to_string(),
        Value::String(summary.download_limit_bps.unwrap_or(0).to_string()),
    );
    map.insert("http-user".to_string(), Value::String(String::new()));
    map.insert("http-passwd".to_string(), Value::String(String::new()));
    map.insert("all-proxy".to_string(), Value::String(String::new()));
    map.insert("all-proxy-user".to_string(), Value::String(String::new()));
    map.insert("all-proxy-passwd".to_string(), Value::String(String::new()));
    map.insert("checksum".to_string(), Value::String(String::new()));
    map.insert(
        "check-integrity".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("bt-max-peers".to_string(), Value::String("128".to_string()));
    map.insert(
        "bt-request-peer-speed-limit".to_string(),
        Value::String("51200".to_string()),
    );
    map.insert(
        "bt-remove-unselected-file".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "bt-stop-timeout".to_string(),
        Value::String("0".to_string()),
    );
    map.insert("bt-tracker".to_string(), Value::String(String::new()));

    Ok(Value::Object(map))
}
