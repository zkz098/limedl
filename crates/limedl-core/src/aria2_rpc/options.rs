//! Per-task and global option methods plus aria2 option parsing helpers.

use std::path::Path;

use base64::Engine;

use super::{ChecksumMode, ERR_INTERNAL, ERR_INVALID_PARAMS, JsonRpcError, PathBuf, RpcContext, TaskId, Value, extract_gid, get_all_summaries, has_header, make_error, resolve_gid};

/// Global aria2 option keys AriaNg reads, taken from its
/// `aria2GlobalAvailableOptions` (`src/scripts/config/aria2Options.js`) plus the
/// quick-settings speed limits. `getGlobalOption` must answer all of them; the
/// Tier 1 and Tier 2 tests assert exactly this set, so a client-facing key is
/// never dropped silently.
#[cfg(test)]
pub(crate) const ARIA2NG_GLOBAL_OPTION_KEYS: &[&str] = &[
    // basic
    "dir",
    "log",
    "max-concurrent-downloads",
    "check-integrity",
    "continue",
    // http / ftp / sftp
    "all-proxy",
    "all-proxy-user",
    "all-proxy-passwd",
    "connect-timeout",
    "dry-run",
    "lowest-speed-limit",
    "max-connection-per-server",
    "max-file-not-found",
    "max-tries",
    "min-split-size",
    "netrc-path",
    "no-netrc",
    "no-proxy",
    "proxy-method",
    "remote-time",
    "reuse-uri",
    "retry-wait",
    "server-stat-of",
    "server-stat-timeout",
    "split",
    "stream-piece-selector",
    "timeout",
    "uri-selector",
    // http
    "check-certificate",
    "http-accept-gzip",
    "http-auth-challenge",
    "http-no-cache",
    "http-user",
    "http-passwd",
    "http-proxy",
    "http-proxy-user",
    "http-proxy-passwd",
    "https-proxy",
    "https-proxy-user",
    "https-proxy-passwd",
    "referer",
    "enable-http-keep-alive",
    "enable-http-pipelining",
    "header",
    "save-cookies",
    "use-head",
    "user-agent",
    // ftp / sftp
    "ftp-user",
    "ftp-passwd",
    "ftp-pasv",
    "ftp-proxy",
    "ftp-proxy-user",
    "ftp-proxy-passwd",
    "ftp-type",
    "ftp-reuse-connection",
    "ssh-host-key-md",
    // bittorrent
    "bt-detach-seed-only",
    "bt-enable-hook-after-hash-check",
    "bt-enable-lpd",
    "bt-exclude-tracker",
    "bt-external-ip",
    "bt-force-encryption",
    "bt-hash-check-seed",
    "bt-load-saved-metadata",
    "bt-max-open-files",
    "bt-max-peers",
    "bt-metadata-only",
    "bt-min-crypto-level",
    "bt-prioritize-piece",
    "bt-remove-unselected-file",
    "bt-require-crypto",
    "bt-request-peer-speed-limit",
    "bt-save-metadata",
    "bt-seed-unverified",
    "bt-stop-timeout",
    "bt-tracker",
    "bt-tracker-connect-timeout",
    "bt-tracker-interval",
    "bt-tracker-timeout",
    "dht-file-path",
    "dht-file-path6",
    "dht-listen-port",
    "dht-message-timeout",
    "enable-dht",
    "enable-dht6",
    "enable-peer-exchange",
    "follow-torrent",
    "listen-port",
    "max-overall-upload-limit",
    "max-upload-limit",
    "peer-id-prefix",
    "peer-agent",
    "seed-ratio",
    "seed-time",
    // metalink
    "follow-metalink",
    "metalink-base-uri",
    "metalink-language",
    "metalink-location",
    "metalink-os",
    "metalink-version",
    "metalink-preferred-protocol",
    "metalink-enable-unique-protocol",
    // rpc
    "enable-rpc",
    "pause-metadata",
    "rpc-allow-origin-all",
    "rpc-listen-all",
    "rpc-listen-port",
    "rpc-max-request-size",
    "rpc-save-upload-metadata",
    "rpc-secure",
    // advanced
    "allow-overwrite",
    "allow-piece-length-change",
    "always-resume",
    "async-dns",
    "auto-file-renaming",
    "auto-save-interval",
    "conditional-get",
    "conf-path",
    "console-log-level",
    "content-disposition-default-utf8",
    "daemon",
    "deferred-input",
    "disable-ipv6",
    "disk-cache",
    "download-result",
    "dscp",
    "rlimit-nofile",
    "enable-color",
    "enable-mmap",
    "event-poll",
    "file-allocation",
    "force-save",
    "save-not-found",
    "hash-check-only",
    "human-readable",
    "keep-unfinished-download-result",
    "max-download-result",
    "max-mmap-limit",
    "max-resume-failure-tries",
    "min-tls-version",
    "log-level",
    "optimize-concurrent-downloads",
    "piece-length",
    "show-console-readout",
    "summary-interval",
    "max-overall-download-limit",
    "max-download-limit",
    "no-conf",
    "no-file-allocation-limit",
    "parameterized-uri",
    "quiet",
    "realtime-chunk-checksum",
    "remove-control-file",
    "save-session",
    "save-session-interval",
    "socket-recv-buffer-size",
    "stop",
    "truncate-console-readout",
];

