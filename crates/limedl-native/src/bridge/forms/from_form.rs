use limedl_core::types::{
    AdaptiveProfile, AppSettings, BtAntiLeechAction, BtChokingAlgorithm, BtEncryptionMode,
    BtPreallocateMode, BtSeedChokingAlgorithm, ChunkSizeStrategy, CloseBehavior, ColorMode,
    DoubleClickOnCompleted, DoubleClickOnUncompleted, SchedulerMode, ThemeColor,
};

use crate::SettingsFormData;
use crate::bridge::models::COLUMN_KEYS;
use crate::i18n::{self, Language};

use super::combo;
use super::enums::{str_to_background_opacity, str_to_checksum, str_to_log_level, str_to_proxy_mode};

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
    validate_form(form, lang)?;
    apply_download(settings, form);
    apply_appearance(settings, form);
    apply_proxy(settings, form);
    apply_schedule(settings, form);
    apply_bt(settings, form);
    apply_io(settings, form);
    apply_log(settings, form);
    apply_aria2(settings, form);
    apply_advanced(settings, form);
    Ok(())
}

/// Reject non-empty but unparsable numeric fields before anything is written,
/// so a failed save leaves `settings` untouched.
fn validate_form(form: &SettingsFormData, lang: Language) -> Result<(), String> {
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
    check_u64(
        form.bt_anti_leech_grace_secs.trim(),
        F::BtAntiLeechGraceSecs,
    )?;
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
    check_u32(
        form.bt_smart_ban_max_failures.trim(),
        F::BtSmartBanMaxFailures,
    )?;
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
    check_u32(
        form.io_game_mode_max_parallel.trim(),
        F::IoGameModeMaxParallel,
    )?;
    check_u64(form.io_ssd_write_combine_mb.trim(), F::IoSsdWriteCombineMb)?;
    // 日志
    if !form.logging_retention_count.trim().is_empty() {
        check_u32(
            form.logging_retention_count.trim(),
            F::LoggingRetentionCount,
        )?;
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
    Ok(())
}

fn apply_download(settings: &mut AppSettings, form: &SettingsFormData) {
    // ── 下载 ──
    let dir = form.default_download_dir.trim();
    if !dir.is_empty() {
        settings.download.default_download_dir = dir.to_string();
    }
    if let Ok(v) = form.download_max_retries.trim().parse::<u32>() {
        settings.download.default_max_retries = v.min(100);
    }
    if let Some(c) = str_to_checksum(combo::value_at(
        combo::CHECKSUMS,
        form.download_checksum_idx,
    )) {
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
}

fn apply_appearance(settings: &mut AppSettings, form: &SettingsFormData) {
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
    settings.appearance.language =
        combo::value_at(combo::LANGUAGES, form.appearance_language_idx).to_string();
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
    match combo::value_at(
        combo::DOUBLE_CLICK_COMPLETED,
        form.double_click_completed_idx,
    ) {
        "open_file" => settings.double_click.on_completed = DoubleClickOnCompleted::OpenFile,
        "open_in_explorer" => {
            settings.double_click.on_completed = DoubleClickOnCompleted::OpenInExplorer
        }
        "open_download_dir" => {
            settings.double_click.on_completed = DoubleClickOnCompleted::OpenDownloadDir
        }
        _ => settings.double_click.on_completed = DoubleClickOnCompleted::None,
    }
    match combo::value_at(
        combo::DOUBLE_CLICK_UNCOMPLETED,
        form.double_click_uncompleted_idx,
    ) {
        "toggle_pause_resume" => {
            settings.double_click.on_uncompleted = DoubleClickOnUncompleted::TogglePauseResume
        }
        _ => settings.double_click.on_uncompleted = DoubleClickOnUncompleted::None,
    }
}

fn apply_proxy(settings: &mut AppSettings, form: &SettingsFormData) {
    // ── 代理 ──
    if let Some(m) = str_to_proxy_mode(combo::value_at(combo::PROXY_MODES, form.proxy_mode_idx)) {
        settings.proxy.mode = m;
    }
    settings.proxy.manual_url = form.proxy_manual_url.trim().to_string();
}

fn apply_schedule(settings: &mut AppSettings, form: &SettingsFormData) {
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
    match combo::value_at(
        combo::ADAPTIVE_PROFILES,
        form.scheduler_adaptive_profile_idx,
    ) {
        "conservative" => {
            settings.scheduler.automatic.adaptive_profile = AdaptiveProfile::Conservative
        }
        "aggressive" => settings.scheduler.automatic.adaptive_profile = AdaptiveProfile::Aggressive,
        _ => settings.scheduler.automatic.adaptive_profile = AdaptiveProfile::Balanced,
    }
    match combo::value_at(combo::CHUNK_STRATEGIES, form.scheduler_chunk_strategy_idx) {
        "fixed" => settings.scheduler.chunk_size_strategy = ChunkSizeStrategy::Fixed,
        _ => settings.scheduler.chunk_size_strategy = ChunkSizeStrategy::Adaptive,
    }
    settings.scheduler.tail_sprint_enabled = form.scheduler_tail_sprint;
    settings.scheduler.connection_warmup_enabled = form.scheduler_connection_warmup;
}

fn apply_bt(settings: &mut AppSettings, form: &SettingsFormData) {
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
}

fn apply_io(settings: &mut AppSettings, form: &SettingsFormData) {
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
}

fn apply_log(settings: &mut AppSettings, form: &SettingsFormData) {
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
}

fn apply_aria2(settings: &mut AppSettings, form: &SettingsFormData) {
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
}

fn apply_advanced(settings: &mut AppSettings, form: &SettingsFormData) {
    // ── 高级 ──
    if let Ok(v) = form.max_in_memory_downloads.trim().parse::<usize>()
        && v > 0
    {
        settings.max_in_memory_downloads = v.clamp(10, 10000);
    }
}
