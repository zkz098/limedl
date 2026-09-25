use limedl_core::types::{
    AdaptiveProfile, AppSettings, BackgroundOpacityPreset,
    BtAntiLeechAction, BtChokingAlgorithm, BtEncryptionMode, BtPreallocateMode,
    BtSeedChokingAlgorithm, ChecksumMode, ChunkSizeStrategy, CloseBehavior, ColorMode,
    DoubleClickOnCompleted, DoubleClickOnUncompleted, LogLevel, ProxyMode, SchedulerMode,
    ThemeColor,
};
use slint::SharedString;

use crate::i18n::{self, Language};
use crate::{SettingsFormData, SpeedLimitSlotItem};
use super::models::{COLUMN_KEYS, column_is_visible};

/// Canonical option lists for the settings dialog ComboBoxes (persisted values).
/// The display labels live in `ui/components/settings_dialog.slint` via `@tr` and
/// MUST keep the same order as these arrays. Form structs carry the selected
/// index (see `SettingsFormData` in `ui/types.slint`), never the raw string.
pub mod combo {
    pub const COLOR_MODES: [&str; 3] = ["system", "light", "dark"];
    pub const THEME_COLORS: [&str; 3] = ["amber", "sky", "lime"];
    pub const OPACITY_PRESETS: [&str; 3] = ["default", "acrylic", "frosted"];
    pub const LANGUAGES: [&str; 3] = ["zh-CN", "zh-TW", "en-US"];
    pub const CLOSE_BEHAVIORS: [&str; 2] = ["minimizeToTray", "exit"];
    pub const DOUBLE_CLICK_COMPLETED: [&str; 4] =
        ["none", "open_file", "open_in_explorer", "open_download_dir"];
    pub const DOUBLE_CLICK_UNCOMPLETED: [&str; 2] = ["none", "toggle_pause_resume"];
    pub const CHECKSUMS: [&str; 5] = ["blake3", "sha256", "xxh3_128", "none", "sha1"];
    pub const PROXY_MODES: [&str; 3] = ["disabled", "system", "manual"];
    pub const SCHEDULER_MODES: [&str; 2] = ["automatic", "traditional"];
    pub const ADAPTIVE_PROFILES: [&str; 3] = ["conservative", "balanced", "aggressive"];
    pub const CHUNK_STRATEGIES: [&str; 2] = ["adaptive", "fixed"];
    pub const ENCRYPTION_MODES: [&str; 3] = ["enabled", "disabled", "forced"];
    pub const PREALLOC_MODES: [&str; 2] = ["none", "full"];
    pub const ANTI_LEECH_ACTIONS: [&str; 2] = ["ban", "limit_slots"];
    pub const SEED_CHOKING: [&str; 3] = ["fastest_upload", "round_robin", "anti_leech"];
    pub const CHOKING_ALGOS: [&str; 2] = ["fixed_slots", "rate_based"];
    pub const LOG_LEVELS: [&str; 5] = ["trace", "debug", "info", "warn", "error"];

