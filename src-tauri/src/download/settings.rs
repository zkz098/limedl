use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use reqwest::{Client, Proxy, Url, header, redirect::Policy};

use super::{
    error::{DownloadError, Result},
    types::{
        AppSettings, Aria2RpcSettings, AutomaticSchedulerSettings, BtSettings,
        CdnAccelerationSettings, DownloadDefaultsSettings, LogSettings, NetworkLearningMetrics,
        NetworkLearningSettings, NetworkSceneProfile, NotificationSettings, ProxyMode, ProxySettings, SchedulerSettings,
        TraditionalSchedulerSettings, default_http_user_agent, default_tracker_list_url,
    },
};

fn default_network_scene() -> NetworkSceneProfile {
    NetworkSceneProfile {
        id: String::from("default"),
        name: String::from("默认场景"),
        learning_enabled: true,
        learned_metrics: None,
        updated_at_ms: 0,
    }
}

fn normalize_network_learning_settings(
    settings: NetworkLearningSettings,
    scheduler_cap: usize,
) -> NetworkLearningSettings {
    let mut scenes = settings.scenes;
    let selected_scene = scenes
        .iter()
        .position(|scene| scene.id == settings.current_scene_id)
        .map(|index| scenes.remove(index))
        .or_else(|| scenes.into_iter().next());

    let mut scene = selected_scene.unwrap_or_else(default_network_scene);
    scene.id = String::from("default");
    scene.name = String::from("默认场景");
    scene.learned_metrics = scene
        .learned_metrics
        .map(|metrics| normalize_learning_metrics(metrics, scheduler_cap));

    NetworkLearningSettings {
        device_mode: settings.device_mode,
        current_scene_id: String::from("default"),
        scenes: vec![scene],
    }
}

pub(super) fn normalize_learning_metrics(
    metrics: NetworkLearningMetrics,
    scheduler_cap: usize,
) -> NetworkLearningMetrics {
    NetworkLearningMetrics {
        estimated_bandwidth_bps: metrics.estimated_bandwidth_bps.max(0.0),
        stability_score: metrics.stability_score.clamp(0.0, 1.0),
        penalty_rate: metrics.penalty_rate.clamp(0.0, 1.0),
        recommended_initial_threads: metrics.recommended_initial_threads.clamp(1, scheduler_cap),
        recommended_max_threads_per_task_cap: metrics
            .recommended_max_threads_per_task_cap
            .clamp(1, scheduler_cap)
            .max(metrics.recommended_initial_threads.clamp(1, scheduler_cap)),
        sample_count: metrics.sample_count,
        last_observed_at_ms: metrics.last_observed_at_ms,
    }
}

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

pub(crate) fn normalize_settings(settings: AppSettings) -> Result<AppSettings> {
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
    let network_learning =
        normalize_network_learning_settings(settings.network_learning, max_threads_per_task.max(1));
    let bt = normalize_bt_settings(settings.bt)?;
    let logging = normalize_logging_settings(settings.logging);
    let default_user_agent = normalize_user_agent(&settings.download.default_user_agent)?;
    let default_download_dir = normalize_download_dir(&settings.download.default_download_dir);

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
            enable_metalink: settings.download.enable_metalink,
            enable_sftp: settings.download.enable_sftp,
        },
        bt,
        network_learning,
        logging,
        aria2_rpc: settings.aria2_rpc.clone(),
        cdn_acceleration: settings.cdn_acceleration.clone(),
        global_speed_limit_bps: settings.global_speed_limit_bps,
        notifications: settings.notifications.clone(),
    })
}

fn normalize_min_threads(raw: usize, max_per_task: usize) -> usize {
    if raw == 0 {
        (max_per_task / 2).max(1)
    } else {
        raw.clamp(1, max_per_task)
    }
}

fn normalize_logging_settings(settings: LogSettings) -> LogSettings {
    LogSettings {
        enabled: settings.enabled,
        level: settings.level,
        file_path: settings.file_path.trim().to_string(),
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
    })
}

pub(super) fn normalize_user_agent(user_agent: &str) -> Result<String> {
    let normalized = user_agent.trim();
    if normalized.is_empty() {
        return Ok(default_http_user_agent());
    }
    if normalized.len() > 512 || header::HeaderValue::from_str(normalized).is_err() {
        return Err(DownloadError::InvalidResponse(String::from(
            "invalid user-agent value",
        )));
    }

    Ok(normalized.to_string())
}

pub(crate) fn resolve_user_agent(
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

pub(super) fn normalize_tracker_list_lossy(tracker_list: &str) -> String {
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

pub(super) fn normalize_tracker_list_url(tracker_list_url: &str) -> Result<String> {
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

pub(crate) fn build_http_client(settings: &AppSettings) -> Result<Client> {
    let default_user_agent = normalize_user_agent(&settings.download.default_user_agent)?;
    let mut builder = Client::builder()
        .redirect(Policy::limited(10))
        .tcp_nodelay(true)
        .read_timeout(Duration::from_secs(15))
        .user_agent(default_user_agent);

    match settings.proxy.mode {
        ProxyMode::Disabled => {
            builder = builder.no_proxy();
        }
        ProxyMode::System => {}
        ProxyMode::Manual => {
            let proxy = Proxy::all(&settings.proxy.manual_url)
                .map_err(|error| DownloadError::InvalidProxy(error.to_string()))?;
            builder = builder.proxy(proxy);
        }
    }

    builder.build().map_err(DownloadError::from)
}

pub(crate) fn load_settings(settings_path: &Path) -> Result<AppSettings> {
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
            || value.get("networkLearning").is_some()
            || value.get("logging").is_some()
            || value.get("cdnAcceleration").is_some()
            || value.get("notifications").is_some())
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
        network_learning: NetworkLearningSettings::default(),
        logging: LogSettings::default(),
        aria2_rpc: Aria2RpcSettings::default(),
        cdn_acceleration: CdnAccelerationSettings::default(),
        global_speed_limit_bps: 0,
        notifications: NotificationSettings::default(),
    })
}

pub(crate) async fn persist_settings(settings_path: &Path, settings: &AppSettings) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let temp_path = settings_path.with_extension("json.tmp");
    tokio::fs::write(&temp_path, serde_json::to_vec_pretty(settings)?).await?;
    tokio::fs::rename(&temp_path, settings_path).await?;
    Ok(())
}
