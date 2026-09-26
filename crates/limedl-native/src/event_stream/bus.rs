//! `DownloadEvent` subscriber: one function per event variant, plus the shared
//! "apply to the store and repaint" plumbing.

use std::sync::Arc;
use std::time::Duration;

use notify_rust::Notification;
use parking_lot::Mutex;
use slint::SharedString;

use limedl_core::dispatcher::Dispatcher;
use limedl_core::event_bus::DownloadEvent;
use limedl_core::types::{AppSettings, DownloadProgress, DownloadState, DownloadSummary, TaskId};

use crate::bridge::{TaskStore, summary_to_inspector_info};
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::i18n::{self, Language};
use crate::platform_win;
use crate::toast::{ToastQueue, WarningDedup, push_toast};
use crate::ui_sync::refresh_ui;
use crate::{MainWindow, POWER_GUARD};

/// Handles shared by every event arm.
#[derive(Clone)]
struct BusCtx {
    ui_weak: slint::Weak<MainWindow>,
    store: Arc<Mutex<TaskStore>>,
    active_inspector_id: Arc<Mutex<Option<String>>>,
    toast_queue: ToastQueue,
    current_settings: Arc<Mutex<AppSettings>>,
    dispatcher: Arc<Dispatcher>,
}

impl BusCtx {
    fn lang(&self) -> Language {
        self.store.lock().language()
    }
}

/// Apply a store mutation on the UI thread and repaint.
///
/// The store is always updated, but the repaint is skipped while the window is
/// hidden (tray-only operation). `refresh_inspector_for` re-renders the
/// inspector when the changed task is the one it is showing.
fn push_store_update<F>(ctx: &BusCtx, mutate: F, refresh_inspector_for: Option<String>)
where
    F: FnOnce(&mut TaskStore) + Send + 'static,
{
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();
    let active_inspector_id = ctx.active_inspector_id.clone();
    let _ = slint::invoke_from_event_loop(move || {
        with_ui(&ui_weak, |ui| {
            let mut store = store.lock();
            mutate(&mut store);
            let (_, downloading, _, _, _) = store.counts();
            POWER_GUARD.update(downloading);
            if !platform_win::is_window_visible() {
                return;
            }
            let lang = store.language();
            refresh_ui(&ui, &store);

            if let Some(id) = refresh_inspector_for
                && let Some(ref current_id) = *active_inspector_id.lock()
                && current_id == &id
                && let Some(summary) = store.get_summary(current_id)
            {
                ui.set_inspector_info(summary_to_inspector_info(&summary, lang));
            }
        });
    });
}

/// OS notification + toast when a task reaches a terminal state.
fn notify_state_change(
    ctx: &BusCtx,
    summary: &DownloadSummary,
    notifications_enabled: bool,
    lang: Language,
) {
    let (kind, secs) = if matches!(summary.state, DownloadState::Completed) {
        ("success", 5)
    } else if matches!(summary.state, DownloadState::Failed) {
        ("error", 6)
    } else {
        return;
    };

    if notifications_enabled {
        let (title, body) = if matches!(summary.state, DownloadState::Completed) {
            i18n::format_notification_completed(&summary.file_name, lang)
        } else {
            i18n::format_notification_failed(&summary.file_name, summary.error.as_deref(), lang)
        };
        let mut notification = Notification::new();
        notification.appname("limedl").summary(&title).body(&body);
        #[cfg(windows)]
        {
            notification.app_id("limedl");
        }
        let _ = notification.show();
    }

    push_toast(
        &ctx.ui_weak,
        &ctx.toast_queue,
        i18n::format_toast_state(&summary.file_name, &summary.state, lang),
        kind,
        Duration::from_secs(secs),
    );
}

/// A task was added, changed state or removed.
///
/// Returns `false` when the task no longer exists in the backend: the caller
/// must then skip the rest of the event (it was removed elsewhere).
async fn on_updated(ctx: &BusCtx, id: String, summary_json: serde_json::Value) -> bool {
    if let Ok(task_id) = TaskId::from_wire_string(&id)
        && ctx.dispatcher.status(&task_id).await.is_err()
    {
        let ui_weak = ctx.ui_weak.clone();
        let store = ctx.store.clone();
        let active_inspector_id = ctx.active_inspector_id.clone();
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui_weak, |ui| {
                let mut store = store.lock();
                store.remove(&id);
                refresh_ui(&ui, &store);

                if let Some(ref current_id) = *active_inspector_id.lock()
                    && current_id == &id
                {
                    ui.set_show_inspector(false);
                }
            });
        });
        return false;
    }

    let Ok(summary) = serde_json::from_value::<DownloadSummary>(summary_json) else {
        return true;
    };

    let lang = ctx.lang();
    let notifications_enabled = ctx.current_settings.lock().notifications.enabled;
    notify_state_change(ctx, &summary, notifications_enabled, lang);

    let inspector_id = summary.id.clone();
    push_store_update(
        ctx,
        move |store| store.insert_or_update(summary),
        Some(inspector_id),
    );
    true
}

/// High-frequency progress tick.
fn on_progress(ctx: &BusCtx, progress_json: serde_json::Value) {
    let Ok(progress) = serde_json::from_value::<DownloadProgress>(progress_json) else {
        return;
    };
    let inspector_id = progress.id.clone();
    push_store_update(
        ctx,
        move |store| store.update_progress(&progress),
        Some(inspector_id),
    );
}

/// Full state recovery after a subscriber lagged.
fn on_full_state(ctx: &BusCtx, downloads: Vec<DownloadSummary>) {
    push_store_update(ctx, move |store| store.replace_all(downloads), None);
}

