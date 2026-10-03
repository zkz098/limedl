use limedl_core::types::AppSettings;
use slint::SharedString;

use crate::SettingsFormData;
use crate::bridge::models::column_is_visible;
use crate::i18n::{self, Language};

use super::combo;
use super::enums::{adaptive_profile_to_str, anti_leech_action_to_str, aria2_auth_mode_to_str, background_opacity_to_str, checksum_to_str, choking_to_str, chunk_strategy_to_str, close_behavior_to_str, color_mode_to_str, double_click_completed_to_str, double_click_uncompleted_to_str, encryption_to_str, log_level_to_str, preallocate_to_str, proxy_mode_to_str, scheduler_mode_to_str, seed_choking_to_str, theme_color_to_str};

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
            settings
                .scheduler
                .traditional
                .max_parallel_tasks
                .to_string(),
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
        appearance_column_status: column_is_visible(&settings.appearance.visible_columns, "status"),
        appearance_column_progress: column_is_visible(
            &settings.appearance.visible_columns,
            "progress",
        ),
        appearance_column_speed: column_is_visible(&settings.appearance.visible_columns, "speed"),
        appearance_column_priority: column_is_visible(
            &settings.appearance.visible_columns,
            "priority",
        ),
        appearance_column_upload_speed: column_is_visible(
            &settings.appearance.visible_columns,
            "uploadSpeed",
        ),
        appearance_column_seeds: column_is_visible(&settings.appearance.visible_columns, "seeds"),
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
        proxy_mode_idx: combo::idx_of(combo::PROXY_MODES, &proxy_mode_to_str(settings.proxy.mode)),
        proxy_manual_url: SharedString::from(&settings.proxy.manual_url),
        // 调度
        scheduler_mode_idx: combo::idx_of(
            combo::SCHEDULER_MODES,
            &scheduler_mode_to_str(settings.scheduler.mode),
        ),
        scheduler_max_parallel_threads: SharedString::from(
            settings
                .scheduler
                .automatic
                .max_parallel_threads
                .to_string(),
        ),
        scheduler_max_threads_per_task: SharedString::from(
            settings
                .scheduler
                .automatic
                .max_threads_per_task
                .to_string(),
        ),
        scheduler_min_threads_per_task: SharedString::from(
            settings
                .scheduler
                .automatic
                .min_threads_per_task
                .to_string(),
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
        bt_lightweight_mode: settings.bt.lightweight_mode,
        dht_enabled: settings.bt.dht_enabled,
        listen_port: SharedString::from(
            settings
                .bt
                .listen_port
                .map(|p| p.to_string())
                .unwrap_or_default(),
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
            if r == 0.0 {
                "0".to_string()
            } else {
                r.to_string()
            }
        }),
        bt_anti_leech_enabled: settings.bt.anti_leech_enabled,
        bt_anti_leech_action_idx: combo::idx_of(
            combo::ANTI_LEECH_ACTIONS,
            &anti_leech_action_to_str(settings.bt.anti_leech_action),
        ),
        bt_anti_leech_grace_secs: SharedString::from(settings.bt.anti_leech_grace_secs.to_string()),
        bt_anti_leech_ratio: SharedString::from({
            let r = settings.bt.anti_leech_ratio;
            if r == 0.0 {
                "0".to_string()
            } else {
                r.to_string()
            }
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
        bt_smart_ban_max_failures: SharedString::from(
            settings.bt.smart_ban_max_failures.to_string(),
        ),
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
            settings
                .logging
                .retention_count
                .map(|v| v.to_string())
                .unwrap_or_default(),
        ),
        logging_retention_days: SharedString::from(
            settings
                .logging
                .retention_days
                .map(|v| v.to_string())
                .unwrap_or_default(),
        ),
        // Aria2
        aria2_enabled: settings.aria2_rpc.enabled,
        aria2_port: SharedString::from(settings.aria2_rpc.port.to_string()),
        aria2_secret: SharedString::from(settings.aria2_rpc.secret.clone().unwrap_or_default()),
        aria2_auth_mode_idx: combo::idx_of(
            combo::ARIA2_AUTH_MODES,
            &aria2_auth_mode_to_str(settings.aria2_rpc.auth_mode),
        ),
        // 运行态
        game_mode,
        overclock_mode,
        io_status_text: SharedString::from(io_status_text),
        disk_type_text: SharedString::from(disk_type_text),
        max_in_memory_downloads: SharedString::from(settings.max_in_memory_downloads.to_string()),
        app_version: SharedString::from(format!("v{}", env!("CARGO_PKG_VERSION"))),
        engine_version: SharedString::from(format!("limedl-core v{}", env!("CARGO_PKG_VERSION"))),
        arch_info: SharedString::from(i18n::format_platform_description(
            &crate::platform_win::os_description(),
            std::env::consts::ARCH,
            crate::renderer::NAME,
        )),
        // Interpolated into the `@tr` core-tech line; the label comes from the one
        // place that names the renderer (`src/renderer.rs`).
        graphics_renderer: SharedString::from(crate::renderer::NAME),
    }
}
