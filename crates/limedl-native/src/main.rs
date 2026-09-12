#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod autostart;
mod bridge;
mod i18n;
mod migrate;
mod platform_win;
mod power;
mod protocol;
mod single_instance;
mod update;

use power::PowerGuard;

static POWER_GUARD: std::sync::LazyLock<PowerGuard> = std::sync::LazyLock::new(PowerGuard::new);

use std::collections::HashSet;
use std::net::IpAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tokio::sync::watch;
use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use notify_rust::Notification;
use parking_lot::Mutex;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, CloseRequestResponse};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

use limedl_core::aria2_rpc::Aria2RpcServer;
use limedl_core::bootstrap::bootstrap;
use limedl_core::dispatcher::Dispatcher;
use limedl_core::event_bus::DownloadEvent;
use limedl_core::types::{
    AppSettings, ChecksumMode, CloseBehavior, ColorMode, DoubleClickOnCompleted,
    DoubleClickOnUncompleted, DownloadProgress, DownloadState, DownloadSummary, MatchType,
    ReplacementMode, RewriteTarget, SortDirection, StartDownloadRequest, TaskId, ThemeColor,
    TorrentFileEntry, UrlRewriteRule,
};

use crate::bridge::{
    SortField, SpeedLimitSlotText, TaskStore, app_settings_to_form, app_settings_to_labs_form,
    app_settings_to_setup_form, cdn_candidates_to_slint, column_is_visible,
    create_url_rewrite_preset, evaluate_url_rewrite, field_to_sort_key, file_status_to_item,
    format_disk_types_map, format_io_status_json, format_speed, format_timestamp_ms,
    generate_piece_map_image, parse_speed_limit_slots, peer_info_to_item, sort_key_to_field,
    speed_limit_slots_from_settings, speed_limit_slots_to_slint, str_to_match_type,
    str_to_priority, str_to_replacement_mode, summary_to_inspector_info, torrent_entry_to_item,
    tracker_info_to_item, update_app_settings_from_form, update_app_settings_from_labs_form,
    update_app_settings_from_setup_form, url_rewrite_rules_to_slint,
};
use crate::i18n::Language;

// ── In-app toast notifications ───────────────────────────────────────

/// One pending in-app toast (auto-expires after `duration`).
struct ToastEntry {
    id: usize,
    message: String,
    kind: &'static str, // success / error / warning / info
}

type ToastQueue = Arc<Mutex<Vec<ToastEntry>>>;

static TOAST_SEQ: AtomicUsize = AtomicUsize::new(1);

/// Push the current queue contents into the Slint property (any thread).
fn sync_toasts(ui_weak: &slint::Weak<MainWindow>, queue: &ToastQueue) {
    let items: Vec<ToastItem> = queue
        .lock()
        .iter()
        .map(|e| ToastItem {
            id: e.id as i32,
            message: SharedString::from(e.message.as_str()),
            kind: SharedString::from(e.kind),
        })
        .collect();
    let weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_toasts(Rc::new(VecModel::from(items)).into());
        }
    });
}

/// Show an in-app toast; auto-dismisses after `duration`. Safe from any thread.
fn push_toast(
    ui_weak: &slint::Weak<MainWindow>,
    queue: &ToastQueue,
    message: String,
    kind: &'static str,
    duration: Duration,
) {
    let id = TOAST_SEQ.fetch_add(1, Ordering::Relaxed);
    queue.lock().push(ToastEntry {
        id,
        message,
        kind,
    });
    sync_toasts(ui_weak, queue);
    let ui_weak = ui_weak.clone();
    let queue = queue.clone();
    tokio::spawn(async move {
        tokio::time::sleep(duration).await;
        queue.lock().retain(|e| e.id != id);
        sync_toasts(&ui_weak, &queue);
    });
}

/// Dismiss a toast immediately (from the UI close button).
fn dismiss_toast(ui_weak: &slint::Weak<MainWindow>, queue: &ToastQueue, id: i32) {
    queue.lock().retain(|e| e.id != id as usize);
    sync_toasts(ui_weak, queue);
}

/// Collapse duplicate download warnings into a single toast.
///
/// Core emits warnings per peer (anti-leech bans), per mirror failover and per
/// disk-space check, so an unfiltered feed would flood the toast stack.
struct WarningDedup {
    last_key: String,
    last_at: Option<std::time::Instant>,
}

impl WarningDedup {
    const WINDOW: Duration = Duration::from_secs(5);

    fn new() -> Self {
        Self {
            last_key: String::new(),
            last_at: None,
        }
    }

    /// Returns `true` when the warning is new enough to be worth surfacing.
    fn should_show(&mut self, id: &str, message: &str) -> bool {
        let key = format!("{id}:{message}");
        let now = std::time::Instant::now();
        let is_duplicate = key == self.last_key
            && self
                .last_at
                .is_some_and(|prev| now.duration_since(prev) < Self::WINDOW);
        self.last_key = key;
        self.last_at = Some(now);
        !is_duplicate
    }
}

/// Push the table-density / column-visibility preferences into the window.
///
/// Kept next to `refresh_settings_state` so a settings save immediately
/// re-layouts the list without waiting for the next task event.
fn apply_view_preferences(ui: &MainWindow, settings: &AppSettings) {
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
///
/// Failures are non-fatal (the in-memory sort already applied) and only logged:
/// a settings write error must never block list interaction.
fn persist_sort_preference(
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
fn read_schedule_rows(ui: &MainWindow) -> Vec<SpeedLimitSlotText> {
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
///
/// `requested_hidden` comes from `--hidden` (registry / `.desktop` /
/// LaunchAgent autostart) and `login_launch` from the MSIX login-time heuristic,
/// because `windows.startupTask` cannot pass arguments. The first-run wizard
/// always wins so a fresh install can never end up window-less.
fn should_start_hidden(requested_hidden: bool, login_launch: bool, setup_completed: bool) -> bool {
    (requested_hidden || login_launch) && setup_completed
}

/// Time window after logon in which an MSIX launch is attributed to the
/// startup task instead of the user opening the app.
const MSIX_LOGIN_LAUNCH_WINDOW: Duration = Duration::from_secs(150);

fn refresh_ui(ui: &MainWindow, store: &TaskStore) {
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

fn refresh_settings_state(
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
fn refresh_labs_state(
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
fn push_update_state(
    ui_weak: &slint::Weak<MainWindow>,
    mutate: impl FnOnce(&mut UpdateState),
) {
    if let Some(ui) = ui_weak.upgrade() {
        let mut st = ui.get_update_state();
        mutate(&mut st);
        ui.set_update_state(st);
    }
}

fn set_update_error(ui_weak: &slint::Weak<MainWindow>, msg: &str) {
    push_update_state(ui_weak, |st| {
        st.phase = "error".into();
        st.error_text = msg.into();
    });
}

fn build_tray_menu(lang: Language, speed_limit_active: bool) -> Menu {
    let t = i18n::get_tray_strings(lang);
    let tray_menu = Menu::new();
    let menu_show = MenuItem::with_id("show", t.show_window, true, None);
    let sep1 = PredefinedMenuItem::separator();
    let menu_pause_all = MenuItem::with_id("pause_all", t.pause_all, true, None);
    let menu_resume_all = MenuItem::with_id("resume_all", t.resume_all, true, None);
    let menu_speed_limit = CheckMenuItem::with_id(
        "speed_limit",
        t.speed_limit_toggle,
        true,
        speed_limit_active,
        None,
    );
    let menu_game_mode = MenuItem::with_id("game_mode", t.game_mode_toggle, true, None);
    let menu_open_dir = MenuItem::with_id("open_dir", t.open_download_dir, true, None);
    let sep2 = PredefinedMenuItem::separator();
    let menu_quit = MenuItem::with_id("quit", t.quit, true, None);

    let _ = tray_menu.append_items(&[
        &menu_show,
        &sep1,
        &menu_pause_all,
        &menu_resume_all,
        &menu_speed_limit,
        &menu_game_mode,
        &menu_open_dir,
        &sep2,
        &menu_quit,
    ]);
    tray_menu
}

/// Speed applied when the user enables the tray "speed limit" shortcut.
/// Kept at the historical 1 MiB/s default.
const TRAY_SPEED_LIMIT_BPS: u64 = 1_048_576;

/// Open the new task dialog and pre-fill / trigger actions based on an incoming payload
/// (e.g. from Drag-and-Drop, secondary instance WM_COPYDATA IPC, or cold CLI argument).
fn open_new_task_with_payload(
    raw_payload: &str,
    ui_weak: &slint::Weak<MainWindow>,
    dispatcher: &Arc<Dispatcher>,
    store: &Arc<Mutex<TaskStore>>,
    entries_cache: &Arc<Mutex<Vec<TorrentFileEntry>>>,
    included_cache: &Arc<Mutex<Vec<bool>>>,
) {
    let payload = raw_payload.trim().trim_matches('"').trim_matches('\'').trim();
    if payload.is_empty() {
        return;
    }

    // Check limedl:// deep link scheme
    let normalized = if let Some(stripped) = payload.strip_prefix("limedl://") {
        let after = stripped.trim_start_matches('/');
        if let Some(url_part) = after.strip_prefix("download?url=") {
            percent_encoding::percent_decode_str(url_part)
                .decode_utf8_lossy()
                .to_string()
        } else if let Some(magnet_part) = after.strip_prefix("magnet:") {
            format!("magnet:{magnet_part}")
        } else {
            after.to_string()
        }
    } else {
        payload.to_string()
    };

    let normalized = normalized.trim().to_string();

    // Check if it's a local .torrent file
    let path_candidate = if let Some(file_url) = normalized.strip_prefix("file:///") {
        file_url.to_string()
    } else if let Some(file_url) = normalized.strip_prefix("file://") {
        file_url.to_string()
    } else {
        normalized.clone()
    };

    let p = std::path::Path::new(&path_candidate);
    let is_torrent_file = p.is_file()
        && p.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("torrent"))
            .unwrap_or(false);

    if is_torrent_file {
        let full_path = p
            .canonicalize()
            .unwrap_or_else(|_| p.to_path_buf())
            .to_string_lossy()
            .to_string();
        let file_name = p
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        let ui_weak_cl = ui_weak.clone();
        let lang = store.lock().language();
        let path_for_ui = full_path.clone();

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak_cl.upgrade() {
                ui.set_new_task_url(SharedString::from(&path_for_ui));
                if !file_name.is_empty() {
                    ui.set_new_task_filename(SharedString::from(&file_name));
                }
                ui.set_new_task_batch_mode(false);
                ui.set_new_task_preview_state("loading".into());
                ui.set_new_task_preview_status_text(SharedString::from(
                    i18n::format_preview_status("loading", "", lang),
                ));
                ui.set_new_task_preview_summary_text(SharedString::default());
                ui.set_new_task_torrent_files(ModelRc::default());
                ui.set_show_new_task_dialog(true);
            }
        });

        let dispatcher = dispatcher.clone();
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let entries_cache = entries_cache.clone();
        let included_cache = included_cache.clone();

        tokio::spawn(async move {
            let preview = dispatcher.bt_preview_torrent(&full_path).await;
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = ui_weak.upgrade() else {
                    return;
                };
                let lang = store.lock().language();
                match preview {
                    Ok(entries) => {
                        let total_bytes: u64 = entries.iter().map(|e| e.size).sum();
                        *entries_cache.lock() = entries.clone();
                        *included_cache.lock() = vec![true; entries.len()];
                        let items: Vec<NewTaskTorrentFileItem> = entries
                            .iter()
                            .map(|e| torrent_entry_to_item(e, true))
                            .collect();
                        ui.set_new_task_torrent_files(Rc::new(VecModel::from(items)).into());
                        ui.set_new_task_preview_state("ready".into());
                        ui.set_new_task_preview_status_text(SharedString::default());
                        ui.set_new_task_preview_summary_text(SharedString::from(
                            i18n::format_preview_summary(
                                entries.len(),
                                &bridge::format_bytes(total_bytes),
                                lang,
                            ),
                        ));
                    }
                    Err(err) => {
                        entries_cache.lock().clear();
                        included_cache.lock().clear();
                        ui.set_new_task_torrent_files(ModelRc::default());
                        ui.set_new_task_preview_state("error".into());
                        ui.set_new_task_preview_status_text(SharedString::from(
                            i18n::format_preview_status("error", &err.to_string(), lang),
                        ));
                    }
                }
            });
        });
        return;
    }

    // Check magnet link
    if normalized.starts_with("magnet:?") {
        let magnet_url = normalized.clone();
        let mut display_name = String::new();
        if let Some(dn_start) = magnet_url.find("dn=") {
            let slice = &magnet_url[dn_start + 3..];
            let dn_raw = slice.split('&').next().unwrap_or("");
            display_name = percent_encoding::percent_decode_str(dn_raw)
                .decode_utf8_lossy()
                .to_string();
        }

        let ui_weak = ui_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_new_task_url(SharedString::from(&magnet_url));
                if !display_name.is_empty() {
                    ui.set_new_task_filename(SharedString::from(&display_name));
                }
                ui.set_new_task_batch_mode(false);
                ui.set_new_task_probe_state("idle".into());
                ui.set_new_task_probe_status_text(SharedString::default());
                ui.set_new_task_probe_hash(SharedString::default());
                ui.set_new_task_preview_state("none".into());
                ui.set_new_task_preview_status_text(SharedString::default());
                ui.set_new_task_preview_summary_text(SharedString::default());
                ui.set_new_task_torrent_files(ModelRc::default());
                ui.set_show_new_task_dialog(true);
            }
        });
        return;
    }

    // Check multiple lines or URLs via clipboard parser
    match crate::bridge::parse_clipboard_download_text(&normalized) {
        crate::bridge::ClipboardPayload::BatchUrls(urls) => {
            let joined = urls.join("\n");
            let count = urls.len();
            let ui_weak = ui_weak.clone();
            let store = store.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let lang = store.lock().language();
                    ui.set_new_task_batch_text(SharedString::from(&joined));
                    ui.set_new_task_batch_mode(true);
                    ui.set_new_task_batch_count_text(SharedString::from(
                        i18n::format_batch_count(count, lang),
                    ));
                    ui.set_new_task_batch_submitting(false);
                    ui.set_new_task_batch_status_text(SharedString::default());
                    ui.set_show_new_task_dialog(true);
                }
            });
        }
        crate::bridge::ClipboardPayload::SingleUrl(url) => {
            let ui_weak_cl = ui_weak.clone();
            let url_cl = url.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_cl.upgrade() {
                    ui.set_new_task_url(SharedString::from(&url_cl));
                    ui.set_new_task_batch_mode(false);
                    ui.set_new_task_probe_state("idle".into());
                    ui.set_new_task_probe_status_text(SharedString::default());
                    ui.set_new_task_probe_hash(SharedString::default());
                    ui.set_new_task_preview_state("none".into());
                    ui.set_new_task_preview_status_text(SharedString::default());
                    ui.set_new_task_preview_summary_text(SharedString::default());
                    ui.set_new_task_torrent_files(ModelRc::default());
                    ui.set_show_new_task_dialog(true);
                }
            });

            // If auto-detect sha256 or HTTP, probe checksum
            if url.starts_with("http://") || url.starts_with("https://") {
                let dispatcher = dispatcher.clone();
                let ui_weak = ui_weak.clone();
                let store = store.clone();
                tokio::spawn(async move {
                    let detected = dispatcher
                        .probe_checksum(&url, None)
                        .await
                        .ok()
                        .flatten();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let lang = store.lock().language();
                            match detected {
                                Some(hash) => {
                                    ui.set_new_task_probe_state("found".into());
                                    ui.set_new_task_probe_status_text(SharedString::from(
                                        i18n::format_probe_status("found", &hash, lang),
                                    ));
                                    ui.set_new_task_probe_hash(SharedString::from(hash));
                                }
                                None => {
                                    ui.set_new_task_probe_state("missing".into());
                                    ui.set_new_task_probe_status_text(SharedString::from(
                                        i18n::format_probe_status("missing", "", lang),
                                    ));
                                }
                            }
                        }
                    });
                });
            }
        }
        crate::bridge::ClipboardPayload::Empty => {
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_new_task_url(SharedString::from(&normalized));
                    ui.set_new_task_batch_mode(false);
                    ui.set_show_new_task_dialog(true);
                }
            });
        }
    }
}

/// Push the saved appearance (color mode + theme accent) into the Slint Theme
/// global. 'System' mode resolves at render time against the OS scheme via the
/// std Palette.
fn apply_appearance(ui: &MainWindow, mode: ColorMode, theme_color: ThemeColor) {
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
}

