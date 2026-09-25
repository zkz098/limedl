use std::sync::Arc;
use std::time::Duration;
use notify_rust::Notification;
use parking_lot::Mutex;
use slint::ComponentHandle;

use crate::context::AppContext;
use crate::i18n;
use crate::toast::push_toast;
use crate::ui_sync::{push_update_state, set_update_error};
use crate::update;

pub fn register(ctx: &AppContext) {
    let main_window = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();
    let current_settings = ctx.current_settings.clone();
    let toast_queue = ctx.toast_queue.clone();
    let base_dir = ctx.base_dir.clone();
    let initial_lang = ctx.store.lock().language();
    let available_update: Arc<Mutex<Option<update::AvailableUpdate>>> = Arc::new(Mutex::new(None));

    // Check for updates
    {
        let base_dir = base_dir.clone();
        let available = available_update.clone();
        let toast_queue_clone = toast_queue.clone();
        let store_clone = store.clone();
        main_window.on_check_for_updates(move || {
            let ui_weak = ui_weak.clone();
            let base_dir = base_dir.clone();
            let available = available.clone();
            let toast_queue = toast_queue_clone.clone();
            let lang = store_clone.lock().language();

            let ui = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                push_update_state(&ui, |st| {
                    st.phase = "checking".into();
                    st.error_text = "".into();
                });
            });

            if update::detect_install_kind() == update::InstallKind::Store {
                // StoreContext::GetDefault must run on the UI thread.
                let toast_queue = toast_queue.clone();
                let ui_w = ui_weak.clone();
                let _ = slint::spawn_local(async move {
                    match update::store::check_update_available().await {
                        Ok(available) => {
                            push_update_state(&ui_w, |st| {
                                st.phase = if available { "available".into() } else { "up-to-date".into() };
                                st.error_text = "".into();
                            });
                            if available {
                                push_toast(
                                    &ui_w,
                                    &toast_queue,
                                    i18n::format_toast_update_store_triggered(lang).to_string(),
                                    "info",
                                    Duration::from_secs(5),
                                );
                            } else {
                                push_toast(
                                    &ui_w,
                                    &toast_queue,
                                    i18n::format_toast_update_up_to_date(lang).to_string(),
                                    "success",
                                    Duration::from_secs(4),
                                );
                            }
                        }
                        Err(e) => {
                            let msg = format!("{e:#}");
                            set_update_error(&ui_w, &msg);
                            push_toast(
                                &ui_w,
                                &toast_queue,
                                i18n::format_toast_update_check_failed(&msg, lang),
                                "error",
                                Duration::from_secs(5),
                            );
                        }
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
                        let _ = slint::invoke_from_event_loop({
                            let ui_weak = ui_weak.clone();
                            let version = version.clone();
                            move || {
                                push_update_state(&ui_weak, |st| {
                                    st.phase = "available".into();
                                    st.latest_version = version.into();
                                    st.notes = notes.into();
                                    st.error_text = "".into();
                                });
                            }
                        });
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_update_available(&version, lang),
                            "info",
                            Duration::from_secs(5),
                        );
                    }
                    Ok(None) => {
                        let _ = slint::invoke_from_event_loop({
                            let ui_weak = ui_weak.clone();
                            move || {
                                push_update_state(&ui_weak, |st| {
                                    st.phase = "up-to-date".into();
                                    st.latest_version = "".into();
                                    st.notes = "".into();
                                    st.error_text = "".into();
                                });
                            }
                        });
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_update_up_to_date(lang).to_string(),
                            "success",
                            Duration::from_secs(4),
                        );
                    }
                    Err(e) => {
                        let msg = format!("{e:#}");
                        let _ = slint::invoke_from_event_loop({
                            let ui_weak = ui_weak.clone();
                            let msg = msg.clone();
                            move || {
                                set_update_error(&ui_weak, &msg);
                            }
                        });
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_update_check_failed(&msg, lang),
                            "error",
                            Duration::from_secs(5),
                        );
                    }
                }
            });
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let base_dir = base_dir.clone();
        let available = available_update.clone();
        let toast_queue_clone = toast_queue.clone();
        let store_clone = store.clone();
        main_window.on_start_update_download(move || {
            let ui_weak = ui_weak.clone();
            let base_dir = base_dir.clone();
            let toast_queue = toast_queue_clone.clone();
            let lang = store_clone.lock().language();

            let Some(upd) = available.lock().clone() else {
                push_toast(
                    &ui_weak,
                    &toast_queue,
                    i18n::format_toast_update_not_found(lang).to_string(),
                    "warning",
                    Duration::from_secs(4),
                );
                return;
            };

            if update::detect_install_kind() == update::InstallKind::Store {
                let toast_queue = toast_queue.clone();
                let ui_weak = ui_weak.clone();
                push_toast(
                    &ui_weak,
                    &toast_queue,
                    i18n::format_toast_update_store_triggered(lang).to_string(),
                    "info",
                    Duration::from_secs(4),
                );
                let _ = slint::spawn_local(async move {
                    if let Err(e) = update::store::trigger_update().await {
                        let msg = format!("{e:#}");
                        set_update_error(&ui_weak, &msg);
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_update_failed(&msg, lang),
                            "error",
                            Duration::from_secs(5),
                        );
                    }
                    // On success the OS replaces the package and relaunches the app.
                });
                return;
            }

            // Immediately reflect downloading phase on UI thread
            push_update_state(&ui_weak, |st| {
                st.phase = "downloading".into();
                st.progress_percent = 0.0;
                st.progress_label = "".into();
                st.error_text = "".into();
            });

            // Show immediate toast prompt
            push_toast(
                &ui_weak,
                &toast_queue,
                i18n::format_toast_update_downloading(&upd.version, lang),
                "info",
                Duration::from_secs(4),
            );

            let toast_queue = toast_queue.clone();
            tokio::spawn(async move {
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
                        let _ = slint::invoke_from_event_loop({
                            let ui_weak = ui_weak.clone();
                            let msg = msg.clone();
                            move || {
                                set_update_error(&ui_weak, &msg);
                            }
                        });
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_update_failed(&msg, lang),
                            "error",
                            Duration::from_secs(6),
                        );
                        return;
                    }
                };

                match update::install_verified(&upd, &verified) {
                    Ok(update::InstallOutcome::ReplacedRestartPending) => {
                        let _ = slint::invoke_from_event_loop({
                            let ui_weak = ui_weak.clone();
                            move || {
                                push_update_state(&ui_weak, |st| st.phase = "ready".into());
                            }
                        });
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_update_ready(&upd.version, lang),
                            "success",
                            Duration::from_secs(6),
                        );
                    }
                    Ok(update::InstallOutcome::InstallerLaunched) => {
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_update_installer_launched(lang).to_string(),
                            "info",
                            Duration::from_secs(4),
                        );
                        tokio::time::sleep(Duration::from_millis(800)).await;
                        std::process::exit(0);
                    }
                    Err(e) => {
                        let msg = format!("{e:#}");
                        let _ = slint::invoke_from_event_loop({
                            let ui_weak = ui_weak.clone();
                            let msg = msg.clone();
                            move || {
                                set_update_error(&ui_weak, &msg);
                            }
                        });
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_update_failed(&msg, lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let toast_queue_clone = toast_queue.clone();
        let store_clone = store.clone();
        main_window.on_restart_after_update(move || {
            let lang = store_clone.lock().language();
            // On success this never returns (spawns the new binary, exits).
            if let Err(e) = update::restart_application() {
                let msg = format!("restart failed: {e:#}");
                set_update_error(&ui_weak, &msg);
                push_toast(
                    &ui_weak,
                    &toast_queue_clone,
                    i18n::format_toast_update_restart_failed(&msg, lang),
                    "error",
                    Duration::from_secs(5),
                );
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


}
