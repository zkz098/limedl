//! Labs dialog lifecycle: open / close / tab switch and saving the form.

use std::time::Duration;

use crate::LabsFormData;
use crate::MainWindow;
use crate::bridge::TaskStore;
use crate::bridge::update_app_settings_from_labs_form;
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::i18n;
use crate::toast::ToastQueue;
use crate::toast::push_toast;
use crate::ui_sync::refresh_labs_state;
use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::UrlRewriteRule;
use parking_lot::Mutex;
use std::sync::Arc;

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;

    // Open: repaint the whole Labs state from the current settings and rules.
    {
        let dispatcher = ctx.dispatcher.clone();
        let rewrite_rules = ctx.rewrite_rules.clone();
        let expanded = ctx.labs_expanded_ids.clone();
        let sandbox_test_url = ctx.sandbox_test_url.clone();
        let candidates = ctx.labs_candidates.clone();
        let store = ctx.store.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_open_labs(move || {
            with_ui(&ui_weak, |ui| {
                let settings = dispatcher.get_settings_blocking().unwrap_or_default();
                let rules = rewrite_rules.lock().clone();
                let expanded = expanded.lock().clone();
                let url = sandbox_test_url.lock().clone();
                let candidates = candidates.lock().clone();
                let lang = store.lock().language();
                refresh_labs_state(
                    &ui,
                    &settings,
                    &rules,
                    &expanded,
                    &url,
                    false,
                    &candidates,
                    lang,
                );
                ui.set_show_labs(true);
            });
        });
    }

    {
        let ui_weak = ctx.ui_weak.clone();
        ui.on_close_labs(move || {
            with_ui(&ui_weak, |ui| ui.set_show_labs(false));
        });
    }

    {
        let ui_weak = ctx.ui_weak.clone();
        ui.on_set_labs_tab(move |tab| {
            with_ui(&ui_weak, |ui| ui.set_labs_tab(tab));
        });
    }

    // Save: merge the form back into the settings, persist, then close the
    // dialog on success.
    {
        let dispatcher = ctx.dispatcher.clone();
        let rewrite_rules = ctx.rewrite_rules.clone();
        let store = ctx.store.clone();
        let toast_queue = ctx.toast_queue.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_save_labs(move |form_data| {
            save_labs(
                ui_weak.clone(),
                dispatcher.clone(),
                rewrite_rules.clone(),
                store.clone(),
                toast_queue.clone(),
                form_data,
            );
        });
    }
}

/// Merge the labs form into the settings, persist and close on success.
fn save_labs(
    ui_weak: slint::Weak<MainWindow>,
    dispatcher: Arc<Dispatcher>,
    rewrite_rules: Arc<Mutex<Vec<UrlRewriteRule>>>,
    store: Arc<Mutex<TaskStore>>,
    toast_queue: ToastQueue,
    form_data: LabsFormData,
) {
    let dispatcher = dispatcher.clone();
    let rewrite_rules = rewrite_rules.clone();
    let store = store.clone();
    let toast_queue = toast_queue.clone();
    let ui_weak = ui_weak.clone();

    tokio::spawn(async move {
        let lang = store.lock().language();

        match dispatcher
            .save_settings_with(|settings| {
                update_app_settings_from_labs_form(settings, &form_data);
                settings.url_rewrite.rules = rewrite_rules.lock().clone();
                Ok(())
            })
            .await
        {
            Ok(_saved) => {
                push_toast(
                    &ui_weak,
                    &toast_queue,
                    i18n::format_toast_labs_saved(lang).to_string(),
                    "success",
                    Duration::from_secs(4),
                );
                let _ = slint::invoke_from_event_loop(move || {
                    with_ui(&ui_weak, |ui| ui.set_show_labs(false));
                });
            }
            Err(err) => {
                tracing::error!("保存实验室设置失败: {err:#}");
                let msg = format!("{err:#}");
                push_toast(
                    &ui_weak,
                    &toast_queue,
                    i18n::format_toast_labs_save_failed(&msg, lang),
                    "error",
                    Duration::from_secs(6),
                );
            }
        }
    });
}
