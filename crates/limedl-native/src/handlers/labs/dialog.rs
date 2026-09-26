//! Labs dialog lifecycle: open / close / tab switch and saving the form.

use std::time::Duration;

use crate::bridge::update_app_settings_from_labs_form;
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::i18n;
use crate::toast::push_toast;
use crate::ui_sync::refresh_labs_state;

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;

    // Open: repaint the whole Labs state from the cached settings and rules.
    {
        let current_settings = ctx.current_settings.clone();
        let rewrite_rules = ctx.rewrite_rules.clone();
        let expanded = ctx.labs_expanded_ids.clone();
        let sandbox_test_url = ctx.sandbox_test_url.clone();
        let candidates = ctx.labs_candidates.clone();
        let store = ctx.store.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_open_labs(move || {
            with_ui(&ui_weak, |ui| {
                let settings = current_settings.lock().clone();
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

    // Save: merge the form back into the cached settings, persist, then close
    // the dialog on success.
    {
        let dispatcher = ctx.dispatcher.clone();
        let current_settings = ctx.current_settings.clone();
        let rewrite_rules = ctx.rewrite_rules.clone();
        let store = ctx.store.clone();
        let toast_queue = ctx.toast_queue.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_save_labs(move |form_data| {
            let dispatcher = dispatcher.clone();
            let current_settings = current_settings.clone();
            let rewrite_rules = rewrite_rules.clone();
            let store = store.clone();
            let toast_queue = toast_queue.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let mut settings = current_settings.lock().clone();
                let lang = store.lock().language();

                update_app_settings_from_labs_form(&mut settings, &form_data);
                settings.url_rewrite.rules = rewrite_rules.lock().clone();

                match dispatcher.save_settings(&settings).await {
                    Ok(saved) => {
                        *current_settings.lock() = saved;
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
        });
    }
}
