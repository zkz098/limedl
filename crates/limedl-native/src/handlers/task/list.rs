//! List view state: category / search filters, sorting, view mode and the
//! periodic background refresh.

use std::sync::Arc;

use parking_lot::Mutex;

use crate::bridge::{SortField, TaskStore};
use crate::context::AppContext;
use crate::handlers::common::{mutate_store, reload_tasks, with_ui};
use crate::ui_sync::{persist_sort_preference, refresh_ui};
use crate::MainWindow;

/// Apply a sort mutation, refresh the window and report the resulting sort
/// state. `None` means the window was already gone, so callers must not persist
/// the user's preference (it was never shown).
fn mutate_sort(
    ui: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
    f: impl FnOnce(&mut TaskStore),
) -> Option<(i32, bool)> {
    let mut guard = store.lock();
    f(&mut guard);
    let state = (guard.sort_field(), guard.sort_asc());
    let ui = ui.upgrade()?;
    refresh_ui(&ui, &guard);
    Some(state)
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();
    let dispatcher = ctx.dispatcher.clone();
    let current_settings = ctx.current_settings.clone();

    // Category filter
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_select_category(move |cat| {
            mutate_store(&ui_weak, &store, |store| store.set_category(cat));
            with_ui(&ui_weak, |ui| ui.set_active_category(cat));
        });
    }

    // Search query
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_search_changed(move |query| {
            let query = query.to_string();
            mutate_store(&ui_weak, &store, |store| store.set_search_query(query));
        });
    }

    // Sort field & order (dropdown)
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let dispatcher = dispatcher.clone();
        let current_settings = current_settings.clone();
        ui.on_set_sort_field(move |field_idx| {
            let Some((field, asc)) =
                mutate_sort(&ui_weak, &store, |store| store.set_sort_field(SortField::from(field_idx)))
            else {
                return;
            };
            persist_sort_preference(&dispatcher, &current_settings, field, asc);
        });
    }

    // Sort direction toggle (toolbar button)
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let dispatcher = dispatcher.clone();
        let current_settings = current_settings.clone();
        ui.on_toggle_sort_asc(move || {
            let Some((field, asc)) =
                mutate_sort(&ui_weak, &store, |store| {
                    store.toggle_sort_order();
                })
            else {
                return;
            };
            persist_sort_preference(&dispatcher, &current_settings, field, asc);
        });
    }

    // Table column header click: same field toggles the order, a new field sorts
    // ascending. Unlike the dropdown this is a transient re-sort, so it is not
    // persisted.
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_table_sort_clicked(move |col_idx| {
            let _ = mutate_sort(&ui_weak, &store, |store| {
                let target_field = SortField::from(col_idx);
                if store.sort_field() == target_field as i32 {
                    store.toggle_sort_order();
                } else {
                    store.set_sort_field(target_field);
                }
            });
        });
    }

    // View mode toggle (cards <-> table)
    {
        let ui_weak = ui_weak.clone();
        ui.on_toggle_view_mode(move || {
            with_ui(&ui_weak, |ui| {
                let current = ui.get_view_mode();
                ui.set_view_mode(if current == 0 { 1 } else { 0 });
            });
        });
    }

    // Background refresh timer: resynchronize the whole list from the backend.
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let dispatcher = dispatcher.clone();
        ui.on_background_refresh(move || {
            reload_tasks(&ui_weak, &store, &dispatcher);
        });
    }
}