    /// Index of `value` in `list`; `0` when missing (Slint ComboBox default).
    pub fn idx_of(list: impl AsRef<[&'static str]>, value: &str) -> i32 {
        list.as_ref()
            .iter()
            .position(|v| *v == value)
            .map_or(0, |i| i as i32)
    }

    /// Canonical value at `index`; `list[0]` when out of range.
    pub fn value_at(list: impl AsRef<[&'static str]>, index: i32) -> &'static str {
        let list = list.as_ref();
        list.get(index.max(0) as usize).copied().unwrap_or(list[0])
    }
}

pub(crate) fn proxy_mode_to_str(m: ProxyMode) -> SharedString {
    SharedString::from(match m {
        ProxyMode::Disabled => "disabled",
        ProxyMode::System => "system",
        ProxyMode::Manual => "manual",
    })
}
pub(crate) fn str_to_proxy_mode(s: &str) -> Option<ProxyMode> {
    match s.trim() {
        "disabled" => Some(ProxyMode::Disabled),
        "system" => Some(ProxyMode::System),
        "manual" => Some(ProxyMode::Manual),
        _ => None,
    }
}
fn scheduler_mode_to_str(m: SchedulerMode) -> SharedString {
    SharedString::from(match m {
        SchedulerMode::Traditional => "traditional",
        SchedulerMode::Automatic => "automatic",
    })
}
fn adaptive_profile_to_str(p: AdaptiveProfile) -> SharedString {
    SharedString::from(match p {
        AdaptiveProfile::Conservative => "conservative",
        AdaptiveProfile::Balanced => "balanced",
        AdaptiveProfile::Aggressive => "aggressive",
    })
}
pub(crate) fn chunk_strategy_to_str(s: ChunkSizeStrategy) -> SharedString {
    SharedString::from(match s {
        ChunkSizeStrategy::Adaptive => "adaptive",
        ChunkSizeStrategy::Fixed => "fixed",
    })
}
fn checksum_to_str(c: ChecksumMode) -> SharedString {
    SharedString::from(match c {
        ChecksumMode::Blake3 => "blake3",
        ChecksumMode::Sha256 => "sha256",
        ChecksumMode::Xxh3128 => "xxh3_128",
        ChecksumMode::None => "none",
        ChecksumMode::Sha1 => "sha1",
    })
}
fn str_to_checksum(s: &str) -> Option<ChecksumMode> {
    match s.trim() {
        "blake3" => Some(ChecksumMode::Blake3),
        "sha256" => Some(ChecksumMode::Sha256),
        "xxh3_128" => Some(ChecksumMode::Xxh3128),
        "none" => Some(ChecksumMode::None),
        "sha1" => Some(ChecksumMode::Sha1),
        _ => None,
    }
}
pub(crate) fn color_mode_to_str(c: &ColorMode) -> SharedString {
    SharedString::from(match c {
        ColorMode::System => "system",
        ColorMode::Light => "light",
        ColorMode::Dark => "dark",
    })
}
pub(crate) fn theme_color_to_str(c: &ThemeColor) -> SharedString {
    SharedString::from(match c {
        ThemeColor::Amber => "amber",
        ThemeColor::Sky => "sky",
        ThemeColor::Lime => "lime",
    })
}
fn close_behavior_to_str(c: &CloseBehavior) -> SharedString {
    SharedString::from(match c {
        CloseBehavior::MinimizeToTray => "minimizeToTray",
        CloseBehavior::Exit => "exit",
    })
}
fn log_level_to_str(l: LogLevel) -> SharedString {
    SharedString::from(match l {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    })
}
fn str_to_log_level(s: &str) -> Option<LogLevel> {
    match s.trim() {
        "trace" => Some(LogLevel::Trace),
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warn" => Some(LogLevel::Warn),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}
fn encryption_to_str(m: BtEncryptionMode) -> SharedString {
    SharedString::from(match m {
        BtEncryptionMode::Enabled => "enabled",
        BtEncryptionMode::Disabled => "disabled",
        BtEncryptionMode::Forced => "forced",
    })
}
fn preallocate_to_str(m: BtPreallocateMode) -> SharedString {
    SharedString::from(match m {
        BtPreallocateMode::None => "none",
        BtPreallocateMode::Full => "full",
    })
}
fn background_opacity_to_str(v: &BackgroundOpacityPreset) -> SharedString {
    SharedString::from(match v {
        BackgroundOpacityPreset::Default => "default",
        BackgroundOpacityPreset::Acrylic => "acrylic",
        BackgroundOpacityPreset::Frosted => "frosted",
    })
}
fn str_to_background_opacity(s: &str) -> BackgroundOpacityPreset {
    match s {
        "acrylic" => BackgroundOpacityPreset::Acrylic,
        "frosted" => BackgroundOpacityPreset::Frosted,
        _ => BackgroundOpacityPreset::Default,
    }
}
fn double_click_completed_to_str(v: DoubleClickOnCompleted) -> SharedString {
    SharedString::from(match v {
        DoubleClickOnCompleted::None => "none",
        DoubleClickOnCompleted::OpenFile => "open_file",
        DoubleClickOnCompleted::OpenInExplorer => "open_in_explorer",
        DoubleClickOnCompleted::OpenDownloadDir => "open_download_dir",
    })
}
fn double_click_uncompleted_to_str(v: DoubleClickOnUncompleted) -> SharedString {
    SharedString::from(match v {
        DoubleClickOnUncompleted::None => "none",
        DoubleClickOnUncompleted::TogglePauseResume => "toggle_pause_resume",
    })
}
fn anti_leech_action_to_str(v: BtAntiLeechAction) -> SharedString {
    SharedString::from(match v {
        BtAntiLeechAction::Ban => "ban",
        BtAntiLeechAction::LimitSlots => "limit_slots",
    })
}
fn seed_choking_to_str(v: BtSeedChokingAlgorithm) -> SharedString {
    SharedString::from(match v {
        BtSeedChokingAlgorithm::FastestUpload => "fastest_upload",
        BtSeedChokingAlgorithm::RoundRobin => "round_robin",
        BtSeedChokingAlgorithm::AntiLeech => "anti_leech",
    })
}
fn choking_to_str(v: BtChokingAlgorithm) -> SharedString {
    SharedString::from(match v {
        BtChokingAlgorithm::FixedSlots => "fixed_slots",
        BtChokingAlgorithm::RateBased => "rate_based",
    })
}

/// Convert `AppSettings` and runtime modes to `SettingsFormData`.
pub fn app_settings_to_form(
    settings: &AppSettings,
    game_mode: bool,
    overclock_mode: bool,
    io_status_text: &str,
    disk_type_text: &str,
    lang: Language,
) -> SettingsFormData {
    let speed_limit_kb = if settings.global_speed_limit_bps > 0 {
        (settings.global_speed_limit_bps / 1024).to_string()
    } else {
        "0".to_string()
    };
    let bt_dl_kb = if settings.bt.global_download_rate_limit > 0 {
        (settings.bt.global_download_rate_limit / 1024).to_string()
    } else {
        "0".to_string()
    };
    let bt_ul_kb = if settings.bt.global_upload_rate_limit > 0 {
        (settings.bt.global_upload_rate_limit / 1024).to_string()
    } else {
        "0".to_string()
    };

    let language_src = if settings.appearance.language.is_empty() {
        lang.as_bcp47()
    } else {
        settings.appearance.language.as_str()
    };

    SettingsFormData {
        // 常规下载
        default_download_dir: SharedString::from(&settings.download.default_download_dir),
        max_parallel_tasks: SharedString::from(
            settings.scheduler.traditional.max_parallel_tasks.to_string(),
        ),
        global_speed_limit_kb: SharedString::from(speed_limit_kb),
        download_max_retries: SharedString::from(settings.download.default_max_retries.to_string()),
        download_checksum_idx: combo::idx_of(
            combo::CHECKSUMS,
            &checksum_to_str(settings.download.default_checksum),
        ),
        download_auto_detect_sha256: settings.download.auto_detect_sha256,
        download_user_agent: SharedString::from(&settings.download.default_user_agent),
        // 外观
        appearance_color_mode_idx: combo::idx_of(
            combo::COLOR_MODES,
            &color_mode_to_str(&settings.appearance.color_mode),
        ),
        appearance_theme_color_idx: combo::idx_of(
            combo::THEME_COLORS,
            &theme_color_to_str(&settings.appearance.theme_color),
        ),
        appearance_background_opacity_idx: combo::idx_of(
            combo::OPACITY_PRESETS,
            &background_opacity_to_str(&settings.appearance.background_opacity),
        ),
        appearance_language_idx: combo::idx_of(combo::LANGUAGES, language_src),
        appearance_close_behavior_idx: combo::idx_of(
            combo::CLOSE_BEHAVIORS,
            &close_behavior_to_str(&settings.appearance.close_behavior),
        ),
        appearance_show_detail_info: settings.appearance.show_detail_info,
        appearance_compact_view: settings.appearance.compact_view,
        appearance_column_file: column_is_visible(&settings.appearance.visible_columns, "file"),
        appearance_column_size: column_is_visible(&settings.appearance.visible_columns, "size"),
        appearance_column_downloaded: column_is_visible(
            &settings.appearance.visible_columns,
            "downloaded",
        ),
        appearance_column_status: column_is_visible(
            &settings.appearance.visible_columns,
            "status",
        ),
        appearance_column_progress: column_is_visible(
            &settings.appearance.visible_columns,
            "progress",
        ),
        appearance_column_speed: column_is_visible(
            &settings.appearance.visible_columns,
            "speed",
        ),
        appearance_column_priority: column_is_visible(
            &settings.appearance.visible_columns,
            "priority",
        ),
        appearance_column_upload_speed: column_is_visible(
            &settings.appearance.visible_columns,
            "uploadSpeed",
        ),
        appearance_column_seeds: column_is_visible(
            &settings.appearance.visible_columns,
            "seeds",
        ),
        appearance_column_eta: column_is_visible(&settings.appearance.visible_columns, "eta"),
        autostart: settings.autostart,
        notifications_enabled: settings.notifications.enabled,
        double_click_completed_idx: combo::idx_of(
            combo::DOUBLE_CLICK_COMPLETED,
            &double_click_completed_to_str(settings.double_click.on_completed),
        ),
        double_click_uncompleted_idx: combo::idx_of(
            combo::DOUBLE_CLICK_UNCOMPLETED,
            &double_click_uncompleted_to_str(settings.double_click.on_uncompleted),
        ),
        // 代理
        proxy_mode_idx: combo::idx_of(
            combo::PROXY_MODES,
            &proxy_mode_to_str(settings.proxy.mode),
        ),
        proxy_manual_url: SharedString::from(&settings.proxy.manual_url),
        // 调度
        scheduler_mode_idx: combo::idx_of(
            combo::SCHEDULER_MODES,
            &scheduler_mode_to_str(settings.scheduler.mode),
        ),
        scheduler_max_parallel_threads: SharedString::from(
            settings.scheduler.automatic.max_parallel_threads.to_string(),
        ),
        scheduler_max_threads_per_task: SharedString::from(
            settings.scheduler.automatic.max_threads_per_task.to_string(),
        ),
        scheduler_min_threads_per_task: SharedString::from(
            settings.scheduler.automatic.min_threads_per_task.to_string(),
        ),
        scheduler_adaptive_profile_idx: combo::idx_of(
            combo::ADAPTIVE_PROFILES,
            &adaptive_profile_to_str(settings.scheduler.automatic.adaptive_profile),
        ),
        scheduler_chunk_strategy_idx: combo::idx_of(
            combo::CHUNK_STRATEGIES,
            &chunk_strategy_to_str(settings.scheduler.chunk_size_strategy),
        ),
        scheduler_tail_sprint: settings.scheduler.tail_sprint_enabled,
        scheduler_connection_warmup: settings.scheduler.connection_warmup_enabled,
        // BT 基础 + 进阶
        dht_enabled: settings.bt.dht_enabled,
        listen_port: SharedString::from(
            settings.bt.listen_port.map(|p| p.to_string()).unwrap_or_default(),
        ),
        max_bt_connections: SharedString::from(settings.bt.max_peers_per_torrent.to_string()),
        tracker_url: SharedString::from(&settings.bt.tracker_list_url),
        bt_upnp_enabled: settings.bt.upnp_enabled,
        bt_natpmp_enabled: settings.bt.enable_natpmp,
        bt_ipv6_enabled: settings.bt.enable_ipv6,
        bt_pex_enabled: settings.bt.enable_pex,
        bt_lsd_enabled: settings.bt.enable_lsd,
        bt_utp_enabled: settings.bt.enable_utp,
        bt_encryption_mode_idx: combo::idx_of(
            combo::ENCRYPTION_MODES,
            &encryption_to_str(settings.bt.encryption_mode),
        ),
        bt_preallocate_mode_idx: combo::idx_of(
            combo::PREALLOC_MODES,
            &preallocate_to_str(settings.bt.preallocate_mode),
        ),
        bt_max_downloads: SharedString::from(settings.bt.max_downloads.to_string()),
        bt_max_seeds: SharedString::from(settings.bt.max_seeds.to_string()),
        bt_max_torrents: SharedString::from(settings.bt.max_torrents.to_string()),
        bt_active_limit: SharedString::from(settings.bt.active_limit.to_string()),
        bt_global_download_rate_limit_kb: SharedString::from(bt_dl_kb),
        bt_global_upload_rate_limit_kb: SharedString::from(bt_ul_kb),
        bt_enable_fast_extension: settings.bt.enable_fast_extension,
        bt_enable_holepunch: settings.bt.enable_holepunch,
        bt_enable_web_seed: settings.bt.enable_web_seed,
        bt_enable_super_seeding: settings.bt.enable_super_seeding,
        bt_pause_upload_when_limit: settings.bt.pause_upload_when_limit_reached,
        bt_upload_limit_kb: SharedString::from(if settings.bt.upload_limit_bytes > 0 {
            (settings.bt.upload_limit_bytes / 1024).to_string()
        } else {
            "0".to_string()
        }),
        bt_upload_ratio_limit: SharedString::from({
            let r = settings.bt.upload_ratio_limit;
            if r == 0.0 { "0".to_string() } else { r.to_string() }
        }),
        bt_anti_leech_enabled: settings.bt.anti_leech_enabled,
        bt_anti_leech_action_idx: combo::idx_of(
            combo::ANTI_LEECH_ACTIONS,
            &anti_leech_action_to_str(settings.bt.anti_leech_action),
        ),
        bt_anti_leech_grace_secs: SharedString::from(settings.bt.anti_leech_grace_secs.to_string()),
        bt_anti_leech_ratio: SharedString::from({
            let r = settings.bt.anti_leech_ratio;
            if r == 0.0 { "0".to_string() } else { r.to_string() }
        }),
        bt_anti_leech_ban_secs: SharedString::from(settings.bt.anti_leech_ban_secs.to_string()),
        bt_anti_leech_max_upload_slots: SharedString::from(
            settings.bt.anti_leech_max_upload_slots.to_string(),
        ),
        bt_blocklist_enabled: settings.bt.blocklist_enabled,
        bt_blocklist_path: SharedString::from(&settings.bt.blocklist_path),
        bt_seed_choking_algorithm_idx: combo::idx_of(
            combo::SEED_CHOKING,
            &seed_choking_to_str(settings.bt.seed_choking_algorithm),
        ),
        bt_choking_algorithm_idx: combo::idx_of(
            combo::CHOKING_ALGOS,
            &choking_to_str(settings.bt.choking_algorithm),
        ),
        bt_max_upload_slots_per_torrent: SharedString::from(
            settings.bt.max_upload_slots_per_torrent.to_string(),
        ),
        bt_smart_ban_max_failures: SharedString::from(settings.bt.smart_ban_max_failures.to_string()),
        bt_smart_ban_parole: settings.bt.smart_ban_parole,
        bt_eviction_ban_duration_secs: SharedString::from(
            settings.bt.eviction_ban_duration_secs.to_string(),
        ),
        bt_data_contribution_timeout_secs: SharedString::from(
            settings.bt.data_contribution_timeout_secs.to_string(),
        ),
        // IO 基线
        io_buffer_limit_mb: SharedString::from(settings.io_baseline.buffer_limit_mb.to_string()),
        io_game_mode_buffer_mb: SharedString::from(
            settings.io_baseline.game_mode_buffer_mb.to_string(),
        ),
        io_max_parallel_hdd: SharedString::from(settings.io_baseline.max_parallel_hdd.to_string()),
        io_game_mode_max_parallel: SharedString::from(
            settings.io_baseline.game_mode_max_parallel.to_string(),
        ),
        io_hdd_buffer_enabled: settings.io_baseline.hdd_buffer_enabled,
        io_ssd_write_combine_mb: SharedString::from(
            settings.io_baseline.ssd_write_combine_mb.to_string(),
        ),
        // 日志
        logging_enabled: settings.logging.enabled,
        logging_level_idx: combo::idx_of(
            combo::LOG_LEVELS,
            &log_level_to_str(settings.logging.level),
        ),
        logging_file_path: SharedString::from(&settings.logging.file_path),
        logging_retention_count: SharedString::from(
            settings.logging.retention_count.map(|v| v.to_string()).unwrap_or_default(),
        ),
        logging_retention_days: SharedString::from(
            settings.logging.retention_days.map(|v| v.to_string()).unwrap_or_default(),
        ),
        // Aria2
        aria2_enabled: settings.aria2_rpc.enabled,
        aria2_port: SharedString::from(settings.aria2_rpc.port.to_string()),
        aria2_secret: SharedString::from(settings.aria2_rpc.secret.clone().unwrap_or_default()),
        // 运行态
        game_mode,
        overclock_mode,
        io_status_text: SharedString::from(io_status_text),
        disk_type_text: SharedString::from(disk_type_text),
        max_in_memory_downloads: SharedString::from(
            settings.max_in_memory_downloads.to_string(),
        ),
        app_version: SharedString::from(format!("v{}", env!("CARGO_PKG_VERSION"))),
        engine_version: SharedString::from(format!("limedl-core v{}", env!("CARGO_PKG_VERSION"))),
        arch_info: SharedString::from(i18n::format_platform_description(
            &crate::platform_win::os_description(),
            std::env::consts::ARCH,
            "Skia",
        )),
    }
}

/// Update `AppSettings` from `SettingsFormData`.
/// Returns `Err` with a human-readable message if any field contains
/// non-empty but unparsable content (e.g. "abc" in a numeric field or
/// an invalid proxy URL). Empty strings retain the previous value for
/// numeric fields (mirroring the previous desktop shell) but mandatory string fields
/// like proxy manual URL are validated eagerly so the user gets immediate
/// feedback instead of a silent no-op or a later `normalize_settings` error.
pub fn update_app_settings_from_form(
    settings: &mut AppSettings,
    form: &SettingsFormData,
    lang: Language,
) -> Result<(), String> {
    // ── 严格校验：非空但无法解析的数值为用户输入错误，必须报错而非静默忽略
    {
        let check_u64 = |raw: &str, field: i18n::SettingsField| -> Result<(), String> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(());
            }
            s.parse::<u64>()
                .map(|_| ())
                .map_err(|_| i18n::format_validation_number(lang, field, s))
        };
        let check_usize = |raw: &str, field: i18n::SettingsField| -> Result<(), String> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(());
            }
            s.parse::<usize>()
                .map(|_| ())
                .map_err(|_| i18n::format_validation_integer(lang, field, s))
        };
        let check_u32 = |raw: &str, field: i18n::SettingsField| -> Result<(), String> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(());
            }
            s.parse::<u32>()
                .map(|_| ())
                .map_err(|_| i18n::format_validation_integer(lang, field, s))
        };
        let check_u16 = |raw: &str, field: i18n::SettingsField| -> Result<(), String> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(());
            }
            s.parse::<u16>()
                .map(|_| ())
                .map_err(|_| i18n::format_validation_port(lang, field, s))
        };
        let check_f64 = |raw: &str, field: i18n::SettingsField| -> Result<(), String> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(());
            }
            s.parse::<f64>()
                .map(|_| ())
                .map_err(|_| i18n::format_validation_number(lang, field, s))
        };
        use i18n::SettingsField as F;
        // 下载
        check_u32(form.download_max_retries.trim(), F::MaxRetries)?;
        check_usize(form.max_parallel_tasks.trim(), F::MaxParallelTasks)?;
        check_u64(form.global_speed_limit_kb.trim(), F::GlobalSpeedLimit)?;
        // 调度
        check_usize(
            form.scheduler_max_parallel_threads.trim(),
            F::SchedulerMaxParallelThreads,
        )?;
        check_usize(
            form.scheduler_max_threads_per_task.trim(),
            F::SchedulerMaxThreadsPerTask,
        )?;
        check_usize(
            form.scheduler_min_threads_per_task.trim(),
            F::SchedulerMinThreadsPerTask,
        )?;
        // BT 基础
        check_u16(form.listen_port.trim(), F::ListenPort)?;
        check_u32(form.max_bt_connections.trim(), F::MaxPeersPerTorrent)?;
        // BT 队列与限速
        check_u32(form.bt_max_downloads.trim(), F::BtMaxDownloads)?;
        check_u32(form.bt_max_seeds.trim(), F::BtMaxSeeds)?;
        check_u32(form.bt_max_torrents.trim(), F::BtMaxTorrents)?;
        check_u32(form.bt_active_limit.trim(), F::BtActiveLimit)?;
        check_u64(
            form.bt_global_download_rate_limit_kb.trim(),
            F::BtGlobalDownloadRateLimit,
        )?;
        check_u64(
            form.bt_global_upload_rate_limit_kb.trim(),
            F::BtGlobalUploadRateLimit,
        )?;
        check_u64(form.bt_upload_limit_kb.trim(), F::BtUploadLimit)?;
        check_f64(form.bt_upload_ratio_limit.trim(), F::BtUploadRatioLimit)?;
        check_u64(form.bt_anti_leech_grace_secs.trim(), F::BtAntiLeechGraceSecs)?;
        check_f64(form.bt_anti_leech_ratio.trim(), F::BtAntiLeechRatio)?;
        check_u64(form.bt_anti_leech_ban_secs.trim(), F::BtAntiLeechBanSecs)?;
        check_u32(
            form.bt_anti_leech_max_upload_slots.trim(),
            F::BtAntiLeechMaxUploadSlots,
        )?;
        check_u32(
            form.bt_max_upload_slots_per_torrent.trim(),
            F::BtMaxUploadSlotsPerTorrent,
        )?;
        check_u32(form.bt_smart_ban_max_failures.trim(), F::BtSmartBanMaxFailures)?;
        check_u64(
            form.bt_eviction_ban_duration_secs.trim(),
            F::BtEvictionBanDurationSecs,
        )?;
        check_u64(
            form.bt_data_contribution_timeout_secs.trim(),
            F::BtDataContributionTimeoutSecs,
        )?;
        // IO
        check_u64(form.io_buffer_limit_mb.trim(), F::IoBufferLimitMb)?;
        check_u64(form.io_game_mode_buffer_mb.trim(), F::IoGameModeBufferMb)?;
        check_u32(form.io_max_parallel_hdd.trim(), F::IoMaxParallelHdd)?;
        check_u32(form.io_game_mode_max_parallel.trim(), F::IoGameModeMaxParallel)?;
        check_u64(form.io_ssd_write_combine_mb.trim(), F::IoSsdWriteCombineMb)?;
        // 日志
        if !form.logging_retention_count.trim().is_empty() {
            check_u32(form.logging_retention_count.trim(), F::LoggingRetentionCount)?;
        }
        if !form.logging_retention_days.trim().is_empty() {
            check_u32(form.logging_retention_days.trim(), F::LoggingRetentionDays)?;
        }
        // Aria2
        if !form.aria2_port.trim().is_empty() {
            check_u16(form.aria2_port.trim(), F::Aria2Port)?;
        }
        // 高级
        if !form.max_in_memory_downloads.trim().is_empty() {
            check_usize(form.max_in_memory_downloads.trim(), F::MaxInMemoryDownloads)?;
        }
        // 代理：若为 manual 则必须提供合法 URL，留空会在 normalize 阶段报错，这里提前给出更友好的提示
        if combo::value_at(combo::PROXY_MODES, form.proxy_mode_idx) == "manual"
            && form.proxy_manual_url.trim().is_empty()
        {
            return Err(i18n::format_proxy_url_required(lang));
        }
    }
    // ── 下载 ──
    let dir = form.default_download_dir.trim();
    if !dir.is_empty() {
        settings.download.default_download_dir = dir.to_string();
    }
    if let Ok(v) = form.download_max_retries.trim().parse::<u32>() {
        settings.download.default_max_retries = v.min(100);
    }
    if let Some(c) = str_to_checksum(combo::value_at(combo::CHECKSUMS, form.download_checksum_idx))
    {
        settings.download.default_checksum = c;
    }
    settings.download.auto_detect_sha256 = form.download_auto_detect_sha256;
    // user-agent 允许清空（回退到默认值由后端处理），此处直接写入
    settings.download.default_user_agent = form.download_user_agent.trim().to_string();

    if let Ok(parallel) = form.max_parallel_tasks.trim().parse::<usize>()
        && parallel > 0
    {
        settings.scheduler.traditional.max_parallel_tasks = parallel.min(64);
    }
    if let Ok(limit_kb) = form.global_speed_limit_kb.trim().parse::<u64>() {
        settings.global_speed_limit_bps = limit_kb * 1024;
    }

    // ── 外观 / 启动 ──
    match combo::value_at(combo::COLOR_MODES, form.appearance_color_mode_idx) {
        "light" => settings.appearance.color_mode = ColorMode::Light,
        "dark" => settings.appearance.color_mode = ColorMode::Dark,
        _ => settings.appearance.color_mode = ColorMode::System,
    }
    match combo::value_at(combo::THEME_COLORS, form.appearance_theme_color_idx) {
        "amber" => settings.appearance.theme_color = ThemeColor::Amber,
        "sky" => settings.appearance.theme_color = ThemeColor::Sky,
        _ => settings.appearance.theme_color = ThemeColor::Lime,
    }
    settings.appearance.background_opacity = str_to_background_opacity(combo::value_at(
        combo::OPACITY_PRESETS,
        form.appearance_background_opacity_idx,
    ));
    settings.appearance.language = combo::value_at(combo::LANGUAGES, form.appearance_language_idx)
        .to_string();
    match combo::value_at(combo::CLOSE_BEHAVIORS, form.appearance_close_behavior_idx) {
        "exit" => settings.appearance.close_behavior = CloseBehavior::Exit,
        _ => settings.appearance.close_behavior = CloseBehavior::MinimizeToTray,
    }
    settings.appearance.show_detail_info = form.appearance_show_detail_info;
    settings.appearance.compact_view = form.appearance_compact_view;
    // Column visibility: rebuild the persisted key list in canonical order.
    // An all-false selection would hide every column, so the file column is
    // always kept (mirroring the web client's guard).
    let column_enabled = |key: &str| -> bool {
        match key {
            "file" => true,
            "size" => form.appearance_column_size,
            "downloaded" => form.appearance_column_downloaded,
            "status" => form.appearance_column_status,
            "progress" => form.appearance_column_progress,
            "speed" => form.appearance_column_speed,
            "priority" => form.appearance_column_priority,
            "uploadSpeed" => form.appearance_column_upload_speed,
            "seeds" => form.appearance_column_seeds,
            "eta" => form.appearance_column_eta,
            _ => false,
        }
    };
    settings.appearance.visible_columns = COLUMN_KEYS
        .iter()
        .filter(|key| column_enabled(key))
        .map(|key| (*key).to_string())
        .collect();
    settings.autostart = form.autostart;
    settings.notifications.enabled = form.notifications_enabled;
    match combo::value_at(combo::DOUBLE_CLICK_COMPLETED, form.double_click_completed_idx) {
        "open_file" => settings.double_click.on_completed = DoubleClickOnCompleted::OpenFile,
        "open_in_explorer" => {
            settings.double_click.on_completed = DoubleClickOnCompleted::OpenInExplorer
        }
        "open_download_dir" => {
            settings.double_click.on_completed = DoubleClickOnCompleted::OpenDownloadDir
        }
        _ => settings.double_click.on_completed = DoubleClickOnCompleted::None,
    }
    match combo::value_at(combo::DOUBLE_CLICK_UNCOMPLETED, form.double_click_uncompleted_idx) {
        "toggle_pause_resume" => {
            settings.double_click.on_uncompleted = DoubleClickOnUncompleted::TogglePauseResume
        }
        _ => settings.double_click.on_uncompleted = DoubleClickOnUncompleted::None,
    }

    // ── 代理 ──
    if let Some(m) = str_to_proxy_mode(combo::value_at(combo::PROXY_MODES, form.proxy_mode_idx)) {
        settings.proxy.mode = m;
    }
    settings.proxy.manual_url = form.proxy_manual_url.trim().to_string();

    // ── 调度 ──
    if let Some(m) = match combo::value_at(combo::SCHEDULER_MODES, form.scheduler_mode_idx) {
        "traditional" => Some(SchedulerMode::Traditional),
        "automatic" => Some(SchedulerMode::Automatic),
        _ => None,
    } {
        settings.scheduler.mode = m;
    }
    if let Ok(v) = form.scheduler_max_parallel_threads.trim().parse::<usize>()
        && v > 0
    {
        settings.scheduler.automatic.max_parallel_threads = v.min(128);
    }
    if let Ok(v) = form.scheduler_max_threads_per_task.trim().parse::<usize>()
        && v > 0
    {
        settings.scheduler.automatic.max_threads_per_task = v.min(64);
    }
    if let Ok(v) = form.scheduler_min_threads_per_task.trim().parse::<usize>() {
        settings.scheduler.automatic.min_threads_per_task =
            v.min(settings.scheduler.automatic.max_threads_per_task);
    }
    match combo::value_at(combo::ADAPTIVE_PROFILES, form.scheduler_adaptive_profile_idx) {
        "conservative" => {
            settings.scheduler.automatic.adaptive_profile = AdaptiveProfile::Conservative
        }
        "aggressive" => {
            settings.scheduler.automatic.adaptive_profile = AdaptiveProfile::Aggressive
        }
        _ => settings.scheduler.automatic.adaptive_profile = AdaptiveProfile::Balanced,
    }
    match combo::value_at(combo::CHUNK_STRATEGIES, form.scheduler_chunk_strategy_idx) {
        "fixed" => settings.scheduler.chunk_size_strategy = ChunkSizeStrategy::Fixed,
        _ => settings.scheduler.chunk_size_strategy = ChunkSizeStrategy::Adaptive,
    }
    settings.scheduler.tail_sprint_enabled = form.scheduler_tail_sprint;
    settings.scheduler.connection_warmup_enabled = form.scheduler_connection_warmup;

    // ── BT ──
    settings.bt.dht_enabled = form.dht_enabled;
    let lp = form.listen_port.trim();
    if lp.is_empty() {
        settings.bt.listen_port = None;
    } else if let Ok(port) = lp.parse::<u16>() {
        if port == 0 {
            settings.bt.listen_port = None;
        } else {
            settings.bt.listen_port = Some(port);
        }
    }
    if let Ok(conns) = form.max_bt_connections.trim().parse::<u32>()
        && conns > 0
    {
        settings.bt.max_peers_per_torrent = conns.min(4096);
    }
    let tracker_url = form.tracker_url.trim();
    // 允许清空
    settings.bt.tracker_list_url = tracker_url.to_string();
    settings.bt.upnp_enabled = form.bt_upnp_enabled;
    settings.bt.enable_natpmp = form.bt_natpmp_enabled;
    settings.bt.enable_ipv6 = form.bt_ipv6_enabled;
    settings.bt.enable_pex = form.bt_pex_enabled;
    settings.bt.enable_lsd = form.bt_lsd_enabled;
    settings.bt.enable_utp = form.bt_utp_enabled;
    match combo::value_at(combo::ENCRYPTION_MODES, form.bt_encryption_mode_idx) {
        "disabled" => settings.bt.encryption_mode = BtEncryptionMode::Disabled,
        "forced" => settings.bt.encryption_mode = BtEncryptionMode::Forced,
        _ => settings.bt.encryption_mode = BtEncryptionMode::Enabled,
    }
    match combo::value_at(combo::PREALLOC_MODES, form.bt_preallocate_mode_idx) {
        "full" => settings.bt.preallocate_mode = BtPreallocateMode::Full,
        _ => settings.bt.preallocate_mode = BtPreallocateMode::None,
    }
    if let Ok(v) = form.bt_max_downloads.trim().parse::<u32>() {
        settings.bt.max_downloads = v.clamp(1, 1000);
    }
    if let Ok(v) = form.bt_max_seeds.trim().parse::<u32>() {
        settings.bt.max_seeds = v.clamp(0, 1000);
    }
    if let Ok(v) = form.bt_max_torrents.trim().parse::<u32>() {
        settings.bt.max_torrents = v.clamp(1, 10000);
    }
    if let Ok(v) = form.bt_active_limit.trim().parse::<u32>() {
        settings.bt.active_limit = v.clamp(1, 10000);
    }
    if let Ok(v) = form.bt_global_download_rate_limit_kb.trim().parse::<u64>() {
        settings.bt.global_download_rate_limit = v * 1024;
    }
    if let Ok(v) = form.bt_global_upload_rate_limit_kb.trim().parse::<u64>() {
        settings.bt.global_upload_rate_limit = v * 1024;
    }
    settings.bt.enable_fast_extension = form.bt_enable_fast_extension;
    settings.bt.enable_holepunch = form.bt_enable_holepunch;
    settings.bt.enable_web_seed = form.bt_enable_web_seed;
    settings.bt.enable_super_seeding = form.bt_enable_super_seeding;
    settings.bt.pause_upload_when_limit_reached = form.bt_pause_upload_when_limit;
    if let Ok(v) = form.bt_upload_limit_kb.trim().parse::<u64>() {
        settings.bt.upload_limit_bytes = v * 1024;
    }
    if let Ok(v) = form.bt_upload_ratio_limit.trim().parse::<f64>() {
        settings.bt.upload_ratio_limit = v.clamp(0.0, 1000.0);
    }
    settings.bt.anti_leech_enabled = form.bt_anti_leech_enabled;
    match combo::value_at(combo::ANTI_LEECH_ACTIONS, form.bt_anti_leech_action_idx) {
        "limit_slots" => settings.bt.anti_leech_action = BtAntiLeechAction::LimitSlots,
        _ => settings.bt.anti_leech_action = BtAntiLeechAction::Ban,
    }
    if let Ok(v) = form.bt_anti_leech_grace_secs.trim().parse::<u64>() {
        settings.bt.anti_leech_grace_secs = v;
    }
    if let Ok(v) = form.bt_anti_leech_ratio.trim().parse::<f64>() {
        settings.bt.anti_leech_ratio = v.clamp(0.0, 1.0);
    }
    if let Ok(v) = form.bt_anti_leech_ban_secs.trim().parse::<u64>() {
        settings.bt.anti_leech_ban_secs = v;
    }
    if let Ok(v) = form.bt_anti_leech_max_upload_slots.trim().parse::<u32>()
        && v > 0
    {
        settings.bt.anti_leech_max_upload_slots = v.clamp(1, 64);
    }
    settings.bt.blocklist_enabled = form.bt_blocklist_enabled;
    settings.bt.blocklist_path = form.bt_blocklist_path.trim().to_string();
    match combo::value_at(combo::SEED_CHOKING, form.bt_seed_choking_algorithm_idx) {
        "round_robin" => settings.bt.seed_choking_algorithm = BtSeedChokingAlgorithm::RoundRobin,
        "anti_leech" => settings.bt.seed_choking_algorithm = BtSeedChokingAlgorithm::AntiLeech,
        _ => settings.bt.seed_choking_algorithm = BtSeedChokingAlgorithm::FastestUpload,
    }
    match combo::value_at(combo::CHOKING_ALGOS, form.bt_choking_algorithm_idx) {
        "rate_based" => settings.bt.choking_algorithm = BtChokingAlgorithm::RateBased,
        _ => settings.bt.choking_algorithm = BtChokingAlgorithm::FixedSlots,
    }
    if let Ok(v) = form.bt_max_upload_slots_per_torrent.trim().parse::<u32>()
        && v > 0
    {
        settings.bt.max_upload_slots_per_torrent = v.clamp(1, 64);
    }
    if let Ok(v) = form.bt_smart_ban_max_failures.trim().parse::<u32>()
        && v > 0
    {
        settings.bt.smart_ban_max_failures = v.clamp(1, 100);
    }
    settings.bt.smart_ban_parole = form.bt_smart_ban_parole;
    if let Ok(v) = form.bt_eviction_ban_duration_secs.trim().parse::<u64>() {
        settings.bt.eviction_ban_duration_secs = v;
    }
    if let Ok(v) = form.bt_data_contribution_timeout_secs.trim().parse::<u64>() {
        settings.bt.data_contribution_timeout_secs = v;
    }

    // ── IO 基线 ──
    if let Ok(v) = form.io_buffer_limit_mb.trim().parse::<u64>() {
        settings.io_baseline.buffer_limit_mb = v.clamp(64, 32768);
    }
    if let Ok(v) = form.io_game_mode_buffer_mb.trim().parse::<u64>() {
        settings.io_baseline.game_mode_buffer_mb = v.clamp(16, 4096);
    }
    if let Ok(v) = form.io_max_parallel_hdd.trim().parse::<u32>() {
        settings.io_baseline.max_parallel_hdd = v.clamp(1, 16);
    }
    if let Ok(v) = form.io_game_mode_max_parallel.trim().parse::<u32>() {
        settings.io_baseline.game_mode_max_parallel = v.clamp(1, 8);
    }
    settings.io_baseline.hdd_buffer_enabled = form.io_hdd_buffer_enabled;
    if let Ok(v) = form.io_ssd_write_combine_mb.trim().parse::<u64>() {
        settings.io_baseline.ssd_write_combine_mb = v.min(4096);
    }

    // ── 日志 ──
    settings.logging.enabled = form.logging_enabled;
    if let Some(l) = str_to_log_level(combo::value_at(combo::LOG_LEVELS, form.logging_level_idx)) {
        settings.logging.level = l;
    }
    settings.logging.file_path = form.logging_file_path.trim().to_string();
    let rc = form.logging_retention_count.trim();
    if rc.is_empty() {
        settings.logging.retention_count = None;
    } else if let Ok(v) = rc.parse::<u32>() {
        settings.logging.retention_count = Some(v.min(10000));
    }
    let rd = form.logging_retention_days.trim();
    if rd.is_empty() {
        settings.logging.retention_days = None;
    } else if let Ok(v) = rd.parse::<u32>() {
        settings.logging.retention_days = Some(v.min(36500));
    }

    // ── Aria2 ──
    settings.aria2_rpc.enabled = form.aria2_enabled;
    if let Ok(v) = form.aria2_port.trim().parse::<u16>()
        && v != 0
    {
        settings.aria2_rpc.port = v;
    }
    let sec = form.aria2_secret.trim();
    if sec.is_empty() {
        settings.aria2_rpc.secret = None;
    } else {
        settings.aria2_rpc.secret = Some(sec.to_string());
    }

    // ── 高级 ──
    if let Ok(v) = form.max_in_memory_downloads.trim().parse::<usize>()
        && v > 0
    {
        settings.max_in_memory_downloads = v.clamp(10, 10000);
    }

    Ok(())
}

