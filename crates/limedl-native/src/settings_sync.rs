//! Post-save side effects shared by the settings dialog and the setup wizard.
//!
//! Both paths persist `AppSettings` and then have to do the same three things:
//! sync the OS autostart registration, hot-reload the Aria2 RPC server, and push
//! the new settings into the UI (language, tray menu, appearance, default
//! directory). Keeping that in one place is what stops the two dialogs from
//! drifting apart — they had already grown three separate copies of the Aria2
//! hot-reload block before this module existed.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use slint::SharedString;
use tokio::sync::watch;

use limedl_core::aria2_rpc::Aria2RpcServer;
use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::AppSettings;

use crate::bridge::{TaskStore, summary_to_inspector_info};
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::i18n::{self, Language};
use crate::toast::{ToastQueue, push_toast};
use crate::tray::update_tray_menu_and_tooltip;
use crate::ui_sync::{apply_appearance, refresh_ui};
use crate::MainWindow;

/// Extras each caller wants on top of the shared UI update.
#[derive(Clone, Copy, Default)]
pub struct PushOptions {
    /// Mirror the default download directory into the new-task dialog
    /// (setup wizard only).
    pub sync_new_task_dir: bool,
    /// Re-render the inspector for its current subject (settings dialog only).
    pub refresh_inspector: bool,
    pub close_settings: bool,
    pub close_wizard: bool,
}

/// Handles needed to apply persisted settings to the running app.
#[derive(Clone)]
pub struct SettingsSync {
    ui_weak: slint::Weak<MainWindow>,
    dispatcher: Arc<Dispatcher>,
    store: Arc<Mutex<TaskStore>>,
    current_settings: Arc<Mutex<AppSettings>>,
    toast_queue: ToastQueue,
    rpc_shutdown: Arc<Mutex<Option<watch::Sender<bool>>>>,
    tray_speed_limit_active: Arc<AtomicBool>,
    active_inspector_id: Arc<Mutex<Option<String>>>,
}

impl SettingsSync {
    pub fn new(ctx: &AppContext) -> Self {
        Self {
            ui_weak: ctx.ui_weak.clone(),
            dispatcher: ctx.dispatcher.clone(),
            store: ctx.store.clone(),
            current_settings: ctx.current_settings.clone(),
            toast_queue: ctx.toast_queue.clone(),
            rpc_shutdown: ctx.rpc_shutdown.clone(),
            tray_speed_limit_active: ctx.tray_speed_limit_active.clone(),
            active_inspector_id: ctx.active_inspector_id.clone(),
        }
    }

    pub fn lang(&self) -> Language {
        self.store.lock().language()
    }

    pub fn toast(&self, message: String, kind: &'static str, secs: u64) {
        push_toast(
            &self.ui_weak,
            &self.toast_queue,
            message,
            kind,
            Duration::from_secs(secs),
        );
    }

    /// Record the saved settings in memory and sync the tray limit checkmark.
    pub fn commit(&self, saved: &AppSettings) {
        *self.current_settings.lock() = saved.clone();
        self.tray_speed_limit_active
            .store(saved.global_speed_limit_bps > 0, Ordering::Relaxed);
    }

    /// Mirror the autostart flag into the OS registration.
    pub fn sync_autostart(&self, saved: &AppSettings, old: &AppSettings, lang: Language) {
        if saved.autostart == old.autostart {
            return;
        }
        if let Err(err) = crate::autostart::set_enabled(saved.autostart) {
            tracing::warn!("autostart 切换失败: {err:#}");
            self.toast(
                i18n::format_toast_autostart_failed(&format!("{err:#}"), lang),
                "error",
                6,
            );
        } else {
            tracing::info!("autostart 已切换为 {}", saved.autostart);
        }
    }

    /// Hot-reload the Aria2 RPC server when its settings changed.
    pub fn sync_aria2_rpc(&self, saved: &AppSettings, old: &AppSettings, lang: Language) {
        if saved.aria2_rpc == old.aria2_rpc {
            return;
        }
        if let Some(tx) = self.rpc_shutdown.lock().take() {
            let _ = tx.send(true);
            tracing::info!("Aria2 RPC 旧服务已停止");
        }
        if !saved.aria2_rpc.enabled {
            tracing::info!("Aria2 RPC 已停止");
            self.toast(
                i18n::format_toast_aria2_rpc_stopped(lang).to_string(),
                "info",
                5,
            );
            return;
        }

        let (tx, rx) = watch::channel(false);
        let rpc_server = Aria2RpcServer::new(
            self.dispatcher.registry().clone(),
            &saved.aria2_rpc,
            self.dispatcher.event_bus().clone(),
        );
        let cors = saved.aria2_rpc.cors_allowed_origins.clone();
        let port = saved.aria2_rpc.port;
        tokio::spawn(async move {
            if let Err(err) = rpc_server.serve(rx, cors).await {
                tracing::error!("Aria2 RPC server stopped: {err:#}");
            }
        });
        *self.rpc_shutdown.lock() = Some(tx);
        tracing::info!("Aria2 RPC 已重启 (port: {port})");
        self.toast(i18n::format_toast_aria2_rpc_started(port, lang), "info", 5);
    }

    /// Push the saved settings into the UI thread: language, tray menu,
    /// appearance, default directory and the task list.
    pub fn push_ui(&self, saved: &AppSettings, opts: PushOptions) {
        let default_dir = saved.download.default_download_dir.clone();
        let new_lang = Language::from_code(&saved.appearance.language);
        let color_mode = saved.appearance.color_mode.clone();
        let theme_color = saved.appearance.theme_color.clone();
        let speed_limit_active = saved.global_speed_limit_bps > 0;

        let ui_weak = self.ui_weak.clone();
        let store = self.store.clone();
        let active_inspector_id = self.active_inspector_id.clone();
        let _ = slint::invoke_from_event_loop(move || {
            i18n::apply_translation(new_lang);
            update_tray_menu_and_tooltip(new_lang, speed_limit_active);
            with_ui(&ui_weak, |ui| {
                apply_appearance(&ui, color_mode, theme_color);
                let mut store = store.lock();
                store.set_language(new_lang);
                ui.set_default_download_dir(SharedString::from(&default_dir));
                if opts.sync_new_task_dir {
                    ui.set_new_task_dir(SharedString::from(&default_dir));
                }
                refresh_ui(&ui, &store);

                if opts.refresh_inspector
                    && let Some(ref current_id) = *active_inspector_id.lock()
                    && let Some(summary) = store.get_summary(current_id)
                {
                    ui.set_inspector_info(summary_to_inspector_info(&summary, new_lang));
                }
                if opts.close_settings {
                    ui.set_show_settings(false);
                }
                if opts.close_wizard {
                    ui.set_show_setup_wizard(false);
                }
            });
        });
    }
}
