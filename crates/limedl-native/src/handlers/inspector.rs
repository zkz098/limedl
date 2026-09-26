//! Task inspector callbacks (open / close / tab / BT file selection).

use std::time::Duration;

use limedl_core::types::TaskId;
use slint::Model;

use crate::bridge::summary_to_inspector_info;
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::i18n;
use crate::toast::push_toast;

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let active_inspector_id = ctx.active_inspector_id.clone();
        ui.on_open_inspector(move |id_str| {
            let id = id_str.to_string();
            *active_inspector_id.lock() = Some(id.clone());

            with_ui(&ui_weak, |ui| {
                let store = store.lock();
                if let Some(summary) = store.get_summary(&id) {
                    ui.set_inspector_info(summary_to_inspector_info(&summary, store.language()));
                }
                ui.set_inspector_tab(0);
                ui.set_show_inspector(true);
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let active_inspector_id = ctx.active_inspector_id.clone();
        ui.on_close_inspector(move || {
            *active_inspector_id.lock() = None;
            with_ui(&ui_weak, |ui| ui.set_show_inspector(false));
        });
    }

    {
        let ui_weak = ui_weak.clone();
        ui.on_set_inspector_tab(move |tab_idx| {
            with_ui(&ui_weak, |ui| ui.set_inspector_tab(tab_idx));
        });
    }

    // BT file selection inside the inspector: at least one file must stay
    // selected, otherwise the task would download nothing.
    {
        let ui_weak = ui_weak.clone();
        let dispatcher = ctx.dispatcher.clone();
        let store = store.clone();
        let active_inspector_id = ctx.active_inspector_id.clone();
        let toast_queue = ctx.toast_queue.clone();
        ui.on_toggle_inspector_file(move |index, currently_included| {
            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            let store = store.clone();
            let active_inspector_id = active_inspector_id.clone();
            let toast_queue = toast_queue.clone();

            with_ui(&ui_weak, |ui| {
                let files = ui.get_inspector_files();
                let included: Vec<usize> = files
                    .iter()
                    .filter(|file| {
                        if file.index == index {
                            !currently_included
                        } else {
                            file.included
                        }
                    })
                    .map(|file| file.index as usize)
                    .collect();

                if included.is_empty() {
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_bt_files_keep_one(store.lock().language()).to_string(),
                        "warning",
                        Duration::from_secs(5),
                    );
                    return;
                }
                let Some(task_id_str) = active_inspector_id.lock().clone() else {
                    return;
                };
                let Ok(task_id) = TaskId::from_wire_string(&task_id_str) else {
                    return;
                };

                let ui_weak = ui_weak.clone();
                tokio::spawn(async move {
                    let lang = store.lock().language();
                    if let Err(err) = dispatcher.bt_update_files(&task_id, included).await {
                        tracing::error!("更新 BT 文件选择失败: {err:#}");
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_bt_files_failed(&format!("{err:#}"), lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                });
            });
        });
    }
}
