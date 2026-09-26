//! Self-update callbacks: manual check, download + install, relaunch and the
//! background silent check.
//!
//! The two large flows (check / download) are split into named steps so the
//! store channel and the GitHub channel read as separate paths.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify_rust::Notification;
use parking_lot::Mutex;

use limedl_core::types::AppSettings;

use crate::bridge::TaskStore;
use crate::context::AppContext;
use crate::i18n::{self, Language};
use crate::toast::{ToastQueue, push_toast};
use crate::ui_sync::{push_update_state, set_update_error};
use crate::update::{self, AvailableUpdate, InstallKind, InstallOutcome};
use crate::MainWindow;

/// Handles shared by the update flows, cloned once per callback.
#[derive(Clone)]
struct UpdateCtx {
    ui_weak: slint::Weak<MainWindow>,
    base_dir: PathBuf,
    store: Arc<Mutex<TaskStore>>,
    current_settings: Arc<Mutex<AppSettings>>,
    toast_queue: ToastQueue,
    available: Arc<Mutex<Option<AvailableUpdate>>>,
}

impl UpdateCtx {
    fn lang(&self) -> Language {
        self.store.lock().language()
    }

    fn toast(&self, message: String, kind: &'static str, secs: u64) {
        push_toast(
            &self.ui_weak,
            &self.toast_queue,
            message,
            kind,
            Duration::from_secs(secs),
        );
    }

    /// Surface a failed step in both the update dialog and a toast. Safe to call
    /// from a background task.
    fn report_failure(&self, err: &str, lang: Language, secs: u64) {
        let ui_weak = self.ui_weak.clone();
        let msg = err.to_string();
        let _ = slint::invoke_from_event_loop(move || set_update_error(&ui_weak, &msg));
        self.toast(i18n::format_toast_update_failed(err, lang), "error", secs);
    }

    /// Like [`UpdateCtx::report_failure`], but for a failed *check*: the dialog
    /// keeps the previous phase and only the message differs.
    fn report_check_failure(&self, err: &str, lang: Language) {
        let ui_weak = self.ui_weak.clone();
        let msg = err.to_string();
        let _ = slint::invoke_from_event_loop(move || set_update_error(&ui_weak, &msg));
        self.toast(
            i18n::format_toast_update_check_failed(err, lang),
            "error",
            5,
        );
    }

    /// Mark the dialog as busy and clear the previous error.
    fn set_phase(&self, phase: &str, clear_error: bool) {
        let ui_weak = self.ui_weak.clone();
        let phase = phase.to_string();
        let _ = slint::invoke_from_event_loop(move || {
            push_update_state(&ui_weak, |st| {
                st.phase = phase.into();
                if clear_error {
                    st.error_text = "".into();
                }
            });
        });
    }
}

/// Throttled download progress reporter (150 ms, but always emits the final
/// tick so the dialog never stops just short of 100%).
fn progress_reporter(ui: &slint::Weak<MainWindow>) -> impl Fn(u64, Option<u64>) + Send + Sync + 'static {
    let ui = ui.clone();
    let last_emit = Mutex::new(Instant::now() - Duration::from_secs(10));
    move |done: u64, total: Option<u64>| {
        let now = Instant::now();
        let finished = total.is_some_and(|total| done >= total);
        if !finished && now - *last_emit.lock() < Duration::from_millis(150) {
            return;
        }
        *last_emit.lock() = now;
        let percent = total
            .map(|total| done as f32 / total as f32 * 100.0)
            .unwrap_or(0.0);
        let label = match total {
            Some(total) => format!(
                "{:.1} / {:.1} MB",
                done as f64 / 1_048_576.0,
                total as f64 / 1_048_576.0
            ),
            None => format!("{:.1} MB", done as f64 / 1_048_576.0),
        };
        let ui = ui.clone();
        let _ = slint::invoke_from_event_loop(move || {
            push_update_state(&ui, |st| {
                st.progress_percent = percent;
                st.progress_label = label.into();
            });
        });
    }
}

