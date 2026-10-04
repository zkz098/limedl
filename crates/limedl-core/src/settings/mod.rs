use std::{
    fs, io,
    path::{Path, PathBuf},
};

use reqwest::Url;
use tokio::io::AsyncWriteExt as _;

use super::{
    error::{DownloadError, Result},
    http_client_factory::normalize_user_agent,
    types::{
        AppSettings, Aria2RpcSettings, AutomaticSchedulerSettings, BtSettings,
        DownloadDefaultsSettings, IoBaselineSettings, LogSettings, ProxyMode, ProxySettings,
        RewriteTarget, SchedulerSettings, TraditionalSchedulerSettings, UrlRewriteRule,
        UrlRewriteSettings, default_tracker_list_url,
    },
};

fn normalize_proxy_settings(settings: ProxySettings) -> Result<ProxySettings> {
    match settings.mode {
        ProxyMode::Disabled | ProxyMode::System => Ok(ProxySettings {
            mode: settings.mode,
            manual_url: String::new(),
        }),
        ProxyMode::Manual => {
            let manual_url = settings.manual_url.trim().to_string();
            if manual_url.is_empty() {
                return Err(DownloadError::InvalidProxy(String::from(
                    "manual proxy url is required",
                )));
            }

            Url::parse(&manual_url)
                .map_err(|error| DownloadError::InvalidProxy(error.to_string()))?;

            Ok(ProxySettings {
                mode: ProxyMode::Manual,
                manual_url,
            })
        }
    }
}

pub fn normalize_settings(settings: AppSettings) -> Result<AppSettings> {
    let proxy = normalize_proxy_settings(settings.proxy)?;
    let max_parallel_tasks = settings
        .scheduler
        .traditional
        .max_parallel_tasks
        .clamp(1, 32);
    let max_parallel_threads = settings
        .scheduler
        .automatic
        .max_parallel_threads
        .clamp(1, 64);
    let max_threads_per_task = settings
        .scheduler
        .automatic
        .max_threads_per_task
        .clamp(1, 32)
        .min(max_parallel_threads);
    let bt = normalize_bt_settings(settings.bt)?;
    let logging = normalize_logging_settings(settings.logging);
    let default_user_agent = normalize_user_agent(&settings.download.default_user_agent)?;
    let default_download_dir = normalize_download_dir(&settings.download.default_download_dir);

    let url_rewrite = normalize_url_rewrite_settings(settings.url_rewrite);
    let aria2_rpc = normalize_aria2_rpc_settings(settings.aria2_rpc.clone());
    let io_baseline = IoBaselineSettings {
        buffer_limit_mb: settings.io_baseline.buffer_limit_mb.clamp(64, 32768),
        game_mode_buffer_mb: settings.io_baseline.game_mode_buffer_mb.clamp(16, 4096),
        game_mode: settings.io_baseline.game_mode,
        max_parallel_hdd: settings.io_baseline.max_parallel_hdd.clamp(1, 16),
        game_mode_max_parallel: settings.io_baseline.game_mode_max_parallel.clamp(1, 4),
        disk_type_overrides: settings.io_baseline.disk_type_overrides,
        hdd_buffer_enabled: settings.io_baseline.hdd_buffer_enabled,
        ssd_write_combine_mb: settings.io_baseline.ssd_write_combine_mb.clamp(0, 4096),
    };

    Ok(AppSettings {
        appearance: settings.appearance,
        proxy,
        scheduler: SchedulerSettings {
            mode: settings.scheduler.mode,
            traditional: TraditionalSchedulerSettings { max_parallel_tasks },
            automatic: AutomaticSchedulerSettings {
                max_parallel_threads,
                max_threads_per_task,
                min_threads_per_task: normalize_min_threads(
                    settings.scheduler.automatic.min_threads_per_task,
                    max_threads_per_task,
                ),
                adaptive_profile: settings.scheduler.automatic.adaptive_profile,
            },
            chunk_size_strategy: settings.scheduler.chunk_size_strategy,
            tail_sprint_enabled: settings.scheduler.tail_sprint_enabled,
            connection_warmup_enabled: settings.scheduler.connection_warmup_enabled,
        },
        download: DownloadDefaultsSettings {
            default_download_dir,
            default_max_retries: settings.download.default_max_retries.clamp(0, 20),
            default_checksum: settings.download.default_checksum,
            default_user_agent,
            auto_detect_sha256: settings.download.auto_detect_sha256,
        },
        bt,
        logging,
        aria2_rpc,
        cdn_acceleration: settings.cdn_acceleration.clone(),
        url_rewrite,
        global_speed_limit_bps: settings.global_speed_limit_bps,
        speed_limit_schedule: settings.speed_limit_schedule.clone(),
        notifications: settings.notifications.clone(),
        io_baseline,
        autostart: settings.autostart,
        setup_completed: settings.setup_completed,
        last_setup_step: settings.last_setup_step.map(|s| s.clamp(0, 9)),
        double_click: settings.double_click,
        max_in_memory_downloads: clamp_max_in_memory(settings.max_in_memory_downloads),
    })
}