/// Text state of one schedule row as typed by the user (authoritative until
/// save, so half-typed values survive model rebuilds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedLimitSlotText {
    pub start_hour: String,
    pub end_hour: String,
    pub limit_kb: String,
}

impl Default for SpeedLimitSlotText {
    fn default() -> Self {
        Self {
            start_hour: "0".to_string(),
            end_hour: "6".to_string(),
            limit_kb: "0".to_string(),
        }
    }
}

/// Parse a schedule text field into u32, rejecting empty/garbage input.
fn parse_schedule_u32(
    raw: &str,
    field: i18n::SettingsField,
    lang: Language,
) -> Result<u32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(i18n::format_validation_integer(lang, field, raw));
    }
    trimmed
        .parse::<u32>()
        .map_err(|_| i18n::format_validation_integer(lang, field, trimmed))
}

/// Validate + convert the schedule rows into the core type.
///
/// Hours must be 0-23 (24-hour clock); the limit may be any u64 KB/s value
/// (0 = unlimited). Returns a localized message on the first invalid row.
pub fn parse_speed_limit_slots(
    rows: &[SpeedLimitSlotText],
    lang: Language,
) -> Result<Vec<limedl_core::types::SpeedLimitSlot>, String> {
    let mut slots = Vec::with_capacity(rows.len());
    for row in rows {
        let start = parse_schedule_u32(&row.start_hour, i18n::SettingsField::SpeedLimitStartHour, lang)?;
        let end = parse_schedule_u32(&row.end_hour, i18n::SettingsField::SpeedLimitEndHour, lang)?;
        let limit_kb = parse_schedule_u32(&row.limit_kb, i18n::SettingsField::SpeedLimitLimit, lang)?;
        if start > 23 {
            return Err(i18n::format_validation_range(
                lang,
                i18n::SettingsField::SpeedLimitStartHour,
                0,
                23,
            ));
        }
        if end > 23 {
            return Err(i18n::format_validation_range(
                lang,
                i18n::SettingsField::SpeedLimitEndHour,
                0,
                23,
            ));
        }
        slots.push(limedl_core::types::SpeedLimitSlot {
            start_hour: start as u8,
            end_hour: end as u8,
            limit_bps: limit_kb as u64 * 1024,
        });
    }
    Ok(slots)
}

