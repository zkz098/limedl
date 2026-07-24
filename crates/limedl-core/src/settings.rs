use std::{
    fs, io,
    path::{Path, PathBuf},
};

use reqwest::Url;

use super::{
    error::{DownloadError, Result},
    http_client_factory::normalize_user_agent,
    types::{
        AppSettings, Aria2RpcSettings, AutomaticSchedulerSettings, BtSettings,
        CdnAccelerationSettings, DownloadDefaultsSettings, GitHubMirrorSettings,
        IoBaselineSettings, LogSettings, MirrorEntry, NotificationSettings, ProxyMode,
        ProxySettings, SchedulerSettings, TraditionalSchedulerSettings, default_tracker_list_url,
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

    let github_mirror = normalize_github_mirror_settings(settings.github_mirror);
    let io_baseline = IoBaselineSettings {
        buffer_limit_mb: settings.io_baseline.buffer_limit_mb.clamp(64, 32768),
        game_mode_buffer_mb: settings.io_baseline.game_mode_buffer_mb.clamp(16, 4096),
        game_mode: settings.io_baseline.game_mode,
        max_parallel_hdd: settings.io_baseline.max_parallel_hdd.clamp(1, 16),
        game_mode_max_parallel: settings.io_baseline.game_mode_max_parallel.clamp(1, 4),
        disk_type_overrides: settings.io_baseline.disk_type_overrides,
        hdd_buffer_enabled: settings.io_baseline.hdd_buffer_enabled,
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
        },
        download: DownloadDefaultsSettings {
            default_download_dir,
            default_max_retries: settings.download.default_max_retries.clamp(0, 20),
            default_checksum: settings.download.default_checksum,
            default_user_agent,
        },
        bt,
        logging,
        aria2_rpc: settings.aria2_rpc.clone(),
        cdn_acceleration: settings.cdn_acceleration.clone(),
        github_mirror,
        global_speed_limit_bps: settings.global_speed_limit_bps,
        speed_limit_schedule: settings.speed_limit_schedule.clone(),
        notifications: settings.notifications.clone(),
        io_baseline,
        autostart: settings.autostart,
        setup_completed: settings.setup_completed,
        last_setup_step: settings.last_setup_step.map(|s| s.clamp(0, 9)),
        download_limits: settings.download_limits.clone(),
        max_in_memory_downloads: clamp_max_in_memory(settings.max_in_memory_downloads),
    })
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

fn normalize_github_mirror_settings(settings: GitHubMirrorSettings) -> GitHubMirrorSettings {
    let mirrors: Vec<MirrorEntry> = settings
        .mirrors
        .into_iter()
        .map(|mut entry| {
            entry.url = entry.url.trim().to_string();
            entry
        })
        .filter(|entry| {
            if entry.url.is_empty() {
                return false;
            }
            // Validate URL format and ensure it starts with http:// or https://
            if !entry.url.starts_with("http://") && !entry.url.starts_with("https://") {
                return false;
            }
            Url::parse(&entry.url).is_ok()
        })
        .enumerate()
        .map(|(index, mut entry)| {
            entry.order = index as u32;
            entry
        })
        .collect();

    GitHubMirrorSettings {
        enabled: settings.enabled,
        mirrors,
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

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content)
        && (value.get("appearance").is_some()
            || value.get("proxy").is_some()
            || value.get("scheduler").is_some()
            || value.get("download").is_some()
            || value.get("bt").is_some()
            || value.get("logging").is_some()
            || value.get("cdnAcceleration").is_some()
            || value.get("notifications").is_some()
            || value.get("ioBaseline").is_some())
    {
        let parsed = serde_json::from_value::<AppSettings>(value)?;
        return normalize_settings(parsed);
    }

    let legacy_proxy = serde_json::from_str::<ProxySettings>(&content)?;
    normalize_settings(AppSettings {
        appearance: Default::default(),
        proxy: legacy_proxy,
        scheduler: SchedulerSettings::default(),
        download: DownloadDefaultsSettings::default(),
        bt: BtSettings::default(),
        logging: LogSettings::default(),
        aria2_rpc: Aria2RpcSettings::default(),
        cdn_acceleration: CdnAccelerationSettings::default(),
        github_mirror: GitHubMirrorSettings::default(),
        global_speed_limit_bps: 0,
        speed_limit_schedule: Vec::new(),
        notifications: NotificationSettings::default(),
        io_baseline: IoBaselineSettings::default(),
        autostart: false,
        setup_completed: false,
        last_setup_step: None,
        download_limits: None,
        max_in_memory_downloads: 200,
    })
}