/// Per-task aria2 option keys AriaNg reads (its `aria2TaskAvailableOptions`).
#[cfg(test)]
pub(crate) const ARIA2NG_TASK_OPTION_KEYS: &[&str] = &[
    "dir",
    "out",
    "allow-overwrite",
    "max-download-limit",
    "max-upload-limit",
    "split",
    "min-split-size",
    "max-connection-per-server",
    "lowest-speed-limit",
    "stream-piece-selector",
    "http-user",
    "http-passwd",
    "all-proxy",
    "all-proxy-user",
    "all-proxy-passwd",
    "checksum",
    "continue",
    "referer",
    "header",
    "bt-max-peers",
    "bt-request-peer-speed-limit",
    "bt-remove-unselected-file",
    "bt-stop-timeout",
    "bt-tracker",
    "seed-ratio",
    "seed-time",
    "pause-metadata",
    "conditional-get",
    "check-integrity",
    "file-allocation",
    "parameterized-uri",
    "force-save",
];

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

/// aria2 serialises every boolean option as a string over JSON-RPC
/// (`"pause": "true"`); accept raw booleans too.
pub(crate) fn aria2_bool(value: &Value) -> bool {
    value
        .as_bool()
        .unwrap_or_else(|| value.as_str().is_some_and(|s| s.eq_ignore_ascii_case("true") || s == "1"))
}

/// Read a boolean option from an aria2 options object.
pub(crate) fn option_is_true(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> bool {
    options.and_then(|o| o.get(key)).is_some_and(aria2_bool)
}

/// The optional trailing arguments of an aria2 RPC call: `(uris, options,
/// position)`, each absent unless the caller sent it.
type Aria2Tail = (
    Option<Vec<String>>,
    Option<serde_json::Map<String, Value>>,
    Option<i64>,
);

/// Split the trailing arguments of an aria2 RPC call by JSON type.
///
/// aria2's method signatures are type-driven: `addTorrent` is
/// `(torrent[, uris[, options[, position]]])`, so `uris` may be present or
/// omitted before `options`. AriaNg sends `addTorrent` as
/// `[torrent, [], options]`, which a naive `params.get(1)` read mistakes for
/// the options object and silently drops `dir`/`out`/`pause`/`select-file`;
/// the existing tests only exercised `[torrent, options]`, so the bug stayed
/// invisible. Scanning by type accepts both shapes and any future mix.
///
/// `from` is the index of the first optional argument (1 for both `addUri`
/// and `addTorrent`, whose first element is the URI list / torrent payload).
/// Returns the first URI array, the first options object and the first
/// integer position, each optional.
pub(crate) fn split_aria2_tail(params: &[Value], from: usize) -> Aria2Tail {
    let mut uris = None;
    let mut options = None;
    let mut position = None;
    for value in params.iter().skip(from) {
        match value {
            Value::Array(items) if uris.is_none() => {
                uris = Some(
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                );
            }
            Value::Object(map) if options.is_none() => options = Some(map.clone()),
            Value::Number(n) if position.is_none() => position = n.as_i64(),
            _ => {}
        }
    }
    (uris, options, position)
}

/// Parse aria2's `select-file`: a 1-based comma-separated index list, into the
/// engine's 0-based file indices.
pub(crate) fn parse_select_file(
    value: &Value,
) -> std::result::Result<Vec<usize>, JsonRpcError> {
    let raw = value.as_str().ok_or_else(|| {
        make_error(
            ERR_INVALID_PARAMS,
            "select-file must be a comma-separated string",
        )
    })?;

    let mut indices = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let index: usize = part.parse().map_err(|_| {
            make_error(ERR_INVALID_PARAMS, format!("invalid select-file index: {part}"))
        })?;
        if index == 0 {
            return Err(make_error(
                ERR_INVALID_PARAMS,
                "select-file indices are 1-based",
            ));
        }
        indices.push(index - 1);
    }
    Ok(indices)
}

