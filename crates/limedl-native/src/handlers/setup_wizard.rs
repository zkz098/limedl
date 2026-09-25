use std::sync::atomic::Ordering;
use std::time::Duration;
use slint::{ComponentHandle, SharedString};
use tokio::sync::watch;

use limedl_core::aria2_rpc::Aria2RpcServer;
use limedl_core::types::{ColorMode, ThemeColor};

use crate::context::AppContext;
use crate::bridge::update_app_settings_from_setup_form;
use crate::i18n::{self, Language};
use crate::toast::push_toast;
use crate::tray::update_tray_menu_and_tooltip;
use crate::ui_sync::{apply_appearance, refresh_ui};

pub fn register(ctx: &AppContext) {
    let main_window = &ctx.ui;
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let current_settings = ctx.current_settings.clone();
    let toast_queue = ctx.toast_queue.clone();
    let rpc_shutdown = ctx.rpc_shutdown.clone();
    let tray_speed_limit_active = ctx.tray_speed_limit_active.clone();

    // Interrupted close: persist the current step so the wizard reopens there.
    {
        let dispatcher = dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_close_setup_wizard(move |step| {
            // Sync the task-store language with the wizard's live language
            // selection (the @tr translation was already switched on change),
            // so interpolated Rust-side strings stay consistent after closing.
            if let Some(ui) = ui_weak.upgrade() {
                let form = ui.get_setup_form();
                let lang = match form.language_idx {
                    0 => Language::ZhCn,
                    1 => Language::ZhTw,
                    _ => Language::EnUs,
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
                let lang = match idx {
                    0 => Language::ZhCn,
                    1 => Language::ZhTw,
                    _ => Language::EnUs,
                };
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
        let dispatcher = dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let store_clone = store.clone();
        let rpc_shutdown_clone = rpc_shutdown.clone();
        let toast_queue_clone = toast_queue.clone();
        let tray_speed_limit_setup = tray_speed_limit_active.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_finish_setup(move |form| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let store_clone = store_clone.clone();
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
                        let speed_limit_active = saved.global_speed_limit_bps > 0;
                        tray_speed_limit_setup.store(
                            speed_limit_active,
                            Ordering::Relaxed,
                        );
                        let default_dir = saved.download.default_download_dir.clone();
                        let new_lang = Language::from_code(&saved.appearance.language);
                        let new_color_mode = saved.appearance.color_mode;
                        let new_theme_color = saved.appearance.theme_color;

                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_setup_finished(lang).to_string(),
                            "success",
                            Duration::from_secs(5),
                        );

                        let _ = slint::invoke_from_event_loop(move || {
                            i18n::apply_translation(new_lang);
                            update_tray_menu_and_tooltip(new_lang, speed_limit_active);
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
}
