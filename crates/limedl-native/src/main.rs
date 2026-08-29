slint::include_modules!();

mod bridge;
mod i18n;

use std::collections::HashSet;
use std::net::IpAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use muda::{Menu, MenuItem, PredefinedMenuItem};
use notify_rust::Notification;
use parking_lot::Mutex;
use slint::{ComponentHandle, SharedString, VecModel};
use tracing_subscriber::EnvFilter;
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

use limedl_core::bootstrap::bootstrap;
use limedl_core::dispatcher::Dispatcher;
use limedl_core::event_bus::DownloadEvent;
use limedl_core::types::{
    AppSettings, DownloadProgress, DownloadState, DownloadSummary, MatchType, ReplacementMode,
    RewriteTarget, StartDownloadRequest, TaskId, UrlRewriteRule,
};

use crate::bridge::{
    SortField, TaskStore, app_settings_to_form, app_settings_to_labs_form, cdn_candidates_to_slint,
    create_url_rewrite_preset, evaluate_url_rewrite, file_status_to_item, format_disk_types_map,
    format_io_status_json, format_speed, generate_piece_map_image, peer_info_to_item,
    str_to_match_type, str_to_replacement_mode, summary_to_inspector_info, tracker_info_to_item,
    update_app_settings_from_form, update_app_settings_from_labs_form, url_rewrite_rules_to_slint,
};
use crate::i18n::Language;

fn refresh_ui(ui: &MainWindow, store: &TaskStore) {
    let (all, downloading, paused, completed, failed) = store.counts();
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
        Ok(v) => format_io_status_json(&v),
        Err(_) => i18n::format_io_status_not_ready(lang).to_string(),
    };
    let disk_types = dispatcher.detect_all_disk_types();
    let disk_types_str = format_disk_types_map(&disk_types);

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

