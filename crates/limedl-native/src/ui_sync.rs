use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::{
    AppSettings, ColorMode, SortDirection, ThemeColor, UrlRewriteRule,
};

use crate::bridge::{
    SpeedLimitSlotText, TaskStore, app_settings_to_form, app_settings_to_labs_form,
    cdn_candidates_to_slint, column_is_visible, evaluate_url_rewrite, field_to_sort_key,
    format_disk_types_map, format_io_status_json, format_speed,
    speed_limit_slots_from_settings, speed_limit_slots_to_slint, url_rewrite_rules_to_slint,
};
use crate::i18n::{self, Language};
use crate::toast::{ToastQueue, push_toast};
use crate::{
    ColorModePref, MainWindow, Theme, ThemeAccent, UpdateState, migrate, platform_win,
    POWER_GUARD,
};

/// Push the table-density / column-visibility preferences into the window.
pub fn apply_view_preferences(ui: &MainWindow, settings: &AppSettings) {
    let appearance = &settings.appearance;
    ui.set_compact_view(appearance.compact_view);
    // The file-name column is always shown.
    ui.set_column_file(true);
    ui.set_column_size(column_is_visible(&appearance.visible_columns, "size"));
    ui.set_column_downloaded(column_is_visible(&appearance.visible_columns, "downloaded"));
    ui.set_column_status(column_is_visible(&appearance.visible_columns, "status"));
    ui.set_column_progress(column_is_visible(&appearance.visible_columns, "progress"));
    ui.set_column_speed(column_is_visible(&appearance.visible_columns, "speed"));
    ui.set_column_priority(column_is_visible(&appearance.visible_columns, "priority"));
    ui.set_column_upload_speed(column_is_visible(&appearance.visible_columns, "uploadSpeed"));
    ui.set_column_seeds(column_is_visible(&appearance.visible_columns, "seeds"));
    ui.set_column_eta(column_is_visible(&appearance.visible_columns, "eta"));
}

/// Persist a user sort change into `appearance.sortKey` / `sortDirection`.
pub fn persist_sort_preference(
    dispatcher: &Dispatcher,
    current_settings: &Arc<Mutex<AppSettings>>,
    field: i32,
    asc: bool,
) {
    let dispatcher = dispatcher.clone();
    let current_settings = current_settings.clone();
    tokio::spawn(async move {
        let mut settings = current_settings.lock().clone();
        let key = field_to_sort_key(field);
        let dir = if asc {
            SortDirection::Asc
        } else {
            SortDirection::Desc
        };
        if settings.appearance.sort_key == key && settings.appearance.sort_direction == dir {
            return;
        }
        settings.appearance.sort_key = key;
        settings.appearance.sort_direction = dir;
        match dispatcher.save_settings(&settings).await {
            Ok(saved) => *current_settings.lock() = saved,
            Err(e) => tracing::debug!("persisting sort preference failed: {e:#}"),
        }
    });
}

/// Read the schedule editor rows out of the UI model (used before rebuilding or
/// persisting them).
pub fn read_schedule_rows(ui: &MainWindow) -> Vec<SpeedLimitSlotText> {
    ui.get_speed_limit_slots()
        .iter()
        .map(|item| SpeedLimitSlotText {
            start_hour: item.start_hour.to_string(),
            end_hour: item.end_hour.to_string(),
            limit_kb: item.limit_kb.to_string(),
        })
        .collect()
}

/// Decide whether the main window must stay hidden for this launch.
pub fn should_start_hidden(requested_hidden: bool, login_launch: bool, setup_completed: bool) -> bool {
    (requested_hidden || login_launch) && setup_completed
}

/// Time window after logon in which an MSIX launch is attributed to the
/// startup task instead of the user opening the app.
pub const MSIX_LOGIN_LAUNCH_WINDOW: Duration = Duration::from_secs(150);

pub fn refresh_ui(ui: &MainWindow, store: &TaskStore) {
    let (all, downloading, paused, completed, failed) = store.counts();
    POWER_GUARD.update(downloading);
    ui.set_count_all(SharedString::from(all.to_string()));
    ui.set_count_downloading(SharedString::from(downloading.to_string()));
    ui.set_count_paused(SharedString::from(paused.to_string()));
    ui.set_count_completed(SharedString::from(completed.to_string()));
    ui.set_count_failed(SharedString::from(failed.to_string()));

    let total_speed = store.total_speed();
    let speed_text = if total_speed > 0.0 {
        format_speed(Some(total_speed))
    } else {
        "0 B/s".to_string()
    };
    ui.set_global_speed_text(SharedString::from(speed_text));
    ui.set_selected_count(store.selected_count() as i32);
    ui.set_sort_field(store.sort_field());
    ui.set_sort_asc(store.sort_asc());

    let items = store.filtered_items();
    ui.set_tasks(Rc::new(VecModel::from(items)).into());
}

pub fn refresh_settings_state(
    ui: &MainWindow,
    dispatcher: &Dispatcher,
    settings: &AppSettings,
    game_mode: bool,
    overclock_mode: bool,
    lang: Language,
) {
    let io_status_str = match dispatcher.get_io_status() {
        Ok(v) => format_io_status_json(&v, lang),
        Err(_) => i18n::format_io_status_not_ready(lang).to_string(),
    };
    let disk_types = dispatcher.detect_all_disk_types();
    let disk_types_str = format_disk_types_map(&disk_types, lang);
    apply_view_preferences(ui, settings);
    ui.set_speed_limit_slots(ModelRc::new(VecModel::from(speed_limit_slots_to_slint(
        &speed_limit_slots_from_settings(settings),
        lang,
    ))));

    let form_data = app_settings_to_form(
        settings,
        game_mode,
        overclock_mode,
        &io_status_str,
        &disk_types_str,
        lang,
    );

    ui.set_settings_form(form_data);
    ui.set_game_mode_active(game_mode);
    ui.set_overclock_mode_active(overclock_mode);
}

