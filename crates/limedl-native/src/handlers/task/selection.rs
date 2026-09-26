//! Multi-selection callbacks (click, shift-range, select-all, clear).

use crate::context::AppContext;
use crate::handlers::common::mutate_store;

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_toggle_select_task(move |id_str| {
            mutate_store(&ui_weak, &store, |store| store.toggle_select(&id_str));
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_range_select_task(move |id_str| {
            mutate_store(&ui_weak, &store, |store| store.select_range(&id_str));
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_select_all(move || {
            mutate_store(&ui_weak, &store, |store| store.select_all());
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_clear_selection(move || {
            mutate_store(&ui_weak, &store, |store| store.clear_selection());
        });
    }
}
