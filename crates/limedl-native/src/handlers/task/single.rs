//! Single-task callbacks: per-row pause/resume, priority, open, double-click
//! behavior, remove and purge.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::TaskId;

use crate::bridge::{TaskStore, str_to_priority};
use crate::context::AppContext;
use crate::handlers::common::{TaskAction, drop_locally, reload_tasks, spawn_action};
use crate::i18n;
use crate::task_ops::handle_task_double_click;
use crate::toast::push_toast;
use crate::MainWindow;

/// Remove (or purge) one task: the row disappears immediately, the backend call
/// runs in the background and the list is reloaded if it fails.
fn remove_one(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    active_inspector_id: &Arc<Mutex<Option<String>>>,
    dispatcher: &Dispatcher,
    id: String,
    action: TaskAction,
) {
    let label = action.label();
    drop_locally(ui, store, active_inspector_id, std::slice::from_ref(&id));

    let ui = ui.clone();
    let store = store.clone();
    let dispatcher = dispatcher.clone();
    tokio::spawn(async move {
        if let Err(err) = action.apply(&dispatcher, &id).await {
            tracing::error!("{label}失败: {err:#}");
            reload_tasks(&ui, &store, &dispatcher);
        }
    });
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;

    {
        let dispatcher = ctx.dispatcher.clone();
        ui.on_pause_task(move |id_str| {
            spawn_action(&dispatcher, id_str.to_string(), TaskAction::Pause);
        });
    }

    {
        let dispatcher = ctx.dispatcher.clone();
        ui.on_resume_task(move |id_str| {
            spawn_action(&dispatcher, id_str.to_string(), TaskAction::Resume);
        });
    }

    {
        let dispatcher = ctx.dispatcher.clone();
        ui.on_open_task_explorer(move |id_str| {
            spawn_action(&dispatcher, id_str.to_string(), TaskAction::OpenInExplorer);
        });
    }

    // Remove / purge a single task
    {
        let ui_weak = ctx.ui_weak.clone();
        let store = ctx.store.clone();
        let dispatcher = ctx.dispatcher.clone();
        let active_inspector_id = ctx.active_inspector_id.clone();
        ui.on_remove_task(move |id_str| {
            remove_one(
                &ui_weak,
                &store,
                &active_inspector_id,
                &dispatcher,
                id_str.to_string(),
                TaskAction::Remove,
            );
        });
    }

    {
        let ui_weak = ctx.ui_weak.clone();
        let store = ctx.store.clone();
        let dispatcher = ctx.dispatcher.clone();
        let active_inspector_id = ctx.active_inspector_id.clone();
        ui.on_purge_single_task(move |id_str| {
            remove_one(
                &ui_weak,
                &store,
                &active_inspector_id,
                &dispatcher,
                id_str.to_string(),
                TaskAction::Purge,
            );
        });
    }

    // Double-click behavior (Settings → General: double_click on_completed /
    // on_uncompleted). Mirrors the web client: completed tasks open the file /
    // explorer / download dir; uncompleted tasks toggle pause/resume.
    {
        let dispatcher = ctx.dispatcher.clone();
        let store = ctx.store.clone();
        let current_settings = ctx.current_settings.clone();
        ui.on_task_double_clicked(move |id_str| {
            let dispatcher = dispatcher.clone();
            let store = store.clone();
            let current_settings = current_settings.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Err(err) =
                    handle_task_double_click(&dispatcher, &store, &current_settings, &id_str).await
                {
                    tracing::error!("双击任务操作失败: {err:#}");
                }
            });
        });
    }

    // Priority change from the row badge / context menu
    {
        let ui_weak = ctx.ui_weak.clone();
        let store = ctx.store.clone();
        let dispatcher = ctx.dispatcher.clone();
        let toast_queue = ctx.toast_queue.clone();
        ui.on_set_task_priority(move |id_str, priority_code| {
            let id_str = id_str.to_string();
            let priority_code = priority_code.to_string();
            let ui_weak = ui_weak.clone();
            let store = store.clone();
            let dispatcher = dispatcher.clone();
            let toast_queue = toast_queue.clone();
            tokio::spawn(async move {
                let lang = store.lock().language();
                let priority = str_to_priority(&priority_code);
                let file_name = store
                    .lock()
                    .get_summary(&id_str)
                    .map(|summary| summary.file_name)
                    .unwrap_or_default();
                let Ok(task_id) = TaskId::from_wire_string(&id_str) else {
                    tracing::warn!("优先级设置失败: 无法解析任务 ID {id_str}");
                    return;
                };
                match dispatcher.set_priority(&task_id, priority).await {
                    Ok(()) => push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_priority_set(&file_name, priority, lang),
                        "info",
                        Duration::from_secs(4),
                    ),
                    Err(err) => {
                        tracing::error!("优先级设置失败: {err:#}");
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_priority_failed(&format!("{err:#}"), lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }
}
