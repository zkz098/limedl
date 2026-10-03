//! First-run setup wizard callbacks.
//!
//! The wizard shares its post-save side effects (autostart, Aria2 RPC, UI
//! update) with the settings dialog through [`SettingsSync`]; only the form
//! validation and the wizard-specific UI extras live here.

use std::sync::Arc;

use parking_lot::Mutex;
use slint::SharedString;

use limedl_core::dispatcher::Dispatcher;
use limedl_core::error::DownloadError;
use limedl_core::types::{ColorMode, ThemeColor};

use crate::MainWindow;
use crate::SetupFormData;
use crate::bridge::TaskStore;
use crate::bridge::update_app_settings_from_setup_form;
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::i18n::{self, Language};
use crate::settings_sync::{PushOptions, SettingsSync};
use crate::ui_sync::apply_appearance;

/// Map the wizard's language dropdown index to a language.
fn language_from_idx(idx: i32) -> Language {
    match idx {
        0 => Language::ZhCn,
        1 => Language::ZhTw,
        _ => Language::EnUs,
    }
}

/// Map the wizard's appearance dropdown indices to the typed settings.
fn appearance_from_idx(color_idx: i32, theme_idx: i32) -> (ColorMode, ThemeColor) {
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
    (mode, theme)
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let sync = SettingsSync::new(ctx);

    // Interrupted close: persist the current step so the wizard reopens there.
    {
        let ui_weak = ui_weak.clone();
        let dispatcher = dispatcher.clone();
        let store = store.clone();
        ui.on_close_setup_wizard(move |step| {
            close_setup_wizard(ui_weak.clone(), dispatcher.clone(), store.clone(), step);
        });
    }

    // Language selection in the wizard: switch the @tr translation immediately.
    {
        let ui_weak = ui_weak.clone();
        ui.on_setup_set_language(move |idx| {
            with_ui(&ui_weak, |ui| {
                let mut form = ui.get_setup_form();
                form.language_idx = idx;
                ui.set_setup_form(form);
                i18n::apply_translation(language_from_idx(idx));
            });
        });
    }

    // Appearance selection in the wizard: live preview via the Theme global.
    {
        let ui_weak = ui_weak.clone();
        ui.on_setup_set_appearance(move |color_idx, theme_idx| {
            with_ui(&ui_weak, |ui| {
                let mut form = ui.get_setup_form();
                form.color_mode_idx = color_idx;
                form.theme_color_idx = theme_idx;
                ui.set_setup_form(form);
                let (mode, theme) = appearance_from_idx(color_idx, theme_idx);
                apply_appearance(&ui, mode, theme);
            });
        });
    }

    // Directory picker for the wizard (native dialog).
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_pick_setup_directory(move || {
            let ui_weak = ui_weak.clone();
            let store = store.clone();
            tokio::spawn(async move {
                let lang = store.lock().language();
                let folder = rfd::AsyncFileDialog::new()
                    .set_title(i18n::pick_download_dir_title(lang))
                    .pick_folder()
                    .await;
                let Some(handle) = folder else {
                    return;
                };
                let path = handle.path().to_string_lossy().to_string();
                let _ = slint::invoke_from_event_loop(move || {
                    with_ui(&ui_weak, |ui| {
                        let mut form = ui.get_setup_form();
                        form.default_dir = SharedString::from(&path);
                        ui.set_setup_form(form);
                    });
                });
            });
        });
    }

    // Finish / skip-all: persist the wizard settings and apply the same side
    // effects as saving the settings dialog.
    {
        let dispatcher = dispatcher.clone();
        let sync = sync.clone();
        ui.on_finish_setup(move |form| {
            finish_setup(dispatcher.clone(), sync.clone(), form);
        });
    }
}

/// Persist the wizard settings and run the shared save side effects.
fn finish_setup(dispatcher: Arc<Dispatcher>, sync: SettingsSync, form: SetupFormData) {
    let dispatcher = dispatcher.clone();
    let sync = sync.clone();

    tokio::spawn(async move {
        let old_settings = dispatcher.get_settings_blocking().unwrap_or_default();
        let lang = sync.lang();

        let saved = match dispatcher
            .save_settings_with(|settings| {
                update_app_settings_from_setup_form(settings, &form, lang)
                    .map_err(DownloadError::InvalidRequest)?;
                settings.setup_completed = true;
                settings.last_setup_step = Some(8);
                Ok(())
            })
            .await
        {
            Ok(saved) => saved,
            Err(DownloadError::InvalidRequest(msg)) => {
                tracing::error!("设置向导表单校验失败: {msg}");
                sync.toast(i18n::format_toast_settings_invalid(&msg, lang), "error", 6);
                return;
            }
            Err(err) => {
                tracing::error!("设置向导保存失败: {err:#}");
                let msg = format!("{err:#}");
                sync.toast(
                    i18n::format_toast_settings_save_failed(&msg, lang),
                    "error",
                    6,
                );
                return;
            }
        };

        sync.sync_autostart(&saved, &old_settings, lang);
        sync.sync_aria2_rpc(&saved, &old_settings, lang);
        sync.commit(&saved);
        sync.toast(
            i18n::format_toast_setup_finished(lang).to_string(),
            "success",
            5,
        );
        sync.push_ui(
            &saved,
            PushOptions {
                sync_new_task_dir: true,
                close_wizard: true,
                ..Default::default()
            },
        );
    });
}

/// Persist the wizard's current step so it reopens where the user left off.
fn close_setup_wizard(
    ui_weak: slint::Weak<MainWindow>,
    dispatcher: Arc<Dispatcher>,
    store: Arc<Mutex<TaskStore>>,
    step: i32,
) {
    // Sync the task-store language with the wizard's live language
    // selection (the @tr translation was already switched on change),
    // so interpolated Rust-side strings stay consistent after closing.
    with_ui(&ui_weak, |ui| {
        let form = ui.get_setup_form();
        store
            .lock()
            .set_language(language_from_idx(form.language_idx));
    });

    let ui_weak = ui_weak.clone();
    let dispatcher = dispatcher.clone();
    tokio::spawn(async move {
        if let Err(err) = dispatcher
            .save_settings_with(|settings| {
                settings.last_setup_step = Some(step.max(0) as u32);
                Ok(())
            })
            .await
        {
            tracing::warn!("保存设置向导进度失败: {err:#}");
        }
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui_weak, |ui| ui.set_show_setup_wizard(false));
        });
    });
}