#[allow(clippy::too_many_arguments)]
pub fn refresh_labs_state(
    ui: &MainWindow,
    settings: &AppSettings,
    rules: &[UrlRewriteRule],
    expanded_ids: &HashSet<String>,
    test_url: &str,
    is_testing: bool,
    candidates: &[limedl_core::cdn::speed_test::SpeedTestResult],
    lang: Language,
) {
    let (matched_rule, candidate_urls) = evaluate_url_rewrite(rules, test_url);

    let ranges_text = "173.245.48.0/20, 103.21.244.0/22, 103.22.200.0/22, 103.31.4.0/22, 141.101.64.0/18, 108.162.192.0/18, 190.93.240.0/20, 188.114.96.0/20, 197.234.240.0/22, 198.41.128.0/17, 162.158.0.0/15, 104.16.0.0/13, 104.24.0.0/14, 172.64.0.0/13, 131.0.72.0/22";

    let active_ip_str = settings.cdn_acceleration.active_ip.clone().unwrap_or_default();
    let speed_imp = settings.cdn_acceleration.active_speed_mbps.map(|s| format!("+{:.1}%", (s * 0.75).min(180.0)));

    let form_data = app_settings_to_labs_form(
        settings,
        is_testing,
        i18n::format_cdn_status_label(is_testing, lang),
        if is_testing { 50.0 } else { 100.0 },
        i18n::format_cdn_phase_label(is_testing, lang),
        speed_imp.as_deref(),
        Some("-28.5 ms"),
        Some(i18n::format_cdn_default_node(lang)),
        ranges_text,
        false,
        test_url,
        &matched_rule,
        &candidate_urls,
        lang,
    );

    ui.set_labs_form(form_data);
    ui.set_cdn_candidates(cdn_candidates_to_slint(candidates, &active_ip_str));
    ui.set_rewrite_rules(url_rewrite_rules_to_slint(rules, expanded_ids));
}

/// Push a mutation into the self-update UI state (must run on the UI thread).
pub fn push_update_state(
    ui_weak: &slint::Weak<MainWindow>,
    mutate: impl FnOnce(&mut UpdateState),
) {
    if let Some(ui) = ui_weak.upgrade() {
        let mut st = ui.get_update_state();
        mutate(&mut st);
        ui.set_update_state(st);
    }
}

pub fn set_update_error(ui_weak: &slint::Weak<MainWindow>, msg: &str) {
    push_update_state(ui_weak, |st| {
        st.phase = "error".into();
        st.error_text = msg.into();
    });
}

pub fn restore_and_show_window(ui: &MainWindow, store: Option<&TaskStore>) {
    platform_win::set_window_visible(true);
    let _ = ui.show();
    ui.window().set_minimized(false);
    ui.window().request_redraw();
    if let Some(base_dir) = platform_win::get_base_dir() {
        platform_win::apply_restored_window_placement(
            ui.window(),
            &base_dir,
            ui.window().is_maximized(),
        );
    }
    #[cfg(windows)]
    {
        platform_win::try_install_window_hooks(ui.window());
    }
    if let Some(s) = store {
        refresh_ui(ui, s);
    }
    platform_win::bring_to_foreground(ui.window());
}

pub fn schedule_window_placement_restore(ui: &MainWindow, base_dir: &std::path::Path) {
    if platform_win::apply_restored_window_placement(
        ui.window(),
        base_dir,
        ui.window().is_maximized(),
    ) {
        return;
    }

    let weak = ui.as_weak();
    let base_dir = base_dir.to_path_buf();
    let timer = Rc::new(slint::Timer::default());
    let timer_for_cb = timer.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(250),
        move || {
            let Some(ui) = weak.upgrade() else {
                timer_for_cb.stop();
                return;
            };
            if platform_win::apply_restored_window_placement(
                ui.window(),
                &base_dir,
                ui.window().is_maximized(),
            ) {
                timer_for_cb.stop();
            }
        },
    );
}

pub fn apply_appearance(ui: &MainWindow, mode: ColorMode, theme_color: ThemeColor) {
    let pref = match mode {
        ColorMode::System => ColorModePref::System,
        ColorMode::Light => ColorModePref::Light,
        ColorMode::Dark => ColorModePref::Dark,
    };
    let accent = match theme_color {
        ThemeColor::Lime => ThemeAccent::Lime,
        ThemeColor::Amber => ThemeAccent::Amber,
        ThemeColor::Sky => ThemeAccent::Sky,
    };
    ui.global::<Theme>().set_mode(pref);
    ui.global::<Theme>().set_accent(accent);
    let is_dark = ui.global::<Theme>().get_dark();
    platform_win::sync_window_theme(ui.window(), is_dark);
}

/// Push a one-shot toast telling the user that data was migrated from the
/// Tauri edition (shown once, right after the window exists).
pub fn announce_migration(
    report: &migrate::MigrationReport,
    ui_weak: &slint::Weak<MainWindow>,
    queue: &ToastQueue,
    lang: Language,
) {
    let msg = i18n::format_toast_tauri_migration(report.copied_files, lang);
    push_toast(ui_weak, queue, msg, "info", Duration::from_secs(8));
}
