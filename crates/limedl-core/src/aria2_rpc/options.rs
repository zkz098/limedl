//! Per-task and global option methods plus aria2 option parsing helpers.

use super::*;

pub(crate) fn extract_option_str(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<String> {
    options
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_str())
        .map(String::from)
}

pub(crate) fn extract_option_usize(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<usize> {
    options
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_str().and_then(|s| s.parse::<usize>().ok()))
}

pub(crate) fn extract_option_u32(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<u32> {
    options
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_str().and_then(|s| s.parse::<u32>().ok()))
}

/// Collect the aria2 `header` option into `"Name: value"` strings.
///
/// aria2 documents `header` as a repeatable option; over JSON-RPC it is an
/// array of strings (AriaNg, Motrix), but some clients send a single string.
/// Malformed entries without a colon are dropped.
pub(crate) fn extract_option_headers(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Vec<String> {
    let mut headers = Vec::new();
    match options.and_then(|o| o.get(key)) {
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(raw) = item.as_str() {
                    push_header(&mut headers, raw);
                }
            }
        }
        Some(Value::String(raw)) => push_header(&mut headers, raw),
        _ => {}
    }
    headers
}

pub(crate) fn push_header(headers: &mut Vec<String>, raw: &str) {
    let trimmed = raw.trim();
    if !trimmed.is_empty() && trimmed.contains(':') {
        headers.push(trimmed.to_string());
    }
}

/// Build the effective extra-header list from aria2 request options.
///
/// Maps `header` (array/string), `referer` (`*` means the download URL
/// itself) and `http-user`/`http-passwd` (Basic auth) onto the header list
/// consumed by `StartDownloadRequest::headers`.
pub(crate) fn collect_request_headers(
    options: Option<&serde_json::Map<String, Value>>,
    url: &str,
) -> Vec<String> {
    let mut headers = extract_option_headers(options, "header");

    if let Some(referer) = extract_option_str(options, "referer") {
        let referer = referer.trim();
        if !referer.is_empty() && !has_header(&headers, "referer") {
            let value = if referer == "*" { url } else { referer };
            headers.push(format!("Referer: {value}"));
        }
    }

    if let Some(user) = extract_option_str(options, "http-user") {
        let user = user.trim();
        if !user.is_empty() && !has_header(&headers, "authorization") {
            let password = extract_option_str(options, "http-passwd").unwrap_or_default();
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
            headers.push(format!("Authorization: Basic {encoded}"));
        }
    }

    headers
}

/// Parse the aria2 `checksum` option (`"TYPE=DIGEST"`, e.g.
/// `sha-256=abcdef…`) into limedl checksum fields.
///
/// Unsupported hash types (md5, sha-1, adler32, …) and malformed values are
/// ignored, matching aria2's own tolerant behaviour for optional metadata.
/// sha-512 is accepted because it is part of limedl's supported set, even
/// though aria2 itself only knows sha-1/md5/sha-256.
pub(crate) fn parse_checksum_option(
    options: Option<&serde_json::Map<String, Value>>,
) -> (Option<ChecksumMode>, Option<String>) {
    let Some(raw) = extract_option_str(options, "checksum") else {
        return (None, None);
    };
    let Some((hash_type, digest)) = raw.trim().split_once('=') else {
        return (None, None);
    };
    let mode = match hash_type.trim().to_ascii_lowercase().as_str() {
        "sha-256" | "sha256" => ChecksumMode::Sha256,
        "sha-512" | "sha512" => ChecksumMode::Sha512,
        "blake3" => ChecksumMode::Blake3,
        _ => return (None, None),
    };
    let digest = digest.trim().to_ascii_lowercase();
    if digest.is_empty() {
        return (None, None);
    }
    (Some(mode), Some(digest))
}

/// Case-insensitive lookup of a `"Name: value"` entry's value.
pub(crate) fn find_header_value(headers: &[String], name: &str) -> Option<String> {
    headers.iter().find_map(|header| {
        let (header_name, value) = header.split_once(':')?;
        header_name
            .trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}

/// Read a task's effective User-Agent and extra headers from the in-memory
/// download map (all persisted downloads are loaded there at startup).
pub(crate) async fn task_request_headers(ctx: &RpcContext, id: &str) -> (String, Vec<String>) {
    let Some(dm) = ctx.registry.get_typed::<DownloadManager>() else {
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

pub(crate) async fn handle_get_global_option(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let dm = ctx
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| make_error(ERR_INTERNAL, "HTTP backend not available"))?;
    let settings = dm
        .settings()
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    Ok(serde_json::json!({
        "dir": settings.download.default_download_dir,
        "max-concurrent-downloads": settings.scheduler.traditional.max_parallel_tasks.to_string(),
        "max-connection-per-server": "16",
        "min-split-size": "20M",
        "split": "5",
        "max-overall-download-limit": "0",
    }))
}

pub(crate) async fn handle_change_global_option(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;

    let options = params
        .first()
        .and_then(|v| v.as_object())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing options object"))?;

    let dm = ctx
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| make_error(ERR_INTERNAL, "HTTP backend not available"))?;
    let mut settings = dm
        .settings()
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    if let Some(dir) = options.get("dir").and_then(|v| v.as_str()) {
        let dir = dir.trim();
        if dir.is_empty() {
            return Err(make_error(ERR_INVALID_PARAMS, "dir cannot be empty"));
        }
        if !PathBuf::from(dir).is_absolute() {
            return Err(make_error(
                ERR_INVALID_PARAMS,
                "dir must be an absolute path",
            ));
        }
        settings.download.default_download_dir = dir.to_string();
    }
    if let Some(max_tasks) = options
        .get("max-concurrent-downloads")
        .and_then(|v| v.as_str())
        && let Ok(n) = max_tasks.parse::<usize>()
    {
        settings.scheduler.traditional.max_parallel_tasks = n;
    }
    if let Some(limit) = options
        .get("max-overall-download-limit")
        .and_then(|v| v.as_str())
        && let Ok(_n) = limit.parse::<u64>()
    {
        // Per-task download limits are managed via the AIMD controller;
        // global limits are not yet implemented.
    }

    dm.apply_settings(settings)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    Ok(Value::String("OK".to_string()))
}
pub(crate) async fn handle_get_option(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
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
    let dir = std::path::Path::new(&summary.destination_path)
        .parent()
        .and_then(|p| p.to_str())
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
        Value::String("false".to_string()),
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

    Ok(Value::Object(map))
}