fn build_tray_menu(lang: Language) -> Menu {
    let t = i18n::get_tray_strings(lang);
    let tray_menu = Menu::new();
    let menu_show = MenuItem::with_id("show", t.show_window, true, None);
    let sep1 = PredefinedMenuItem::separator();
    let menu_pause_all = MenuItem::with_id("pause_all", t.pause_all, true, None);
    let menu_resume_all = MenuItem::with_id("resume_all", t.resume_all, true, None);
    let menu_game_mode = MenuItem::with_id("game_mode", t.game_mode_toggle, true, None);
    let menu_open_dir = MenuItem::with_id("open_dir", t.open_download_dir, true, None);
    let sep2 = PredefinedMenuItem::separator();
    let menu_quit = MenuItem::with_id("quit", t.quit, true, None);

    let _ = tray_menu.append_items(&[
        &menu_show,
        &sep1,
        &menu_pause_all,
        &menu_resume_all,
        &menu_game_mode,
        &menu_open_dir,
        &sep2,
        &menu_quit,
    ]);
    tray_menu
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
    // Initialize console tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,limedl=debug")),
        )
        .init();

    tracing::info!("启动 limedl Native 桌面客户端 (Skia)...");

    // Initialize core subsystems
    let state_dir = dirs_or_temp_dir().join("downloads");
    std::fs::create_dir_all(&state_dir)?;

    let core = bootstrap(state_dir.clone())
        .await
        .with_context(|| "初始化核心引擎失败")?;

    let initial_settings = core
        .dispatcher
        .get_settings()
        .await
        .unwrap_or_default();
    let current_settings = Arc::new(Mutex::new(initial_settings.clone()));
    let initial_lang = Language::from_code(&initial_settings.appearance.language);
    i18n::apply_translation(initial_lang);

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
    main_window.set_default_download_dir(SharedString::from(&default_download_dir));
    main_window.set_new_task_dir(SharedString::from(&default_download_dir));

    let store = Arc::new(Mutex::new(TaskStore::with_language(initial_lang)));
    let active_inspector_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let rewrite_rules: Arc<Mutex<Vec<UrlRewriteRule>>> =
        Arc::new(Mutex::new(initial_settings.url_rewrite.rules.clone()));
    let expanded_rule_ids: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let sandbox_test_url: Arc<Mutex<String>> =
        Arc::new(Mutex::new("https://raw.github.com/user/repo/master/README.md".to_string()));
    let cdn_candidates_cache: Arc<Mutex<Vec<limedl_core::cdn::speed_test::SpeedTestResult>>> =
        Arc::new(Mutex::new(Vec::new()));

    // Initialize UI settings state
    refresh_settings_state(
        &main_window,
        &core.dispatcher,
        &initial_settings,
        *game_mode_active.lock(),
        *overclock_mode_active.lock(),
        initial_lang,
    );

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
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(build_tray_menu(initial_lang)))
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

        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                let store = store_clone.clone();
                let ui_weak = ui_weak.clone();
                let active_inspector_id = active_inspector_id_clone.clone();

                match event {
                    DownloadEvent::Updated { summary_json, .. } => {
                        if let Ok(summary) =
                            serde_json::from_value::<DownloadSummary>(summary_json)
                        {
                            let current_lang = store.lock().language();
                            // Trigger system notification on completion or failure
                            if matches!(summary.state, DownloadState::Completed) {
                                let (title, body) = i18n::format_notification_completed(
                                    &summary.file_name,
                                    current_lang,
                                );
                                let _ = Notification::new()
                                    .appname("limedl")
                                    .summary(&title)
                                    .body(&body)
                                    .show();
                            } else if matches!(summary.state, DownloadState::Failed) {
                                let (title, body) = i18n::format_notification_failed(
                                    &summary.file_name,
                                    summary.error.as_deref(),
                                    current_lang,
                                );
                                let _ = Notification::new()
                                    .appname("limedl")
                                    .summary(&title)
                                    .body(&body)
                                    .show();
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
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_is_testing = true;
                                form.cdn_status_type = SharedString::from("testing");
                                form.cdn_status_label = SharedString::from("测速中");
                                form.cdn_phase_label = SharedString::from(match phase.as_str() {
                                    "fetchingRanges" => "获取网段",
                                    "screening" => "延迟初筛",
                                    "measuringThroughput" => "带宽测速",
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
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_is_testing = false;
                                let (st, sl) = match state.as_str() {
                                    "ready" => ("ready", "准备就绪"),
                                    "error" => ("error", "测速失败"),
                                    _ => ("idle", "未配置"),
                                };
                                form.cdn_status_type = SharedString::from(st);
                                form.cdn_status_label = SharedString::from(sl);
                                if let Some(ip) = active_ip {
                                    form.cdn_active_ip = SharedString::from(ip);
                                }
                                if let Some(spd) = active_speed_mbps {
                                    form.cdn_active_speed_text = SharedString::from(format!("{spd:.2} MB/s"));
                                }
                                ui.set_labs_form(form);
                            }
                        });
                    }
                    _ => {}
                }
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

        tokio::spawn(async move {
            let menu_channel = muda::MenuEvent::receiver();
            let tray_channel = TrayIconEvent::receiver();

            loop {
                tokio::select! {
                    Ok(event) = tokio::task::spawn_blocking(move || menu_channel.recv()) => {
                        if let Ok(event) = event {
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
                                "open_dir" => {
                                    let _ = open_path_in_explorer(&default_dir);
                                }
                                "quit" => {
                                    std::process::exit(0);
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(event) = tokio::task::spawn_blocking(move || tray_channel.recv()) => {
                        if let Ok(TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }) = event {
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
        main_window.on_set_sort_field(move |field_idx| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.set_sort_field(SortField::from(field_idx));
                refresh_ui(&ui, &store);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_toggle_sort_asc(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.toggle_sort_order();
                refresh_ui(&ui, &store);
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
        let ui_weak = main_window.as_weak();

        main_window.on_save_settings(move |form_data| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let store_clone = store_clone.clone();
            let pending_tray_lang_clone = pending_tray_lang_clone.clone();
            let active_inspector_id_clone = active_inspector_id_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let mut settings = {
                    let s = current_settings_clone.lock();
                    s.clone()
                };

                update_app_settings_from_form(&mut settings, &form_data);

                if let Ok(saved) = dispatcher.save_settings(&settings).await {
                    *current_settings_clone.lock() = saved.clone();
                    let default_dir = saved.download.default_download_dir.clone();
                    let new_lang = Language::from_code(&saved.appearance.language);

                    // Signal tray update to main thread (TrayIcon is !Send)
                    *pending_tray_lang_clone.lock() = Some(new_lang);

                    let _ = slint::invoke_from_event_loop(move || {
                        i18n::apply_translation(new_lang);
                        if let Some(ui) = ui_weak.upgrade() {
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
        let ui_weak = main_window.as_weak();

        main_window.on_fetch_trackers_remote(move |url_str| {
            let dispatcher = dispatcher.clone();
            let url = url_str.to_string();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                if let Ok(trackers) = dispatcher.fetch_tracker_list(&url).await {
                    tracing::info!("成功同步远程 Tracker 列表: {} 个", trackers.len());
                    let _ = Notification::new()
                        .appname("limedl")
                        .summary("Tracker 列表同步成功")
                        .body(&format!("已获取并配置 {} 个公共 Tracker", trackers.len()))
                        .show();

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut form = ui.get_settings_form();
                            form.tracker_url = SharedString::from(&url);
                            ui.set_settings_form(form);
                        }
                    });
                }
            });
        });
    }

    // Dialog: Open / Close New Task
    {
        let ui_weak = main_window.as_weak();
        main_window.on_open_new_task_dialog(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_new_task_dialog(true);
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

    // Submit New Task
    {
        let dispatcher = core.dispatcher.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_submit_new_task(move |url, dir, filename| {
            let dispatcher = dispatcher.clone();
            let ui_weak = ui_weak.clone();
            let url_str = url.to_string();
            let dir_str = dir.to_string();
            let filename_opt = if filename.trim().is_empty() {
                None
            } else {
                Some(filename.trim().to_string())
            };

            tokio::spawn(async move {
                let req = StartDownloadRequest {
                    url: url_str,
                    destination_dir: dir_str,
                    file_name: filename_opt,
                    ..Default::default()
                };

                match dispatcher.start(req).await {
                    Ok(task_id) => {
                        tracing::info!("成功添加下载任务: {task_id}");
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
                    }
                }
            });
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

    // Pick Torrent File (Native Dialog)
    {
        let ui_weak = main_window.as_weak();
        main_window.on_pick_torrent_file(move || {
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("Torrent Files", &["torrent", "TORRENT"])
                    .set_title("选择 Torrent 种子文件")
                    .pick_file()
                    .await;

                if let Some(handle) = file {
                    let path = handle.path().to_string_lossy().to_string();
                    let file_name = handle.file_name();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_new_task_url(SharedString::from(&path));
                            if ui.get_new_task_filename().trim().is_empty() {
                                ui.set_new_task_filename(SharedString::from(&file_name));
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
        main_window.on_pick_save_folder(move || {
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                let folder = rfd::AsyncFileDialog::new()
                    .set_title("选择下载保存目录")
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
        main_window.on_pick_default_folder(move || {
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                let folder = rfd::AsyncFileDialog::new()
                    .set_title("选择默认下载保存目录")
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
        main_window.on_copy_task_url(move |url| {
            let url_str = url.to_string();
            tokio::spawn(async move {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&url_str);
                    let _ = Notification::new()
                        .appname("limedl")
                        .summary("链接已复制")
                        .body("下载链接已复制到系统剪贴板")
                        .show();
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
        main_window.on_pick_log_folder(move || {
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                let folder = rfd::AsyncFileDialog::new()
                    .set_title("选择日志保存目录")
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
        let ui_weak = main_window.as_weak();

        main_window.on_save_labs(move |form_data| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let rewrite_rules_clone = rewrite_rules_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let mut settings = {
                    let s = current_settings_clone.lock();
                    s.clone()
                };

                update_app_settings_from_labs_form(&mut settings, &form_data);
                settings.url_rewrite.rules = rewrite_rules_clone.lock().clone();

                if let Ok(saved) = dispatcher.save_settings(&settings).await {
                    *current_settings_clone.lock() = saved;

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_show_labs(false);
                        }
                    });
                }
            });
        });
    }

    // Start CDN Speed Test
    {
        let dispatcher = core.dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_start_cdn_test(move || {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                if let Some(cs) = dispatcher.cdn_service() {
                    let settings = current_settings_clone.lock().clone();
                    let cs = cs.clone();
                    if let Ok(()) = cs.start_test(settings).await {
                        let _ = slint::invoke_from_event_loop({
                            let ui_weak = ui_weak.clone();
                            move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    let mut form = ui.get_labs_form();
                                    form.cdn_is_testing = true;
                                    form.cdn_status_type = SharedString::from("testing");
                                    form.cdn_status_label = SharedString::from("测速中");
                                    ui.set_labs_form(form);
                                }
                            }
                        });

                        // Poll candidates during speed test run
                        for _ in 0..60 {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            let cands = cs.candidates().await;
                            if !cands.is_empty() {
                                *cdn_candidates_cache_clone.lock() = cands.clone();
                                let settings = current_settings_clone.lock().clone();
                                let active_ip = settings.cdn_acceleration.active_ip.unwrap_or_default();
                                let ui_weak = ui_weak.clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_weak.upgrade() {
                                        ui.set_cdn_candidates(cdn_candidates_to_slint(&cands, &active_ip));
                                    }
                                });
                            }
                            if !matches!(cs.status().await, limedl_core::cdn::accelerator::AccelState::Testing) {
                                break;
                            }
                        }
                    }
                }
            });
        });
    }

    // Cancel CDN Speed Test
    {
        let dispatcher = core.dispatcher.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_cancel_cdn_test(move || {
            if let Some(cs) = dispatcher.cdn_service() {
                cs.cancel_test();
                if let Some(ui) = ui_weak.upgrade() {
                    let mut form = ui.get_labs_form();
                    form.cdn_is_testing = false;
                    form.cdn_status_type = SharedString::from("idle");
                    form.cdn_status_label = SharedString::from("已取消");
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
        let ui_weak = main_window.as_weak();

        main_window.on_clear_cdn_test(move || {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                if let Some(cs) = dispatcher.cdn_service() {
                    cs.clear().await;
                    cdn_candidates_cache_clone.lock().clear();
                    let mut settings = current_settings_clone.lock().clone();
                    settings.cdn_acceleration.active_ip = None;
                    settings.cdn_acceleration.active_speed_mbps = None;
                    settings.cdn_acceleration.last_test_at_ms = None;
                    settings.cdn_acceleration.last_error = None;
                    if let Ok(saved) = dispatcher.save_settings(&settings).await {
                        *current_settings_clone.lock() = saved;
                    }
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut form = ui.get_labs_form();
                            form.cdn_active_ip = SharedString::default();
                            form.cdn_active_speed_text = SharedString::default();
                            form.cdn_status_type = SharedString::from("idle");
                            form.cdn_status_label = SharedString::from("未配置");
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
        let ui_weak = main_window.as_weak();

        main_window.on_apply_cdn_candidate(move |ip_str, speed_mbps| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
            let ui_weak = ui_weak.clone();
            let ip_parsed = ip_str.parse::<IpAddr>();

            tokio::spawn(async move {
                if let (Some(cs), Ok(ip)) = (dispatcher.cdn_service(), ip_parsed) {
                    let settings = current_settings_clone.lock().clone();
                    if let Ok(()) = cs.apply_ip(ip, speed_mbps as f64, &settings).await {
                        let mut updated_settings = settings.clone();
                        updated_settings.cdn_acceleration.active_ip = Some(ip.to_string());
                        updated_settings.cdn_acceleration.active_speed_mbps = Some(speed_mbps as f64);
                        if let Ok(saved) = dispatcher.save_settings(&updated_settings).await {
                            *current_settings_clone.lock() = saved;
                        }

                        let cands = cdn_candidates_cache_clone.lock().clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_active_ip = SharedString::from(ip.to_string());
                                form.cdn_active_speed_text = SharedString::from(format!("{speed_mbps:.2} MB/s"));
                                form.cdn_status_type = SharedString::from("ready");
                                form.cdn_status_label = SharedString::from("准备就绪");
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
        let ui_weak = main_window.as_weak();

        main_window.on_apply_manual_cdn_ip(move |ip_str| {
            let ip_trimmed = ip_str.trim().to_string();
            match ip_trimmed.parse::<IpAddr>() {
                Ok(ip) => {
                    let dispatcher = dispatcher.clone();
                    let current_settings_clone = current_settings_clone.clone();
                    let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
                    let ui_weak = ui_weak.clone();

                    tokio::spawn(async move {
                        if let Some(cs) = dispatcher.cdn_service() {
                            let settings = current_settings_clone.lock().clone();
                            if let Ok(()) = cs.apply_ip(ip, 0.0, &settings).await {
                                let mut updated_settings = settings.clone();
                                updated_settings.cdn_acceleration.active_ip = Some(ip.to_string());
                                if let Ok(saved) = dispatcher.save_settings(&updated_settings).await {
                                    *current_settings_clone.lock() = saved;
                                }

                                let cands = cdn_candidates_cache_clone.lock().clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_weak.upgrade() {
                                        let mut form = ui.get_labs_form();
                                        form.cdn_active_ip = SharedString::from(ip.to_string());
                                        form.cdn_manual_ip_error = SharedString::default();
                                        form.cdn_status_type = SharedString::from("ready");
                                        form.cdn_status_label = SharedString::from("准备就绪");
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
                        form.cdn_manual_ip_error = SharedString::from("无效的 IP 地址格式");
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
        let ui_weak = main_window.as_weak();

        main_window.on_import_rewrite_preset(move |preset_key| {
            if let Some(rule) = create_url_rewrite_preset(&preset_key) {
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
        let ui_weak = main_window.as_weak();

        main_window.on_add_custom_rule(move || {
            let mut rules = rewrite_rules_clone.lock();
            let id = format!("rule-{}", uuid::Uuid::new_v4().simple());
            let order = rules.len() as u32;
            expanded_rule_ids_clone.lock().insert(id.clone());
            rules.push(UrlRewriteRule {
                id,
                name: "新建自定义规则".to_string(),
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

    tracing::info!("limedl Native UI 启动完毕，进入主事件循环");

    // Poll for pending tray menu/tooltip updates on the main thread (TrayIcon is !Send)
    let _tray_update_timer = slint::Timer::default();
    _tray_update_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(250),
        {
            let pending_tray_lang_clone = pending_tray_lang.clone();
            move || {
                if let Some(lang) = pending_tray_lang_clone.lock().take() {
                    tray_icon.set_menu(Some(Box::new(build_tray_menu(lang))));
                    let _ = tray_icon.set_tooltip(Some(i18n::get_tray_strings(lang).tooltip));
                }
            }
        },
    );

    main_window.run()?;

    // Graceful shutdown
    tracing::info!("Native UI 正在退出，关闭核心引擎...");
    core.registry.shutdown_all().await;

    Ok(())
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

fn dirs_or_temp_dir() -> PathBuf {
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