/// Trim per-client tokens and drop entries a hand-edit left without a hash:
/// a malformed/empty PHC string can never verify, so it would only add a dead
/// row to the UI. The plaintext token is never present here — settings.json
/// only carries the Argon2 hash.
fn normalize_aria2_rpc_settings(mut rpc: Aria2RpcSettings) -> Aria2RpcSettings {
    rpc.clients
        .retain(|client| !client.token_hash.trim().is_empty());
    for client in &mut rpc.clients {
        client.name = client.name.trim().to_string();
        client.token_hash = client.token_hash.trim().to_string();
    }
    // A hand-edited (or legacy) settings file may have no address, an empty
    // one, or a value that can never be a host (`host:port`, a URL). Fall back
    // to loopback rather than failing to bind on a value that is not an address.
    let listen = rpc.listen_address.trim();
    rpc.listen_address = if is_plausible_bind_host(listen) {
        listen.to_string()
    } else {
        "127.0.0.1".to_string()
    };
    rpc.cors_allowed_origins = rpc
        .cors_allowed_origins
        .into_iter()
        .map(|origin| origin.trim().to_string())
        .filter(|origin| !origin.is_empty())
        .collect();
    rpc
}

/// Whether `host` can plausibly be handed to `TcpListener::bind`: an IPv4/IPv6
/// literal (optionally bracketed) or a bare hostname label. A `host:port`, a
/// URL or whitespace is not an address, and `format_bind_addr` would silently
/// append a second port to it.
fn is_plausible_bind_host(host: &str) -> bool {
    let host = host.trim();
    if host.is_empty() || host.contains('/') || host.contains(char::is_whitespace) {
        return false;
    }
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if bare.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    !bare.contains(':')
        && bare
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

fn normalize_min_threads(raw: usize, max_per_task: usize) -> usize {
    if raw == 0 {
        (max_per_task / 2).max(1)
    } else {
        raw.clamp(1, max_per_task)
    }
}

/// 0 = unlimited (no eviction). Positive values clamped to [10, 10000].
fn clamp_max_in_memory(raw: usize) -> usize {
    if raw == 0 { 0 } else { raw.clamp(10, 10000) }
}

fn normalize_logging_settings(settings: LogSettings) -> LogSettings {
    LogSettings {
        enabled: settings.enabled,
        level: settings.level,
        file_path: settings.file_path.trim().to_string(),
        retention_count: settings.retention_count.map(|c| c.clamp(0, 1000)),
        retention_days: settings.retention_days.map(|d| d.clamp(0, 3650)),
    }
}

fn normalize_download_dir(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return String::new();
    }
    trimmed.to_string()
}

