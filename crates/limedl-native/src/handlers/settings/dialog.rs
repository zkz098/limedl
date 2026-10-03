//! Settings dialog lifecycle, the save flow, performance modes and the
//! destructive actions (restart wizard / factory reset).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify_rust::Notification;
use parking_lot::Mutex;

use limedl_core::backend_registry::BackendRegistry;
use limedl_core::dispatcher::Dispatcher;
use limedl_core::error::Result as CoreResult;
use limedl_core::error::DownloadError;

use crate::bridge::TaskStore;
use crate::bridge::{
    app_settings_to_setup_form, parse_aria2_clients, parse_disk_type_overrides,
    parse_speed_limit_slots, update_app_settings_from_form,
};
use crate::context::AppContext;
use crate::handlers::common::{read_ui, with_ui};
use crate::i18n;
use crate::settings_sync::{PushOptions, SettingsSync};
use crate::task_ops::open_url_in_browser;
use crate::toast::{ToastQueue, push_toast};
use crate::ui_sync::{read_aria2_client_rows, read_disk_override_rows, read_schedule_rows, refresh_settings_state};
use crate::{MainWindow, POWER_GUARD, SettingsFormData};

/// Validation + persistence of the settings form. Everything that has to happen
/// *after* a successful save (autostart, Aria2 RPC, UI update) lives in
/// [`SettingsSync`], which the setup wizard shares.
#[derive(Clone)]
struct SaveCtx {
    ui_weak: slint::Weak<MainWindow>,
    dispatcher: Arc<Dispatcher>,
    sync: SettingsSync,
}

/// Persist the dialog form and apply every side effect of the change.
async fn save_settings(ctx: SaveCtx, form_data: SettingsFormData) {
    let lang = ctx.sync.lang();
    let old_settings = ctx.dispatcher.get_settings_blocking().unwrap_or_default();

    // Speed limit schedule rows live in the UI model until Save, so validate
    // them here (a mistyped hour must not silently disable the schedule).
    let schedule_rows = read_ui(&ctx.ui_weak, read_schedule_rows).unwrap_or_default();
    let parsed_schedule = match parse_speed_limit_slots(&schedule_rows, lang) {
        Ok(slots) => slots,
        Err(msg) => {
            ctx.sync
                .toast(i18n::format_toast_schedule_invalid(&msg, lang), "error", 8);
            return;
        }
    };

    // Media override rows live in the UI model for the same reason, so a
    // half-typed or relative path has to be caught here instead of being
    // persisted as a key that can never match a download destination.
    let override_rows = read_ui(&ctx.ui_weak, read_disk_override_rows).unwrap_or_default();
    let parsed_overrides = match parse_disk_type_overrides(&override_rows, lang) {
        Ok(overrides) => overrides,
        Err(msg) => {
            ctx.sync
                .toast(i18n::format_toast_disk_override_invalid(&msg, lang), "error", 8);
            return;
        }
    };

    // Aria2 per-client token rows also live in the UI model until Save. Names
    // must be usable and every row must carry a hash; the plaintext is never
    // persisted (the add/regenerate handler hashed it already).
    let client_rows = read_ui(&ctx.ui_weak, read_aria2_client_rows).unwrap_or_default();
    let parsed_clients = match parse_aria2_clients(&client_rows, lang) {
        Ok(clients) => clients,
        Err(msg) => {
            ctx.sync
                .toast(i18n::format_toast_settings_invalid(&msg, lang), "error", 8);
            return;
        }
    };

    let saved = match ctx
        .dispatcher
        .save_settings_with(|settings| {
            update_app_settings_from_form(settings, &form_data, lang)
                .map_err(DownloadError::InvalidRequest)?;
            settings.speed_limit_schedule = parsed_schedule;
            // The engine's map is declared with foldhash's hasher, so convert here
            // rather than naming that hasher in the UI crate.
            settings.io_baseline.disk_type_overrides = parsed_overrides.into_iter().collect();
            settings.aria2_rpc.clients = parsed_clients;
            Ok(())
        })
        .await
    {
        Ok(saved) => saved,
        Err(DownloadError::InvalidRequest(msg)) => {
            tracing::error!("设置表单校验失败: {msg}");
            ctx.sync
                .toast(i18n::format_toast_settings_invalid(&msg, lang), "error", 6);
            return;
        }
        Err(err) => {
            tracing::error!("保存设置失败: {err:#}");
            let msg = format!("{err:#}");
            let _ = Notification::new()
                .appname("limedl")
                .summary(i18n::format_notification_settings_save_failed(lang))
                .body(&msg)
                .show();
            ctx.sync.toast(
                i18n::format_toast_settings_save_failed(&msg, lang),
                "error",
                6,
            );
            return;
        }
    };

    ctx.sync.sync_autostart(&saved, &old_settings, lang);
    ctx.sync.sync_aria2_rpc(&saved, &old_settings, lang);
    ctx.sync.commit(&saved);
    ctx.sync.toast(
        i18n::format_toast_settings_saved(lang).to_string(),
        "success",
        4,
    );
    ctx.sync.push_ui(
        &saved,
        PushOptions {
            refresh_inspector: true,
            close_settings: true,
            ..Default::default()
        },
    );
}

