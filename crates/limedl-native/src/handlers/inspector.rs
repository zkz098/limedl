use std::time::Duration;
use limedl_core::types::TaskId;
use slint::{ComponentHandle, Model};

use crate::bridge::summary_to_inspector_info;
use crate::context::AppContext;
use crate::i18n;
use crate::toast::push_toast;

pub fn register(ctx: &AppContext) {
    let main_window = &ctx.ui;
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let toast_queue = ctx.toast_queue.clone();
    let active_inspector_id = ctx.active_inspector_id.clone();

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let active_inspector_id_clone = active_inspector_id.clone();
        main_window.on_open_inspector(move |id_str| {
            let id = id_str.to_string();
            *active_inspector_id_clone.lock() = Some(id.clone());

            if let Some(ui) = ui_weak.upgrade() {
                let s = store_clone.lock();
                if let Some(summary) = s.get_summary(&id) {
                    ui.set_inspector_info(summary_to_inspector_info(&summary, s.language()));
                }
                ui.set_inspector_tab(0);
                ui.set_show_inspector(true);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let active_inspector_id_clone = active_inspector_id.clone();
        main_window.on_close_inspector(move || {
            *active_inspector_id_clone.lock() = None;
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_inspector(false);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_set_inspector_tab(move |tab_idx| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_inspector_tab(tab_idx);
            }
        });
    }

    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let active_inspector_id_clone = active_inspector_id.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_toggle_inspector_file(move |index, currently_included| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let active_inspector_id_clone = active_inspector_id_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            if let Some(ui) = ui_weak.upgrade() {
                let files = ui.get_inspector_files();
                let mut included: Vec<usize> = Vec::new();
                for file in files.iter() {
                    let is_target = file.index == index;
                    let keep = if is_target {
                        !currently_included
                    } else {
                        file.included
                    };
                    if keep {
                        included.push(file.index as usize);
                    }
                }
                if included.is_empty() {
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_bt_files_keep_one(store_clone.lock().language())
                            .to_string(),
                        "warning",
                        Duration::from_secs(5),
                    );
                    return;
                }
                let Some(task_id_str) = active_inspector_id_clone.lock().clone() else {
                    return;
                };
                let Ok(task_id) = TaskId::from_wire_string(&task_id_str) else {
                    return;
                };
                tokio::spawn(async move {
                    let lang = store_clone.lock().language();
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
            }
        });
    }
}