/// CDN speedtest progress.
fn on_cdn_progress(ctx: &BusCtx, phase: String, current: u64, total: u64) {
    let lang = ctx.lang();
    let ui_weak = ctx.ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        with_ui(&ui_weak, |ui| {
            let mut form = ui.get_labs_form();
            form.cdn_is_testing = true;
            form.cdn_status_type = SharedString::from("testing");
            form.cdn_status_label = SharedString::from(i18n::format_cdn_status_label(true, lang));
            form.cdn_phase_label = SharedString::from(i18n::cdn_phase_label(&phase, lang));
            if total > 0 {
                form.cdn_progress_percent =
                    (current as f32 / total as f32 * 100.0).clamp(0.0, 100.0);
                form.cdn_progress_label = SharedString::from(format!("{current} / {total}"));
            }
            ui.set_labs_form(form);
        });
    });
}

/// CDN speedtest finished (ready, failed or cleared).
fn on_cdn_complete(
    ctx: &BusCtx,
    state: String,
    active_ip: Option<String>,
    active_speed_mbps: Option<f64>,
) {
    let lang = ctx.lang();
    let is_ready = state == "Ready" || state == "ready";
    let is_error = state.starts_with("Error") || state == "error";
    let err_msg = if is_error {
        state.strip_prefix("Error: ").unwrap_or(&state).to_string()
    } else {
        String::new()
    };

    let ui_weak = ctx.ui_weak.clone();
    let err_for_ui = err_msg.clone();
    let ip_for_ui = active_ip.clone();
    let _ = slint::invoke_from_event_loop(move || {
        with_ui(&ui_weak, |ui| {
            let mut form = ui.get_labs_form();
            form.cdn_is_testing = false;
            let (status_type, status_label) = if is_ready {
                ("ready", i18n::cdn_ready_label(lang))
            } else if is_error {
                ("error", i18n::cdn_test_failed_label(lang))
            } else {
                ("idle", i18n::cdn_idle_label(lang))
            };
            form.cdn_status_type = SharedString::from(status_type);
            form.cdn_status_label = SharedString::from(status_label);
            if is_ready {
                form.cdn_last_error = SharedString::default();
            } else if is_error {
                form.cdn_last_error = SharedString::from(&err_for_ui);
            }
            if let Some(ip) = ip_for_ui {
                form.cdn_active_ip = SharedString::from(ip);
            }
            if let Some(speed) = active_speed_mbps {
                form.cdn_active_speed_text = SharedString::from(format!("{speed:.2} MB/s"));
            }
            ui.set_labs_form(form);
        });
    });

    if is_ready {
        push_toast(
            &ctx.ui_weak,
            &ctx.toast_queue,
            i18n::format_toast_cdn_test_done(active_ip.as_deref(), lang),
            "success",
            Duration::from_secs(5),
        );
    } else if is_error {
        let msg = if err_msg.is_empty() {
            i18n::cdn_test_failed_label(lang)
        } else {
            &err_msg
        };
        push_toast(
            &ctx.ui_weak,
            &ctx.toast_queue,
            i18n::format_toast_cdn_test_failed(msg, lang),
            "error",
            Duration::from_secs(6),
        );
    }
}

/// A core warning (mirror failover, disk full, anti-leech bans, …).
///
/// Duplicates are collapsed within a short window because anti-leech bans
/// arrive one per peer.
fn on_warning(ctx: &BusCtx, dedup: &mut WarningDedup, id: String, message: String) {
    if !dedup.should_show(&id, &message) {
        return;
    }
    let file_name = ctx.store.lock().get_summary(&id).map(|summary| summary.file_name);
    let text = match file_name {
        Some(name) if !name.is_empty() => i18n::format_warning_with_file(&name, &message),
        _ => message,
    };
    push_toast(
        &ctx.ui_weak,
        &ctx.toast_queue,
        text,
        "warning",
        Duration::from_secs(6),
    );
}

pub fn start_event_bus_listener(
    ctx: &AppContext,
    mut rx: tokio::sync::broadcast::Receiver<DownloadEvent>,
) {
    let bus = BusCtx {
        ui_weak: ctx.ui_weak.clone(),
        store: ctx.store.clone(),
        active_inspector_id: ctx.active_inspector_id.clone(),
        toast_queue: ctx.toast_queue.clone(),
        current_settings: ctx.current_settings.clone(),
        dispatcher: ctx.dispatcher.clone(),
    };

    tokio::spawn(async move {
        let mut warning_dedup = WarningDedup::new();
        while let Ok(event) = rx.recv().await {
            match event {
                DownloadEvent::Updated { id, summary_json } => {
                    on_updated(&bus, id, summary_json).await;
                }
                DownloadEvent::Progress { progress_json, .. } => on_progress(&bus, progress_json),
                DownloadEvent::FullState { downloads } => on_full_state(&bus, downloads),
                DownloadEvent::CdnProgress {
                    phase,
                    current,
                    total,
                } => on_cdn_progress(&bus, phase, current, total),
                DownloadEvent::CdnComplete {
                    state,
                    active_ip,
                    active_speed_mbps,
                } => on_cdn_complete(&bus, state, active_ip, active_speed_mbps),
                DownloadEvent::Warning { id, message } => {
                    on_warning(&bus, &mut warning_dedup, id, message);
                }
                // Aria2 compatibility notifications are pushed by the Aria2 RPC
                // server straight to connected aria2 clients (AriaNg / Motrix);
                // the native UI has no surface for them.
                DownloadEvent::Aria2Notification { .. } => {}
            }
        }
    });
}