/// Build the Slint model items for the schedule editor.
pub fn speed_limit_slots_to_slint(
    rows: &[SpeedLimitSlotText],
    lang: Language,
) -> Vec<SpeedLimitSlotItem> {
    rows.iter()
        .map(|row| {
            let start = row.start_hour.trim().parse::<u32>().unwrap_or(0).min(23);
            let end = row.end_hour.trim().parse::<u32>().unwrap_or(0).min(23);
            let limit = row.limit_kb.trim().parse::<u64>().unwrap_or(0);
            SpeedLimitSlotItem {
                start_hour: SharedString::from(row.start_hour.as_str()),
                end_hour: SharedString::from(row.end_hour.as_str()),
                limit_kb: SharedString::from(row.limit_kb.as_str()),
                wraps: start >= end,
                summary: SharedString::from(i18n::format_schedule_summary(start, end, limit, lang)),
            }
        })
        .collect()
}

/// Convert persisted settings into the editor's per-row text state.
pub fn speed_limit_slots_from_settings(settings: &AppSettings) -> Vec<SpeedLimitSlotText> {
    settings
        .speed_limit_schedule
        .iter()
        .map(|slot| SpeedLimitSlotText {
            start_hour: slot.start_hour.to_string(),
            end_hour: slot.end_hour.to_string(),
            limit_kb: (slot.limit_bps / 1024).to_string(),
        })
        .collect()
}