/// Store channel: ask the Store for an update and report what it found.
/// `StoreContext::GetDefault` must run on the UI thread.
fn check_store(ctx: UpdateCtx, lang: Language) {
    let _ = slint::spawn_local(async move {
        match update::store::check_update_available().await {
            Ok(available) => {
                push_update_state(&ctx.ui_weak, |st| {
                    st.phase = if available {
                        "available".into()
                    } else {
                        "up-to-date".into()
                    };
                    st.error_text = "".into();
                });
                if available {
                    ctx.toast(
                        i18n::format_toast_update_store_triggered(lang).to_string(),
                        "info",
                        5,
                    );
                } else {
                    ctx.toast(
                        i18n::format_toast_update_up_to_date(lang).to_string(),
                        "success",
                        4,
                    );
                }
            }
            Err(err) => {
                let msg = format!("{err:#}");
                ctx.report_check_failure(&msg, lang);
            }
        }
    });
}

/// GitHub channel: query the release manifest in the background.
fn check_github(ctx: UpdateCtx, lang: Language) {
    tokio::spawn(async move {
        update::record_update_check(&ctx.base_dir);
        match update::check_for_update().await {
            Ok(Some(update)) => {
                let version = update.version.clone();
                let notes = update.notes.clone();
                *ctx.available.lock() = Some(update);
                let ui_weak = ctx.ui_weak.clone();
                let version_for_ui = version.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    push_update_state(&ui_weak, |st| {
                        st.phase = "available".into();
                        st.latest_version = version_for_ui.into();
                        st.notes = notes.into();
                        st.error_text = "".into();
                    });
                });
                ctx.toast(
                    i18n::format_toast_update_available(&version, lang),
                    "info",
                    5,
                );
            }
            Ok(None) => {
                let ui_weak = ctx.ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    push_update_state(&ui_weak, |st| {
                        st.phase = "up-to-date".into();
                        st.latest_version = "".into();
                        st.notes = "".into();
                        st.error_text = "".into();
                    });
                });
                ctx.toast(
                    i18n::format_toast_update_up_to_date(lang).to_string(),
                    "success",
                    4,
                );
            }
            Err(err) => {
                let msg = format!("{err:#}");
                ctx.report_check_failure(&msg, lang);
            }
        }
    });
}

/// Store channel: hand the update over to the OS, which replaces the package
/// and relaunches the app.
fn trigger_store_update(ctx: UpdateCtx, lang: Language) {
    ctx.toast(
        i18n::format_toast_update_store_triggered(lang).to_string(),
        "info",
        4,
    );
    let _ = slint::spawn_local(async move {
        if let Err(err) = update::store::trigger_update().await {
            let msg = format!("{err:#}");
            set_update_error(&ctx.ui_weak, &msg);
            ctx.toast(
                i18n::format_toast_update_failed(&msg, lang),
                "error",
                5,
            );
        }
    });
}

/// GitHub channel: download, verify and install the pending update.
fn download_and_install(ctx: UpdateCtx, update: AvailableUpdate, lang: Language) {
    push_update_state(&ctx.ui_weak, |st| {
        st.phase = "downloading".into();
        st.progress_percent = 0.0;
        st.progress_label = "".into();
        st.error_text = "".into();
    });
    ctx.toast(
        i18n::format_toast_update_downloading(&update.version, lang),
        "info",
        4,
    );

    tokio::spawn(async move {
        let progress = progress_reporter(&ctx.ui_weak);
        let verified = match update::download_and_verify(&update, &ctx.base_dir, &progress).await {
            Ok(file) => file,
            Err(err) => {
                let msg = format!("{err:#}");
                ctx.report_failure(&msg, lang, 6);
                return;
            }
        };
        finish_install(ctx, &update, &verified, lang).await;
    });
}

