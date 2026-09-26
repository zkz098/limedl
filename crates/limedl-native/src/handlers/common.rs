//! Shared plumbing for the Slint callback handlers.
//!
//! Every module under `handlers/` wires `MainWindow` callbacks to the
//! dispatcher. That wiring is pure boilerplate: upgrade the window, lock the
//! store, refresh the list, spawn the backend call. Keeping the boilerplate
//! here is what lets each subsystem module read like the flow it implements
//! instead of like a wall of `as_weak()` / `upgrade()` blocks.

use std::sync::Arc;

use parking_lot::Mutex;

use limedl_core::dispatcher::Dispatcher;
use limedl_core::error::Result as CoreResult;
use limedl_core::types::{DownloadSummary, TaskId};

use crate::bridge::TaskStore;
use crate::ui_sync::refresh_ui;
use crate::MainWindow;

/// Run `f` with an upgraded window handle, skipping the callback when the
/// window has already been dropped.
///
/// This is the one place that knows how a `slint::Weak<MainWindow>` is turned
/// back into a live component; everything else in `handlers/` just calls it.
pub fn with_ui(ui: &slint::Weak<MainWindow>, f: impl FnOnce(MainWindow)) {
    if let Some(ui) = ui.upgrade() {
        f(ui);
    }
}

/// Like [`with_ui`], but returns the closure result, for callbacks that must
/// read state out of the window before spawning background work.
pub fn read_ui<T>(ui: &slint::Weak<MainWindow>, f: impl FnOnce(&MainWindow) -> T) -> Option<T> {
    ui.upgrade().map(|ui| f(&ui))
}

/// Apply a store mutation and push the refreshed rows to the window. The store
/// lock is held across `refresh_ui` so the rows match the counters.
pub fn mutate_store(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    mutate: impl FnOnce(&mut TaskStore),
) {
    let mut guard = store.lock();
    mutate(&mut guard);
    with_ui(ui, |ui| refresh_ui(&ui, &guard));
}

/// Refresh the list from a background thread (the callback runs on the UI
/// thread).
pub fn refresh_from_anywhere(ui: &slint::Weak<MainWindow>, store: &Arc<Mutex<TaskStore>>) {
    let ui = ui.clone();
    let store = store.clone();
    let _ = slint::invoke_from_event_loop(move || {
        with_ui(&ui, |ui| {
            let store = store.lock();
            refresh_ui(&ui, &store);
        });
    });
}

/// Re-read the task list from the dispatcher and push the result into the
/// window. Used to resynchronize after a mutation the backend may have only
/// partially applied; safe to call from any thread.
pub fn reload_tasks(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    dispatcher: &Dispatcher,
) {
    let ui = ui.clone();
    let store = store.clone();
    let dispatcher = dispatcher.clone();
    tokio::spawn(async move {
        let Ok(list) = dispatcher.list().await else {
            return;
        };
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui, |ui| {
                let mut store = store.lock();
                store.replace_all(list);
                refresh_ui(&ui, &store);
            });
        });
    });
}

/// Take the current multi-selection out of the store and clear it, so the rows
/// disappear from the list before the backend confirms.
pub fn take_selection(store: &Arc<Mutex<TaskStore>>) -> Vec<String> {
    let mut store = store.lock();
    let ids = store.selected_ids();
    store.clear_selection();
    for id in &ids {
        store.remove(id);
    }
    ids
}

/// Push the refreshed list after `ids` were dropped, closing the inspector when
/// its subject was among them.
pub fn refresh_after_removal(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    active_inspector_id: &Arc<Mutex<Option<String>>>,
    ids: &[String],
) {
    with_ui(ui, |ui| {
        let store = store.lock();
        refresh_ui(&ui, &store);
        if let Some(ref current_id) = *active_inspector_id.lock()
            && ids.contains(current_id)
        {
            ui.set_show_inspector(false);
        }
    });
}

/// Drop `ids` from the local store (optimistic UI) and refresh.
pub fn drop_locally(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    active_inspector_id: &Arc<Mutex<Option<String>>>,
    ids: &[String],
) {
    {
        let mut store = store.lock();
        for id in ids {
            store.remove(id);
        }
    }
    refresh_after_removal(ui, store, active_inspector_id, ids);
}

/// A per-task backend operation, so batch / hotkey / single-task callbacks share
/// one implementation instead of repeating the same match at every call site.
#[derive(Clone, Copy, Debug)]
pub enum TaskAction {
    Pause,
    Resume,
    Remove,
    Purge,
    OpenInExplorer,
}

impl TaskAction {
    /// Apply the action to a wire id, skipping ids that cannot be parsed.
    pub async fn apply(self, dispatcher: &Dispatcher, id: &str) -> CoreResult<()> {
        let Ok(task_id) = TaskId::from_wire_string(id) else {
            tracing::warn!("{}失败: 无法解析任务 ID {id}", self.label());
            return Ok(());
        };
        self.call(dispatcher, &task_id).await
    }

    /// Apply the action to an id that is already parsed.
    pub async fn call(self, dispatcher: &Dispatcher, task_id: &TaskId) -> CoreResult<()> {
        match self {
            TaskAction::Pause => dispatcher.pause(task_id).await.map(|_| ()),
            TaskAction::Resume => dispatcher.resume(task_id).await.map(|_| ()),
            TaskAction::Remove => dispatcher.remove(task_id).await.map(|_| ()),
            TaskAction::Purge => dispatcher.purge(task_id).await.map(|_| ()),
            TaskAction::OpenInExplorer => dispatcher.open_in_explorer(task_id).await,
        }
    }

    /// Log label used when the action fails.
    pub fn label(self) -> &'static str {
        match self {
            TaskAction::Pause => "暂停任务",
            TaskAction::Resume => "恢复任务",
            TaskAction::Remove => "删除任务",
            TaskAction::Purge => "彻底删除任务",
            TaskAction::OpenInExplorer => "打开任务文件目录",
        }
    }
}

/// Run `action` for every id on the current selection and refresh once at the
/// end. Errors are ignored: batch operations are best effort, matching the
/// per-task buttons.
pub fn spawn_batch_action(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    dispatcher: &Dispatcher,
    ids: Vec<String>,
    action: TaskAction,
) {
    if ids.is_empty() {
        return;
    }
    let ui = ui.clone();
    let store = store.clone();
    let dispatcher = dispatcher.clone();
    tokio::spawn(async move {
        for id in &ids {
            let _ = action.apply(&dispatcher, id).await;
        }
        refresh_from_anywhere(&ui, &store);
    });
}

/// Run `action` over every task the backend reports for which `predicate` holds
/// (used by the pause-all / resume-all buttons).
pub fn spawn_for_state(
    dispatcher: &Dispatcher,
    predicate: impl Fn(&DownloadSummary) -> bool + Send + 'static,
    action: TaskAction,
) {
    let dispatcher = dispatcher.clone();
    tokio::spawn(async move {
        let Ok(list) = dispatcher.list().await else {
            return;
        };
        for item in list {
            if predicate(&item) {
                let _ = action.apply(&dispatcher, &item.id).await;
            }
        }
    });
}

/// Fire-and-forget action for a single task, logging failures.
pub fn spawn_action(dispatcher: &Dispatcher, id: String, action: TaskAction) {
    let dispatcher = dispatcher.clone();
    let label = action.label();
    tokio::spawn(async move {
        if let Err(err) = action.apply(&dispatcher, &id).await {
            tracing::error!("{label}失败: {err:#}");
        }
    });
}