fn create_default_tray_icon() -> tray_icon::Icon {
    const ICON_BYTES: &[u8] = include_bytes!("../ui/assets/32x32.png");
    if let Ok(dyn_img) = image::load_from_memory_with_format(ICON_BYTES, image::ImageFormat::Png) {
        let rgba = dyn_img.to_rgba8();
        let (width, height) = rgba.dimensions();
        if let Ok(icon) = tray_icon::Icon::from_rgba(rgba.into_raw(), width, height) {
            return icon;
        }
    }

    // Fallback: 32x32 RGBA icon
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let is_l = ((8..=12).contains(&x) && (6..=26).contains(&y))
                || ((8..=24).contains(&x) && (22..=26).contains(&y));
            if is_l {
                rgba.extend_from_slice(&[132, 204, 22, 255]);
            } else {
                rgba.extend_from_slice(&[24, 27, 31, 230]);
            }
        }
    }
    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).unwrap()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // CLI contract: `limedl-native [--hidden] [<url|magnet|path|limedl://…>]`
    // `--hidden` is what the autostart registration passes so a login start goes
    // straight to the tray; it is a flag, never a payload.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let hidden_flag = args
        .iter()
        .any(|a| matches!(a.as_str(), "--hidden" | "-hidden" | "--minimized"));
    let cli_payload = args
        .iter()
        .find(|a| !matches!(a.as_str(), "--hidden" | "-hidden" | "--minimized"))
        .cloned();

    // Single-instance guard: a second launch activates the existing window
    // and exits before any engine/bootstrap work happens.
    let instance_claim = single_instance::InstanceClaim::claim();
    if instance_claim.is_secondary() {
        instance_claim.notify_primary(cli_payload.as_deref());
        return Ok(());
    }

    // NOTE: do NOT install a global tracing subscriber here — core's
    // init_logging() owns it (registry + reloadable level filter + console +
    // file layers). Pre-installing one makes init_logging's try_init fail,
    // drops its reload layer and turns every settings save after the first
    // into "failed to update tracing level filter".

    // Initialize core subsystems
    let base_dir = dirs_or_temp_dir();
    let state_dir = base_dir.join("downloads");
    // Remove stale artifacts from a previous (possibly interrupted) self-update.
    update::clean_update_work_dir(&base_dir);
    std::fs::create_dir_all(&state_dir)?;
    // ── First Native run: migrate data from the Tauri edition ──
    // Copies settings.json, downloads.db (+WAL/SHM), torrents/ and bt_files/ so
    // a user switching editions keeps their settings and task history.
    let migration_report = migrate::migrate_tauri_data_if_needed(&base_dir, &state_dir);

    let core = bootstrap(state_dir.clone())
        .await
        .with_context(|| "初始化核心引擎失败")?;

    let initial_settings = core
        .dispatcher
        .get_settings()
        .await
        .unwrap_or_default();
    // Own the global tracing subscriber (console + file + reloadable level).
    // Call BEFORE any other work so bootstrap-time settings load logs land in
    // the log file too. Saved log level/path apply immediately.
    limedl_core::init_logging(&initial_settings.logging, &state_dir)
        .with_context(|| "初始化日志失败")?;
    tracing::info!("启动 limedl Native 桌面客户端 (Skia)...");
    // The migration runs before `init_logging` (it supplies the settings that
    // configure logging), so report its outcome here where it is visible.
    if let Some(report) = migration_report.as_ref() {
        tracing::info!(
            "Tauri 数据迁移完成：{} 个文件 / {} 字节（来源 {}）",
            report.copied_files,
            report.copied_bytes,
            report.source.display()
        );
    }
    let cdn_accelerator = core.cdn_service.accelerator().clone();
    core.download_manager.set_cdn_accelerator(cdn_accelerator);
    core.cdn_service.init_from_settings(&initial_settings).await;
    let current_settings = Arc::new(Mutex::new(initial_settings.clone()));
    // Sync OS autostart registration with persisted flag (no-op if already consistent)
    autostart::sync_from_settings(initial_settings.autostart);
    // Aria2 RPC: start server if enabled in persisted settings
    let rpc_shutdown: Arc<Mutex<Option<watch::Sender<bool>>>> = Arc::new(Mutex::new(None));
    if initial_settings.aria2_rpc.enabled {
        let (tx, rx) = watch::channel(false);
        let rpc_server = Aria2RpcServer::new(
            core.registry.clone(),
            &initial_settings.aria2_rpc,
            core.event_bus.clone(),
        );
        let cors = initial_settings.aria2_rpc.cors_allowed_origins.clone();
        tokio::spawn(async move {
            if let Err(e) = rpc_server.serve(rx, cors).await {
                tracing::error!("Aria2 RPC server stopped: {e:#}");
            }
        });
        *rpc_shutdown.lock() = Some(tx);
        tracing::info!(
            "Aria2 RPC 已启动 (port: {})",
            initial_settings.aria2_rpc.port
        );
    }
    let initial_lang = Language::from_code(&initial_settings.appearance.language);

    let default_download_dir = if !initial_settings.download.default_download_dir.is_empty() {
        initial_settings.download.default_download_dir.clone()
    } else {
        core.dispatcher
            .default_download_dir()
            .await
            .unwrap_or_else(|| state_dir.to_string_lossy().to_string())
    };

    let game_mode_active = Arc::new(Mutex::new(core.dispatcher.game_mode()));
    let overclock_mode_active = Arc::new(Mutex::new(core.dispatcher.get_overclock_mode()));

    // Create Main Window
    let main_window = MainWindow::new()?;
    // Slint requires select_bundled_translation() to be called AFTER the first
    // component is created (the translation bundle is registered during
    // creation and auto-selects the system locale). Calling it earlier returns
    // `NoTranslationsBundled` and the saved language would be ignored.
    i18n::apply_translation(initial_lang);

    // Self-update UI state: record the distribution channel once at startup.
    let install_kind = update::detect_install_kind();
    main_window.set_update_state(UpdateState {
        phase: "idle".into(),
        latest_version: "".into(),
        notes: "".into(),
        progress_percent: 0.0,
        progress_label: "".into(),
        error_text: "".into(),
        install_kind: match install_kind {
            update::InstallKind::Store => "store".into(),
            update::InstallKind::Installer => "installer".into(),
            update::InstallKind::Portable => "portable".into(),
        },
    });
    apply_appearance(
        &main_window,
        initial_settings.appearance.color_mode.clone(),
        initial_settings.appearance.theme_color.clone(),
    );
    main_window.set_default_download_dir(SharedString::from(&default_download_dir));
    main_window.set_new_task_dir(SharedString::from(&default_download_dir));


    let store = Arc::new(Mutex::new(TaskStore::with_language(initial_lang)));
    // Restore the persisted sort order before the first list render.
    {
        let mut s = store.lock();
        s.apply_sort(
            sort_key_to_field(initial_settings.appearance.sort_key),
            matches!(initial_settings.appearance.sort_direction, SortDirection::Asc),
        );
    }
    let active_inspector_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let toast_queue: ToastQueue = Arc::new(Mutex::new(Vec::new()));

    // Tell the user (once) when this run imported data from the Tauri edition.
    if let Some(report) = migration_report.as_ref() {
        announce_migration(report, &main_window.as_weak(), &toast_queue, initial_lang);
    }

    let rewrite_rules: Arc<Mutex<Vec<UrlRewriteRule>>> =
        Arc::new(Mutex::new(initial_settings.url_rewrite.rules.clone()));
    let expanded_rule_ids: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let sandbox_test_url: Arc<Mutex<String>> =
        Arc::new(Mutex::new("https://raw.github.com/user/repo/master/README.md".to_string()));
    let cdn_candidates_cache: Arc<Mutex<Vec<limedl_core::cdn::speed_test::SpeedTestResult>>> =
        Arc::new(Mutex::new(Vec::new()));
    // Torrent file pre-selection cache for the new-task dialog: the previewed
    // entries plus a parallel per-file `included` flag vec (position-aligned).
    let new_task_torrent_entries: Arc<Mutex<Vec<TorrentFileEntry>>> =
        Arc::new(Mutex::new(Vec::new()));
    let new_task_torrent_included: Arc<Mutex<Vec<bool>>> = Arc::new(Mutex::new(Vec::new()));

    // Initialize UI settings state
    refresh_settings_state(
        &main_window,
        &core.dispatcher,
        &initial_settings,
        *game_mode_active.lock(),
        *overclock_mode_active.lock(),
        initial_lang,
    );

    // First-run setup wizard: populate the form and show it if the user has
    // not completed setup yet (or resumes from an interrupted run).
    main_window.set_setup_form(app_settings_to_setup_form(&initial_settings, initial_lang));
    if !initial_settings.setup_completed {
        let start_step = initial_settings.last_setup_step.unwrap_or(0).min(8) as i32;
        main_window.set_setup_start_step(start_step);
        main_window.set_show_setup_wizard(true);
        tracing::info!("首次启动：显示设置向导 (从步骤 {start_step} 恢复)");
    }

    // Initialize Labs UI state
    refresh_labs_state(
        &main_window,
        &initial_settings,
        &rewrite_rules.lock(),
        &expanded_rule_ids.lock(),
        &sandbox_test_url.lock(),
        false,
        &cdn_candidates_cache.lock(),
        initial_lang,
    );

    // System Tray Setup
    // Pending update info shared between the update callbacks and the background check.
    let available_update: Arc<Mutex<Option<update::AvailableUpdate>>> = Arc::new(Mutex::new(None));

    // Single-instance activate requests from secondary launches: show window and open payload.
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = core.dispatcher.clone();
        let store = store.clone();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();
        instance_claim.listen_for_activate(move |payload| {
            let ui_weak_cl = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_cl.upgrade() {
                    let _ = ui.show();
                }
            });
            if let Some(p) = payload {
                open_new_task_with_payload(
                    &p,
                    &ui_weak,
                    &dispatcher,
                    &store,
                    &entries_cache,
                    &included_cache,
                );
            }
        });
    }

    // Windows native integrations: Drag & Drop + WM_COPYDATA IPC + HKCU protocol association
    #[cfg(windows)]
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = core.dispatcher.clone();
        let store = store.clone();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();

        let ui_weak_drop = ui_weak.clone();
        let dispatcher_drop = dispatcher.clone();
        let store_drop = store.clone();
        let entries_cache_drop = entries_cache.clone();
        let included_cache_drop = included_cache.clone();

        let on_drop = move |files: Vec<String>| {
            if files.len() == 1 {
                open_new_task_with_payload(
                    &files[0],
                    &ui_weak_drop,
                    &dispatcher_drop,
                    &store_drop,
                    &entries_cache_drop,
                    &included_cache_drop,
                );
            } else if !files.is_empty() {
                let torrent_count = files
                    .iter()
                    .filter(|f| f.to_lowercase().ends_with(".torrent"))
                    .count();
                if torrent_count == 1
                    && let Some(tf) = files.iter().find(|f| f.to_lowercase().ends_with(".torrent"))
                {
                    open_new_task_with_payload(
                        tf,
                        &ui_weak_drop,
                        &dispatcher_drop,
                        &store_drop,
                        &entries_cache_drop,
                        &included_cache_drop,
                    );
                    return;
                }
                let joined = files.join("\n");
                let count = files.len();
                let ui_weak = ui_weak_drop.clone();
                let store = store_drop.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let lang = store.lock().language();
                        ui.set_new_task_batch_text(SharedString::from(&joined));
                        ui.set_new_task_batch_mode(true);
                        ui.set_new_task_batch_count_text(SharedString::from(
                            i18n::format_batch_count(count, lang),
                        ));
                        ui.set_new_task_batch_submitting(false);
                        ui.set_new_task_batch_status_text(SharedString::default());
                        ui.set_show_new_task_dialog(true);
                    }
                });
            }
        };

        let on_copydata = move |text: String| {
            open_new_task_with_payload(
                &text,
                &ui_weak,
                &dispatcher,
                &store,
                &entries_cache,
                &included_cache,
            );
        };

        // The native window handle only exists once Slint created the OS window,
        // which happens when it is shown. Installing the subclass before that
        // failed with "获取原生窗口句柄失败" and left drag-and-drop and the
        // WM_COPYDATA IPC dead, so retry until the handle is available. The
        // timer keeps running while the app starts hidden (`--hidden`) and stops
        // as soon as the hooks are in place.
        {
            platform_win::set_callbacks(on_drop, on_copydata);

            let ui_weak = main_window.as_weak();
            let hook_timer = Rc::new(slint::Timer::default());
            let timer_for_cb = hook_timer.clone();
            hook_timer.start(
                slint::TimerMode::Repeated,
                Duration::from_millis(250),
                move || {
                    let Some(ui) = ui_weak.upgrade() else {
                        timer_for_cb.stop();
                        return;
                    };
                    if platform_win::try_install_window_hooks(ui.window()) {
                        timer_for_cb.stop();
                    }
                },
            );
        }

        // Auto-register magnet:? and limedl:// protocols in HKCU (no admin privileges required)
        let _ = protocol::register_protocols();
    }

    // Smart background clipboard monitor: detect newly copied download URLs
    {
        let ui_weak = main_window.as_weak();
        let toast_queue_clone = toast_queue.clone();
        let store_clone = store.clone();
        tokio::spawn(async move {
            let mut last_clipboard = String::new();
            if let Ok(mut cb) = arboard::Clipboard::new()
                && let Ok(text) = cb.get_text()
            {
                last_clipboard = text.trim().to_string();
            }

            let mut interval = tokio::time::interval(Duration::from_millis(1500));
            loop {
                interval.tick().await;
                let Ok(mut cb) = arboard::Clipboard::new() else {
                    continue;
                };
                let Ok(text) = cb.get_text() else {
                    continue;
                };
                let text = text.trim().to_string();
                if text.is_empty() || text == last_clipboard {
                    continue;
                }
                last_clipboard = text.clone();

                match crate::bridge::parse_clipboard_download_text(&text) {
                    crate::bridge::ClipboardPayload::SingleUrl(url) => {
                        let lang = store_clone.lock().language();
                        let display_url = if url.len() > 50 {
                            format!("{}...", &url[..47])
                        } else {
                            url
                        };
                        let msg = i18n::format_detected_link(&display_url, lang);
                        push_toast(&ui_weak, &toast_queue_clone, msg, "info", Duration::from_secs(5));
                    }
                    crate::bridge::ClipboardPayload::BatchUrls(urls) => {
                        let lang = store_clone.lock().language();
                        let count = urls.len();
                        let msg = i18n::format_detected_batch(count, lang);
                        push_toast(&ui_weak, &toast_queue_clone, msg, "info", Duration::from_secs(5));
                    }
                    crate::bridge::ClipboardPayload::Empty => {}
                }
            }
        });
    }

    // Mirror of the persisted global speed limit, used for the tray checkmark.
    // Updated on every settings save and by the tray "speed limit" toggle.
    let tray_speed_limit_active = Arc::new(AtomicBool::new(
        initial_settings.global_speed_limit_bps > 0,
    ));

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(build_tray_menu(
            initial_lang,
            tray_speed_limit_active.load(Ordering::Relaxed),
        )))
        .with_tooltip(i18n::get_tray_strings(initial_lang).tooltip)
        .with_icon(create_default_tray_icon())
        .build()?;

    // Channel for signalling tray menu/tooltip updates from async tasks (TrayIcon is !Send)
    let pending_tray_lang: Arc<Mutex<Option<Language>>> = Arc::new(Mutex::new(None));

    // Load initial tasks from SQLite via Dispatcher
    if let Ok(initial_tasks) = core.dispatcher.list().await {
        let mut s = store.lock();
        s.replace_all(initial_tasks);
        refresh_ui(&main_window, &s);
    }

    // Subscribe to EventBus and stream updates into Slint event loop
    {
        let mut rx = core.event_bus.subscribe();
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let active_inspector_id_clone = active_inspector_id.clone();
        let toast_queue_clone = toast_queue.clone();
        let current_settings_clone = current_settings.clone();

        tokio::spawn(async move {
            let mut warning_dedup = WarningDedup::new();
            while let Ok(event) = rx.recv().await {
                let store = store_clone.clone();
                let ui_weak = ui_weak.clone();
                let active_inspector_id = active_inspector_id_clone.clone();
                let toast_queue = toast_queue_clone.clone();
                let current_settings = current_settings_clone.clone();

                match event {
                    DownloadEvent::Updated { summary_json, .. } => {
                        if let Ok(summary) =
                            serde_json::from_value::<DownloadSummary>(summary_json)
                        {
                            let current_lang = store.lock().language();
                            let notif_enabled = current_settings.lock().notifications.enabled;
                            // OS notification + in-app toast on completion or failure
                            if matches!(summary.state, DownloadState::Completed) {
                                if notif_enabled {
                                    let (title, body) = i18n::format_notification_completed(
                                        &summary.file_name,
                                        current_lang,
                                    );
                                    let mut notif = Notification::new();
                                    notif.appname("limedl")
                                        .summary(&title)
                                        .body(&body);
                                    #[cfg(windows)]
                                    {
                                        notif.app_id("limedl");
                                    }
                                    let _ = notif.show();
                                }
                                push_toast(
                                    &ui_weak,
                                    &toast_queue,
                                    i18n::format_toast_state(&summary.file_name, &summary.state, current_lang),
                                    "success",
                                    Duration::from_secs(5),
                                );
                            } else if matches!(summary.state, DownloadState::Failed) {
                                if notif_enabled {
                                    let (title, body) = i18n::format_notification_failed(
                                        &summary.file_name,
                                        summary.error.as_deref(),
                                        current_lang,
                                    );
                                    let mut notif = Notification::new();
                                    notif.appname("limedl")
                                        .summary(&title)
                                        .body(&body);
                                    #[cfg(windows)]
                                    {
                                        notif.app_id("limedl");
                                    }
                                    let _ = notif.show();
                                }
                                push_toast(
                                    &ui_weak,
                                    &toast_queue,
                                    i18n::format_toast_state(&summary.file_name, &summary.state, current_lang),
                                    "error",
                                    Duration::from_secs(6),
                                );
                            }

                            let summary_clone = summary.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    let mut s = store.lock();
                                    let lang = s.language();
                                    s.insert_or_update(summary_clone.clone());
                                    refresh_ui(&ui, &s);

                                    // Refresh Inspector if this task is currently viewed
                                    if let Some(ref current_id) = *active_inspector_id.lock()
                                        && current_id == &summary_clone.id
                                    {
                                        ui.set_inspector_info(summary_to_inspector_info(&summary_clone, lang));
                                    }
                                }
                            });
                        }
                    }
                    DownloadEvent::Progress { progress_json, .. } => {
                        if let Ok(progress) =
                            serde_json::from_value::<DownloadProgress>(progress_json)
                        {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    let mut s = store.lock();
                                    let lang = s.language();
                                    s.update_progress(&progress);
                                    refresh_ui(&ui, &s);

                                    // Refresh Inspector progress if active
                                    if let Some(ref current_id) = *active_inspector_id.lock()
                                        && current_id == &progress.id
                                        && let Some(summary) = s.get_summary(current_id)
                                    {
                                        ui.set_inspector_info(summary_to_inspector_info(&summary, lang));
                                    }
                                }
                            });
                        }
                    }
                    DownloadEvent::FullState { downloads } => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut s = store.lock();
                                s.replace_all(downloads);
                                refresh_ui(&ui, &s);
                            }
                        });
                    }
                    DownloadEvent::CdnProgress { phase, current, total } => {
                        let current_lang = store_clone.lock().language();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_is_testing = true;
                                form.cdn_status_type = SharedString::from("testing");
                                form.cdn_status_label = SharedString::from(match current_lang {
                                    Language::ZhCn => "测速中",
                                    Language::EnUs => "Testing",
                                });
                                form.cdn_phase_label = SharedString::from(match phase.as_str() {
                                    "fetchingRanges" => match current_lang {
                                        Language::ZhCn => "获取网段",
                                        Language::EnUs => "Fetching IP ranges",
                                    },
                                    "screening" => match current_lang {
                                        Language::ZhCn => "延迟初筛",
                                        Language::EnUs => "Screening latency",
                                    },
                                    "measuringThroughput" => match current_lang {
                                        Language::ZhCn => "带宽测速",
                                        Language::EnUs => "Measuring bandwidth",
                                    },
                                    other => other,
                                });
                                if total > 0 {
                                    form.cdn_progress_percent = (current as f32 / total as f32 * 100.0).clamp(0.0, 100.0);
                                    form.cdn_progress_label = SharedString::from(format!("{current} / {total}"));
                                }
                                ui.set_labs_form(form);
                            }
                        });
                    }
                    DownloadEvent::CdnComplete { state, active_ip, active_speed_mbps } => {
                        let ui_weak_evt = ui_weak.clone();
                        let state_evt = state.clone();
                        let ip_evt = active_ip.clone();
                        let current_lang = store_clone.lock().language();
                        let is_ready = state_evt == "Ready" || state_evt == "ready";
                        let is_error = state_evt.starts_with("Error") || state_evt == "error";
                        let err_msg = if is_error {
                            state_evt.strip_prefix("Error: ").unwrap_or(&state_evt).to_string()
                        } else {
                            String::new()
                        };
                        let err_msg_ui = err_msg.clone();

                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_evt.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_is_testing = false;
                                let (st, sl) = if is_ready {
                                    ("ready", match current_lang {
                                        Language::ZhCn => "准备就绪",
                                        Language::EnUs => "Ready",
                                    })
                                } else if is_error {
                                    ("error", match current_lang {
                                        Language::ZhCn => "测速失败",
                                        Language::EnUs => "Failed",
                                    })
                                } else {
                                    ("idle", match current_lang {
                                        Language::ZhCn => "未配置",
                                        Language::EnUs => "Not Configured",
                                    })
                                };
                                form.cdn_status_type = SharedString::from(st);
                                form.cdn_status_label = SharedString::from(sl);
                                if is_ready {
                                    form.cdn_last_error = SharedString::default();
                                } else if is_error {
                                    form.cdn_last_error = SharedString::from(&err_msg_ui);
                                }
                                if let Some(ip) = ip_evt {
                                    form.cdn_active_ip = SharedString::from(ip);
                                }
                                if let Some(spd) = active_speed_mbps {
                                    form.cdn_active_speed_text = SharedString::from(format!("{spd:.2} MB/s"));
                                }
                                ui.set_labs_form(form);
                            }
                        });
                        // In-app toast for the async test result.
                        if is_ready {
                            let ip = active_ip.as_deref();
                            push_toast(
                                &ui_weak,
                                &toast_queue_clone,
                                i18n::format_toast_cdn_test_done(ip, current_lang),
                                "success",
                                Duration::from_secs(5),
                            );
                        } else if is_error {
                            let msg = if err_msg.is_empty() {
                                i18n::cdn_test_failed_label(current_lang)
                            } else {
                                &err_msg
                            };
                            push_toast(
                                &ui_weak,
                                &toast_queue_clone,
                                i18n::format_toast_cdn_test_failed(msg, current_lang),
                                "error",
                                Duration::from_secs(6),
                            );
                        }
                    }
                    DownloadEvent::Warning { id, message } => {
                        // Warnings originate in core (mirror failover, disk full,
                        // anti-leech bans, …). Surface them the same way the web
                        // client does: an in-app warning toast. Duplicate messages
                        // are collapsed within a short window because anti-leech
                        // bans arrive one per peer.
                        if !warning_dedup.should_show(&id, &message) {
                            continue;
                        }
                        let file_name = store.lock().get_summary(&id).map(|s| s.file_name);
                        let text = match file_name {
                            Some(name) if !name.is_empty() => {
                                i18n::format_warning_with_file(&name, &message)
                            }
                            _ => message,
                        };
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            text,
                            "warning",
                            Duration::from_secs(6),
                        );
                    }
                    DownloadEvent::Aria2Notification { .. } => {
                        // Aria2 compatibility notifications are pushed by the
                        // Aria2 RPC server straight to connected aria2 clients
                        // (AriaNg / Motrix); the native UI has no surface for
                        // them, so nothing to do here.
                    }
                }
            }
        });
    }

    // BT runtime status pills (DHT nodes / upload speed / peers), matching the
    // web client's toolbar status strip. Polled on a slow interval because the
    // DHT node count only changes gradually.
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = core.dispatcher.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                let status = dispatcher.bt_runtime_status();
                let ui_weak = ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else {
                        return;
                    };
                    match status {
                        Ok(s) if s.connected => {
                            ui.set_bt_status_visible(true);
                            ui.set_bt_dht_text(SharedString::from(match s.dht_nodes {
                                Some(n) if s.dht_enabled => n.to_string(),
                                _ => "-".to_string(),
                            }));
                            ui.set_bt_upload_speed_text(SharedString::from(format_speed(
                                s.upload_speed_bytes_per_second,
                            )));
                            ui.set_bt_peers_text(SharedString::from(s.peer_count.to_string()));
                        }
                        // No BT session yet (or the backend is gone): hide the
                        // pills exactly like the web client does with a null
                        // status payload.
                        _ => ui.set_bt_status_visible(false),
                    }
                });
            }
        });
    }

    // Phase 3: Periodic Inspector Polling (Peers, Trackers, Piece Map, Files)
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = core.dispatcher.clone();
        let active_inspector_id_clone = active_inspector_id.clone();
        let store_clone = store.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(1000));
            loop {
                interval.tick().await;

                let current_id = {
                    let lock = active_inspector_id_clone.lock();
                    lock.clone()
                };

                if let Some(task_id_str) = current_id
                    && let Ok(task_id) = TaskId::from_wire_string(&task_id_str)
                {
                    let summary_opt = {
                        let s = store_clone.lock();
                        s.get_summary(&task_id_str)
                    };

                    if let Some(summary) = summary_opt {
                        let lang = store_clone.lock().language();
                        if summary.kind == limedl_core::types::TaskKind::Bt {
                            let peers = dispatcher.bt_get_peers(&task_id).unwrap_or_default();
                            let trackers = dispatcher.bt_get_trackers(&task_id).unwrap_or_default();
                            let pieces = dispatcher.bt_get_pieces(&task_id).unwrap_or_default();
                            let files = dispatcher.bt_get_files(&task_id).unwrap_or_default();

                            let peer_items: Vec<PeerItem> = peers.iter().map(peer_info_to_item).collect();
                            let tracker_items: Vec<TrackerItem> = trackers.iter().map(tracker_info_to_item).collect();
                            let file_items: Vec<TorrentFileItem> = files.iter().map(file_status_to_item).collect();
                            let insp_info = summary_to_inspector_info(&summary, lang);

                            let ui_weak = ui_weak.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    let (piece_map_img, piece_count_text) = generate_piece_map_image(&pieces, lang);
                                    ui.set_inspector_info(insp_info);
                                    ui.set_inspector_peers(Rc::new(VecModel::from(peer_items)).into());
                                    ui.set_inspector_trackers(Rc::new(VecModel::from(tracker_items)).into());
                                    ui.set_inspector_piece_map(piece_map_img);
                                    ui.set_inspector_pieces_count_text(SharedString::from(piece_count_text));
                                    ui.set_inspector_files(Rc::new(VecModel::from(file_items)).into());
                                }
                            });
                        } else {
                            let insp_info = summary_to_inspector_info(&summary, lang);
                            let ui_weak = ui_weak.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    ui.set_inspector_info(insp_info);
                                }
                            });
                        }
                    }
                }
            }
        });
    }

    // System Tray Event Loop
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = core.dispatcher.clone();
        let default_dir = default_download_dir.clone();
        let game_mode_active_clone = game_mode_active.clone();
        let current_settings_tray = current_settings.clone();
        let tray_speed_limit_active_clone = tray_speed_limit_active.clone();
        let pending_tray_lang_tray = pending_tray_lang.clone();
        let store_tray = store.clone();
        let toast_queue_tray = toast_queue.clone();

        // Dedicated OS threads pump the *blocking* std receivers into
        // cancellable async channels. Never call `recv()` inside
        // `tokio::select!` via `spawn_blocking()`: cancelling the select arm
        // mid-`recv()` leaks a blocked `recv()` on the single shared receiver,
        // and that leaked waiter steals the next event from the channel — so
        // menu clicks (tray exit, show-window, …) were silently dropped.
        let (menu_tx, mut menu_rx) = tokio::sync::mpsc::unbounded_channel::<muda::MenuEvent>();
        let (tray_tx, mut tray_rx) = tokio::sync::mpsc::unbounded_channel::<TrayIconEvent>();
        std::thread::Builder::new()
            .name("tray-menu-events".into())
            .spawn(move || {
                let receiver = muda::MenuEvent::receiver();
                while let Ok(event) = receiver.recv() {
                    if menu_tx.send(event).is_err() {
                        break;
                    }
                }
            })
            .expect("failed to spawn tray menu event thread");
        std::thread::Builder::new()
            .name("tray-icon-events".into())
            .spawn(move || {
                let receiver = TrayIconEvent::receiver();
                while let Ok(event) = receiver.recv() {
                    if tray_tx.send(event).is_err() {
                        break;
                    }
                }
            })
            .expect("failed to spawn tray icon event thread");

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(event) = menu_rx.recv() => {
                        match event.id.as_ref() {
                            "show" => {
                                let _ = slint::invoke_from_event_loop({
                                    let ui_weak = ui_weak.clone();
                                    move || {
                                        if let Some(ui) = ui_weak.upgrade() {
                                            let _ = ui.show();
                                        }
                                    }
                                });
                            }
                            "pause_all" => {
                                if let Ok(list) = dispatcher.list().await {
                                    for item in list {
                                        if matches!(item.state, DownloadState::Downloading)
                                            && let Ok(task_id) = TaskId::from_wire_string(&item.id)
                                        {
                                            let _ = dispatcher.pause(&task_id).await;
                                        }
                                    }
                                }
                            }
                            "resume_all" => {
                                if let Ok(list) = dispatcher.list().await {
                                    for item in list {
                                        if matches!(item.state, DownloadState::Paused)
                                            && let Ok(task_id) = TaskId::from_wire_string(&item.id)
                                        {
                                            let _ = dispatcher.resume(&task_id).await;
                                        }
                                    }
                                }
                            }
                            "game_mode" => {
                                if let Ok(new_val) = dispatcher.toggle_game_mode(None) {
                                    *game_mode_active_clone.lock() = new_val;
                                    let ui_weak = ui_weak.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = ui_weak.upgrade() {
                                            ui.set_game_mode_active(new_val);
                                        }
                                    });
                                }
                            }
                            "speed_limit" => {
                                // Quick global speed limit shortcut: unlimited ↔ 1 MB/s.
                                let mut settings = current_settings_tray.lock().clone();
                                let enabling = settings.global_speed_limit_bps == 0;
                                settings.global_speed_limit_bps = if enabling {
                                    TRAY_SPEED_LIMIT_BPS
                                } else {
                                    0
                                };
                                let lang = store_tray.lock().language();
                                match dispatcher.save_settings(&settings).await {
                                    Ok(saved) => {
                                        *current_settings_tray.lock() = saved.clone();
                                        tray_speed_limit_active_clone.store(
                                            saved.global_speed_limit_bps > 0,
                                            Ordering::Relaxed,
                                        );
                                        // TrayIcon is !Send, so ask the UI-thread timer to
                                        // rebuild the menu and refresh the checkmark.
                                        *pending_tray_lang_tray.lock() = Some(Language::from_code(
                                            &saved.appearance.language,
                                        ));
                                        push_toast(
                                            &ui_weak,
                                            &toast_queue_tray,
                                            i18n::format_toast_speed_limit(enabling, lang),
                                            "info",
                                            Duration::from_secs(4),
                                        );
                                        let limit_kb =
                                            (saved.global_speed_limit_bps / 1024).to_string();
                                        let ui_weak = ui_weak.clone();
                                        let _ = slint::invoke_from_event_loop(move || {
                                            if let Some(ui) = ui_weak.upgrade() {
                                                let mut form = ui.get_settings_form();
                                                form.global_speed_limit_kb =
                                                    SharedString::from(limit_kb);
                                                ui.set_settings_form(form);
                                            }
                                        });
                                    }
                                    Err(e) => tracing::warn!("托盘限速切换失败: {e:#}"),
                                }
                            }
                            "open_dir" => {
                                let _ = open_path_in_explorer(&default_dir);
                            }
                            "quit" => {
                                POWER_GUARD.release();
                                // Quit through the Slint event loop so the
                                // runtime teardown (engine shutdown, registry
                                // shutdown_all) runs; fall back to a hard exit
                                // only if the event loop is already gone.
                                if slint::quit_event_loop().is_err() {
                                    std::process::exit(0);
                                }
                                break;
                            }
                            _ => {}
                        }
                    }
                    Some(event) = tray_rx.recv() => {
                        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                            let _ = slint::invoke_from_event_loop({
                                let ui_weak = ui_weak.clone();
                                move || {
                                    if let Some(ui) = ui_weak.upgrade() {
                                        let _ = ui.show();
                                    }
                                }
                            });
                        }
                    }
                    else => break,
                }
            }
        });
    }

    // UI Callbacks
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_select_category(move |cat| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.set_category(cat);
                ui.set_active_category(cat);
                refresh_ui(&ui, &store);
            }
        });
    }

    // Phase 2: Search changed
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_search_changed(move |query| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.set_search_query(query.to_string());
                refresh_ui(&ui, &store);
            }
        });
    }

    // Phase 2: Sort field & order
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        main_window.on_set_sort_field(move |field_idx| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            if let Some(ui) = ui_weak.upgrade() {
                let (field, asc) = {
                    let mut store = store_clone.lock();
                    store.set_sort_field(SortField::from(field_idx));
                    refresh_ui(&ui, &store);
                    (store.sort_field(), store.sort_asc())
                };
                persist_sort_preference(&dispatcher, &current_settings_clone, field, asc);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        main_window.on_toggle_sort_asc(move || {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            if let Some(ui) = ui_weak.upgrade() {
                let (field, asc) = {
                    let mut store = store_clone.lock();
                    store.toggle_sort_order();
                    refresh_ui(&ui, &store);
                    (store.sort_field(), store.sort_asc())
                };
                persist_sort_preference(&dispatcher, &current_settings_clone, field, asc);
            }
        });
    }

    // Phase 2: Multi-selection
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_toggle_select_task(move |id_str| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.toggle_select(&id_str);
                refresh_ui(&ui, &store);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_range_select_task(move |id_str| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.select_range(&id_str);
                refresh_ui(&ui, &store);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_select_all(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.select_all();
                refresh_ui(&ui, &store);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_clear_selection(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.clear_selection();
                refresh_ui(&ui, &store);
            }
        });
    }

    // Phase 2: Batch actions
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_batch_pause(move || {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let ids = {
                    let store = store_clone.lock();
                    store.selected_ids()
                };

                for id in ids {
                    if let Ok(task_id) = TaskId::from_wire_string(&id) {
                        let _ = dispatcher.pause(&task_id).await;
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let store = store_clone.lock();
                        refresh_ui(&ui, &store);
                    }
                });
            });
        });
    }

    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_batch_resume(move || {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let ids = {
                    let store = store_clone.lock();
                    store.selected_ids()
                };

                for id in ids {
                    if let Ok(task_id) = TaskId::from_wire_string(&id) {
                        let _ = dispatcher.resume(&task_id).await;
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let store = store_clone.lock();
                        refresh_ui(&ui, &store);
                    }
                });
            });
        });
    }

    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_batch_remove(move |delete_files| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let ids = {
                    let mut store = store_clone.lock();
                    let ids = store.selected_ids();
                    store.clear_selection();
                    ids
                };

                for id in &ids {
                    if let Ok(task_id) = TaskId::from_wire_string(id) {
                        if delete_files {
                            let _ = dispatcher.purge(&task_id).await;
                        } else {
                            let _ = dispatcher.remove(&task_id).await;
                        }
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let mut store = store_clone.lock();
                        for id in ids {
                            store.remove(&id);
                        }
                        refresh_ui(&ui, &store);
                    }
                });
            });
        });
    }

    // View mode toggle (Cards <-> Table)
    {
        let ui_weak = main_window.as_weak();
        main_window.on_toggle_view_mode(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let current = ui.get_view_mode();
                ui.set_view_mode(if current == 0 { 1 } else { 0 });
            }
        });
    }

    // Table column header sort clicked
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_table_sort_clicked(move |col_idx| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                let target_field = SortField::from(col_idx);
                if store.sort_field() == target_field as i32 {
                    store.toggle_sort_order();
                } else {
                    store.set_sort_field(target_field);
                }
                refresh_ui(&ui, &store);
            }
        });
    }

    // Desktop keyboard shortcuts: Space (toggle pause/resume)
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_hotkey_space(move || {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let (ids, any_downloading) = {
                    let store = store_clone.lock();
                    let sel = store.selected_ids();
                    if !sel.is_empty() {
                        let downloading = store.filtered_items().iter().any(|item| {
                            sel.contains(&item.id.to_string()) && item.can_pause
                        });
                        (sel, downloading)
                    } else {
                        let all_ids: Vec<String> = store
                            .filtered_items()
                            .iter()
                            .map(|it| it.id.to_string())
                            .collect();
                        let (_, downloading_count, _, _, _) = store.counts();
                        (all_ids, downloading_count > 0)
                    }
                };

                for id in ids {
                    if let Ok(task_id) = TaskId::from_wire_string(&id) {
                        if any_downloading {
                            let _ = dispatcher.pause(&task_id).await;
                        } else {
                            let _ = dispatcher.resume(&task_id).await;
                        }
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let store = store_clone.lock();
                        refresh_ui(&ui, &store);
                    }
                });
            });
        });
    }

    // Desktop keyboard shortcuts: Delete (delete selected tasks, shift: purge files)
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_hotkey_delete(move |delete_files| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let ids = {
                    let mut store = store_clone.lock();
                    let ids = store.selected_ids();
                    store.clear_selection();
                    ids
                };

                if ids.is_empty() {
                    return;
                }

                for id in &ids {
                    if let Ok(task_id) = TaskId::from_wire_string(id) {
                        if delete_files {
                            let _ = dispatcher.purge(&task_id).await;
                        } else {
                            let _ = dispatcher.remove(&task_id).await;
                        }
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let mut store = store_clone.lock();
                        for id in ids {
                            store.remove(&id);
                        }
                        refresh_ui(&ui, &store);
                    }
                });
            });
        });
    }

    // Phase 3: Task Inspector callbacks
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let active_inspector_id_clone = active_inspector_id.clone();
        main_window.on_open_inspector(move |id_str| {
            let id = id_str.to_string();
            *active_inspector_id_clone.lock() = Some(id.clone());

            if let Some(ui) = ui_weak.upgrade() {
                let s = store_clone.lock();
                if let Some(summary) = s.get_summary(&id) {
                    ui.set_inspector_info(summary_to_inspector_info(&summary, s.language()));
                }
                ui.set_inspector_tab(0);
                ui.set_show_inspector(true);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let active_inspector_id_clone = active_inspector_id.clone();
        main_window.on_close_inspector(move || {
            *active_inspector_id_clone.lock() = None;
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_inspector(false);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_set_inspector_tab(move |tab_idx| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_inspector_tab(tab_idx);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_open_speed_limit_dialog(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_speed_limit_dialog(true);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_close_speed_limit_dialog(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_speed_limit_dialog(false);
            }
        });
    }

    {
        let dispatcher = core.dispatcher.clone();
        let active_inspector_id_clone = active_inspector_id.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_submit_speed_limit(move |dl_kb_str, ul_kb_str| {
            let dl_bps = dl_kb_str.trim().parse::<u64>().ok().filter(|&v| v > 0).map(|kb| kb * 1024);
            let ul_bps = ul_kb_str.trim().parse::<u64>().ok().filter(|&v| v > 0).map(|kb| kb * 1024);

            let current_id = active_inspector_id_clone.lock().clone();
            if let Some(task_id_str) = current_id
                && let Ok(task_id) = TaskId::from_wire_string(&task_id_str)
            {
                let _ = dispatcher.bt_set_speed_limit(&task_id, dl_bps, ul_bps);
            }

            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_speed_limit_dialog(false);
            }
        });
    }

    // Phase 4: Settings Dialog & Performance Modes
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let game_mode_clone = game_mode_active.clone();
        let overclock_mode_clone = overclock_mode_active.clone();
        let store_clone = store.clone();

        main_window.on_open_settings(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let settings = current_settings_clone.lock().clone();
                let gm = *game_mode_clone.lock();
                let oc = *overclock_mode_clone.lock();
                let lang = store_clone.lock().language();
                refresh_settings_state(&ui, &dispatcher, &settings, gm, oc, lang);
                ui.set_show_settings(true);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_close_settings(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_settings(false);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_set_settings_tab(move |tab| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_settings_tab(tab);
            }
        });
    }

    {
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let store_clone = store.clone();
        let pending_tray_lang_clone = pending_tray_lang.clone();
        let active_inspector_id_clone = active_inspector_id.clone();
        let rpc_shutdown_clone = rpc_shutdown.clone();
        let toast_queue_clone = toast_queue.clone();
        let tray_speed_limit_settings = tray_speed_limit_active.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_save_settings(move |form_data| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let store_clone = store_clone.clone();
            let pending_tray_lang_clone = pending_tray_lang_clone.clone();
            let active_inspector_id_clone = active_inspector_id_clone.clone();
            let rpc_shutdown = rpc_shutdown_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let tray_speed_limit_settings = tray_speed_limit_settings.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let old_settings = {
                    let s = current_settings_clone.lock();
                    s.clone()
                };
                let mut settings = old_settings.clone();
                let lang = store_clone.lock().language();

                // Speed limit schedule rows live in the UI model until Save, so
                // validate them here (a mistyped hour must not silently disable
                // the schedule).
                let schedule_rows = ui_weak
                    .upgrade()
                    .map(|ui| read_schedule_rows(&ui))
                    .unwrap_or_default();
                let parsed_schedule = match parse_speed_limit_slots(&schedule_rows, lang) {
                    Ok(slots) => slots,
                    Err(msg) => {
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_schedule_invalid(&msg, lang),
                            "error",
                            Duration::from_secs(8),
                        );
                        return;
                    }
                };

                if let Err(msg) = update_app_settings_from_form(&mut settings, &form_data, lang) {
                    tracing::error!("设置表单校验失败: {msg}");
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_settings_invalid(&msg, lang),
                        "error",
                        Duration::from_secs(6),
                    );
                    return;
                }

                settings.speed_limit_schedule = parsed_schedule;

                match dispatcher.save_settings(&settings).await {
                    Ok(saved) => {
                        // ── Autostart OS 同步 ──
                        if saved.autostart != old_settings.autostart {
                            if let Err(e) = crate::autostart::set_enabled(saved.autostart) {
                                tracing::warn!("autostart 切换失败: {e:#}");
                                push_toast(
                                    &ui_weak,
                                    &toast_queue,
                                    i18n::format_toast_autostart_failed(&format!("{e:#}"), lang),
                                    "error",
                                    Duration::from_secs(6),
                                );
                            } else {
                                tracing::info!("autostart 已切换为 {}", saved.autostart);
                            }
                        }
                        // ── Aria2 RPC 热重载 ──
                        if saved.aria2_rpc != old_settings.aria2_rpc {
                            // Shutdown old server if any
                            if let Some(tx) = rpc_shutdown.lock().take() {
                                let _ = tx.send(true);
                                tracing::info!("Aria2 RPC 旧服务已停止");
                            }
                            if saved.aria2_rpc.enabled {
                                let (tx, rx) = watch::channel(false);
                                let rpc_server = Aria2RpcServer::new(
                                    dispatcher.registry().clone(),
                                    &saved.aria2_rpc,
                                    dispatcher.event_bus().clone(),
                                );
                                let cors = saved.aria2_rpc.cors_allowed_origins.clone();
                                let port = saved.aria2_rpc.port;
                                tokio::spawn(async move {
                                    if let Err(e) = rpc_server.serve(rx, cors).await {
                                        tracing::error!("Aria2 RPC server stopped: {e:#}");
                                    }
                                });
                                *rpc_shutdown.lock() = Some(tx);
                                tracing::info!("Aria2 RPC 已重启 (port: {port})");
                                push_toast(
                                    &ui_weak,
                                    &toast_queue,
                                    i18n::format_toast_aria2_rpc_started(port, lang),
                                    "info",
                                    Duration::from_secs(5),
                                );
                            } else {
                                tracing::info!("Aria2 RPC 已停止");
                                push_toast(
                                    &ui_weak,
                                    &toast_queue,
                                    i18n::format_toast_aria2_rpc_stopped(lang).to_string(),
                                    "info",
                                    Duration::from_secs(5),
                                );
                            }
                        }
                        *current_settings_clone.lock() = saved.clone();
                        // Keep the tray speed-limit checkmark in sync with the
                        // limit edited inside the settings dialog.
                        tray_speed_limit_settings.store(
                            saved.global_speed_limit_bps > 0,
                            Ordering::Relaxed,
                        );
                        let default_dir = saved.download.default_download_dir.clone();
                        let new_lang = Language::from_code(&saved.appearance.language);
                        let new_color_mode = saved.appearance.color_mode;
                        let new_theme_color = saved.appearance.theme_color;

                        // In-app success toast.
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_settings_saved(lang).to_string(),
                            "success",
                            Duration::from_secs(4),
                        );

                        // Signal tray update to main thread (TrayIcon is !Send)
                        *pending_tray_lang_clone.lock() = Some(new_lang);

                        let _ = slint::invoke_from_event_loop(move || {
                            i18n::apply_translation(new_lang);
                            if let Some(ui) = ui_weak.upgrade() {
                                apply_appearance(&ui, new_color_mode, new_theme_color);
                                let mut s = store_clone.lock();
                                s.set_language(new_lang);
                                ui.set_default_download_dir(SharedString::from(&default_dir));
                                refresh_ui(&ui, &s);

                                // Refresh Inspector if active
                                if let Some(ref current_id) = *active_inspector_id_clone.lock()
                                    && let Some(summary) = s.get_summary(current_id)
                                {
                                    ui.set_inspector_info(summary_to_inspector_info(&summary, new_lang));
                                }

                                ui.set_show_settings(false);
                            }
                        });
                    }
                    Err(err) => {
                        tracing::error!("保存设置失败: {err:#}");
                        let msg = format!("{err:#}");
                        let _ = Notification::new()
                            .appname("limedl")
                            .summary(i18n::format_notification_settings_save_failed(lang))
                            .body(&msg)
                            .show();
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_settings_save_failed(&msg, lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    // Toggle Game Mode
    {
        let dispatcher = core.dispatcher.clone();
        let game_mode_clone = game_mode_active.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_toggle_game_mode(move || {
            if let Ok(new_val) = dispatcher.toggle_game_mode(None) {
                *game_mode_clone.lock() = new_val;
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_game_mode_active(new_val);
                    let mut form = ui.get_settings_form();
                    form.game_mode = new_val;
                    ui.set_settings_form(form);
                }
            }
        });
    }

    // Toggle Overclock Mode
    {
        let dispatcher = core.dispatcher.clone();
        let overclock_mode_clone = overclock_mode_active.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_toggle_overclock_mode(move || {
            if let Ok(new_val) = dispatcher.toggle_overclock_mode(None) {
                *overclock_mode_clone.lock() = new_val;
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_overclock_mode_active(new_val);
                    let mut form = ui.get_settings_form();
                    form.overclock_mode = new_val;
                    ui.set_settings_form(form);
                }
            }
        });
    }

    // Fetch Remote Trackers
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_fetch_trackers_remote(move |url_str| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let url = url_str.to_string();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                match dispatcher.fetch_tracker_list(&url).await {
                    Ok(trackers) => {
                        tracing::info!("成功同步远程 Tracker 列表: {} 个", trackers.len());
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_tracker_synced(trackers.len(), lang),
                            "success",
                            Duration::from_secs(4),
                        );

                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_settings_form();
                                form.tracker_url = SharedString::from(&url);
                                ui.set_settings_form(form);
                            }
                        });
                    }
                    Err(err) => {
                        tracing::error!("同步 Tracker 列表失败: {err:#}");
                        let msg = format!("{err:#}");
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_tracker_sync_failed(&msg, lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    // Dialog: Open / Close New Task
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_open_new_task_dialog(move || {
            if let Some(ui) = ui_weak.upgrade() {
                // Reset transient dialog state (probe / torrent preview / batch)
                let lang = store_clone.lock().language();
                ui.set_new_task_probe_state("idle".into());
                ui.set_new_task_probe_status_text(SharedString::default());
                ui.set_new_task_probe_hash(SharedString::default());
                ui.set_new_task_preview_state("none".into());
                ui.set_new_task_preview_status_text(SharedString::default());
                ui.set_new_task_preview_summary_text(SharedString::default());
                ui.set_new_task_torrent_files(ModelRc::default());
                ui.set_new_task_batch_mode(false);
                ui.set_new_task_batch_text(SharedString::default());
                ui.set_new_task_batch_count_text(
                    SharedString::from(i18n::format_batch_count(0, lang)),
                );
                ui.set_new_task_batch_submitting(false);
                ui.set_new_task_batch_status_text(SharedString::default());
                ui.set_show_new_task_dialog(true);

                // Clipboard auto-fill: if the URL field is still empty, read
                // the system clipboard and prefill single link or switch to batch mode.
                if ui.get_new_task_url().trim().is_empty() {
                    let ui_weak = ui_weak.clone();
                    let store_clone = store_clone.clone();
                    tokio::spawn(async move {
                        let Ok(mut clipboard) = arboard::Clipboard::new() else {
                            return;
                        };
                        let Ok(text) = clipboard.get_text() else {
                            return;
                        };
                        match crate::bridge::parse_clipboard_download_text(&text) {
                            crate::bridge::ClipboardPayload::SingleUrl(url) => {
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_weak.upgrade()
                                        && ui.get_new_task_url().trim().is_empty()
                                    {
                                        ui.set_new_task_url(SharedString::from(url));
                                        ui.set_new_task_batch_mode(false);
                                    }
                                });
                            }
                            crate::bridge::ClipboardPayload::BatchUrls(urls) => {
                                let joined = urls.join("\n");
                                let count = urls.len();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_weak.upgrade()
                                        && ui.get_new_task_url().trim().is_empty()
                                    {
                                        ui.set_new_task_batch_text(SharedString::from(&joined));
                                        ui.set_new_task_batch_mode(true);
                                        let lang = store_clone.lock().language();
                                        ui.set_new_task_batch_count_text(
                                            SharedString::from(i18n::format_batch_count(count, lang)),
                                        );
                                    }
                                });
                            }
                            crate::bridge::ClipboardPayload::Empty => {}
                        }
                    });
                }
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_close_new_task_dialog(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_new_task_dialog(false);
            }
        });
    }

    // New Task: reset checksum probe (dialog calls this whenever the URL is edited,
    // because a detected hash is only valid for the URL it was probed against)
    {
        let ui_weak = main_window.as_weak();
        main_window.on_reset_new_task_probe(move || {
            if let Some(ui) = ui_weak.upgrade()
                && ui.get_new_task_probe_state().as_str() != "idle"
            {
                ui.set_new_task_probe_state("idle".into());
                ui.set_new_task_probe_status_text(SharedString::default());
                ui.set_new_task_probe_hash(SharedString::default());
            }
        });
    }

    // New Task: probe .sha256 / SHA256SUMS for the entered HTTP link
    {
        let dispatcher = core.dispatcher.clone();
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_probe_new_task_checksum(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let url = ui.get_new_task_url().trim().to_string();
            if url.is_empty() {
                return;
            }
            let custom_name = ui.get_new_task_filename().trim().to_string();
            let file_name = (!custom_name.is_empty()).then_some(custom_name);
            let lang = store_clone.lock().language();
            if !url.starts_with("http://") && !url.starts_with("https://") {
                ui.set_new_task_probe_state("not_http".into());
                ui.set_new_task_probe_status_text(
                    SharedString::from(i18n::format_probe_status("not_http", "", lang)),
                );
                return;
            }
            ui.set_new_task_probe_state("probing".into());
            ui.set_new_task_probe_status_text(
                SharedString::from(i18n::format_probe_status("probing", "", lang)),
            );
            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            tokio::spawn(async move {
                let detected = dispatcher
                    .probe_checksum(&url, file_name.as_deref())
                    .await
                    .ok()
                    .flatten();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        match detected {
                            Some(hash) => {
                                ui.set_new_task_probe_state("found".into());
                                ui.set_new_task_probe_status_text(SharedString::from(
                                    i18n::format_probe_status("found", &hash, lang),
                                ));
                                ui.set_new_task_probe_hash(SharedString::from(hash));
                            }
                            None => {
                                ui.set_new_task_probe_state("missing".into());
                                ui.set_new_task_probe_status_text(SharedString::from(
                                    i18n::format_probe_status("missing", "", lang),
                                ));
                            }
                        }
                    }
                });
            });
        });
    }

    // New Task: torrent file pre-selection toggles
    {
        let ui_weak = main_window.as_weak();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();
        main_window.on_toggle_new_task_file(move |idx| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let mut included = included_cache.lock();
            let pos = idx.max(0) as usize;
            if let Some(flag) = included.get_mut(pos) {
                *flag = !*flag;
                let entries = entries_cache.lock();
                let items: Vec<NewTaskTorrentFileItem> = entries
                    .iter()
                    .zip(included.iter())
                    .map(|(e, inc)| torrent_entry_to_item(e, *inc))
                    .collect();
                drop(entries);
                drop(included);
                ui.set_new_task_torrent_files(Rc::new(VecModel::from(items)).into());
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();
        main_window.on_set_all_new_task_files(move |all| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let mut included = included_cache.lock();
            if included.is_empty() {
                return;
            }
            included.iter_mut().for_each(|flag| *flag = all);
            let entries = entries_cache.lock();
            let items: Vec<NewTaskTorrentFileItem> = entries
                .iter()
                .zip(included.iter())
                .map(|(e, inc)| torrent_entry_to_item(e, *inc))
                .collect();
            drop(entries);
            drop(included);
            ui.set_new_task_torrent_files(Rc::new(VecModel::from(items)).into());
        });
    }

    // Submit New Task (single mode: with checksum probe result and BT file selection)
    {
        let dispatcher = core.dispatcher.clone();
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();
        let toast_queue_clone = toast_queue.clone();
        main_window.on_submit_new_task(move |url, dir, filename| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let url_str = url.to_string();
            let dir_str = dir.to_string();
            let filename_opt = if filename.trim().is_empty() {
                None
            } else {
                Some(filename.trim().to_string())
            };

            // Detected checksum (cleared by the dialog whenever the URL changes,
            // so a `found` state here is always bound to the current URL)
            let (checksum, expected_checksum) = if ui.get_new_task_probe_state().as_str() == "found"
            {
                let hash = ui.get_new_task_probe_hash().to_string();
                (
                    (!hash.is_empty()).then_some(ChecksumMode::Sha256),
                    (!hash.is_empty()).then_some(hash),
                )
            } else {
                (None, None)
            };

            // BT file selection: None downloads everything; an empty selection
            // is rejected with a visible hint instead of a silent no-op task.
            let selected_file_indices = {
                let entries = entries_cache.lock();
                let included = included_cache.lock();
                if entries.is_empty() || ui.get_new_task_preview_state().as_str() != "ready" {
                    None
                } else {
                    let chosen: Vec<usize> = entries
                        .iter()
                        .zip(included.iter())
                        .filter(|(_, inc)| **inc)
                        .map(|(e, _)| e.index)
                        .collect();
                    if chosen.len() == entries.len() {
                        None
                    } else {
                        Some(chosen)
                    }
                }
            };
            if selected_file_indices.as_ref().is_some_and(Vec::is_empty) {
                let lang = store_clone.lock().language();
                ui.set_new_task_preview_status_text(SharedString::from(
                    i18n::no_files_selected_text(lang),
                ));
                return;
            }

            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let toast_queue_clone = toast_queue_clone.clone();
            tokio::spawn(async move {
                let req = StartDownloadRequest {
                    url: url_str,
                    destination_dir: dir_str,
                    file_name: filename_opt,
                    checksum,
                    expected_checksum,
                    selected_file_indices,
                    ..Default::default()
                };

                let file_name = req.file_name.clone().unwrap_or_default();
                match dispatcher.start(req).await {
                    Ok(task_id) => {
                        tracing::info!("成功添加下载任务: {task_id}");
                        let lang = store_clone.lock().language();
                        let display_name = if file_name.is_empty() {
                            i18n::format_unnamed_task(lang)
                        } else {
                            file_name.as_str()
                        };
                        push_toast(
                            &ui_weak,
                            &toast_queue_clone,
                            i18n::format_toast_task_added(display_name, lang),
                            "success",
                            Duration::from_secs(4),
                        );
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_new_task_url(SharedString::default());
                                ui.set_new_task_filename(SharedString::default());
                                ui.set_show_new_task_dialog(false);
                            }
                        });
                    }
                    Err(err) => {
                        tracing::error!("添加下载任务失败: {err}");
                        let lang = store_clone.lock().language();
                        push_toast(
                            &ui_weak,
                            &toast_queue_clone,
                            i18n::format_toast_task_add_failed(&format!("{err}"), lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    // New Task: batch mode — live link count under the textarea
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_new_task_batch_text_changed(move |text| {
            if let Some(ui) = ui_weak.upgrade() {
                let count = parse_batch_urls(&text).len();
                let lang = store_clone.lock().language();
                ui.set_new_task_batch_count_text(SharedString::from(i18n::format_batch_count(
                    count, lang,
                )));
            }
        });
    }

    // New Task: batch mode — submit all parsed links concurrently
    {
        let dispatcher = core.dispatcher.clone();
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        main_window.on_submit_new_task_batch(move |text, dir| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let urls = parse_batch_urls(&text);
            let dir_str = dir.to_string();
            let lang = store_clone.lock().language();
            if urls.is_empty() {
                ui.set_new_task_batch_status_text(SharedString::from(i18n::format_batch_count(
                    0, lang,
                )));
                return;
            }
            ui.set_new_task_batch_submitting(true);
            ui.set_new_task_batch_status_text(SharedString::from(i18n::format_batch_status(
                0,
                urls.len(),
                lang,
            )));

            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            let toast_queue_clone = toast_queue_clone.clone();
            tokio::spawn(async move {
                let total = urls.len();
                let done = Arc::new(AtomicUsize::new(0));
                let ok_count = Arc::new(AtomicUsize::new(0));
                for entry_url in urls {
                    let dispatcher = dispatcher.clone();
                    let ui_weak = ui_weak.clone();
                    let toast_queue = toast_queue_clone.clone();
                    let done = done.clone();
                    let ok_count = ok_count.clone();
                    let dir_str = dir_str.clone();
                    let file_name = extract_batch_file_name(&entry_url);
                    tokio::spawn(async move {
                        let req = StartDownloadRequest {
                            url: entry_url,
                            destination_dir: dir_str,
                            file_name,
                            ..Default::default()
                        };
                        if dispatcher.start(req).await.is_ok() {
                            ok_count.fetch_add(1, Ordering::Relaxed);
                        }
                        let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if finished >= total {
                            let succeeded = ok_count.load(Ordering::Relaxed);
                            // In-app toast on the final result (best effort).
                            push_toast(
                                &ui_weak,
                                &toast_queue,
                                i18n::format_toast_batch_done(succeeded, total, lang),
                                if succeeded == total { "success" } else { "warning" },
                                Duration::from_secs(5),
                            );
                        }
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                if finished < total {
                                    ui.set_new_task_batch_status_text(SharedString::from(
                                        i18n::format_batch_status(finished, total, lang),
                                    ));
                                } else {
                                    let succeeded = ok_count.load(Ordering::Relaxed);
                                    ui.set_new_task_batch_submitting(false);
                                    ui.set_new_task_batch_status_text(SharedString::from(
                                        i18n::format_batch_status(succeeded, total, lang),
                                    ));
                                    if succeeded == total {
                                        ui.set_show_new_task_dialog(false);
                                    }
                                }
                            }
                        });
                    });
                }
            });
        });
    }

    // Set task priority (from the priority popup opened by the table badge or
    // the task context menu).
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_set_task_priority(move |id_str, priority_code| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            let id_str = id_str.to_string();
            let priority_code = priority_code.to_string();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                let priority = str_to_priority(&priority_code);
                let file_name = store_clone
                    .lock()
                    .get_summary(&id_str)
                    .map(|s| s.file_name)
                    .unwrap_or_default();
                let Ok(task_id) = TaskId::from_wire_string(&id_str) else {
                    tracing::warn!("优先级设置失败: 无法解析任务 ID {id_str}");
                    return;
                };
                match dispatcher.set_priority(&task_id, priority).await {
                    Ok(()) => push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_priority_set(&file_name, priority, lang),
                        "info",
                        Duration::from_secs(4),
                    ),
                    Err(err) => {
                        tracing::error!("优先级设置失败: {err:#}");
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_priority_failed(&format!("{err:#}"), lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    // Toggle BT file inclusion for an existing task (Inspector → Files).
    // Mirrors the web client: the backend gets the full included index list and
    // at least one file must stay selected.
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let active_inspector_id_clone = active_inspector_id.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_toggle_inspector_file(move |index, currently_included| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let active_inspector_id_clone = active_inspector_id_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            if let Some(ui) = ui_weak.upgrade() {
                let files = ui.get_inspector_files();
                let mut included: Vec<usize> = Vec::new();
                for file in files.iter() {
                    let is_target = file.index == index;
                    let keep = if is_target {
                        !currently_included
                    } else {
                        file.included
                    };
                    if keep {
                        included.push(file.index as usize);
                    }
                }
                if included.is_empty() {
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_bt_files_keep_one(store_clone.lock().language())
                            .to_string(),
                        "warning",
                        Duration::from_secs(5),
                    );
                    return;
                }
                let Some(task_id_str) = active_inspector_id_clone.lock().clone() else {
                    return;
                };
                let Ok(task_id) = TaskId::from_wire_string(&task_id_str) else {
                    return;
                };
                tokio::spawn(async move {
                    let lang = store_clone.lock().language();
                    if let Err(err) = dispatcher.bt_update_files(&task_id, included).await {
                        tracing::error!("更新 BT 文件选择失败: {err:#}");
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_bt_files_failed(&format!("{err:#}"), lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                });
            }
        });
    }

    // ── Speed limit schedule callbacks ───────────────────────────────────
    // The VecModel behind `speed_limit_slots` is the authoritative editor state;
    // it is parsed and persisted by the settings Save handler.
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_schedule_set_enabled(move |enabled| {
            let lang = store_clone.lock().language();
            if let Some(ui) = ui_weak.upgrade() {
                let rows: Vec<SpeedLimitSlotText> = if enabled {
                    vec![SpeedLimitSlotText::default()]
                } else {
                    Vec::new()
                };
                ui.set_speed_limit_slots(ModelRc::new(VecModel::from(speed_limit_slots_to_slint(
                    &rows, lang,
                ))));
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_schedule_add(move || {
            let lang = store_clone.lock().language();
            if let Some(ui) = ui_weak.upgrade() {
                let mut rows = read_schedule_rows(&ui);
                rows.push(SpeedLimitSlotText::default());
                ui.set_speed_limit_slots(ModelRc::new(VecModel::from(speed_limit_slots_to_slint(
                    &rows, lang,
                ))));
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_schedule_remove(move |idx| {
            let lang = store_clone.lock().language();
            if let Some(ui) = ui_weak.upgrade() {
                let mut rows = read_schedule_rows(&ui);
                let idx = idx.max(0) as usize;
                if idx < rows.len() {
                    rows.remove(idx);
                }
                ui.set_speed_limit_slots(ModelRc::new(VecModel::from(speed_limit_slots_to_slint(
                    &rows, lang,
                ))));
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_schedule_update(move |idx, field, value| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let lang = store_clone.lock().language();
            let model = ui.get_speed_limit_slots();
            let Some(vec_model) = model.as_any().downcast_ref::<VecModel<SpeedLimitSlotItem>>()
            else {
                return;
            };
            let idx = idx.max(0) as usize;
            let Some(mut item) = vec_model.row_data(idx) else {
                return;
            };
            match field.as_str() {
                "start" => item.start_hour = value.clone(),
                "end" => item.end_hour = value.clone(),
                "limit" => item.limit_kb = value.clone(),
                _ => return,
            }
            // Refresh the derived row text (range summary + midnight marker)
            // without rebuilding the model, so the focused input keeps its caret.
            let start = item.start_hour.trim().parse::<u32>().unwrap_or(0).min(23);
            let end = item.end_hour.trim().parse::<u32>().unwrap_or(0).min(23);
            let limit = item.limit_kb.trim().parse::<u64>().unwrap_or(0);
            item.wraps = start >= end;
            item.summary =
                SharedString::from(i18n::format_schedule_summary(start, end, limit, lang));
            vec_model.set_row_data(idx, item);
        });
    }

    // Pause Task
    {
        let dispatcher = core.dispatcher.clone();
        main_window.on_pause_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str)
                    && let Err(err) = dispatcher.pause(&task_id).await
                {
                    tracing::error!("暂停任务失败: {err}");
                }
            });
        });
    }

    // Resume Task
    {
        let dispatcher = core.dispatcher.clone();
        main_window.on_resume_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str)
                    && let Err(err) = dispatcher.resume(&task_id).await
                {
                    tracing::error!("恢复任务失败: {err}");
                }
            });
        });
    }

    // Open in Explorer
    {
        let dispatcher = core.dispatcher.clone();
        main_window.on_open_task_explorer(move |id_str| {
            let dispatcher = dispatcher.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str)
                    && let Err(err) = dispatcher.open_in_explorer(&task_id).await
                {
                    tracing::error!("打开任务文件目录失败: {err}");
                }
            });
        });
    }

    // Double-click behavior (Settings → General: double_click on_completed /
    // on_uncompleted). Mirrors the web client: completed tasks open the file /
    // explorer / download dir; uncompleted tasks toggle pause/resume.
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let current_settings_clone = current_settings.clone();
        main_window.on_task_double_clicked(move |id_str| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let current_settings_clone = current_settings_clone.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Err(err) = handle_task_double_click(
                    &dispatcher,
                    &store_clone,
                    &current_settings_clone,
                    &id_str,
                )
                .await
                {
                    tracing::error!("双击任务操作失败: {err:#}");
                }
            });
        });
    }

    // Remove Task
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_remove_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();
            let id_str = id_str.to_string();

            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str) {
                    if let Err(err) = dispatcher.remove(&task_id).await {
                        tracing::error!("删除任务失败: {err}");
                    } else {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut store = store_clone.lock();
                                store.remove(&id_str);
                                refresh_ui(&ui, &store);
                            }
                        });
                    }
                }
            });
        });
    }

    // Pause All
    {
        let dispatcher = core.dispatcher.clone();
        main_window.on_pause_all(move || {
            let dispatcher = dispatcher.clone();
            tokio::spawn(async move {
                if let Ok(list) = dispatcher.list().await {
                    for item in list {
                        if matches!(item.state, DownloadState::Downloading)
                            && let Ok(task_id) = TaskId::from_wire_string(&item.id)
                        {
                            let _ = dispatcher.pause(&task_id).await;
                        }
                    }
                }
            });
        });
    }

    // Resume All
    {
        let dispatcher = core.dispatcher.clone();
        main_window.on_resume_all(move || {
            let dispatcher = dispatcher.clone();
            tokio::spawn(async move {
                if let Ok(list) = dispatcher.list().await {
                    for item in list {
                        if matches!(item.state, DownloadState::Paused)
                            && let Ok(task_id) = TaskId::from_wire_string(&item.id)
                        {
                            let _ = dispatcher.resume(&task_id).await;
                        }
                    }
                }
            });
        });
    }

    // Clear Completed: drop the records of every finished task (files stay on
    // disk, matching the web client's "clear completed" action).
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_clear_completed(move || {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                let Ok(list) = dispatcher.list().await else {
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_clear_completed_failed(lang).to_string(),
                        "error",
                        Duration::from_secs(5),
                    );
                    return;
                };
                let mut cleared = 0usize;
                for item in list {
                    if !matches!(item.state, DownloadState::Completed) {
                        continue;
                    }
                    if let Ok(task_id) = TaskId::from_wire_string(&item.id)
                        && dispatcher.remove(&task_id).await.is_ok()
                    {
                        cleared += 1;
                    }
                }
                if cleared == 0 {
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_clear_completed_none(lang).to_string(),
                        "info",
                        Duration::from_secs(4),
                    );
                    return;
                }
                push_toast(
                    &ui_weak,
                    &toast_queue,
                    i18n::format_toast_clear_completed(cleared, lang),
                    "success",
                    Duration::from_secs(4),
                );
            });
        });
    }

    // Pick Torrent File (Native Dialog) + start file pre-selection preview
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();
        main_window.on_pick_torrent_file(move || {
            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let entries_cache = entries_cache.clone();
            let included_cache = included_cache.clone();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("Torrent Files", &["torrent", "TORRENT"])
                    .set_title(i18n::pick_torrent_title(lang))
                    .pick_file()
                    .await;

                if let Some(handle) = file {
                    let path = handle.path().to_string_lossy().to_string();
                    let file_name = handle.file_name();
                    let lang = store_clone.lock().language();
                    let path_for_ui = path.clone();
                    let ui_weak_first = ui_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_first.upgrade() {
                            ui.set_new_task_url(SharedString::from(&path_for_ui));
                            if ui.get_new_task_filename().trim().is_empty() {
                                ui.set_new_task_filename(SharedString::from(&file_name));
                            }
                            // Torrent pre-selection: parse the file list now
                            ui.set_new_task_preview_state("loading".into());
                            ui.set_new_task_preview_status_text(SharedString::from(
                                i18n::format_preview_status("loading", "", lang),
                            ));
                            ui.set_new_task_preview_summary_text(SharedString::default());
                            ui.set_new_task_torrent_files(ModelRc::default());
                        }
                    });

                    let preview = dispatcher.bt_preview_torrent(&path).await;
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_weak.upgrade() else {
                            return;
                        };
                        match preview {
                            Ok(entries) => {
                                let total_bytes: u64 = entries.iter().map(|e| e.size).sum();
                                *entries_cache.lock() = entries.clone();
                                *included_cache.lock() = vec![true; entries.len()];
                                let items: Vec<NewTaskTorrentFileItem> = entries
                                    .iter()
                                    .map(|e| torrent_entry_to_item(e, true))
                                    .collect();
                                ui.set_new_task_torrent_files(
                                    Rc::new(VecModel::from(items)).into(),
                                );
                                ui.set_new_task_preview_state("ready".into());
                                ui.set_new_task_preview_status_text(SharedString::default());
                                ui.set_new_task_preview_summary_text(SharedString::from(
                                    i18n::format_preview_summary(
                                        entries.len(),
                                        &bridge::format_bytes(total_bytes),
                                        lang,
                                    ),
                                ));
                            }
                            Err(err) => {
                                entries_cache.lock().clear();
                                included_cache.lock().clear();
                                ui.set_new_task_torrent_files(ModelRc::default());
                                ui.set_new_task_preview_state("error".into());
                                ui.set_new_task_preview_status_text(SharedString::from(
                                    i18n::format_preview_status("error", &err.to_string(), lang),
                                ));
                            }
                        }
                    });
                }
            });
        });
    }

    // Pick Save Folder (Native Dialog)
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_pick_save_folder(move || {
            let ui_weak = ui_weak.clone();
            let store_clone = store_clone.clone();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                let folder = rfd::AsyncFileDialog::new()
                    .set_title(i18n::pick_download_dir_title(lang))
                    .pick_folder()
                    .await;

                if let Some(handle) = folder {
                    let path = handle.path().to_string_lossy().to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_new_task_dir(SharedString::from(&path));
                        }
                    });
                }
            });
        });
    }

    // Pick Default Save Folder in Settings (Native Dialog)
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_pick_default_folder(move || {
            let ui_weak = ui_weak.clone();
            let store_clone = store_clone.clone();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                let folder = rfd::AsyncFileDialog::new()
                    .set_title(i18n::pick_download_dir_title(lang))
                    .pick_folder()
                    .await;

                if let Some(handle) = folder {
                    let path = handle.path().to_string_lossy().to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut form = ui.get_settings_form();
                            form.default_download_dir = SharedString::from(&path);
                            ui.set_settings_form(form);
                        }
                    });
                }
            });
        });
    }

    // Copy URL to Clipboard
    {
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_copy_task_url(move |url| {
            let url_str = url.to_string();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&url_str);
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_link_copied(store_clone.lock().language()).to_string(),
                        "success",
                        Duration::from_secs(3),
                    );
                }
            });
        });
    }

    // Purge Single Task
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_purge_single_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();
            let id = id_str.to_string();

            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id) {
                    if let Err(err) = dispatcher.purge(&task_id).await {
                        tracing::error!("彻底删除任务失败: {err}");
                    } else {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut store = store_clone.lock();
                                store.remove(&id);
                                refresh_ui(&ui, &store);
                            }
                        });
                    }
                }
            });
        });
    }

    // Pick Log Folder (Native Dialog)
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_pick_log_folder(move || {
            let ui_weak = ui_weak.clone();
            let store_clone = store_clone.clone();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                let folder = rfd::AsyncFileDialog::new()
                    .set_title(i18n::pick_log_dir_title(lang))
                    .pick_folder()
                    .await;

                if let Some(handle) = folder {
                    let path = handle.path().join("limedl.log").to_string_lossy().to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut form = ui.get_settings_form();
                            form.logging_file_path = SharedString::from(&path);
                            ui.set_settings_form(form);
                        }
                    });
                }
            });
        });
    }

    // Open Log Folder in Explorer
    {
        let current_settings_clone = current_settings.clone();
        main_window.on_open_log_folder(move || {
            let settings = current_settings_clone.lock().clone();
            let log_path = if !settings.logging.file_path.trim().is_empty() {
                PathBuf::from(&settings.logging.file_path)
            } else {
                dirs_or_temp_dir().join("logs").join("limedl.log")
            };
            let parent_dir = log_path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let _ = std::fs::create_dir_all(parent_dir);
            let _ = open_path_in_explorer(&parent_dir.to_string_lossy());
        });
    }

    // Open the MiSans font license page (About tab attribution link)
    {
        main_window.on_open_font_license(move || {
            let _ = open_url_in_browser("https://hyperos.mi.com/font/zh/download");
        });
    }

    // ── In-app toast dismiss (close button) ───────────────────────────────
    {
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_dismiss_toast(move |id| {
            dismiss_toast(&ui_weak, &toast_queue_clone, id);
        });
    }

    // ── Task-list background menu: refresh ────────────────────────────────
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_background_refresh(move || {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                if let Ok(list) = dispatcher.list().await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut store = store_clone.lock();
                            store.replace_all(list);
                            refresh_ui(&ui, &store);
                        }
                    });
                }
            });
        });
    }

    // ── First-run setup wizard ────────────────────────────────────────────

    // Interrupted close: persist the current step so the wizard reopens there.
    {
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_close_setup_wizard(move |step| {
            // Sync the task-store language with the wizard's live language
            // selection (the @tr translation was already switched on change),
            // so interpolated Rust-side strings stay consistent after closing.
            if let Some(ui) = ui_weak.upgrade() {
                let form = ui.get_setup_form();
                let lang = if form.language_idx == 0 {
                    Language::ZhCn
                } else {
                    Language::EnUs
                };
                store_clone.lock().set_language(lang);
            }
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                let mut settings = {
                    let s = current_settings_clone.lock();
                    s.clone()
                };
                settings.last_setup_step = Some(step.max(0) as u32);
                match dispatcher.save_settings(&settings).await {
                    Ok(saved) => *current_settings_clone.lock() = saved,
                    Err(err) => tracing::warn!("保存设置向导进度失败: {err:#}"),
                }
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_show_setup_wizard(false);
                    }
                });
            });
        });
    }

    // Language selection in the wizard: switch @tr translation immediately.
    {
        let ui_weak = main_window.as_weak();
        main_window.on_setup_set_language(move |idx| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut form = ui.get_setup_form();
                form.language_idx = idx;
                ui.set_setup_form(form);
                let lang = if idx == 0 { Language::ZhCn } else { Language::EnUs };
                i18n::apply_translation(lang);
            }
        });
    }

    // Appearance selection in the wizard: live preview via Theme global.
    {
        let ui_weak = main_window.as_weak();
        main_window.on_setup_set_appearance(move |color_idx, theme_idx| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut form = ui.get_setup_form();
                form.color_mode_idx = color_idx;
                form.theme_color_idx = theme_idx;
                ui.set_setup_form(form);
                let mode = match color_idx {
                    1 => ColorMode::Light,
                    2 => ColorMode::Dark,
                    _ => ColorMode::System,
                };
                let theme = match theme_idx {
                    0 => ThemeColor::Amber,
                    1 => ThemeColor::Sky,
                    _ => ThemeColor::Lime,
                };
                apply_appearance(&ui, mode, theme);
            }
        });
    }

    // Directory picker for the wizard (native dialog).
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_pick_setup_directory(move || {
            let ui_weak = ui_weak.clone();
            let store_clone = store_clone.clone();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                let folder = rfd::AsyncFileDialog::new()
                    .set_title(i18n::pick_download_dir_title(lang))
                    .pick_folder()
                    .await;
                if let Some(handle) = folder {
                    let path = handle.path().to_string_lossy().to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut form = ui.get_setup_form();
                            form.default_dir = SharedString::from(&path);
                            ui.set_setup_form(form);
                        }
                    });
                }
            });
        });
    }

    // Finish / skip-all: persist the wizard settings and apply side effects
    // (language, appearance, default dir, autostart, Aria2 RPC hot reload).
    {
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let store_clone = store.clone();
        let pending_tray_lang_clone = pending_tray_lang.clone();
        let rpc_shutdown_clone = rpc_shutdown.clone();
        let toast_queue_clone = toast_queue.clone();
        let tray_speed_limit_setup = tray_speed_limit_active.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_finish_setup(move |form| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let store_clone = store_clone.clone();
            let pending_tray_lang_clone = pending_tray_lang_clone.clone();
            let rpc_shutdown = rpc_shutdown_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let tray_speed_limit_setup = tray_speed_limit_setup.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let old_settings = {
                    let s = current_settings_clone.lock();
                    s.clone()
                };
                let mut settings = old_settings.clone();
                let lang = store_clone.lock().language();

                if let Err(msg) = update_app_settings_from_setup_form(&mut settings, &form, lang) {
                    tracing::error!("设置向导表单校验失败: {msg}");
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_settings_invalid(&msg, lang),
                        "error",
                        Duration::from_secs(6),
                    );
                    return;
                }

                settings.setup_completed = true;
                settings.last_setup_step = Some(8);

                match dispatcher.save_settings(&settings).await {
                    Ok(saved) => {
                        // ── Autostart OS 同步 ──
                        if saved.autostart != old_settings.autostart
                            && let Err(e) = crate::autostart::set_enabled(saved.autostart)
                        {
                            tracing::warn!("autostart 切换失败: {e:#}");
                            push_toast(
                                &ui_weak,
                                &toast_queue,
                                i18n::format_toast_autostart_failed(&format!("{e:#}"), lang),
                                "error",
                                Duration::from_secs(6),
                            );
                        }
                        // ── Aria2 RPC 热重载（与设置页保存路径一致） ──
                        if saved.aria2_rpc != old_settings.aria2_rpc {
                            if let Some(tx) = rpc_shutdown.lock().take() {
                                let _ = tx.send(true);
                            }
                            if saved.aria2_rpc.enabled {
                                let (tx, rx) = watch::channel(false);
                                let rpc_server = Aria2RpcServer::new(
                                    dispatcher.registry().clone(),
                                    &saved.aria2_rpc,
                                    dispatcher.event_bus().clone(),
                                );
                                let cors = saved.aria2_rpc.cors_allowed_origins.clone();
                                let port = saved.aria2_rpc.port;
                                tokio::spawn(async move {
                                    if let Err(e) = rpc_server.serve(rx, cors).await {
                                        tracing::error!("Aria2 RPC server stopped: {e:#}");
                                    }
                                });
                                *rpc_shutdown.lock() = Some(tx);
                                tracing::info!("Aria2 RPC 已重启 (port: {port})");
                                push_toast(
                                    &ui_weak,
                                    &toast_queue,
                                    i18n::format_toast_aria2_rpc_started(port, lang),
                                    "info",
                                    Duration::from_secs(5),
                                );
                            } else {
                                push_toast(
                                    &ui_weak,
                                    &toast_queue,
                                    i18n::format_toast_aria2_rpc_stopped(lang).to_string(),
                                    "info",
                                    Duration::from_secs(5),
                                );
                            }
                        }

                        *current_settings_clone.lock() = saved.clone();
                        tray_speed_limit_setup.store(
                            saved.global_speed_limit_bps > 0,
                            Ordering::Relaxed,
                        );
                        let default_dir = saved.download.default_download_dir.clone();
                        let new_lang = Language::from_code(&saved.appearance.language);
                        let new_color_mode = saved.appearance.color_mode;
                        let new_theme_color = saved.appearance.theme_color;
                        *pending_tray_lang_clone.lock() = Some(new_lang);

                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_setup_finished(lang).to_string(),
                            "success",
                            Duration::from_secs(5),
                        );

                        let _ = slint::invoke_from_event_loop(move || {
                            i18n::apply_translation(new_lang);
                            if let Some(ui) = ui_weak.upgrade() {
                                apply_appearance(&ui, new_color_mode, new_theme_color);
                                let mut s = store_clone.lock();
                                s.set_language(new_lang);
                                ui.set_default_download_dir(SharedString::from(&default_dir));
                                ui.set_new_task_dir(SharedString::from(&default_dir));
                                refresh_ui(&ui, &s);
                                ui.set_show_setup_wizard(false);
                            }
                        });
                    }
                    Err(err) => {
                        tracing::error!("设置向导保存失败: {err:#}");
                        let msg = format!("{err:#}");
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_settings_save_failed(&msg, lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    // Re-run setup wizard from Settings → About: reset flags and open it.
    {
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_restart_setup(move || {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                let mut settings = {
                    let s = current_settings_clone.lock();
                    s.clone()
                };
                settings.setup_completed = false;
                settings.last_setup_step = None;
                let lang = store_clone.lock().language();
                let form = app_settings_to_setup_form(&settings, lang);
                match dispatcher.save_settings(&settings).await {
                    Ok(saved) => *current_settings_clone.lock() = saved,
                    Err(err) => tracing::warn!("保存设置向导重置状态失败: {err:#}"),
                }
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_setup_form(form);
                        ui.set_setup_start_step(0);
                        ui.set_show_settings(false);
                        ui.set_show_setup_wizard(true);
                    }
                });
            });
        });
    }

    // Factory reset: shut the backends down, delete the whole data directory
    // (settings.json + downloads/ incl. the SQLite database) and restart the
    // process so the first-run wizard comes back.
    {
        let dispatcher = core.dispatcher.clone();
        let registry = core.registry.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();
        let data_dir = base_dir.clone();
        main_window.on_factory_reset(move || {
            let dispatcher = dispatcher.clone();
            let registry = registry.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            let data_dir = data_dir.clone();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                // 1. Stop every backend so no file handle survives the wipe.
                registry.shutdown_all().await;
                // 2. Reset the in-memory settings first (the file is deleted
                //    afterwards, so a failure here must not leave stale state).
                let _ = dispatcher.factory_reset().await;
                // 3. Delete the data directory, retrying briefly for Windows
                //    file locking before giving up.
                let mut last_err: Option<std::io::Error> = None;
                for attempt in 0..3u8 {
                    match std::fs::remove_dir_all(&data_dir) {
                        Ok(()) => {
                            last_err = None;
                            break;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            last_err = None;
                            break;
                        }
                        Err(e) => {
                            last_err = Some(e);
                            if attempt < 2 {
                                tokio::time::sleep(Duration::from_millis(500)).await;
                            }
                        }
                    }
                }
                if let Some(e) = last_err {
                    tracing::error!("工厂重置失败: {e:#}");
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_factory_reset_failed(&format!("{e:#}"), lang),
                        "error",
                        Duration::from_secs(8),
                    );
                    return;
                }
                push_toast(
                    &ui_weak,
                    &toast_queue,
                    i18n::format_toast_factory_reset_done(lang).to_string(),
                    "success",
                    Duration::from_secs(4),
                );
                // 4. Relaunch into a fresh state (spawns a new process and
                //    exits the current one).
                POWER_GUARD.release();
                tokio::time::sleep(Duration::from_millis(600)).await;
                if let Err(e) = crate::update::restart_application() {
                    tracing::error!("工厂重置后重启失败: {e:#}");
                    std::process::exit(0);
                }
            });
        });
    }

    // ── Labs Callbacks ──────────────────────────────────────────────────

    // Open Labs Dialog
    {
        let current_settings_clone = current_settings.clone();
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let sandbox_test_url_clone = sandbox_test_url.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_open_labs(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let settings = current_settings_clone.lock().clone();
                let rules = rewrite_rules_clone.lock().clone();
                let exp = expanded_rule_ids_clone.lock().clone();
                let url = sandbox_test_url_clone.lock().clone();
                let cands = cdn_candidates_cache_clone.lock().clone();
                let lang = store_clone.lock().language();
                refresh_labs_state(&ui, &settings, &rules, &exp, &url, false, &cands, lang);
                ui.set_show_labs(true);
            }
        });
    }

    // Close Labs Dialog
    {
        let ui_weak = main_window.as_weak();
        main_window.on_close_labs(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_labs(false);
            }
        });
    }

    // Switch Labs Tab
    {
        let ui_weak = main_window.as_weak();
        main_window.on_set_labs_tab(move |tab| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_labs_tab(tab);
            }
        });
    }

    // Save Labs Configuration
    {
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let rewrite_rules_clone = rewrite_rules.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_save_labs(move |form_data| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let rewrite_rules_clone = rewrite_rules_clone.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let mut settings = {
                    let s = current_settings_clone.lock();
                    s.clone()
                };
                let lang = store_clone.lock().language();

                update_app_settings_from_labs_form(&mut settings, &form_data);
                settings.url_rewrite.rules = rewrite_rules_clone.lock().clone();

                match dispatcher.save_settings(&settings).await {
                    Ok(saved) => {
                        *current_settings_clone.lock() = saved;
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_labs_saved(lang).to_string(),
                            "success",
                            Duration::from_secs(4),
                        );
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_show_labs(false);
                            }
                        });
                    }
                    Err(err) => {
                        tracing::error!("保存实验室设置失败: {err:#}");
                        let msg = format!("{err:#}");
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_labs_save_failed(&msg, lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    // Start CDN Speed Test
    {
        let dispatcher = core.dispatcher.clone();
        let event_bus = core.event_bus.clone();
        let current_settings_clone = current_settings.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_start_cdn_test(move || {
            let dispatcher = dispatcher.clone();
            let event_bus = event_bus.clone();
            let current_settings_clone = current_settings_clone.clone();
            let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
            let store_clone = store_clone.clone();
            let toast_queue_clone = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();

            let form = if let Some(ui) = ui_weak.upgrade() {
                let mut form = ui.get_labs_form();
                let current_lang = store_clone.lock().language();
                form.cdn_is_testing = true;
                form.cdn_status_type = SharedString::from("testing");
                form.cdn_status_label = SharedString::from(match current_lang {
                    Language::ZhCn => "测速中",
                    Language::EnUs => "Testing",
                });
                form.cdn_phase_label = SharedString::from(match current_lang {
                    Language::ZhCn => "获取网段",
                    Language::EnUs => "Fetching IP ranges",
                });
                form.cdn_progress_percent = 0.0;
                form.cdn_progress_label = SharedString::from("0 / 0");
                form.cdn_last_error = SharedString::default();
                ui.set_labs_form(form.clone());
                form
            } else {
                return;
            };

            let mut settings = current_settings_clone.lock().clone();
            update_app_settings_from_labs_form(&mut settings, &form);

            tokio::spawn(async move {
                let Some(cs) = dispatcher.cdn_service() else {
                    return;
                };
                let cs = cs.clone();

                match cs.start_test(settings).await {
                    Ok(()) => {
                        let outcome = cs.monitor_test(event_bus).await;

                        let now_ms = limedl_core::now_ms();
                        if let Ok(mut current) = dispatcher.get_settings().await {
                            use limedl_core::cdn::accelerator::AccelState;
                            match &outcome.state {
                                AccelState::Ready => {
                                    current.cdn_acceleration.active_ip =
                                        outcome.active_ip.map(|i| i.to_string());
                                    current.cdn_acceleration.active_speed_mbps =
                                        outcome.active_speed_mbps;
                                    current.cdn_acceleration.last_test_at_ms = Some(now_ms);
                                    current.cdn_acceleration.last_error = None;
                                }
                                AccelState::Error(msg) => {
                                    current.cdn_acceleration.last_error = Some(msg.clone());
                                    current.cdn_acceleration.last_test_at_ms = Some(now_ms);
                                }
                                _ => {}
                            }
                            if let Ok(saved) = dispatcher.save_settings(&current).await {
                                *current_settings_clone.lock() = saved;
                            }
                        }

                        *cdn_candidates_cache_clone.lock() = outcome.candidates.clone();
                        let active_ip_str = outcome
                            .active_ip
                            .map(|i| i.to_string())
                            .unwrap_or_default();
                        let cands = outcome.candidates.clone();

                        let default_node_text = outcome.default_node.as_ref().and_then(|dn| {
                            dn.ip.as_deref().map(|ip| {
                                if let Some(spd) = dn.throughput_mbps {
                                    format!("{ip} ({spd:.2} MB/s)")
                                } else if dn.tcp_latency_ms > 0.0 {
                                    format!("{ip} ({:.1} ms)", dn.tcp_latency_ms)
                                } else {
                                    ip.to_string()
                                }
                            })
                        });

                        let speed_improvement = match (
                            outcome.active_speed_mbps,
                            outcome.default_node.as_ref().and_then(|dn| dn.throughput_mbps),
                        ) {
                            (Some(act), Some(base)) if base > 0.0 => {
                                let diff = (act - base) / base * 100.0;
                                Some(format!("{diff:+.1}%"))
                            }
                            _ => None,
                        };

                        let best_latency = outcome
                            .candidates
                            .iter()
                            .map(|c| c.tcp_latency_ms)
                            .fold(f64::INFINITY, f64::min);
                        let latency_improvement = match (
                            best_latency,
                            outcome.default_node.as_ref().map(|dn| dn.tcp_latency_ms),
                        ) {
                            (act, Some(base)) if act.is_finite() && base > 0.0 => {
                                let diff = (act - base) / base * 100.0;
                                Some(format!("{diff:+.1}%"))
                            }
                            _ => None,
                        };

                        let last_test_time = format_timestamp_ms(now_ms);
                        let is_ready = matches!(outcome.state, limedl_core::cdn::accelerator::AccelState::Ready);

                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                if let Some(ref text) = default_node_text {
                                    form.cdn_default_node_text = SharedString::from(text);
                                }
                                if let Some(ref text) = speed_improvement {
                                    form.cdn_speed_improvement_text = SharedString::from(text);
                                }
                                if let Some(ref text) = latency_improvement {
                                    form.cdn_latency_improvement_text = SharedString::from(text);
                                }
                                if is_ready {
                                    form.cdn_last_test_time = SharedString::from(last_test_time);
                                }
                                ui.set_labs_form(form);
                                ui.set_cdn_candidates(cdn_candidates_to_slint(
                                    &cands,
                                    &active_ip_str,
                                ));
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("启动 CDN 测速失败: {e:#}");
                        let err_msg = e.to_string();
                        let current_lang = store_clone.lock().language();
                        push_toast(
                            &ui_weak,
                            &toast_queue_clone,
                            i18n::format_toast_cdn_test_failed(&err_msg, current_lang),
                            "error",
                            Duration::from_secs(6),
                        );
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_is_testing = false;
                                form.cdn_status_type = SharedString::from("error");
                                form.cdn_status_label = SharedString::from(match current_lang {
                                    Language::ZhCn => "测速失败",
                                    Language::EnUs => "Failed",
                                });
                                form.cdn_last_error = SharedString::from(err_msg);
                                ui.set_labs_form(form);
                            }
                        });
                    }
                }
            });
        });
    }

    // Cancel CDN Speed Test
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_cancel_cdn_test(move || {
            if let Some(cs) = dispatcher.cdn_service() {
                cs.cancel_test();
                let current_lang = store_clone.lock().language();
                if let Some(ui) = ui_weak.upgrade() {
                    let mut form = ui.get_labs_form();
                    form.cdn_is_testing = false;
                    form.cdn_status_type = SharedString::from("idle");
                    form.cdn_status_label = SharedString::from(match current_lang {
                        Language::ZhCn => "已取消",
                        Language::EnUs => "Cancelled",
                    });
                    ui.set_labs_form(form);
                }
            }
        });
    }

    // Clear CDN Speed Test State
    {
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_clear_cdn_test(move || {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                if let Some(cs) = dispatcher.cdn_service() {
                    cs.clear().await;
                    cdn_candidates_cache_clone.lock().clear();
                    let lang = store_clone.lock().language();
                    let mut settings = current_settings_clone.lock().clone();
                    settings.cdn_acceleration.active_ip = None;
                    settings.cdn_acceleration.active_speed_mbps = None;
                    settings.cdn_acceleration.last_test_at_ms = None;
                    settings.cdn_acceleration.last_error = None;
                    if let Ok(saved) = dispatcher.save_settings(&settings).await {
                        *current_settings_clone.lock() = saved;
                    }
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_cdn_cleared(lang).to_string(),
                        "info",
                        Duration::from_secs(4),
                    );
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut form = ui.get_labs_form();
                            form.cdn_active_ip = SharedString::default();
                            form.cdn_active_speed_text = SharedString::default();
                            form.cdn_status_type = SharedString::from("idle");
                            form.cdn_status_label = SharedString::from(i18n::cdn_idle_label(lang));
                            ui.set_labs_form(form);
                            ui.set_cdn_candidates(cdn_candidates_to_slint(&[], ""));
                        }
                    });
                }
            });
        });
    }

    // Apply CDN Candidate IP
    {
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_apply_cdn_candidate(move |ip_str, speed_mbps| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            let ip_parsed = ip_str.parse::<IpAddr>();

            tokio::spawn(async move {
                if let (Some(cs), Ok(ip)) = (dispatcher.cdn_service(), ip_parsed) {
                    let lang = store_clone.lock().language();
                    let settings = current_settings_clone.lock().clone();
                    if let Ok(()) = cs.apply_ip(ip, speed_mbps as f64, &settings).await {
                        let mut updated_settings = settings.clone();
                        updated_settings.cdn_acceleration.active_ip = Some(ip.to_string());
                        updated_settings.cdn_acceleration.active_speed_mbps = Some(speed_mbps as f64);
                        if let Ok(saved) = dispatcher.save_settings(&updated_settings).await {
                            *current_settings_clone.lock() = saved;
                        }

                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_cdn_applied(&ip.to_string(), lang),
                            "success",
                            Duration::from_secs(4),
                        );

                        let cands = cdn_candidates_cache_clone.lock().clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_active_ip = SharedString::from(ip.to_string());
                                form.cdn_active_speed_text = SharedString::from(format!("{speed_mbps:.2} MB/s"));
                                form.cdn_status_type = SharedString::from("ready");
                                form.cdn_status_label = SharedString::from(i18n::cdn_ready_label(lang));
                                ui.set_labs_form(form);
                                ui.set_cdn_candidates(cdn_candidates_to_slint(&cands, &ip.to_string()));
                            }
                        });
                    }
                }
            });
        });
    }

    // Toggle CDN Advanced Panel
    {
        let ui_weak = main_window.as_weak();
        main_window.on_toggle_cdn_advanced(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let mut form = ui.get_labs_form();
                form.cdn_show_advanced = !form.cdn_show_advanced;
                ui.set_labs_form(form);
            }
        });
    }

    // Apply Manual CDN IP
    {
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let store_cl = store.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_apply_manual_cdn_ip(move |ip_str| {
            let ip_trimmed = ip_str.trim().to_string();
            match ip_trimmed.parse::<IpAddr>() {
                Ok(ip) => {
                    let dispatcher = dispatcher.clone();
                    let current_settings_clone = current_settings_clone.clone();
                    let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
                    let store_clone = store_cl.clone();
                    let toast_queue_clone = toast_queue.clone();
                    let ui_weak = ui_weak.clone();

                    tokio::spawn(async move {
                        if let Some(cs) = dispatcher.cdn_service() {
                            let lang = store_clone.lock().language();
                            let settings = current_settings_clone.lock().clone();
                            if let Ok(()) = cs.apply_ip(ip, 0.0, &settings).await {
                                let mut updated_settings = settings.clone();
                                updated_settings.cdn_acceleration.active_ip = Some(ip.to_string());
                                if let Ok(saved) = dispatcher.save_settings(&updated_settings).await {
                                    *current_settings_clone.lock() = saved;
                                }

                                push_toast(
                                    &ui_weak,
                                    &toast_queue_clone,
                                    i18n::format_toast_cdn_applied(&ip.to_string(), lang),
                                    "success",
                                    Duration::from_secs(4),
                                );

                                let cands = cdn_candidates_cache_clone.lock().clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_weak.upgrade() {
                                        let mut form = ui.get_labs_form();
                                        form.cdn_active_ip = SharedString::from(ip.to_string());
                                        form.cdn_manual_ip_error = SharedString::default();
                                        form.cdn_status_type = SharedString::from("ready");
                                        form.cdn_status_label = SharedString::from(i18n::cdn_ready_label(lang));
                                        ui.set_labs_form(form);
                                        ui.set_cdn_candidates(cdn_candidates_to_slint(&cands, &ip.to_string()));
                                    }
                                });
                            }
                        }
                    });
                }
                Err(_) => {
                    if let Some(ui) = ui_weak.upgrade() {
                        let mut form = ui.get_labs_form();
                        form.cdn_manual_ip_error =
                            SharedString::from(i18n::format_invalid_ip(store_cl.lock().language()));
                        ui.set_labs_form(form);
                    }
                }
            }
        });
    }

    // ── URL Rewrite Rule Callbacks ──────────────────────────────────────

    // Import Presets
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let sandbox_test_url_clone = sandbox_test_url.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_import_rewrite_preset(move |preset_key| {
            let lang = store_clone.lock().language();
            if let Some(rule) = create_url_rewrite_preset(&preset_key, lang) {
                let mut rules = rewrite_rules_clone.lock();
                rules.retain(|r| r.name != rule.name);
                rules.push(rule);

                if let Some(ui) = ui_weak.upgrade() {
                    let exp = expanded_rule_ids_clone.lock();
                    let test_url = sandbox_test_url_clone.lock();
                    let (matched, cands) = evaluate_url_rewrite(&rules, &test_url);
                    let mut form = ui.get_labs_form();
                    form.url_rewrite_test_matched_rule = SharedString::from(matched);
                    form.url_rewrite_test_candidates_count = cands.len() as i32;
                    form.url_rewrite_test_result_1 = SharedString::from(cands.first().cloned().unwrap_or_default());
                    form.url_rewrite_test_result_2 = SharedString::from(cands.get(1).cloned().unwrap_or_default());
                    form.url_rewrite_test_result_3 = SharedString::from(cands.get(2).cloned().unwrap_or_default());
                    ui.set_labs_form(form);
                    ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
                }
            }
        });
    }

    // Add Custom Rule
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_add_custom_rule(move || {
            let mut rules = rewrite_rules_clone.lock();
            let id = format!("rule-{}", uuid::Uuid::new_v4().simple());
            let order = rules.len() as u32;
            expanded_rule_ids_clone.lock().insert(id.clone());
            rules.push(UrlRewriteRule {
                id,
                name: i18n::new_rewrite_rule_name(store_clone.lock().language()).to_string(),
                enabled: true,
                match_type: MatchType::Host,
                pattern: "*.example.com".to_string(),
                replacement_mode: ReplacementMode::PrefixProxy,
                encode_url: true,
                fallback_to_original: true,
                order,
                targets: vec![RewriteTarget {
                    url_template: "https://mirror.example.com".to_string(),
                    enabled: true,
                    order: 0,
                }],
            });
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }

    // Remove Rule
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let sandbox_test_url_clone = sandbox_test_url.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_remove_rule(move |idx| {
            let mut rules = rewrite_rules_clone.lock();
            if (idx as usize) < rules.len() {
                let removed = rules.remove(idx as usize);
                expanded_rule_ids_clone.lock().remove(&removed.id);
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                let test_url = sandbox_test_url_clone.lock();
                let (matched, cands) = evaluate_url_rewrite(&rules, &test_url);
                let mut form = ui.get_labs_form();
                form.url_rewrite_test_matched_rule = SharedString::from(matched);
                form.url_rewrite_test_candidates_count = cands.len() as i32;
                form.url_rewrite_test_result_1 = SharedString::from(cands.first().cloned().unwrap_or_default());
                form.url_rewrite_test_result_2 = SharedString::from(cands.get(1).cloned().unwrap_or_default());
                form.url_rewrite_test_result_3 = SharedString::from(cands.get(2).cloned().unwrap_or_default());
                ui.set_labs_form(form);
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }

    // Toggle Rule Expanded
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_toggle_rule_expanded(move |idx| {
            let rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get(idx as usize) {
                let mut exp = expanded_rule_ids_clone.lock();
                if exp.contains(&rule.id) {
                    exp.remove(&rule.id);
                } else {
                    exp.insert(rule.id.clone());
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }

    // Toggle Rule Enabled
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let sandbox_test_url_clone = sandbox_test_url.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_toggle_rule_enabled(move |idx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.enabled = !rule.enabled;
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                let test_url = sandbox_test_url_clone.lock();
                let (matched, cands) = evaluate_url_rewrite(&rules, &test_url);
                let mut form = ui.get_labs_form();
                form.url_rewrite_test_matched_rule = SharedString::from(matched);
                form.url_rewrite_test_candidates_count = cands.len() as i32;
                form.url_rewrite_test_result_1 = SharedString::from(cands.first().cloned().unwrap_or_default());
                form.url_rewrite_test_result_2 = SharedString::from(cands.get(1).cloned().unwrap_or_default());
                form.url_rewrite_test_result_3 = SharedString::from(cands.get(2).cloned().unwrap_or_default());
                ui.set_labs_form(form);
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }

    // Update Rule Fields
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_update_rule_name(move |idx, val| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.name = val.to_string();
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_update_rule_match_type(move |idx, val| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.match_type = str_to_match_type(val.as_str());
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_update_rule_pattern(move |idx, val| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.pattern = val.to_string();
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_update_rule_mode(move |idx, val| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.replacement_mode = str_to_replacement_mode(val.as_str());
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_toggle_rule_encode(move |idx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.encode_url = !rule.encode_url;
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_toggle_rule_fallback(move |idx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.fallback_to_original = !rule.fallback_to_original;
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_add_rule_target(move |idx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                let order = rule.targets.len() as u32;
                rule.targets.push(RewriteTarget {
                    url_template: "https://mirror.example.com".to_string(),
                    enabled: true,
                    order,
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_remove_rule_target(move |ridx, tidx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(ridx as usize)
                && (tidx as usize) < rule.targets.len()
            {
                rule.targets.remove(tidx as usize);
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_update_rule_target(move |ridx, tidx, val| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(ridx as usize)
                && let Some(target) = rule.targets.get_mut(tidx as usize)
            {
                target.url_template = val.to_string();
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_toggle_rule_target(move |ridx, tidx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(ridx as usize)
                && let Some(target) = rule.targets.get_mut(tidx as usize)
            {
                target.enabled = !target.enabled;
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }

    // Live Test Sandbox Input Changed
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let sandbox_test_url_clone = sandbox_test_url.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_test_url_changed(move |val| {
            *sandbox_test_url_clone.lock() = val.to_string();
            let rules = rewrite_rules_clone.lock();
            let (matched, cands) = evaluate_url_rewrite(&rules, &val);

            if let Some(ui) = ui_weak.upgrade() {
                let mut form = ui.get_labs_form();
                form.url_rewrite_test_matched_rule = SharedString::from(matched);
                form.url_rewrite_test_candidates_count = cands.len() as i32;
                form.url_rewrite_test_result_1 = SharedString::from(cands.first().cloned().unwrap_or_default());
                form.url_rewrite_test_result_2 = SharedString::from(cands.get(1).cloned().unwrap_or_default());
                form.url_rewrite_test_result_3 = SharedString::from(cands.get(2).cloned().unwrap_or_default());
                ui.set_labs_form(form);
            }
        });
    }

    // ── Self-update callbacks ────────────────────────────────────────────────
    {
        let ui_weak = main_window.as_weak();
        let base_dir = base_dir.clone();
        let available = available_update.clone();
        main_window.on_check_for_updates(move || {
            let ui_weak = ui_weak.clone();
            let base_dir = base_dir.clone();
            let available = available.clone();

            let ui = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                push_update_state(&ui, |st| {
                    st.phase = "checking".into();
                    st.error_text = "".into();
                });
            });

            if update::detect_install_kind() == update::InstallKind::Store {
                // StoreContext::GetDefault must run on the UI thread.
                let _ = slint::spawn_local(async move {
                    match update::store::check_update_available().await {
                        Ok(available) => {
                            push_update_state(&ui_weak, |st| {
                                st.phase = if available { "available".into() } else { "up-to-date".into() };
                                st.error_text = "".into();
                            });
                        }
                        Err(e) => set_update_error(&ui_weak, &format!("{e:#}")),
                    }
                });
                return;
            }

            tokio::spawn(async move {
                update::record_update_check(&base_dir);
                match update::check_for_update().await {
                    Ok(Some(upd)) => {
                        let version = upd.version.clone();
                        let notes = upd.notes.clone();
                        *available.lock() = Some(upd);
                        let _ = slint::invoke_from_event_loop(move || {
                            push_update_state(&ui_weak, |st| {
                                st.phase = "available".into();
                                st.latest_version = version.into();
                                st.notes = notes.into();
                                st.error_text = "".into();
                            });
                        });
                    }
                    Ok(None) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            push_update_state(&ui_weak, |st| {
                                st.phase = "up-to-date".into();
                                st.latest_version = "".into();
                                st.notes = "".into();
                                st.error_text = "".into();
                            });
                        });
                    }
                    Err(e) => {
                        let msg = format!("{e:#}");
                        let _ = slint::invoke_from_event_loop(move || {
                            set_update_error(&ui_weak, &msg);
                        });
                    }
                }
            });
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let base_dir = base_dir.clone();
        let available = available_update.clone();
        main_window.on_start_update_download(move || {
            let Some(upd) = available.lock().clone() else {
                return;
            };
            let ui_weak = ui_weak.clone();
            let base_dir = base_dir.clone();

            if update::detect_install_kind() == update::InstallKind::Store {
                let _ = slint::spawn_local(async move {
                    if let Err(e) = update::store::trigger_update().await {
                        set_update_error(&ui_weak, &format!("{e:#}"));
                    }
                    // On success the OS replaces the package and relaunches the app.
                });
                return;
            }

            tokio::spawn(async move {
                push_update_state(&ui_weak, |st| {
                    st.phase = "downloading".into();
                    st.progress_percent = 0.0;
                    st.progress_label = "".into();
                    st.error_text = "".into();
                });

                let prog_ui = ui_weak.clone();
                let last_emit = parking_lot::Mutex::new(
                    std::time::Instant::now() - Duration::from_secs(10),
                );
                let progress = move |done: u64, total: Option<u64>| {
                    let now = std::time::Instant::now();
                    let finished = total.is_some_and(|t| done >= t);
                    if !finished && now - *last_emit.lock() < Duration::from_millis(150) {
                        return;
                    }
                    *last_emit.lock() = now;
                    let pct = total
                        .map(|t| done as f32 / t as f32 * 100.0)
                        .unwrap_or(0.0);
                    let label = match total {
                        Some(t) => format!(
                            "{:.1} / {:.1} MB",
                            done as f64 / 1_048_576.0,
                            t as f64 / 1_048_576.0
                        ),
                        None => format!("{:.1} MB", done as f64 / 1_048_576.0),
                    };
                    let _ = slint::invoke_from_event_loop({
                        let prog_ui = prog_ui.clone();
                        move || {
                            push_update_state(&prog_ui, |st| {
                                st.progress_percent = pct;
                                st.progress_label = label.into();
                            });
                        }
                    });
                };

                let verified = match update::download_and_verify(&upd, &base_dir, &progress).await {
                    Ok(file) => file,
                    Err(e) => {
                        let msg = format!("{e:#}");
                        let _ = slint::invoke_from_event_loop(move || {
                            set_update_error(&ui_weak, &msg);
                        });
                        return;
                    }
                };

                match update::install_verified(&upd, &verified) {
                    Ok(update::InstallOutcome::ReplacedRestartPending) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            push_update_state(&ui_weak, |st| st.phase = "ready".into());
                        });
                    }
                    Ok(update::InstallOutcome::InstallerLaunched) => {
                        // The NSIS installer takes over: stop this process now
                        // so it can replace the executable and relaunch us.
                        std::process::exit(0);
                    }
                    Err(e) => {
                        let msg = format!("{e:#}");
                        let _ = slint::invoke_from_event_loop(move || {
                            set_update_error(&ui_weak, &msg);
                        });
                    }
                }
            });
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_restart_after_update(move || {
            // On success this never returns (spawns the new binary, exits).
            if let Err(e) = update::restart_application() {
                set_update_error(&ui_weak, &format!("restart failed: {e:#}"));
            }
        });
    }

    // ── Background silent update check (GitHub channel only) ──────────────
    // Store installs update via the OS, so there is nothing to poll here.
    if update::detect_install_kind() != update::InstallKind::Store {
        let ui_weak = main_window.as_weak();
        let base_dir = base_dir.clone();
        let available = available_update.clone();
        let current_settings_clone = current_settings.clone();
        let lang = initial_lang;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(45)).await;
            if !update::should_check_for_update_now(&base_dir, Duration::from_secs(24 * 60 * 60)) {
                return;
            }
            update::record_update_check(&base_dir);
            match update::check_for_update().await {
                Ok(Some(upd)) => {
                    let version = upd.version.clone();
                    *available.lock() = Some(upd);
                    let _ = slint::invoke_from_event_loop({
                        let version = version.clone();
                        move || {
                            push_update_state(&ui_weak, |st| {
                                st.phase = "available".into();
                                st.latest_version = version.into();
                                st.error_text = "".into();
                            });
                        }
                    });
                    if current_settings_clone.lock().notifications.enabled {
                        let (title, body) = i18n::format_notification_update(&version, lang);
                        let mut notif = Notification::new();
                        notif.appname("limedl")
                            .summary(&title)
                            .body(&body);
                        #[cfg(windows)]
                        {
                            notif.app_id("limedl");
                        }
                        let _ = notif.show();
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::debug!("background update check failed: {e:#}"),
            }
        });
    }

    // Cold start with command line arguments (e.g. magnet link, torrent file, or deep link)
    if let Some(ref arg) = cli_payload {
        open_new_task_with_payload(
            arg,
            &main_window.as_weak(),
            &core.dispatcher,
            &store,
            &new_task_torrent_entries,
            &new_task_torrent_included,
        );
    }

    // Window close behavior (Settings → Appearance): either minimize to the tray
    // or exit. Implemented explicitly because the event loop is started with
    // `run_event_loop_until_quit()` (needed for the tray-only `--hidden` start),
    // which no longer quits when the last window is closed.
    {
        let ui_weak = main_window.as_weak();
        let current_settings_clone = current_settings.clone();
        main_window.window().on_close_requested(move || {
            let minimize_to_tray = matches!(
                current_settings_clone.lock().appearance.close_behavior,
                CloseBehavior::MinimizeToTray
            );
            if minimize_to_tray {
                if let Some(ui) = ui_weak.upgrade() {
                    let _ = ui.hide();
                }
                tracing::info!("窗口已最小化到托盘");
            } else {
                tracing::info!("窗口关闭，退出应用");
                let _ = slint::quit_event_loop();
            }
            CloseRequestResponse::HideWindow
        });
    }

    // Tray-only start. Two sources:
    //  - `--hidden`, passed by the registry / `.desktop` / LaunchAgent autostart
    //    registrations;
    //  - the MSIX channel, whose `windows.startupTask` entry cannot carry
    //    arguments, so a launch shortly after logon (with autostart enabled and
    //    no payload) is attributed to the startup task.
    let msix_login_launch = !hidden_flag
        && cli_payload.is_none()
        && initial_settings.autostart
        && update::has_package_identity()
        && platform_win::launched_at_logon(MSIX_LOGIN_LAUNCH_WINDOW);
    let start_hidden = should_start_hidden(
        hidden_flag,
        msix_login_launch,
        initial_settings.setup_completed,
    );
    if start_hidden {
        tracing::info!(
            "以静默模式启动（仅托盘；来源：{}）",
            if hidden_flag { "--hidden" } else { "MSIX 登录启动" }
        );
    }

    // Poll for pending tray menu/tooltip updates on the main thread (TrayIcon is !Send)
    let _tray_update_timer = slint::Timer::default();
    _tray_update_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(250),
        {
            let pending_tray_lang_clone = pending_tray_lang.clone();
            let tray_speed_limit_clone = tray_speed_limit_active.clone();
            move || {
                if let Some(lang) = pending_tray_lang_clone.lock().take() {
                    tray_icon.set_menu(Some(Box::new(build_tray_menu(
                        lang,
                        tray_speed_limit_clone.load(Ordering::Relaxed),
                    ))));
                    let _ = tray_icon.set_tooltip(Some(i18n::get_tray_strings(lang).tooltip));
                }
            }
        },
    );

    // `run_event_loop_until_quit` rather than `MainWindow::run()`: with
    // `--hidden` no window is ever shown, and the default loop would return
    // immediately (it counts visible windows, and our tray lives in
    // `tray-icon`/`muda` rather than Slint's SystemTrayIcon). Exiting is driven
    // by the tray "Quit" item and the window close handler above, both of which
    // call `quit_event_loop()`.
    if !start_hidden {
        main_window.show()?;
    }
    slint::run_event_loop_until_quit()?;

    // Graceful shutdown
    POWER_GUARD.release();
    tracing::info!("Native UI 正在退出，关闭核心引擎...");
    // Stop Aria2 RPC server first
    if let Some(tx) = rpc_shutdown.lock().take() {
        let _ = tx.send(true);
    }
    core.registry.shutdown_all().await;

    Ok(())
}

/// Push a one-shot toast telling the user that data was migrated from the
/// Tauri edition (shown once, right after the window exists).
fn announce_migration(
    report: &migrate::MigrationReport,
    ui_weak: &slint::Weak<MainWindow>,
    queue: &ToastQueue,
    lang: Language,
) {
    let msg = i18n::format_toast_tauri_migration(report.copied_files, lang);
    push_toast(ui_weak, queue, msg, "info", Duration::from_secs(8));
}


fn open_path_in_explorer(path: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(path).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
}

/// Open a task's downloaded file with the OS default handler (via backend).
/// The `Dispatcher` has no inherent `open_file`/`open_dir`, so route through
/// the backend registry exactly like the RPC handlers do.
async fn open_task_file(dispatcher: &Dispatcher, task_id: &TaskId) -> anyhow::Result<()> {
    let backend = dispatcher.registry().dispatch(task_id)?;
    Ok(backend.open_file(task_id).await?)
}

/// Open a task's download directory in the file explorer (via backend).
async fn open_task_dir(dispatcher: &Dispatcher, task_id: &TaskId) -> anyhow::Result<()> {
    let backend = dispatcher.registry().dispatch(task_id)?;
    Ok(backend.open_dir(task_id).await?)
}

/// Execute the configured double-click behavior for a task. Mirrors the web
/// client's semantics: completed tasks open the file / explorer / download
/// dir; uncompleted tasks toggle pause/resume (pause for queued/downloading/
/// retrying/verifying, resume for paused/failed).
async fn handle_task_double_click(
    dispatcher: &Dispatcher,
    store: &Mutex<TaskStore>,
    current_settings: &Mutex<AppSettings>,
    id_str: &str,
) -> anyhow::Result<()> {
    // Snapshot the task state + configured behavior.
    let (state, double_click) = {
        let store = store.lock();
        let Some(summary) = store.get_summary(id_str) else {
            return Ok(());
        };
        let settings = current_settings.lock();
        (summary.state, settings.double_click.clone())
    };
    let task_id = TaskId::from_wire_string(id_str)?;

    if matches!(state, DownloadState::Completed) {
        match double_click.on_completed {
            DoubleClickOnCompleted::None => {}
            DoubleClickOnCompleted::OpenFile => {
                open_task_file(dispatcher, &task_id).await?;
            }
            DoubleClickOnCompleted::OpenInExplorer => {
                dispatcher.open_in_explorer(&task_id).await?;
            }
            DoubleClickOnCompleted::OpenDownloadDir => {
                open_task_dir(dispatcher, &task_id).await?;
            }
        }
    } else {
        match double_click.on_uncompleted {
            DoubleClickOnUncompleted::None => {}
            DoubleClickOnUncompleted::TogglePauseResume => {
                if matches!(
                    state,
                    DownloadState::Queued
                        | DownloadState::Downloading
                        | DownloadState::Retrying
                        | DownloadState::Verifying
                ) {
                    dispatcher.pause(&task_id).await?;
                } else if matches!(state, DownloadState::Paused | DownloadState::Failed) {
                    dispatcher.resume(&task_id).await?;
                }
            }
        }
    }
    Ok(())
}

/// Opens a URL in the system default browser.
fn open_url_in_browser(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        // explorer.exe hands URLs to the default browser without cmd quoting quirks.
        std::process::Command::new("explorer").arg(url).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}

// ── New-task dialog: batch URL parsing ──────────────────────────────

/// Split batch text into expanded download URLs. Blank lines and lines
/// starting with `#` are skipped; `[01-20]` style ranges are expanded.
fn parse_batch_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        urls.extend(expand_url_ranges(trimmed));
    }
    urls
}

/// Expand the first `[start-end]` numeric range in a URL (same semantics as
/// the Vue composer's `expandUrlRanges`: first occurrence only, zero-padded
/// to the width of the start token). A safety cap of 1000 expansions guards
/// against accidental giant ranges.
fn expand_url_ranges(url: &str) -> Vec<String> {
    let Some(open) = url.find('[') else {
        return vec![url.to_string()];
    };
    let after_open = &url[open + 1..];
    let Some(close) = after_open.find(']') else {
        return vec![url.to_string()];
    };
    let inner = &after_open[..close];
    let Some((start_raw, end_raw)) = inner.split_once('-') else {
        return vec![url.to_string()];
    };
    if start_raw.is_empty()
        || end_raw.is_empty()
        || !start_raw.chars().all(|c| c.is_ascii_digit())
        || !end_raw.chars().all(|c| c.is_ascii_digit())
    {
        return vec![url.to_string()];
    }
    let Ok(start) = start_raw.parse::<u64>() else {
        return vec![url.to_string()];
    };
    let Ok(end) = end_raw.parse::<u64>() else {
        return vec![url.to_string()];
    };
    if start > end || end - start >= 1000 {
        return vec![url.to_string()];
    }

    let pattern = format!("[{inner}]");
    (start..=end)
        .map(|i| {
            let mut replacement = i.to_string();
            while replacement.len() < start_raw.len() {
                replacement.insert(0, '0');
            }
            url.replacen(&pattern, &replacement, 1)
        })
        .collect()
}

/// Best-effort filename for a batch entry: the last percent-decoded path
/// segment of an HTTP(S) URL (matching the Vue composer's per-entry fileName).
fn extract_batch_file_name(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return None;
    }
    let segment = reqwest::Url::parse(trimmed)
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|mut segments| segments.next_back().map(ToOwned::to_owned))
        })
        .map(|seg| percent_decode(&seg))
        .unwrap_or_default();
    (!segment.is_empty()).then_some(segment)
}

/// Minimal percent-decoding for URL path segments (UTF-8 lossy).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len()
            && let Ok(value) = u8::from_str_radix(&input[i + 1..i + 3], 16)
        {
            out.push(value);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn dirs_or_temp_dir() -> PathBuf {
    // Explicit override (same env var the headless server honors) — makes it
    // possible to run a throwaway instance against a temp data dir.
    if let Some(dir) = std::env::var_os("LIMEDL_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(dir) = dirs_local_data_dir() {
        dir.join("limedl")
    } else {
        std::env::temp_dir().join("limedl")
    }
}

fn dirs_local_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_hidden_requires_a_reason_and_a_finished_setup() {
        // First run always shows the wizard.
        assert!(!should_start_hidden(true, false, false));
        assert!(!should_start_hidden(false, true, false));
        // `--hidden` and the MSIX login heuristic both hide the window.
        assert!(should_start_hidden(true, false, true));
        assert!(should_start_hidden(false, true, true));
        // A plain manual launch stays visible.
        assert!(!should_start_hidden(false, false, true));
    }

    #[test]
    fn test_expand_url_ranges() {
        assert_eq!(
            expand_url_ranges("https://host/file[01-03].zip"),
            vec![
                "https://host/file01.zip",
                "https://host/file02.zip",
                "https://host/file03.zip",
            ]
        );
        // No padding when the start token is not zero-padded
        assert_eq!(
            expand_url_ranges("https://host/file[1-2].zip"),
            vec!["https://host/file1.zip", "https://host/file2.zip"]
        );
        // Only the first range expands (mirrors the Vue composer)
        assert_eq!(
            expand_url_ranges("https://host/a[1-2]b[3-4].zip"),
            vec!["https://host/a1b[3-4].zip", "https://host/a2b[3-4].zip"]
        );
        // Non-range inputs pass through unchanged
        assert_eq!(expand_url_ranges("https://host/a.zip"), vec!["https://host/a.zip"]);
        assert_eq!(expand_url_ranges("https://host/a[-1].zip"), vec!["https://host/a[-1].zip"]);
        assert_eq!(expand_url_ranges("https://host/a[x-y].zip"), vec!["https://host/a[x-y].zip"]);
        // Reversed range: no expansion
        assert_eq!(expand_url_ranges("https://host/a[3-1].zip"), vec!["https://host/a[3-1].zip"]);
        // Giant range guard
        assert_eq!(expand_url_ranges("https://host/a[1-1001].zip"), vec!["https://host/a[1-1001].zip"]);
    }

    #[test]
    fn test_parse_batch_urls() {
        let text = "\
https://host/a.zip\n\
\n\
# comment line\n\
  https://host/b[1-2].zip  \n\
magnet:?xt=urn:btih:abcdef\n";
        assert_eq!(
            parse_batch_urls(text),
            vec![
                "https://host/a.zip",
                "https://host/b1.zip",
                "https://host/b2.zip",
                "magnet:?xt=urn:btih:abcdef",
            ]
        );
        assert!(parse_batch_urls("# only comments\n\n").is_empty());
    }

    #[test]
    fn test_extract_batch_file_name() {
        assert_eq!(
            extract_batch_file_name("https://host.com/path/to/file%20name.zip").as_deref(),
            Some("file name.zip")
        );
        assert_eq!(extract_batch_file_name("https://host.com/dir/").as_deref(), None);
        assert_eq!(extract_batch_file_name("magnet:?xt=urn:btih:ab").as_deref(), None);
        assert_eq!(extract_batch_file_name("not a url").as_deref(), None);
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("a%20b+c"), "a b+c"); // '+' is not decoded in paths
        assert_eq!(percent_decode("a+b"), "a+b");
        assert_eq!(percent_decode("%E4%B8%AD%E6%96%87.zip"), "中文.zip");
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("bad%zz"), "bad%zz");
        assert_eq!(percent_decode("trunc%2"), "trunc%2");
    }
}