/// Apply a verified artifact (portable self-replace or installer handoff).
async fn finish_install(
    ctx: UpdateCtx,
    update: &AvailableUpdate,
    verified: &Path,
    lang: Language,
) {
    match update::install_verified(update, verified) {
        Ok(InstallOutcome::ReplacedRestartPending) => {
            let ui_weak = ctx.ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                push_update_state(&ui_weak, |st| st.phase = "ready".into());
            });
            ctx.toast(
                i18n::format_toast_update_ready(&update.version, lang),
                "success",
                6,
            );
        }
        Ok(InstallOutcome::InstallerLaunched) => {
            ctx.toast(
                i18n::format_toast_update_installer_launched(lang).to_string(),
                "info",
                4,
            );
            tokio::time::sleep(Duration::from_millis(800)).await;
            std::process::exit(0);
        }
        Err(err) => {
            let msg = format!("{err:#}");
            ctx.report_failure(&msg, lang, 6);
        }
    }
}

/// Silent check a minute after startup (GitHub channel only; Store installs are
/// updated by the OS, so there is nothing to poll).
fn spawn_background_check(ctx: UpdateCtx, lang: Language) {
    if update::detect_install_kind() == InstallKind::Store {
        return;
    }
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(45)).await;
        if !update::should_check_for_update_now(&ctx.base_dir, Duration::from_secs(24 * 60 * 60)) {
            return;
        }
        update::record_update_check(&ctx.base_dir);
        match update::check_for_update().await {
            Ok(Some(update)) => {
                let version = update.version.clone();
                *ctx.available.lock() = Some(update);
                let ui_weak = ctx.ui_weak.clone();
                let version_for_ui = version.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    push_update_state(&ui_weak, |st| {
                        st.phase = "available".into();
                        st.latest_version = version_for_ui.into();
                        st.error_text = "".into();
                    });
                });

                if ctx.current_settings.lock().notifications.enabled {
                    let (title, body) = i18n::format_notification_update(&version, lang);
                    let mut notification = Notification::new();
                    notification.appname("limedl").summary(&title).body(&body);
                    #[cfg(windows)]
                    {
                        notification.app_id("limedl");
                    }
                    let _ = notification.show();
                }
            }
            Ok(None) => {}
            Err(err) => tracing::debug!("background update check failed: {err:#}"),
        }
    });
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let update_ctx = UpdateCtx {
        ui_weak: ctx.ui_weak.clone(),
        base_dir: ctx.base_dir.clone(),
        store: ctx.store.clone(),
        current_settings: ctx.current_settings.clone(),
        toast_queue: ctx.toast_queue.clone(),
        available: Arc::new(Mutex::new(None)),
    };

    {
        let ctx = update_ctx.clone();
        ui.on_check_for_updates(move || {
            let ctx = ctx.clone();
            let lang = ctx.lang();
            ctx.set_phase("checking", true);
            if update::detect_install_kind() == InstallKind::Store {
                // StoreContext::GetDefault must run on the UI thread.
                check_store(ctx, lang);
                return;
            }
            check_github(ctx, lang);
        });
    }

    {
        let ctx = update_ctx.clone();
        ui.on_start_update_download(move || {
            let ctx = ctx.clone();
            let lang = ctx.lang();
            let Some(update) = ctx.available.lock().clone() else {
                ctx.toast(
                    i18n::format_toast_update_not_found(lang).to_string(),
                    "warning",
                    4,
                );
                return;
            };

            if update::detect_install_kind() == InstallKind::Store {
                trigger_store_update(ctx, lang);
                return;
            }
            download_and_install(ctx, update, lang);
        });
    }

    {
        let ctx = update_ctx.clone();
        ui.on_restart_after_update(move || {
            let lang = ctx.lang();
            // On success this never returns (spawns the new binary and exits).
            if let Err(err) = update::restart_application() {
                let msg = format!("restart failed: {err:#}");
                set_update_error(&ctx.ui_weak, &msg);
                ctx.toast(
                    i18n::format_toast_update_restart_failed(&msg, lang),
                    "error",
                    5,
                );
            }
        });
    }

    spawn_background_check(update_ctx.clone(), update_ctx.lang());
}