/// Toggle a performance mode: run the backend switch, then mirror the result
/// into both the dedicated active flag and the settings form.
fn toggle_mode(
    dispatcher: &Arc<Dispatcher>,
    active: &Arc<Mutex<bool>>,
    ui_weak: &slint::Weak<MainWindow>,
    toggle: fn(&Dispatcher) -> CoreResult<bool>,
    set_active: fn(&MainWindow, bool),
    set_form: fn(&mut SettingsFormData, bool),
) {
    let Ok(new_value) = toggle(dispatcher) else {
        return;
    };
    *active.lock() = new_value;
    with_ui(ui_weak, |ui| {
        set_active(&ui, new_value);
        let mut form = ui.get_settings_form();
        set_form(&mut form, new_value);
        ui.set_settings_form(form);
    });
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let sync = SettingsSync::new(ctx);

    // Open / close / tab switch
    {
        let ui_weak = ui_weak.clone();
        let dispatcher = dispatcher.clone();
        let store = store.clone();
        let game_mode = ctx.game_mode_active.clone();
        let overclock_mode = ctx.is_overclock_mode.clone();
        ui.on_open_settings(move || {
            with_ui(&ui_weak, |ui| {
                let settings = dispatcher.get_settings_blocking().unwrap_or_default();
                let game_mode = *game_mode.lock();
                let overclock = *overclock_mode.lock();
                let lang = store.lock().language();
                refresh_settings_state(&ui, &dispatcher, &settings, game_mode, overclock, lang);
                ui.set_reset_confirm(false);
                ui.set_show_settings(true);
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        ui.on_close_settings(move || {
            with_ui(&ui_weak, |ui| {
                // The About tab's "Factory Reset" is a two-click gate. Disarm it
                // while closing, or the next visit opens with "Confirm Reset"
                // already showing and the data directory one click away.
                ui.set_reset_confirm(false);
                ui.set_show_settings(false);
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        ui.on_set_settings_tab(move |tab| {
            with_ui(&ui_weak, |ui| ui.set_settings_tab(tab));
        });
    }

    // Save
    {
        let save_ctx = SaveCtx {
            ui_weak: ui_weak.clone(),
            dispatcher: dispatcher.clone(),
            sync: sync.clone(),
        };
        ui.on_save_settings(move |form_data| {
            let ctx = save_ctx.clone();
            tokio::spawn(async move { save_settings(ctx, form_data).await });
        });
    }

    // Performance modes
    {
        let dispatcher = dispatcher.clone();
        let game_mode = ctx.game_mode_active.clone();
        let ui_weak = ui_weak.clone();
        ui.on_toggle_game_mode(move || {
            toggle_mode(
                &dispatcher,
                &game_mode,
                &ui_weak,
                |dispatcher| dispatcher.toggle_game_mode(None),
                |ui, value| ui.set_game_mode_active(value),
                |form, value| form.game_mode = value,
            );
        });
    }

    {
        let dispatcher = dispatcher.clone();
        let overclock_mode = ctx.is_overclock_mode.clone();
        let ui_weak = ui_weak.clone();
        ui.on_toggle_overclock_mode(move || {
            toggle_mode(
                &dispatcher,
                &overclock_mode,
                &ui_weak,
                |dispatcher| dispatcher.toggle_overclock_mode(None),
                |ui, value| ui.set_overclock_mode_active(value),
                |form, value| form.overclock_mode = value,
            );
        });
    }

    // Open the MiSans font license page (About tab attribution link)
    ui.on_open_font_license(move || {
        let _ = open_url_in_browser("https://hyperos.mi.com/font/zh/download");
    });

    // Restart Setup Wizard from Settings
    {
        let ui_weak = ui_weak.clone();
        let dispatcher = dispatcher.clone();
        let store = store.clone();
        ui.on_restart_setup(move || {
            restart_setup(ui_weak.clone(), dispatcher.clone(), store.clone());
        });
    }

    // Factory reset: shut the backends down, delete the whole data directory
    // (settings.json + downloads/ incl. the SQLite database) and restart the
    // process so the first-run wizard comes back.
    {
        let dispatcher = dispatcher.clone();
        let registry = dispatcher.registry().clone();
        let store = store.clone();
        let toast_queue = ctx.toast_queue.clone();
        let ui_weak = ui_weak.clone();
        let data_dir = ctx.base_dir.clone();
        ui.on_factory_reset(move || {
            factory_reset(
                ui_weak.clone(),
                dispatcher.clone(),
                registry.clone(),
                store.clone(),
                toast_queue.clone(),
                data_dir.clone(),
            );
        });
    }
}

/// Delete the data directory, retrying for a moment because Windows may still
/// hold file handles right after the backends shut down.
async fn remove_data_dir(data_dir: &std::path::Path) -> Option<std::io::Error> {
    let mut last_err: Option<std::io::Error> = None;
    for attempt in 0..3u8 {
        match tokio::fs::remove_dir_all(data_dir).await {
            Ok(()) => return None,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
            Err(err) => {
                last_err = Some(err);
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }
    last_err
}

/// Reset the wizard flags and reopen the first-run wizard.
fn restart_setup(
    ui_weak: slint::Weak<MainWindow>,
    dispatcher: Arc<Dispatcher>,
    store: Arc<Mutex<TaskStore>>,
) {
    let dispatcher = dispatcher.clone();
    let store = store.clone();
    let ui_weak = ui_weak.clone();
    tokio::spawn(async move {
        let settings = dispatcher.get_settings_blocking().unwrap_or_default();
        let lang = store.lock().language();
        let form = app_settings_to_setup_form(&settings, lang);
        if let Err(err) = dispatcher
            .save_settings_with(|settings| {
                settings.setup_completed = false;
                settings.last_setup_step = None;
                Ok(())
            })
            .await
        {
            tracing::warn!("保存设置向导重置状态失败: {err:#}");
        }
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui_weak, |ui| {
                ui.set_setup_form(form);
                ui.set_setup_start_step(0);
                ui.set_show_settings(false);
                ui.set_show_setup_wizard(true);
            });
        });
    });
}

/// Shut the backends down, wipe the data directory and relaunch.
#[allow(clippy::too_many_arguments)]
fn factory_reset(
    ui_weak: slint::Weak<MainWindow>,
    dispatcher: Arc<Dispatcher>,
    registry: Arc<BackendRegistry>,
    store: Arc<Mutex<TaskStore>>,
    toast_queue: ToastQueue,
    data_dir: PathBuf,
) {
    let dispatcher = dispatcher.clone();
    let registry = registry.clone();
    let store = store.clone();
    let toast_queue = toast_queue.clone();
    let ui_weak = ui_weak.clone();
    let data_dir = data_dir.clone();
    tokio::spawn(async move {
        let lang = store.lock().language();
        // 1. Stop every backend so no file handle survives the wipe.
        registry.shutdown_all().await;
        // 2. Reset the in-memory settings first (the file is deleted
        //    afterwards, so a failure here must not leave stale state).
        let _ = dispatcher.factory_reset().await;
        // 3. Delete the data directory, retrying briefly for Windows
        //    file locking before giving up.
        if let Some(err) = remove_data_dir(&data_dir).await {
            tracing::error!("工厂重置失败: {err:#}");
            push_toast(
                &ui_weak,
                &toast_queue,
                i18n::format_toast_factory_reset_failed(&format!("{err:#}"), lang),
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
        if let Err(err) = crate::update::restart_application() {
            tracing::error!("工厂重置后重启失败: {err:#}");
            std::process::exit(0);
        }
    });
}
