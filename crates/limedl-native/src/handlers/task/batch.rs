//! Batch operations: multi-select pause/resume/delete, the list keyboard
//! shortcuts and the "clear completed" action.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::{DownloadState, TaskId};

use crate::bridge::TaskStore;
use crate::context::AppContext;
use crate::handlers::common::{
    TaskAction, refresh_after_removal, reload_tasks, spawn_batch_action, spawn_for_state,
    take_selection,
};
use crate::i18n;
use crate::toast::{ToastQueue, push_toast};
use crate::MainWindow;

/// Delete (or purge) the current multi-selection: the rows disappear
/// immediately, the backend catches up in the background and the list is
/// reloaded once it does.
///
/// Shared by the batch-remove button and the `Shift+Delete` shortcut, which
/// used to carry byte-identical copies of this flow.
fn delete_selected(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    active_inspector_id: &Arc<Mutex<Option<String>>>,
    dispatcher: &Dispatcher,
    delete_files: bool,
) {
    let ids = take_selection(store);
    if ids.is_empty() {
        return;
    }
    refresh_after_removal(ui, store, active_inspector_id, &ids);

    let action = if delete_files {
        TaskAction::Purge
    } else {
        TaskAction::Remove
    };
    let ui = ui.clone();
    let store = store.clone();
    let dispatcher = dispatcher.clone();
    tokio::spawn(async move {
        for id in &ids {
            let _ = action.apply(&dispatcher, id).await;
        }
        reload_tasks(&ui, &store, &dispatcher);
    });
}

/// Drop the records of every finished task; the files stay on disk, matching
/// the web client's "clear completed" action.
fn clear_completed(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    active_inspector_id: &Arc<Mutex<Option<String>>>,
    toast_queue: &ToastQueue,
    dispatcher: &Dispatcher,
) {
    let completed_ids = {
        let mut store = store.lock();
        let ids = store.completed_ids();
        for id in &ids {
            store.remove(id);
        }
        ids
    };

    let lang = store.lock().language();
    if completed_ids.is_empty() {
        push_toast(
            ui,
            toast_queue,
            i18n::format_toast_clear_completed_none(lang).to_string(),
            "info",
            Duration::from_secs(4),
        );
        return;
    }

    refresh_after_removal(ui, store, active_inspector_id, &completed_ids);

    let ui = ui.clone();
    let store = store.clone();
    let toast_queue = toast_queue.clone();
    let dispatcher = dispatcher.clone();
    tokio::spawn(async move {
        let mut cleared = 0usize;
        for id in &completed_ids {
            if let Ok(task_id) = TaskId::from_wire_string(id)
                && TaskAction::Remove.call(&dispatcher, &task_id).await.is_ok()
            {
                cleared += 1;
            }
        }
        if cleared == 0 {
            push_toast(
                &ui,
                &toast_queue,
                i18n::format_toast_clear_completed_failed(lang).to_string(),
                "error",
                Duration::from_secs(5),
            );
            return;
        }
        push_toast(
            &ui,
            &toast_queue,
            i18n::format_toast_clear_completed(cleared, lang),
            "success",
            Duration::from_secs(4),
        );
        reload_tasks(&ui, &store, &dispatcher);
    });
}

/// Space hotkey: pause everything when the selection (or, without a selection,
/// the whole list) is downloading, resume it otherwise.
fn toggle_all_matching(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    dispatcher: &Dispatcher,
) {
    let (ids, any_downloading) = {
        let store = store.lock();
        let selected = store.selected_ids();
        if selected.is_empty() {
            let all_ids: Vec<String> = store
                .filtered_items()
                .iter()
                .map(|item| item.id.to_string())
                .collect();
            let (_, downloading_count, _, _, _) = store.counts();
            (all_ids, downloading_count > 0)
        } else {
            let downloading = store
                .filtered_items()
                .iter()
                .any(|item| selected.contains(&item.id.to_string()) && item.can_pause);
            (selected, downloading)
        }
    };

    let action = if any_downloading {
        TaskAction::Pause
    } else {
        TaskAction::Resume
    };
    spawn_batch_action(ui, store, dispatcher, ids, action);
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();
    let dispatcher = ctx.dispatcher.clone();
    let toast_queue = ctx.toast_queue.clone();
    let active_inspector_id = ctx.active_inspector_id.clone();

    // Batch pause / resume
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let dispatcher = dispatcher.clone();
        ui.on_batch_pause(move || {
            let ids = store.lock().selected_ids();
            spawn_batch_action(&ui_weak, &store, &dispatcher, ids, TaskAction::Pause);
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let dispatcher = dispatcher.clone();
        ui.on_batch_resume(move || {
            let ids = store.lock().selected_ids();
            spawn_batch_action(&ui_weak, &store, &dispatcher, ids, TaskAction::Resume);
        });
    }

    // Batch remove (with or without files on disk)
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let dispatcher = dispatcher.clone();
        let active_inspector_id = active_inspector_id.clone();
        ui.on_batch_remove(move |delete_files| {
            delete_selected(
                &ui_weak,
                &store,
                &active_inspector_id,
                &dispatcher,
                delete_files,
            );
        });
    }

    // Keyboard: Space toggles pause/resume, Shift+Delete purges the selection.
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let dispatcher = dispatcher.clone();
        ui.on_hotkey_space(move || {
            toggle_all_matching(&ui_weak, &store, &dispatcher);
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let dispatcher = dispatcher.clone();
        let active_inspector_id = active_inspector_id.clone();
        ui.on_hotkey_delete(move |delete_files| {
            delete_selected(
                &ui_weak,
                &store,
                &active_inspector_id,
                &dispatcher,
                delete_files,
            );
        });
    }

    // Toolbar: pause / resume everything in a given state
    {
        let dispatcher = dispatcher.clone();
        ui.on_pause_all(move || {
            spawn_for_state(
                &dispatcher,
                |item| matches!(item.state, DownloadState::Downloading),
                TaskAction::Pause,
            );
        });
    }

    {
        let dispatcher = dispatcher.clone();
        ui.on_resume_all(move || {
            spawn_for_state(
                &dispatcher,
                |item| matches!(item.state, DownloadState::Paused),
                TaskAction::Resume,
            );
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let dispatcher = dispatcher.clone();
        let toast_queue = toast_queue.clone();
        let active_inspector_id = active_inspector_id.clone();
        ui.on_clear_completed(move || {
            clear_completed(
                &ui_weak,
                &store,
                &active_inspector_id,
                &toast_queue,
                &dispatcher,
            );
        });
    }
}
