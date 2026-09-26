//! Settings paths: remote tracker sync, the native folder pickers and the
//! "open log folder" shortcut.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use slint::SharedString;

use crate::bridge::TaskStore;
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::i18n::{self, Language};
use crate::paths::dirs_or_temp_dir;
use crate::task_ops::open_path_in_explorer;
use crate::toast::push_toast;
use crate::{MainWindow, SettingsFormData};

/// Ask for a folder and store it in one settings-form field.
///
/// `append_name` appends a file name for the pickers that select a directory but
/// edit a file path (the log file).
fn pick_folder_into_form(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    title: fn(Language) -> &'static str,
    append_name: Option<&'static str>,
    apply: fn(&mut SettingsFormData, &str),
) {
    let ui = ui.clone();
    let store = store.clone();
    tokio::spawn(async move {
        let lang = store.lock().language();
        let folder = rfd::AsyncFileDialog::new()
            .set_title(title(lang))
            .pick_folder()
            .await;

        let Some(handle) = folder else {
            return;
        };
        let path = match append_name {
            Some(name) => handle.path().join(name).to_string_lossy().to_string(),
            None => handle.path().to_string_lossy().to_string(),
        };
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui, |ui| {
                let mut form = ui.get_settings_form();
                apply(&mut form, &path);
                ui.set_settings_form(form);
            });
        });
    });
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();

    // Sync the BT tracker list from a remote URL.
    {
        let dispatcher = ctx.dispatcher.clone();
        let store = store.clone();
        let toast_queue = ctx.toast_queue.clone();
        let ui_weak = ui_weak.clone();
        ui.on_fetch_trackers_remote(move |url_str| {
            let dispatcher = dispatcher.clone();
            let store = store.clone();
            let url = url_str.to_string();
            let toast_queue = toast_queue.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let lang = store.lock().language();
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
                            with_ui(&ui_weak, |ui| {
                                let mut form = ui.get_settings_form();
                                form.tracker_url = SharedString::from(&url);
                                ui.set_settings_form(form);
                            });
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

    // Native folder pickers
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_pick_default_folder(move || {
            pick_folder_into_form(
                &ui_weak,
                &store,
                i18n::pick_download_dir_title,
                None,
                |form, path| form.default_download_dir = SharedString::from(path),
            );
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_pick_log_folder(move || {
            pick_folder_into_form(
                &ui_weak,
                &store,
                i18n::pick_log_dir_title,
                Some("limedl.log"),
                |form, path| form.logging_file_path = SharedString::from(path),
            );
        });
    }

    // Reveal the log file's folder in the OS file manager.
    {
        let current_settings = ctx.current_settings.clone();
        ui.on_open_log_folder(move || {
            let settings = current_settings.lock().clone();
            let log_path = if !settings.logging.file_path.trim().is_empty() {
                PathBuf::from(&settings.logging.file_path)
            } else {
                dirs_or_temp_dir().join("logs").join("limedl.log")
            };
            let parent_dir = log_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            let _ = std::fs::create_dir_all(parent_dir);
            let _ = open_path_in_explorer(&parent_dir.to_string_lossy());
        });
    }
}