/// Parse an aria2 byte-rate limit. `""` and `"0"` mean "unlimited".
fn parse_aria2_limit(value: &Value) -> std::result::Result<Option<u64>, JsonRpcError> {
    let raw = match value {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => {
            return Err(make_error(
                ERR_INVALID_PARAMS,
                "limit must be a number or a string",
            ));
        }
    };
    if raw.is_empty() || raw == "0" {
        return Ok(None);
    }
    raw.parse::<u64>()
        .map(Some)
        .map_err(|_| make_error(ERR_INVALID_PARAMS, format!("invalid limit: {raw}")))
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

/// aria2 global options that AriaNg reads.
///
/// The key set is AriaNg's `aria2GlobalAvailableOptions` (its
/// `src/scripts/config/aria2Options.js`, plus the quick-settings speed limits),
/// so its Settings pages can display every field they know about. Values come
/// from limedl's real settings where the engine has an equivalent; the rest are
/// aria2's documented defaults. Everything is a string because aria2 serialises
/// every option value as a string.
fn aria2_global_options(settings: &crate::types::AppSettings) -> serde_json::Map<String, Value> {
    use crate::types::{LogLevel, ProxyMode, SchedulerMode};

    fn flag(value: bool) -> String {
        if value {
            "true".to_string()
        } else {
            "false".to_string()
        }
    }

    let split = settings.scheduler.automatic.max_threads_per_task.to_string();
    let max_concurrent = match settings.scheduler.mode {
        SchedulerMode::Traditional => settings.scheduler.traditional.max_parallel_tasks,
        SchedulerMode::Automatic => settings.scheduler.automatic.max_parallel_threads,
    }
    .to_string();
    let proxy = if matches!(settings.proxy.mode, ProxyMode::Manual) {
        settings.proxy.manual_url.clone()
    } else {
        String::new()
    };
    let bt = &settings.bt;
    let listen_port = bt
        .listen_port
        .map(|port| port.to_string())
        .or_else(|| {
            bt.listen_port_range
                .map(|range| format!("{}-{}", range.start, range.end))
        })
        .unwrap_or_default();
    let log_level = match settings.logging.level {
        LogLevel::Trace | LogLevel::Debug => "debug",
        LogLevel::Info => "notice",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    }
    .to_string();
    let rpc = &settings.aria2_rpc;

    // Keys grouped by AriaNg's categories; a key that appears in several
    // categories is listed once.
    let pairs: &[(&str, String)] = &[
        // basic
        ("dir", settings.download.default_download_dir.clone()),
        ("log", settings.logging.file_path.clone()),
        ("max-concurrent-downloads", max_concurrent),
        ("check-integrity", flag(false)),
        ("continue", flag(true)),
        // http / ftp / sftp
        ("all-proxy", proxy.clone()),
        ("all-proxy-user", String::new()),
        ("all-proxy-passwd", String::new()),
        ("connect-timeout", "60".to_string()),
        ("dry-run", flag(false)),
        ("lowest-speed-limit", "0".to_string()),
        ("max-connection-per-server", split.clone()),
        ("max-file-not-found", "0".to_string()),
        ("max-tries", settings.download.default_max_retries.to_string()),
        ("min-split-size", "20M".to_string()),
        ("netrc-path", String::new()),
        ("no-netrc", flag(true)),
        ("no-proxy", String::new()),
        ("proxy-method", "get".to_string()),
        ("remote-time", flag(false)),
        ("reuse-uri", flag(true)),
        ("retry-wait", "0".to_string()),
        ("server-stat-of", String::new()),
        ("server-stat-timeout", "86400".to_string()),
        ("split", split.clone()),
        ("stream-piece-selector", "default".to_string()),
        ("timeout", "60".to_string()),
        ("uri-selector", "feedback".to_string()),
        // http
        ("check-certificate", flag(true)),
        ("http-accept-gzip", flag(true)),
        ("http-auth-challenge", flag(false)),
        ("http-no-cache", flag(false)),
        ("http-user", String::new()),
        ("http-passwd", String::new()),
        ("http-proxy", proxy.clone()),
        ("http-proxy-user", String::new()),
        ("http-proxy-passwd", String::new()),
        ("https-proxy", proxy.clone()),
        ("https-proxy-user", String::new()),
        ("https-proxy-passwd", String::new()),
        ("referer", String::new()),
        ("enable-http-keep-alive", flag(true)),
        ("enable-http-pipelining", flag(false)),
        ("header", String::new()),
        ("save-cookies", String::new()),
        ("use-head", flag(false)),
        (
            "user-agent",
            settings.download.default_user_agent.clone(),
        ),
        // ftp / sftp
        ("ftp-user", String::new()),
        ("ftp-passwd", String::new()),
        ("ftp-pasv", flag(true)),
        ("ftp-proxy", proxy.clone()),
        ("ftp-proxy-user", String::new()),
        ("ftp-proxy-passwd", String::new()),
        ("ftp-type", "binary".to_string()),
        ("ftp-reuse-connection", flag(true)),
        ("ssh-host-key-md", String::new()),
        // bittorrent
        ("bt-detach-seed-only", flag(false)),
        ("bt-enable-hook-after-hash-check", flag(true)),
        ("bt-enable-lpd", flag(bt.enable_lsd)),
        ("bt-exclude-tracker", String::new()),
        ("bt-external-ip", String::new()),
        ("bt-force-encryption", flag(false)),
        ("bt-hash-check-seed", flag(true)),
        ("bt-load-saved-metadata", flag(false)),
        ("bt-max-open-files", "100".to_string()),
        ("bt-max-peers", bt.max_peers_per_torrent.to_string()),
        ("bt-metadata-only", flag(false)),
        ("bt-min-crypto-level", "plain".to_string()),
        ("bt-prioritize-piece", String::new()),
        ("bt-remove-unselected-file", flag(false)),
        ("bt-require-crypto", flag(false)),
        ("bt-request-peer-speed-limit", "51200".to_string()),
        ("bt-save-metadata", flag(false)),
        ("bt-seed-unverified", flag(false)),
        ("bt-stop-timeout", "0".to_string()),
        ("bt-tracker", bt.tracker_list.clone()),
        ("bt-tracker-connect-timeout", "60".to_string()),
        ("bt-tracker-interval", "0".to_string()),
        ("bt-tracker-timeout", "60".to_string()),
        ("dht-file-path", String::new()),
        ("dht-file-path6", String::new()),
        ("dht-listen-port", listen_port.clone()),
        ("dht-message-timeout", "10".to_string()),
        ("enable-dht", flag(bt.dht_enabled)),
        ("enable-dht6", flag(bt.dht_enabled && bt.enable_ipv6)),
        ("enable-peer-exchange", flag(bt.enable_pex)),
        ("follow-torrent", flag(true)),
        ("listen-port", listen_port.clone()),
        (
            "max-overall-upload-limit",
            bt.global_upload_rate_limit.to_string(),
        ),
        ("max-upload-limit", "0".to_string()),
        ("peer-id-prefix", "A2-1-37-0-".to_string()),
        ("peer-agent", String::new()),
        ("seed-ratio", bt.upload_ratio_limit.to_string()),
        ("seed-time", String::new()),
        // metalink (not implemented; aria2 defaults keep AriaNg's form intact)
        ("follow-metalink", flag(true)),
        ("metalink-base-uri", String::new()),
        ("metalink-language", String::new()),
        ("metalink-location", String::new()),
        ("metalink-os", String::new()),
        ("metalink-version", String::new()),
        ("metalink-preferred-protocol", "https".to_string()),
        ("metalink-enable-unique-protocol", flag(true)),
        // rpc
        ("enable-rpc", flag(true)),
        ("pause-metadata", flag(false)),
        ("rpc-allow-origin-all", flag(rpc.allow_any_origin)),
        (
            "rpc-listen-all",
            flag(!super::is_loopback_bind_address(&rpc.listen_address)),
        ),
        ("rpc-listen-port", rpc.port.to_string()),
        ("rpc-max-request-size", "1M".to_string()),
        ("rpc-save-upload-metadata", flag(true)),
        ("rpc-secure", flag(false)),
        // advanced
        ("allow-overwrite", flag(false)),
        ("allow-piece-length-change", flag(false)),
        ("always-resume", flag(true)),
        ("async-dns", flag(true)),
        ("auto-file-renaming", flag(true)),
        ("auto-save-interval", "60".to_string()),
        ("conditional-get", flag(false)),
        ("conf-path", String::new()),
        ("console-log-level", log_level.clone()),
        ("content-disposition-default-utf8", flag(false)),
        ("daemon", flag(false)),
        ("deferred-input", flag(false)),
        ("disable-ipv6", flag(!bt.enable_ipv6)),
        ("disk-cache", "0".to_string()),
        ("download-result", "default".to_string()),
        ("dscp", "0".to_string()),
        ("rlimit-nofile", "1024".to_string()),
        ("enable-color", flag(true)),
        ("enable-mmap", flag(false)),
        ("event-poll", "epoll".to_string()),
        ("file-allocation", "none".to_string()),
        ("force-save", flag(false)),
        ("save-not-found", flag(true)),
        ("hash-check-only", flag(false)),
        ("human-readable", flag(true)),
        ("keep-unfinished-download-result", flag(true)),
        ("max-download-result", "1000".to_string()),
        ("max-mmap-limit", "9223372036854775807".to_string()),
        ("max-resume-failure-tries", "0".to_string()),
        ("min-tls-version", "TLSv1.2".to_string()),
        ("log-level", log_level),
        ("optimize-concurrent-downloads", flag(false)),
        ("piece-length", "1M".to_string()),
        ("show-console-readout", flag(true)),
        ("summary-interval", "60".to_string()),
        (
            "max-overall-download-limit",
            settings.global_speed_limit_bps.to_string(),
        ),
        ("max-download-limit", "0".to_string()),
        ("no-conf", flag(false)),
        ("no-file-allocation-limit", "5M".to_string()),
        ("parameterized-uri", flag(false)),
        ("quiet", flag(false)),
        ("realtime-chunk-checksum", flag(true)),
        ("remove-control-file", flag(false)),
        ("save-session", String::new()),
        ("save-session-interval", "0".to_string()),
        ("socket-recv-buffer-size", "0".to_string()),
        ("stop", "0".to_string()),
        ("truncate-console-readout", flag(true)),
    ];

    let mut options = serde_json::Map::new();
    for (key, value) in pairs {
        options.insert((*key).to_string(), Value::String(value.clone()));
    }
    options
}

pub(crate) async fn handle_get_global_option(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let dm = ctx
        .http()
        .ok_or_else(|| make_error(ERR_INTERNAL, "HTTP backend not available"))?;
    let settings = dm
        .settings()
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    Ok(Value::Object(aria2_global_options(&settings)))
}

pub(crate) async fn handle_change_global_option(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let options = params
        .first()
        .and_then(|v| v.as_object())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing options object"))?;

    let dm = ctx.http()
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
        // aria2's `max-concurrent-downloads` is the cap the scheduler actually
        // enforces, which depends on limedl's active scheduler mode.
        match settings.scheduler.mode {
            crate::types::SchedulerMode::Traditional => {
                settings.scheduler.traditional.max_parallel_tasks = n;
            }
            crate::types::SchedulerMode::Automatic => {
                settings.scheduler.automatic.max_parallel_threads = n;
            }
        }
    }
    if let Some(limit) = options
        .get("max-overall-download-limit")
        .and_then(|v| v.as_str())
        && let Ok(bytes_per_second) = limit.parse::<u64>()
    {
        // aria2's global download limit is the same token bucket the scheduler
        // uses; 0 = unlimited.
        settings.global_speed_limit_bps = bytes_per_second;
    }
    if let Some(limit) = options
        .get("max-overall-upload-limit")
        .and_then(|v| v.as_str())
        && let Ok(bytes_per_second) = limit.parse::<u64>()
    {
        settings.bt.global_upload_rate_limit = bytes_per_second;
    }
    if let Some(split) = options.get("split").and_then(|v| v.as_str())
        && let Ok(threads) = split.parse::<usize>()
    {
        settings.scheduler.automatic.max_threads_per_task = threads.clamp(1, 32);
    }
    if let Some(max_connections) = options
        .get("max-connection-per-server")
        .and_then(|v| v.as_str())
        && let Ok(threads) = max_connections.parse::<usize>()
    {
        settings.scheduler.automatic.max_threads_per_task = threads.clamp(1, 32);
    }
    if let Some(tries) = options.get("max-tries").and_then(|v| v.as_str())
        && let Ok(retries) = tries.parse::<u32>()
    {
        settings.download.default_max_retries = retries.min(20);
    }
    if let Some(user_agent) = options.get("user-agent").and_then(|v| v.as_str())
        && !user_agent.trim().is_empty()
    {
        settings.download.default_user_agent = user_agent.trim().to_string();
    }
    // BitTorrent options that map onto `BtSettings`.
    if let Some(enabled) = options.get("enable-dht").and_then(|v| v.as_str()) {
        settings.bt.dht_enabled = enabled == "true";
    }
    if let Some(enabled) = options.get("enable-peer-exchange").and_then(|v| v.as_str()) {
        settings.bt.enable_pex = enabled == "true";
    }
    if let Some(peers) = options.get("bt-max-peers").and_then(|v| v.as_str())
        && let Ok(max_peers) = peers.parse::<u32>()
    {
        settings.bt.max_peers_per_torrent = max_peers;
    }
    if let Some(ratio) = options.get("seed-ratio").and_then(|v| v.as_str())
        && let Ok(seed_ratio) = ratio.parse::<f64>()
    {
        settings.bt.upload_ratio_limit = seed_ratio;
    }
    if let Some(trackers) = options.get("bt-tracker").and_then(|v| v.as_str()) {
        settings.bt.tracker_list = trackers.to_string();
    }
    if let Some(port) = options.get("listen-port").and_then(|v| v.as_str())
        && let Ok(port) = port.parse::<u16>()
    {
        settings.bt.listen_port = Some(port);
    }

    dm.apply_settings(settings)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    Ok(Value::String("OK".to_string()))
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
pub(crate) async fn handle_change_option(
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

pub(crate) async fn handle_get_option(
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
    map.insert("check-integrity".to_string(), Value::String("false".to_string()));
    map.insert("bt-max-peers".to_string(), Value::String("128".to_string()));
    map.insert(
        "bt-request-peer-speed-limit".to_string(),
        Value::String("51200".to_string()),
    );
    map.insert(
        "bt-remove-unselected-file".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("bt-stop-timeout".to_string(), Value::String("0".to_string()));
    map.insert("bt-tracker".to_string(), Value::String(String::new()));

    Ok(Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tier 1 half of the AriaNg contract: `getGlobalOption` answers every key
    /// AriaNg's Settings pages read, and every value is a string like aria2's.
    #[test]
    fn global_options_cover_the_ariang_key_set() {
        let options = aria2_global_options(&crate::types::AppSettings::default());
        let missing: Vec<&str> = ARIA2NG_GLOBAL_OPTION_KEYS
            .iter()
            .copied()
            .filter(|key| !options.contains_key(*key))
            .collect();
        assert!(
            missing.is_empty(),
            "getGlobalOption is missing AriaNg keys: {missing:?}"
        );
        for (key, value) in &options {
            assert!(value.is_string(), "{key} must be a string like aria2: {value}");
        }
    }
}
