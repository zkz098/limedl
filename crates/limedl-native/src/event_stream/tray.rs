//! Tray icon event loop: menu items and left-click activation.
//!
//! The menu receivers are not `Send`, so dedicated threads forward them into
//! mpsc channels that the async loop can select on.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use slint::SharedString;
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::{AppSettings, DownloadState};

use crate::bridge::TaskStore;
use crate::context::AppContext;
use crate::handlers::common::{TaskAction, spawn_for_state, with_ui};
use crate::i18n::{self, Language};
use crate::task_ops::open_path_in_explorer;
use crate::toast::{ToastQueue, push_toast};
use crate::tray::{TRAY_SPEED_LIMIT_BPS, update_tray_menu_and_tooltip};
use crate::ui_sync::restore_and_show_window;
use crate::{MainWindow, POWER_GUARD};

/// Handles shared by the tray menu items.
#[derive(Clone)]
struct TrayCtx {
    ui_weak: slint::Weak<MainWindow>,
    dispatcher: Arc<Dispatcher>,
    store: Arc<Mutex<TaskStore>>,
    current_settings: Arc<Mutex<AppSettings>>,
    game_mode_active: Arc<Mutex<bool>>,
    tray_speed_limit_active: Arc<AtomicBool>,
    toast_queue: ToastQueue,
    default_dir: String,
}

impl TrayCtx {
    fn new(ctx: &AppContext) -> Self {
        Self {
            ui_weak: ctx.ui_weak.clone(),
            dispatcher: ctx.dispatcher.clone(),
            store: ctx.store.clone(),
            current_settings: ctx.current_settings.clone(),
            game_mode_active: ctx.game_mode_active.clone(),
            tray_speed_limit_active: ctx.tray_speed_limit_active.clone(),
            toast_queue: ctx.toast_queue.clone(),
            default_dir: ctx
                .current_settings
                .lock()
                .download
                .default_download_dir
                .clone(),
        }
    }

    fn lang(&self) -> Language {
        self.store.lock().language()
    }

    fn show_window(&self) {
        let ui_weak = self.ui_weak.clone();
        let store = self.store.clone();
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui_weak, |ui| {
                restore_and_show_window(&ui, Some(&store.lock()));
            });
        });
    }

    /// Quick global speed limit shortcut: unlimited ↔ 1 MB/s.
    async fn toggle_speed_limit(&self) {
        let mut settings = self.current_settings.lock().clone();
        let enabling = settings.global_speed_limit_bps == 0;
        settings.global_speed_limit_bps = if enabling { TRAY_SPEED_LIMIT_BPS } else { 0 };
        let lang = self.lang();

        match self.dispatcher.save_settings(&settings).await {
            Ok(saved) => {
                *self.current_settings.lock() = saved.clone();
                self.tray_speed_limit_active
                    .store(saved.global_speed_limit_bps > 0, Ordering::Relaxed);
                push_toast(
                    &self.ui_weak,
                    &self.toast_queue,
                    i18n::format_toast_speed_limit(enabling, lang),
                    "info",
                    Duration::from_secs(4),
                );

                let limit_kb = (saved.global_speed_limit_bps / 1024).to_string();
                let lang_code = Language::from_code(&saved.appearance.language);
                let speed_limit_active = saved.global_speed_limit_bps > 0;
                let ui_weak = self.ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    update_tray_menu_and_tooltip(lang_code, speed_limit_active);
                    with_ui(&ui_weak, |ui| {
                        let mut form = ui.get_settings_form();
                        form.global_speed_limit_kb = SharedString::from(limit_kb);
                        ui.set_settings_form(form);
                    });
                });
            }
            Err(err) => tracing::warn!("托盘限速切换失败: {err:#}"),
        }
    }
}

/// Handle one menu event. Returns `false` when the loop must stop.
async fn handle_menu_event(ctx: &TrayCtx, id: &str) -> bool {
    match id {
        "show" => ctx.show_window(),
        "pause_all" => spawn_for_state(
            &ctx.dispatcher,
            |item| matches!(item.state, DownloadState::Downloading),
            TaskAction::Pause,
        ),
        "resume_all" => spawn_for_state(
            &ctx.dispatcher,
            |item| matches!(item.state, DownloadState::Paused),
            TaskAction::Resume,
        ),
        "game_mode" => {
            if let Ok(new_value) = ctx.dispatcher.toggle_game_mode(None) {
                *ctx.game_mode_active.lock() = new_value;
                let ui_weak = ctx.ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    with_ui(&ui_weak, |ui| ui.set_game_mode_active(new_value));
                });
            }
        }
        "speed_limit" => ctx.toggle_speed_limit().await,
        "open_dir" => {
            let _ = open_path_in_explorer(&ctx.default_dir);
        }
        "quit" => {
            POWER_GUARD.release();
            // Quit through the Slint event loop so the runtime teardown (engine
            // shutdown, registry shutdown_all) runs; fall back to a hard exit
            // only if the event loop is already gone.
            if slint::quit_event_loop().is_err() {
                std::process::exit(0);
            }
            return false;
        }
        _ => {}
    }
    true
}

fn handle_tray_click(ctx: &TrayCtx, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        ctx.show_window();
    }
}

/// Forward events from a `!Send` global receiver into an mpsc channel.
fn forward_events<T: Send + 'static>(
    name: &str,
    tx: tokio::sync::mpsc::UnboundedSender<T>,
    next: impl Fn() -> Option<T> + Send + 'static,
) {
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            while let Some(event) = next() {
                if tx.send(event).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn tray event thread");
}

pub fn start_tray_event_loop(ctx: &AppContext) {
    let tray = TrayCtx::new(ctx);

    let (menu_tx, mut menu_rx) = tokio::sync::mpsc::unbounded_channel::<muda::MenuEvent>();
    let (tray_tx, mut tray_rx) = tokio::sync::mpsc::unbounded_channel::<TrayIconEvent>();
    forward_events("tray-menu-events", menu_tx, || {
        muda::MenuEvent::receiver().recv().ok()
    });
    forward_events("tray-icon-events", tray_tx, || {
        TrayIconEvent::receiver().recv().ok()
    });

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(event) = menu_rx.recv() => {
                    if !handle_menu_event(&tray, event.id.as_ref()).await {
                        break;
                    }
                }
                Some(event) = tray_rx.recv() => handle_tray_click(&tray, event),
                else => break,
            }
        }
    });
}
