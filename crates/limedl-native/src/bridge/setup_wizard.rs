use crate::i18n;
use super::forms::{chunk_strategy_to_str, color_mode_to_str, combo, proxy_mode_to_str, str_to_proxy_mode, theme_color_to_str};
use limedl_core::types::{ColorMode, ThemeColor,
    AdaptiveProfile, AppSettings, AutomaticSchedulerSettings, 
    ChunkSizeStrategy, ProxyMode, SchedulerMode,
};
use slint::SharedString;

use crate::i18n::Language;
use crate::SetupFormData;

// ── First-run setup wizard ───────────────────────────────────────────

/// Scheduler preset matching (mirrors the web client's PRESETS table in
/// `useSetupWizard.ts`): 0=energySaver, 1=balanced, 2=maxSpeed, 3=custom.
fn detect_scheduler_preset(settings: &AppSettings) -> i32 {
    let s = &settings.scheduler;
    if s.mode != SchedulerMode::Automatic {
        return 3;
    }
    let a = &s.automatic;
    match (
        a.max_parallel_threads,
        a.max_threads_per_task,
        a.min_threads_per_task,
        a.adaptive_profile,
    ) {
        (8, 4, 2, AdaptiveProfile::Conservative) => 0,
        // Accept the stock default (min 0) as balanced too: `AppSettings::default()`
        // ships min_threads_per_task=0 while the web preset uses 2.
        (16, 8, m, AdaptiveProfile::Balanced) if m == 0 || m == 2 => 1,
        (32, 16, 4, AdaptiveProfile::Aggressive) => 2,
        _ => 3,
    }
}

/// Convert `AppSettings` into the setup wizard's `SetupFormData`.
pub fn app_settings_to_setup_form(settings: &AppSettings, lang: Language) -> SetupFormData {
    let language_src = if settings.appearance.language.is_empty() {
        lang.as_bcp47()
    } else {
        settings.appearance.language.as_str()
    };

    SetupFormData {
        language_idx: combo::idx_of(combo::LANGUAGES, language_src),
        color_mode_idx: combo::idx_of(
            combo::COLOR_MODES,
            &color_mode_to_str(&settings.appearance.color_mode),
        ),
        theme_color_idx: combo::idx_of(
            combo::THEME_COLORS,
            &theme_color_to_str(&settings.appearance.theme_color),
        ),
        cdn_enabled: settings.cdn_acceleration.enabled,
        cdn_active_ip: SharedString::from(
            settings.cdn_acceleration.active_ip.clone().unwrap_or_default(),
        ),
        rpc_enabled: settings.aria2_rpc.enabled,
        rpc_port: SharedString::from(settings.aria2_rpc.port.to_string()),
        rpc_secret: SharedString::from(settings.aria2_rpc.secret.clone().unwrap_or_default()),
        default_dir: SharedString::from(&settings.download.default_download_dir),
        scheduler_preset_idx: detect_scheduler_preset(settings),
        chunk_strategy_idx: combo::idx_of(
            combo::CHUNK_STRATEGIES,
            &chunk_strategy_to_str(settings.scheduler.chunk_size_strategy),
        ),
        autostart: settings.autostart,
        notifications_enabled: settings.notifications.enabled,
        proxy_mode_idx: combo::idx_of(combo::PROXY_MODES, &proxy_mode_to_str(settings.proxy.mode)),
        proxy_manual_url: SharedString::from(&settings.proxy.manual_url),
    }
}

/// Apply the setup wizard form onto `settings`. Does not touch the
/// `setup_completed` / `last_setup_step` flags (caller owns those).
pub fn update_app_settings_from_setup_form(
    settings: &mut AppSettings,
    form: &SetupFormData,
    lang: Language,
) -> Result<(), String> {
    // ── 语言 ──
    // A wizard-run language change is explicit, so always write it (even if it
    // equals the current value — harmless and keeps the state predictable).
    settings.appearance.language = combo::value_at(combo::LANGUAGES, form.language_idx).to_string();

    // ── 外观 ──
    match combo::value_at(combo::COLOR_MODES, form.color_mode_idx) {
        "light" => settings.appearance.color_mode = ColorMode::Light,
        "dark" => settings.appearance.color_mode = ColorMode::Dark,
        _ => settings.appearance.color_mode = ColorMode::System,
    }
    match combo::value_at(combo::THEME_COLORS, form.theme_color_idx) {
        "amber" => settings.appearance.theme_color = ThemeColor::Amber,
        "sky" => settings.appearance.theme_color = ThemeColor::Sky,
        _ => settings.appearance.theme_color = ThemeColor::Lime,
    }

    // ── CDN 加速 ──
    settings.cdn_acceleration.enabled = form.cdn_enabled;
    // active_ip is a read-only status field; never overwrite persisted value.

    // ── Aria2 RPC ──
    settings.aria2_rpc.enabled = form.rpc_enabled;
    let port_raw = form.rpc_port.trim();
    if !port_raw.is_empty() {
        let port = port_raw.parse::<u16>().map_err(|_| {
            i18n::format_validation_port(lang, i18n::SettingsField::Aria2Port, port_raw)
        })?;
        if port == 0 {
            return Err(i18n::format_validation_port_zero(
                lang,
                i18n::SettingsField::Aria2Port,
            ));
        }
        settings.aria2_rpc.port = port;
    }
    settings.aria2_rpc.secret = if form.rpc_secret.trim().is_empty() {
        None
    } else {
        Some(form.rpc_secret.trim().to_string())
    };

    // ── 下载目录 ──
    let dir = form.default_dir.trim();
    if !dir.is_empty() {
        settings.download.default_download_dir = dir.to_string();
    }

    // ── 调度预设 ──
    match form.scheduler_preset_idx {
        0 => {
            settings.scheduler.mode = SchedulerMode::Automatic;
            settings.scheduler.automatic = AutomaticSchedulerSettings {
                max_parallel_threads: 8,
                max_threads_per_task: 4,
                min_threads_per_task: 2,
                adaptive_profile: AdaptiveProfile::Conservative,
            };
        }
        1 => {
            settings.scheduler.mode = SchedulerMode::Automatic;
            settings.scheduler.automatic = AutomaticSchedulerSettings {
                max_parallel_threads: 16,
                max_threads_per_task: 8,
                min_threads_per_task: 2,
                adaptive_profile: AdaptiveProfile::Balanced,
            };
        }
        2 => {
            settings.scheduler.mode = SchedulerMode::Automatic;
            settings.scheduler.automatic = AutomaticSchedulerSettings {
                max_parallel_threads: 32,
                max_threads_per_task: 16,
                min_threads_per_task: 4,
                adaptive_profile: AdaptiveProfile::Aggressive,
            };
        }
        _ => {} // custom: keep current values
    }
    match combo::value_at(combo::CHUNK_STRATEGIES, form.chunk_strategy_idx) {
        "fixed" => settings.scheduler.chunk_size_strategy = ChunkSizeStrategy::Fixed,
        _ => settings.scheduler.chunk_size_strategy = ChunkSizeStrategy::Adaptive,
    }

    // ── 系统 ──
    settings.autostart = form.autostart;
    settings.notifications.enabled = form.notifications_enabled;
    if let Some(m) = str_to_proxy_mode(combo::value_at(combo::PROXY_MODES, form.proxy_mode_idx)) {
        settings.proxy.mode = m;
    }
    settings.proxy.manual_url = form.proxy_manual_url.trim().to_string();
    if settings.proxy.mode == ProxyMode::Manual && settings.proxy.manual_url.is_empty() {
        return Err(i18n::format_proxy_url_required(lang));
    }

    Ok(())
}