fn normalize_bt_settings(settings: BtSettings) -> Result<BtSettings> {
    const MAX_UPLOAD_LIMIT_BYTES: u64 = 10 * 1024 * 1024 * 1024 * 1024;
    const MIN_PORT: u16 = 1025;
    let tracker_list = normalize_tracker_list(&settings.tracker_list)?;
    let tracker_list_url = normalize_tracker_list_url(&settings.tracker_list_url)?;

    let listen_port_range = match settings.listen_port_range {
        Some(range) => {
            if range.start > range.end {
                return Err(DownloadError::InvalidResponse(format!(
                    "listen_port_range: start ({}) must be <= end ({})",
                    range.start, range.end
                )));
            }
            if range.start < MIN_PORT || range.end < MIN_PORT {
                return Err(DownloadError::InvalidResponse(format!(
                    "listen_port_range: ports must be >= {} (got {}-{})",
                    MIN_PORT, range.start, range.end
                )));
            }
            Some(range)
        }
        None => None,
    };

    Ok(BtSettings {
        lightweight_mode: settings.lightweight_mode,
        dht_enabled: settings.dht_enabled,
        tracker_list,
        tracker_list_url,
        pause_upload_when_limit_reached: settings.pause_upload_when_limit_reached,
        upload_limit_bytes: settings.upload_limit_bytes.min(MAX_UPLOAD_LIMIT_BYTES),
        upload_ratio_limit: if settings.upload_ratio_limit.is_finite() {
            settings.upload_ratio_limit.clamp(0.0, 100.0)
        } else {
            0.0
        },
        anti_leech_enabled: settings.anti_leech_enabled,
        anti_leech_action: settings.anti_leech_action,
        anti_leech_grace_secs: settings.anti_leech_grace_secs,
        anti_leech_ratio: if settings.anti_leech_ratio.is_finite() {
            settings.anti_leech_ratio.clamp(0.0, 1.0)
        } else {
            0.0
        },
        anti_leech_ban_secs: settings.anti_leech_ban_secs,
        anti_leech_max_upload_slots: settings.anti_leech_max_upload_slots.max(1),
        seed_choking_algorithm: settings.seed_choking_algorithm,
        choking_algorithm: settings.choking_algorithm,
        max_upload_slots_per_torrent: settings.max_upload_slots_per_torrent.clamp(1, 64),
        max_peers_per_torrent: settings.max_peers_per_torrent.clamp(1, 4096),
        smart_ban_max_failures: settings.smart_ban_max_failures.clamp(1, 100),
        smart_ban_parole: settings.smart_ban_parole,
        eviction_ban_duration_secs: settings.eviction_ban_duration_secs.min(604_800),
        data_contribution_timeout_secs: settings.data_contribution_timeout_secs.min(86_400),
        blocklist_enabled: settings.blocklist_enabled,
        blocklist_path: settings.blocklist_path.trim().to_string(),
        upnp_enabled: settings.upnp_enabled,
        listen_port_range,
        listen_port: settings
            .listen_port
            .filter(|&p| (1024..=65535).contains(&p)),
        enable_natpmp: settings.enable_natpmp,
        enable_ipv6: settings.enable_ipv6,
        enable_pex: settings.enable_pex,
        enable_lsd: settings.enable_lsd,
        enable_utp: settings.enable_utp,
        enable_fast_extension: settings.enable_fast_extension,
        enable_holepunch: settings.enable_holepunch,
        enable_web_seed: settings.enable_web_seed,
        enable_super_seeding: settings.enable_super_seeding,
        global_download_rate_limit: settings.global_download_rate_limit,
        global_upload_rate_limit: settings.global_upload_rate_limit,
        preallocate_mode: settings.preallocate_mode,
        encryption_mode: settings.encryption_mode,
        max_downloads: settings.max_downloads,
        max_seeds: settings.max_seeds,
        max_torrents: settings.max_torrents,
        active_limit: settings.active_limit,
    })
}

pub fn resolve_user_agent(
    request_user_agent: Option<&str>,
    default_user_agent: &str,
) -> Result<String> {
    match request_user_agent
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(user_agent) => normalize_user_agent(user_agent),
        None => normalize_user_agent(default_user_agent),
    }
}

fn normalize_tracker_list(tracker_list: &str) -> Result<String> {
    let mut normalized = Vec::new();

    for raw_tracker in tracker_list.lines() {
        let tracker = raw_tracker.trim();
        if tracker.is_empty() {
            continue;
        }

        normalized.push(parse_tracker_url(tracker)?);
    }

    Ok(finalize_tracker_list(normalized))
}

pub fn normalize_tracker_list_lossy(tracker_list: &str) -> String {
    let normalized = tracker_list
        .lines()
        .map(str::trim)
        .filter(|tracker| !tracker.is_empty())
        .filter_map(|tracker| parse_tracker_url(tracker).ok())
        .collect::<Vec<_>>();

    finalize_tracker_list(normalized)
}

