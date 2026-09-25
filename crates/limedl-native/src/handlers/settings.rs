use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;
use notify_rust::Notification;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use tokio::sync::watch;

use limedl_core::aria2_rpc::Aria2RpcServer;
use limedl_core::types::TaskId;

use crate::context::AppContext;
use crate::bridge::{
    app_settings_to_setup_form, parse_speed_limit_slots, speed_limit_slots_to_slint,
    summary_to_inspector_info, update_app_settings_from_form, SpeedLimitSlotText,
};
use crate::i18n::{self, Language};
use crate::paths::dirs_or_temp_dir;
use crate::task_ops::{open_path_in_explorer, open_url_in_browser};
use crate::toast::push_toast;
use crate::tray::update_tray_menu_and_tooltip;
use crate::ui_sync::{apply_appearance, read_schedule_rows, refresh_settings_state, refresh_ui};
use crate::{SpeedLimitSlotItem, POWER_GUARD};

pub fn register(ctx: &AppContext) {
    let main_window = &ctx.ui;
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let current_settings = ctx.current_settings.clone();
    let toast_queue = ctx.toast_queue.clone();
    let rpc_shutdown = ctx.rpc_shutdown.clone();
    let game_mode_active = ctx.game_mode_active.clone();
    let overclock_mode_active = ctx.is_overclock_mode.clone();
    let tray_speed_limit_active = ctx.tray_speed_limit_active.clone();
    let active_inspector_id = ctx.active_inspector_id.clone();
    let base_dir = ctx.base_dir.clone();

    // Open Speed Limit Dialog
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
        let dispatcher = dispatcher.clone();
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
        let dispatcher = dispatcher.clone();
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
        let dispatcher = dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let store_clone = store.clone();
        let active_inspector_id_clone = active_inspector_id.clone();
        let rpc_shutdown_clone = rpc_shutdown.clone();
        let toast_queue_clone = toast_queue.clone();
        let tray_speed_limit_settings = tray_speed_limit_active.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_save_settings(move |form_data| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let store_clone = store_clone.clone();
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
                        let speed_limit_active = saved.global_speed_limit_bps > 0;
                        tray_speed_limit_settings.store(
                            speed_limit_active,
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

                        let _ = slint::invoke_from_event_loop(move || {
                            i18n::apply_translation(new_lang);
                            update_tray_menu_and_tooltip(new_lang, speed_limit_active);
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
        let dispatcher = dispatcher.clone();
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
        let dispatcher = dispatcher.clone();
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
        let dispatcher = dispatcher.clone();
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

    // Schedule: Set Enabled
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
        let ui_weak = main_window.as_weak();

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


    // Restart Setup Wizard from Settings
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let store_clone = store.clone();
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
        let dispatcher = dispatcher.clone();
        let registry = dispatcher.registry().clone();
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
}
