use std::path::PathBuf;

use serde_json::Value;

use crate::context::RpcContext;
use crate::jsonrpc::{ERR_INTERNAL, ERR_INVALID_PARAMS, JsonRpcError, make_error};

/// aria2 global options that AriaNg reads.
///
/// The key set is AriaNg's `aria2GlobalAvailableOptions` (its
/// `src/scripts/config/aria2Options.js`, plus the quick-settings speed limits),
/// so its Settings pages can display every field they know about. Values come
/// from limedl's real settings where the engine has an equivalent; the rest are
/// aria2's documented defaults. Everything is a string because aria2 serialises
/// every option value as a string.
pub(super) fn aria2_global_options(
    settings: &crate::types::AppSettings,
) -> serde_json::Map<String, Value> {
    use crate::types::{LogLevel, ProxyMode, SchedulerMode};

    fn flag(value: bool) -> String {
        if value {
            "true".to_string()
        } else {
            "false".to_string()
        }
    }

    let split = settings
        .scheduler
        .automatic
        .max_threads_per_task
        .to_string();
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
        (
            "max-tries",
            settings.download.default_max_retries.to_string(),
        ),
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
        ("user-agent", settings.download.default_user_agent.clone()),
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
        ("bt-enable-lpd", flag(false)),
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
        ("enable-peer-exchange", flag(true)),
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
            flag(!crate::is_loopback_bind_address(&rpc.listen_address)),
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

pub async fn handle_get_global_option(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let dm = ctx
        .http()
        .ok_or_else(|| make_error(ERR_INTERNAL, "HTTP backend not available"))?;
    let settings = dm
        .settings()
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    Ok(Value::Object(aria2_global_options(&settings)))
}

pub async fn handle_change_global_option(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let options = params
        .first()
        .and_then(|v| v.as_object())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing options object"))?;

    let dm = ctx
        .http()
        .ok_or_else(|| make_error(ERR_INTERNAL, "HTTP backend not available"))?;
    let mut settings = dm
        .settings()
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    for (key, value) in options {
        if let Some(raw) = value.as_str() {
            apply_global_option(&mut settings, key, raw)?;
        }
    }

    dm.apply_settings(settings)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    Ok(Value::String("OK".to_string()))
}

fn apply_global_option(
    settings: &mut crate::types::AppSettings,
    key: &str,
    raw: &str,
) -> Result<(), JsonRpcError> {
    match key {
        "dir" => apply_dir_option(settings, raw)?,
        "max-concurrent-downloads" => {
            if let Ok(n) = raw.parse::<usize>() {
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
        }
        "max-overall-download-limit" => {
            if let Ok(bps) = raw.parse::<u64>() {
                settings.global_speed_limit_bps = bps;
            }
        }
        "max-overall-upload-limit" => {
            if let Ok(bps) = raw.parse::<u64>() {
                settings.bt.global_upload_rate_limit = bps;
            }
        }
        "split" | "max-connection-per-server" => {
            if let Ok(threads) = raw.parse::<usize>() {
                settings.scheduler.automatic.max_threads_per_task = threads.clamp(1, 32);
            }
        }
        "max-tries" => {
            if let Ok(retries) = raw.parse::<u32>() {
                settings.download.default_max_retries = retries.min(20);
            }
        }
        "user-agent" => {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                settings.download.default_user_agent = trimmed.to_string();
            }
        }
        _ => apply_bt_global_option(&mut settings.bt, key, raw),
    }
    Ok(())
}

fn apply_dir_option(
    settings: &mut crate::types::AppSettings,
    raw: &str,
) -> Result<(), JsonRpcError> {
    let dir = raw.trim();
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
    Ok(())
}

fn apply_bt_global_option(bt: &mut crate::types::BtSettings, key: &str, raw: &str) {
    match key {
        "enable-dht" => bt.dht_enabled = raw == "true",
        "bt-max-peers" => {
            if let Ok(max_peers) = raw.parse::<u32>() {
                bt.max_peers_per_torrent = max_peers;
            }
        }
        "seed-ratio" => {
            if let Ok(seed_ratio) = raw.parse::<f64>() {
                bt.upload_ratio_limit = seed_ratio;
            }
        }
        "bt-tracker" => bt.tracker_list = raw.to_string(),
        "listen-port" => {
            if let Ok(port) = raw.parse::<u16>() {
                bt.listen_port = Some(port);
            }
        }
        _ => {}
    }
}