fn parse_tracker_url(tracker: &str) -> Result<String> {
    let parsed =
        Url::parse(tracker).map_err(|error| DownloadError::InvalidResponse(error.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https" | "udp") {
        return Err(DownloadError::InvalidResponse(format!(
            "unsupported tracker scheme: {}",
            parsed.scheme()
        )));
    }

    Ok(parsed.to_string())
}

fn finalize_tracker_list(mut normalized: Vec<String>) -> String {
    normalized.sort();
    normalized.dedup();
    normalized.join("\n")
}

pub fn normalize_tracker_list_url(tracker_list_url: &str) -> Result<String> {
    let tracker_list_url = tracker_list_url.trim();
    if tracker_list_url.is_empty() {
        return Ok(default_tracker_list_url());
    }

    let parsed = Url::parse(tracker_list_url)
        .map_err(|error| DownloadError::InvalidResponse(error.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(DownloadError::InvalidResponse(format!(
            "unsupported tracker list url scheme: {}",
            parsed.scheme()
        )));
    }

    Ok(parsed.to_string())
}

fn normalize_url_rewrite_settings(settings: UrlRewriteSettings) -> UrlRewriteSettings {
    let rules: Vec<UrlRewriteRule> = settings
        .rules
        .into_iter()
        .filter_map(|mut rule| {
            rule.id = rule.id.trim().to_string();
            if rule.id.is_empty() {
                rule.id = uuid::Uuid::new_v4().to_string();
            }
            rule.name = rule.name.trim().to_string();
            rule.pattern = rule.pattern.trim().to_string();
            if rule.pattern.is_empty() {
                return None;
            }

            let targets: Vec<RewriteTarget> = rule
                .targets
                .into_iter()
                .filter_map(|mut t| {
                    t.url_template = t.url_template.trim().to_string();
                    if t.url_template.is_empty() {
                        None
                    } else {
                        Some(t)
                    }
                })
                .enumerate()
                .map(|(index, mut t)| {
                    t.order = index as u32;
                    t
                })
                .collect();

            rule.targets = targets;
            Some(rule)
        })
        .enumerate()
        .map(|(index, mut rule)| {
            rule.order = index as u32;
            rule
        })
        .collect();

    UrlRewriteSettings {
        enabled: settings.enabled,
        rules,
    }
}

pub fn load_settings(settings_path: &Path) -> Result<AppSettings> {
    let content = match fs::read_to_string(settings_path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(AppSettings::default());
        }
        Err(error) => return Err(error.into()),
    };

    // Every `AppSettings` field is `#[serde(default)]`, so a partial or
    // hand-edited file still loads. Unknown keys are ignored: the old
    // `githubMirror` / `sha1` checksum settings are no longer migrated.
    //
    // Unparseable content must not brick the application. Passing the error up
    // used to abort every start (the GUI has nowhere to show it), so instead the
    // bad file is moved aside, the previous save is tried, and only then do the
    // built-in defaults apply.
    match serde_json::from_str::<AppSettings>(&content)
        .map_err(DownloadError::from)
        .and_then(normalize_settings)
    {
        Ok(settings) => Ok(settings),
        Err(error) => {
            let quarantined = quarantine_unreadable_settings(settings_path);
            let where_kept = match &quarantined {
                Some(path) => format!(" (kept at {})", path.display()),
                None => String::new(),
            };

            if let Some(recovered) = read_settings_backup(settings_path) {
                tracing::error!(
                    "settings at {} are unreadable ({error}){where_kept}; recovered the previous \
                     settings from the backup",
                    settings_path.display()
                );
                return Ok(recovered);
            }

            tracing::error!(
                "settings at {} are unreadable ({error}){where_kept}; starting from defaults",
                settings_path.display()
            );
            Ok(AppSettings::default())
        }
    }
}

/// Move an unreadable settings file aside so the next save can create a fresh
/// one and the user can still inspect what was there.
fn quarantine_unreadable_settings(settings_path: &Path) -> Option<PathBuf> {
    let quarantine = settings_path.with_extension("json.corrupt");
    match fs::rename(settings_path, &quarantine) {
        Ok(()) => Some(quarantine),
        Err(error) => {
            tracing::error!(
                "could not move the unreadable settings file {} aside: {error}",
                settings_path.display()
            );
            None
        }
    }
}

/// Parse the previous `settings.json.bak` snapshot, if there is a usable one.
fn read_settings_backup(settings_path: &Path) -> Option<AppSettings> {
    let backup = settings_path.with_extension("json.bak");
    let content = fs::read_to_string(&backup).ok()?;
    serde_json::from_str::<AppSettings>(&content)
        .ok()
        .and_then(|settings| normalize_settings(settings).ok())
}

pub async fn persist_settings(settings_path: &Path, settings: &AppSettings) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let serialized = serde_json::to_vec_pretty(settings)?;
    let temp_path = settings_path.with_extension("json.tmp");
    // Flush and fsync the temp file *before* the rename. Without it a power loss
    // can make the rename durable while the data is not, leaving an empty
    // settings.json behind — which is exactly the unreadable-file case above.
    {
        let mut file = tokio::fs::File::create(&temp_path).await?;
        file.write_all(&serialized).await?;
        file.flush().await?;
        file.sync_all().await?;
    }

    // Snapshot the outgoing file so a bad save has a way back.
    back_up_settings(settings_path).await;

    tokio::fs::rename(&temp_path, settings_path).await?;
    Ok(())
}

/// Best-effort copy of the current settings to `<name>.json.bak`.
async fn back_up_settings(settings_path: &Path) {
    if !tokio::fs::try_exists(settings_path).await.unwrap_or(false) {
        return;
    }
    let backup = settings_path.with_extension("json.bak");
    if let Err(error) = tokio::fs::copy(settings_path, &backup).await {
        tracing::warn!(
            "could not back up settings to {}: {error}",
            backup.display()
        );
    }
}

#[cfg(test)]
mod tests;