pub async fn persist_settings(settings_path: &Path, settings: &AppSettings) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let temp_path = settings_path.with_extension("json.tmp");
    tokio::fs::write(&temp_path, serde_json::to_vec_pretty(settings)?).await?;
    tokio::fs::rename(&temp_path, settings_path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BtPortRange;
    use std::io::Write;
    use tempfile::tempdir;

    // -----------------------------------------------------------------------
    // normalize_proxy_settings
    // -----------------------------------------------------------------------
    #[test]
    fn test_normalize_proxy_disabled_clears_url() {
        let input = ProxySettings {
            mode: ProxyMode::Disabled,
            manual_url: "http://should-be-cleared".into(),
        };
        let result = normalize_proxy_settings(input).unwrap();
        assert_eq!(result.mode, ProxyMode::Disabled);
        assert!(result.manual_url.is_empty());
    }

    #[test]
    fn test_normalize_proxy_system_clears_url() {
        let input = ProxySettings {
            mode: ProxyMode::System,
            manual_url: "http://should-be-cleared".into(),
        };
        let result = normalize_proxy_settings(input).unwrap();
        assert_eq!(result.mode, ProxyMode::System);
        assert!(result.manual_url.is_empty());
    }

    #[test]
    fn test_normalize_proxy_manual_valid_url_ok() {
        let input = ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: "http://proxy:8080".into(),
        };
        let result = normalize_proxy_settings(input).unwrap();
        assert_eq!(result.mode, ProxyMode::Manual);
        assert_eq!(result.manual_url, "http://proxy:8080");
    }

    #[test]
    fn test_normalize_proxy_manual_empty_url_err() {
        let input = ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: String::new(),
        };
        let err = normalize_proxy_settings(input).unwrap_err();
        assert!(matches!(err, DownloadError::InvalidProxy(_)));
    }

    #[test]
    fn test_normalize_proxy_manual_invalid_url_err() {
        let input = ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: "not-a-url".into(),
        };
        let err = normalize_proxy_settings(input).unwrap_err();
        assert!(matches!(err, DownloadError::InvalidProxy(_)));
    }

    #[test]
    fn test_normalize_proxy_manual_url_trimmed() {
        let input = ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: "  http://proxy:8080  ".into(),
        };
        let result = normalize_proxy_settings(input).unwrap();
        assert_eq!(result.manual_url, "http://proxy:8080");
    }

    // -----------------------------------------------------------------------
    // normalize_min_threads
    // -----------------------------------------------------------------------
    #[test]
    fn test_normalize_min_threads_zero_with_max_eight() {
        assert_eq!(normalize_min_threads(0, 8), 4);
    }

    #[test]
    fn test_normalize_min_threads_zero_with_max_one() {
        assert_eq!(normalize_min_threads(0, 1), 1);
    }

    #[test]
    fn test_normalize_min_threads_within_range() {
        assert_eq!(normalize_min_threads(3, 8), 3);
    }

    #[test]
    fn test_normalize_min_threads_clamped_to_max() {
        assert_eq!(normalize_min_threads(10, 8), 8);
    }

    #[test]
    fn test_normalize_min_threads_zero_with_max_two() {
        assert_eq!(normalize_min_threads(0, 2), 1);
    }

    // -----------------------------------------------------------------------
    // normalize_logging_settings
    // -----------------------------------------------------------------------
    #[test]
    fn test_normalize_logging_retention_count_clamped() {
        let input = LogSettings {
            retention_count: Some(500),
            ..LogSettings::default()
        };
        let result = normalize_logging_settings(input);
        assert_eq!(result.retention_count, Some(500));

        let input = LogSettings {
            retention_count: Some(2000),
            ..LogSettings::default()
        };
        let result = normalize_logging_settings(input);
        assert_eq!(result.retention_count, Some(1000));

        let input = LogSettings {
            retention_count: Some(0),
            ..LogSettings::default()
        };
        let result = normalize_logging_settings(input);
        assert_eq!(result.retention_count, Some(0));
    }

    #[test]
    fn test_normalize_logging_retention_days_clamped() {
        let input = LogSettings {
            retention_days: Some(365),
            ..LogSettings::default()
        };
        let result = normalize_logging_settings(input);
        assert_eq!(result.retention_days, Some(365));

        let input = LogSettings {
            retention_days: Some(5000),
            ..LogSettings::default()
        };
        let result = normalize_logging_settings(input);
        assert_eq!(result.retention_days, Some(3650));

        let input = LogSettings {
            retention_days: Some(0),
            ..LogSettings::default()
        };
        let result = normalize_logging_settings(input);
        assert_eq!(result.retention_days, Some(0));
    }

    #[test]
    fn test_normalize_logging_file_path_trimmed() {
        let input = LogSettings {
            file_path: "  /var/log/app/  ".into(),
            ..LogSettings::default()
        };
        let result = normalize_logging_settings(input);
        assert_eq!(result.file_path, "/var/log/app/");
    }

    #[test]
    fn test_normalize_logging_none_preserved() {
        let input = LogSettings {
            retention_count: None,
            retention_days: None,
            ..LogSettings::default()
        };
        let result = normalize_logging_settings(input);
        assert!(result.retention_count.is_none());
        assert!(result.retention_days.is_none());
    }

    // -----------------------------------------------------------------------
    // normalize_download_dir
    // -----------------------------------------------------------------------
    #[test]
    fn test_normalize_download_dir_empty() {
        assert_eq!(normalize_download_dir(""), "");
    }

    #[test]
    fn test_normalize_download_dir_whitespace_only() {
        assert_eq!(normalize_download_dir("   \t  "), "");
    }

    #[test]
    fn test_normalize_download_dir_relative_path() {
        assert_eq!(normalize_download_dir("downloads"), "");
    }

    #[cfg(unix)]
    #[test]
    fn test_normalize_download_dir_absolute_unix() {
        assert_eq!(
            normalize_download_dir("/home/user/downloads"),
            "/home/user/downloads"
        );
    }

    #[cfg(windows)]
    #[test]
    fn test_normalize_download_dir_absolute_unix_is_not_absolute_on_windows() {
        assert_eq!(normalize_download_dir("/home/user/downloads"), "",);
    }

    #[cfg(windows)]
    #[test]
    fn test_normalize_download_dir_absolute_windows() {
        assert_eq!(
            normalize_download_dir(r"C:\Users\test\downloads"),
            r"C:\Users\test\downloads"
        );
    }

    #[cfg(windows)]
    #[test]
    fn test_normalize_download_dir_absolute_windows_forward_slashes() {
        // On Windows, `C:/Users/test/downloads` is also absolute
        assert_eq!(
            normalize_download_dir("C:/Users/test/downloads"),
            "C:/Users/test/downloads"
        );
    }

    // -----------------------------------------------------------------------
    // normalize_tracker_list
    // -----------------------------------------------------------------------
    #[test]
    fn test_normalize_tracker_list_dedup_sorted() {
        let input = "udp://tracker.opentrackr.org:1337\nhttp://tracker.example.com\nudp://tracker.opentrackr.org:1337";
        let result = normalize_tracker_list(input).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "http://tracker.example.com/");
        assert_eq!(lines[1], "udp://tracker.opentrackr.org:1337");
    }

    #[test]
    fn test_normalize_tracker_list_empty() {
        let result = normalize_tracker_list("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_normalize_tracker_list_invalid_scheme() {
        let err = normalize_tracker_list("ftp://tracker.example.com").unwrap_err();
        assert!(matches!(err, DownloadError::InvalidResponse(_)));
        assert!(err.to_string().contains("unsupported tracker scheme"));
    }

    #[test]
    fn test_normalize_tracker_list_empty_lines_filtered() {
        let input = "udp://tracker.opentrackr.org:1337\n\n\nhttp://example.com\n";
        let result = normalize_tracker_list(input).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_normalize_tracker_list_lines_trimmed() {
        let input = "  udp://tracker.opentrackr.org:1337  \n  http://example.com  ";
        let result = normalize_tracker_list(input).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_normalize_tracker_list_unsupported_scheme_err() {
        let err = normalize_tracker_list("ws://tracker.example.com").unwrap_err();
        assert!(matches!(err, DownloadError::InvalidResponse(_)));
    }

    // -----------------------------------------------------------------------
    // normalize_tracker_list_url
    // -----------------------------------------------------------------------
    #[test]
    fn test_normalize_tracker_list_url_empty_returns_default() {
        let result = normalize_tracker_list_url("").unwrap();
        assert_eq!(result, default_tracker_list_url());
    }

    #[test]
    fn test_normalize_tracker_list_url_valid_http() {
        let result = normalize_tracker_list_url("http://example.com/list.txt").unwrap();
        assert_eq!(result, "http://example.com/list.txt");
    }

    #[test]
    fn test_normalize_tracker_list_url_valid_https() {
        let result = normalize_tracker_list_url("https://trackers.example.com/best.txt").unwrap();
        assert_eq!(result, "https://trackers.example.com/best.txt");
    }

    #[test]
    fn test_normalize_tracker_list_url_invalid_scheme() {
        let err = normalize_tracker_list_url("ftp://example.com/list.txt").unwrap_err();
        assert!(matches!(err, DownloadError::InvalidResponse(_)));
    }

    #[test]
    fn test_normalize_tracker_list_url_invalid_url() {
        let err = normalize_tracker_list_url("not a url").unwrap_err();
        assert!(matches!(err, DownloadError::InvalidResponse(_)));
    }

    // -----------------------------------------------------------------------
    // normalize_github_mirror_settings
    // -----------------------------------------------------------------------
    #[test]
    fn test_normalize_github_mirror_empty() {
        let input = GitHubMirrorSettings::default();
        let result = normalize_github_mirror_settings(input);
        assert!(result.mirrors.is_empty());
    }

    #[test]
    fn test_normalize_github_mirror_valid_urls_filtered_and_ordered() {
        let input = GitHubMirrorSettings {
            enabled: true,
            mirrors: vec![
                MirrorEntry {
                    url: "https://mirror2.example.com".into(),
                    enabled: true,
                    order: 99,
                },
                MirrorEntry {
                    url: "https://mirror1.example.com".into(),
                    enabled: true,
                    order: 42,
                },
            ],
        };
        let result = normalize_github_mirror_settings(input);
        assert_eq!(result.mirrors.len(), 2);
        // order should be reassigned: 0, 1 based on iteration order
        assert_eq!(result.mirrors[0].order, 0);
        assert_eq!(result.mirrors[1].order, 1);
        assert_eq!(result.mirrors[0].url, "https://mirror2.example.com");
        assert_eq!(result.mirrors[1].url, "https://mirror1.example.com");
    }

    #[test]
    fn test_normalize_github_mirror_invalid_urls_filtered() {
        let input = GitHubMirrorSettings {
            enabled: true,
            mirrors: vec![
                MirrorEntry {
                    url: "ftp://mirror.example.com".into(),
                    enabled: true,
                    order: 0,
                },
                MirrorEntry {
                    url: "not-a-url".into(),
                    enabled: true,
                    order: 1,
                },
                MirrorEntry {
                    url: "https://valid.example.com".into(),
                    enabled: true,
                    order: 2,
                },
            ],
        };
        let result = normalize_github_mirror_settings(input);
        assert_eq!(result.mirrors.len(), 1);
        assert_eq!(result.mirrors[0].url, "https://valid.example.com");
    }

    #[test]
    fn test_normalize_github_mirror_empty_url_filtered() {
        let input = GitHubMirrorSettings {
            enabled: true,
            mirrors: vec![
                MirrorEntry {
                    url: "".into(),
                    enabled: true,
                    order: 0,
                },
                MirrorEntry {
                    url: "  ".into(),
                    enabled: true,
                    order: 1,
                },
                MirrorEntry {
                    url: "https://real.example.com".into(),
                    enabled: true,
                    order: 2,
                },
            ],
        };
        let result = normalize_github_mirror_settings(input);
        assert_eq!(result.mirrors.len(), 1);
        assert_eq!(result.mirrors[0].url, "https://real.example.com");
    }

    // -----------------------------------------------------------------------
    // resolve_user_agent
    // -----------------------------------------------------------------------
    #[test]
    fn test_resolve_user_agent_custom() {
        let result = resolve_user_agent(Some("MyAgent/1.0"), "Default/1.0").unwrap();
        assert_eq!(result, "MyAgent/1.0");
    }

    #[test]
    fn test_resolve_user_agent_falls_back_to_default() {
        let result = resolve_user_agent(None, "Default/1.0").unwrap();
        assert_eq!(result, "Default/1.0");
    }

    #[test]
    fn test_resolve_user_agent_empty_string_falls_back() {
        let result = resolve_user_agent(Some(""), "Default/1.0").unwrap();
        assert_eq!(result, "Default/1.0");
    }

    #[test]
    fn test_resolve_user_agent_whitespace_falls_back() {
        let result = resolve_user_agent(Some("   "), "Default/1.0").unwrap();
        assert_eq!(result, "Default/1.0");
    }

    // -----------------------------------------------------------------------
    // normalize_bt_settings
    // -----------------------------------------------------------------------
    #[test]
    fn test_normalize_bt_empty_tracker_list_ok() {
        let input = BtSettings {
            tracker_list: String::new(),
            ..BtSettings::default()
        };
        let result = normalize_bt_settings(input).unwrap();
        assert!(result.tracker_list.is_empty());
    }

    #[test]
    fn test_normalize_bt_upload_limit_clamped() {
        const MAX_UPLOAD_LIMIT_BYTES: u64 = 10 * 1024 * 1024 * 1024 * 1024; // 10 TiB
        let input = BtSettings {
            upload_limit_bytes: MAX_UPLOAD_LIMIT_BYTES + 1,
            ..BtSettings::default()
        };
        let result = normalize_bt_settings(input).unwrap();
        assert_eq!(result.upload_limit_bytes, MAX_UPLOAD_LIMIT_BYTES);

        let input = BtSettings {
            upload_limit_bytes: u64::MAX,
            ..BtSettings::default()
        };
        let result = normalize_bt_settings(input).unwrap();
        assert_eq!(result.upload_limit_bytes, MAX_UPLOAD_LIMIT_BYTES);
    }

    #[test]
    fn test_normalize_bt_upload_ratio_clamped() {
        let input = BtSettings {
            upload_ratio_limit: 200.0,
            ..BtSettings::default()
        };
        let result = normalize_bt_settings(input).unwrap();
        assert!((result.upload_ratio_limit - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_bt_upload_ratio_nan_inf() {
        let input = BtSettings {
            upload_ratio_limit: f64::NAN,
            ..BtSettings::default()
        };
        let result = normalize_bt_settings(input).unwrap();
        assert!((result.upload_ratio_limit - 0.0).abs() < f64::EPSILON);

        let input = BtSettings {
            upload_ratio_limit: f64::INFINITY,
            ..BtSettings::default()
        };
        let result = normalize_bt_settings(input).unwrap();
        assert!((result.upload_ratio_limit - 0.0).abs() < f64::EPSILON);

        let input = BtSettings {
            upload_ratio_limit: f64::NEG_INFINITY,
            ..BtSettings::default()
        };
        let result = normalize_bt_settings(input).unwrap();
        assert!((result.upload_ratio_limit - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_bt_listen_port_range_start_gt_end_err() {
        let input = BtSettings {
            listen_port_range: Some(BtPortRange {
                start: 7000,
                end: 6000,
            }),
            ..BtSettings::default()
        };
        let err = normalize_bt_settings(input).unwrap_err();
        assert!(err.to_string().contains("listen_port_range"));
    }

    #[test]
    fn test_normalize_bt_listen_port_range_below_1025_err() {
        let input = BtSettings {
            listen_port_range: Some(BtPortRange {
                start: 1024,
                end: 2048,
            }),
            ..BtSettings::default()
        };
        let err = normalize_bt_settings(input).unwrap_err();
        assert!(err.to_string().contains("listen_port_range"));
    }

    #[test]
    fn test_normalize_bt_listen_port_below_1024_filtered() {
        let input = BtSettings {
            listen_port: Some(1023),
            ..BtSettings::default()
        };
        let result = normalize_bt_settings(input).unwrap();
        assert!(result.listen_port.is_none());
    }

    #[test]
    fn test_normalize_bt_listen_port_none_preserved() {
        let input = BtSettings {
            listen_port: None,
            ..BtSettings::default()
        };
        let result = normalize_bt_settings(input).unwrap();
        assert!(result.listen_port.is_none());
    }

    #[test]
    fn test_normalize_bt_valid_port_range_and_settings_ok() {
        let input = BtSettings {
            listen_port_range: Some(BtPortRange {
                start: 6881,
                end: 6889,
            }),
            listen_port: Some(6881),
            ..BtSettings::default()
        };
        let result = normalize_bt_settings(input).unwrap();
        assert_eq!(
            result.listen_port_range,
            Some(BtPortRange {
                start: 6881,
                end: 6889
            })
        );
        assert_eq!(result.listen_port, Some(6881));
    }

    // -----------------------------------------------------------------------
    // load_settings
    // -----------------------------------------------------------------------
    #[test]
    fn test_load_settings_file_not_found_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let result = load_settings(&path).unwrap();
        // Compare with default
        let default = AppSettings::default();
        assert_eq!(result.proxy.mode, default.proxy.mode);
        assert_eq!(result.proxy.manual_url, default.proxy.manual_url);
    }

    #[test]
    fn test_load_settings_valid_json_full_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let json = r#"{
            "appearance": {"themeColor": "lime", "backgroundOpacity": "default", "colorMode": "system", "showDetailInfo": true, "showHeatmap": true, "sortKey": "added_at", "sortDirection": "desc", "compactView": false, "visibleColumns": ["file", "size", "downloaded", "status", "progress", "speed", "eta"]},
            "proxy": {"mode": "manual", "manualUrl": "http://proxy:8080"},
            "scheduler": {"mode": "automatic", "traditional": {"maxParallelTasks": 3}, "automatic": {"maxParallelThreads": 8, "maxThreadsPerTask": 4, "minThreadsPerTask": 2, "adaptiveProfile": "balanced"}},
            "download": {"defaultDownloadDir": "", "defaultMaxRetries": 5, "defaultChecksum": "blake3", "defaultUserAgent": "TestAgent/1.0"},
            "bt": {"dhtEnabled": true, "trackerList": "", "trackerListUrl": "", "pauseUploadWhenLimitReached": false, "uploadLimitBytes": 0, "uploadRatioLimit": 0.0},
            "logging": {"enabled": true, "level": "info", "filePath": ""},
            "aria2Rpc": {"enabled": false, "port": 6800, "secret": null},
            "cdnAcceleration": {"enabled": false, "activeIp": null, "activeSpeedMbps": null, "lastTestAtMs": null, "lastError": null},
            "notifications": {"enabled": true},
            "ioBaseline": {"bufferLimitMb": 1024, "gameModeBufferMb": 128, "gameMode": false, "maxParallelHdd": 4, "gameModeMaxParallel": 1}
        }"#;
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(json.as_bytes()).unwrap();
        drop(file);

        let result = load_settings(&path).unwrap();
        assert_eq!(result.proxy.mode, ProxyMode::Manual);
        assert_eq!(result.proxy.manual_url, "http://proxy:8080");
    }

    #[test]
    fn test_load_settings_legacy_proxy_only() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("legacy.json");
        let json = r#"{"mode": "manual", "manualUrl": "http://legacy-proxy:3128"}"#;
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(json.as_bytes()).unwrap();
        drop(file);

        let result = load_settings(&path).unwrap();
        assert_eq!(result.proxy.mode, ProxyMode::Manual);
        assert_eq!(result.proxy.manual_url, "http://legacy-proxy:3128");
    }

    // -----------------------------------------------------------------------
    // persist_settings roundtrip
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_persist_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("roundtrip.json");

        let original = AppSettings {
            proxy: ProxySettings {
                mode: ProxyMode::Manual,
                manual_url: "http://test-proxy:9090".into(),
            },
            ..AppSettings::default()
        };

        // Persist
        persist_settings(&path, &original).await.unwrap();

        // Load back
        let loaded = load_settings(&path).unwrap();

        // Compare fields
        assert_eq!(loaded.proxy.mode, original.proxy.mode);
        assert_eq!(loaded.proxy.manual_url, original.proxy.manual_url);
        assert!(path.exists());
    }

    // -----------------------------------------------------------------------
    // normalize_settings full-field clamping
    // -----------------------------------------------------------------------
    #[test]
    fn test_normalize_settings_clamps_all_fields() {
        let settings = AppSettings {
            scheduler: SchedulerSettings {
                traditional: TraditionalSchedulerSettings {
                    max_parallel_tasks: 100,
                },
                automatic: AutomaticSchedulerSettings {
                    max_parallel_threads: 128,
                    max_threads_per_task: 200, // > 32 AND > max_parallel_threads (128)
                    ..AutomaticSchedulerSettings::default()
                },
                ..SchedulerSettings::default()
            },
            io_baseline: IoBaselineSettings {
                buffer_limit_mb: 16,        // below 64
                game_mode_buffer_mb: 8192,  // above 4096
                max_parallel_hdd: 32,       // above 16
                game_mode_max_parallel: 10, // above 4
                ..IoBaselineSettings::default()
            },
            download: DownloadDefaultsSettings {
                default_max_retries: 100, // above 20
                ..DownloadDefaultsSettings::default()
            },
            last_setup_step: Some(99), // above 9
            max_in_memory_downloads: 5, // in the invalid 1–9 range
            ..AppSettings::default()
        };

        let result = normalize_settings(settings).unwrap();

        // Scheduler clamps
        assert_eq!(result.scheduler.traditional.max_parallel_tasks, 32);
        assert_eq!(result.scheduler.automatic.max_parallel_threads, 64);
        // 200.clamp(1, 32) = 32, .min(64) = 32
        assert_eq!(result.scheduler.automatic.max_threads_per_task, 32);

        // IO baseline clamps
        assert_eq!(result.io_baseline.buffer_limit_mb, 64);
        assert_eq!(result.io_baseline.game_mode_buffer_mb, 4096);
        assert_eq!(result.io_baseline.max_parallel_hdd, 16);
        assert_eq!(result.io_baseline.game_mode_max_parallel, 4);

        // Download defaults clamp
        assert_eq!(result.download.default_max_retries, 20);

        // last_setup_step clamp
        assert_eq!(result.last_setup_step, Some(9));

        // max_in_memory_downloads: 5 in 1–9 → clamped to 10
        assert_eq!(result.max_in_memory_downloads, 10);
    }

    #[test]
    fn test_normalize_settings_clamps_io_opposite_ends() {
        // Test buffer_limit_mb above 32768 and game_mode_buffer_mb below 16
        let settings = AppSettings {
            io_baseline: IoBaselineSettings {
                buffer_limit_mb: 65536,    // above 32768
                game_mode_buffer_mb: 1,    // below 16
                ..IoBaselineSettings::default()
            },
            ..AppSettings::default()
        };
        let result = normalize_settings(settings).unwrap();
        assert_eq!(result.io_baseline.buffer_limit_mb, 32768);
        assert_eq!(result.io_baseline.game_mode_buffer_mb, 16);
    }

    #[test]
    fn test_normalize_settings_max_in_memory_edge_cases() {
        // 0 → stays 0 (unlimited / no eviction)
        let settings = AppSettings {
            max_in_memory_downloads: 0,
            ..AppSettings::default()
        };
        let result = normalize_settings(settings).unwrap();
        assert_eq!(result.max_in_memory_downloads, 0);

        // very high → clamped to 10000
        let settings = AppSettings {
            max_in_memory_downloads: 50000,
            ..AppSettings::default()
        };
        let result = normalize_settings(settings).unwrap();
        assert_eq!(result.max_in_memory_downloads, 10000);
    }

    #[test]
    fn test_normalize_settings_last_setup_step_none_preserved() {
        let settings = AppSettings {
            last_setup_step: None,
            ..AppSettings::default()
        };
        let result = normalize_settings(settings).unwrap();
        assert_eq!(result.last_setup_step, None);
    }
}
