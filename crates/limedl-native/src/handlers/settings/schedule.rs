//! Speed limit callbacks: the per-task limit dialog and the global schedule
//! editor (rows live in the UI model until Save).

use slint::{Model, ModelRc, SharedString, VecModel};

use limedl_core::types::TaskId;

use crate::bridge::{SpeedLimitSlotText, speed_limit_slots_to_slint};
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::i18n;
use crate::i18n::Language;
use crate::ui_sync::read_schedule_rows;
use crate::{MainWindow, SpeedLimitSlotItem};

/// Push schedule rows into the window model.
fn push_schedule_rows(ui: &MainWindow, rows: &[SpeedLimitSlotText], lang: Language) {
    ui.set_speed_limit_slots(ModelRc::new(VecModel::from(speed_limit_slots_to_slint(
        rows, lang,
    ))));
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();

    // Per-task speed limit dialog
    {
        let ui_weak = ui_weak.clone();
        ui.on_open_speed_limit_dialog(move || {
            with_ui(&ui_weak, |ui| ui.set_show_speed_limit_dialog(true));
        });
    }

    {
        let ui_weak = ui_weak.clone();
        ui.on_close_speed_limit_dialog(move || {
            with_ui(&ui_weak, |ui| ui.set_show_speed_limit_dialog(false));
        });
    }

    {
        let dispatcher = ctx.dispatcher.clone();
        let active_inspector_id = ctx.active_inspector_id.clone();
        let ui_weak = ui_weak.clone();
        ui.on_submit_speed_limit(move |dl_kb_str, ul_kb_str| {
            let download_bps = dl_kb_str
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .map(|kb| kb * 1024);
            let upload_bps = ul_kb_str
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .map(|kb| kb * 1024);

            if let Some(ref task_id_str) = *active_inspector_id.lock()
                && let Ok(task_id) = TaskId::from_wire_string(task_id_str)
            {
                let _ = dispatcher.bt_set_speed_limit(&task_id, download_bps, upload_bps);
            }

            with_ui(&ui_weak, |ui| ui.set_show_speed_limit_dialog(false));
        });
    }

    // Global schedule editor
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_schedule_set_enabled(move |enabled| {
            let lang = store.lock().language();
            with_ui(&ui_weak, |ui| {
                let rows: Vec<SpeedLimitSlotText> = if enabled {
                    vec![SpeedLimitSlotText::default()]
                } else {
                    Vec::new()
                };
                push_schedule_rows(&ui, &rows, lang);
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_schedule_add(move || {
            let lang = store.lock().language();
            with_ui(&ui_weak, |ui| {
                let mut rows = read_schedule_rows(&ui);
                rows.push(SpeedLimitSlotText::default());
                push_schedule_rows(&ui, &rows, lang);
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_schedule_remove(move |idx| {
            let lang = store.lock().language();
            with_ui(&ui_weak, |ui| {
                let mut rows = read_schedule_rows(&ui);
                let idx = idx.max(0) as usize;
                if idx < rows.len() {
                    rows.remove(idx);
                }
                push_schedule_rows(&ui, &rows, lang);
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_schedule_update(move |idx, field, value| {
            let lang = store.lock().language();
            with_ui(&ui_weak, |ui| {
                let model = ui.get_speed_limit_slots();
                let Some(vec_model) = model.as_any().downcast_ref::<VecModel<SpeedLimitSlotItem>>()
                else {
                    return;
                };
                let idx = idx.max(0) as usize;
                let Some(mut item) = vec_model.row_data(idx) else {
                    return;
                };
                match field.as_str() {
                    "start" => item.start_hour = value.clone(),
                    "end" => item.end_hour = value.clone(),
                    "limit" => item.limit_kb = value.clone(),
                    _ => return,
                }
                // Refresh the derived row text (range summary + midnight
                // marker) without rebuilding the model, so the focused input
                // keeps its caret.
                let start = item.start_hour.trim().parse::<u32>().unwrap_or(0).min(23);
                let end = item.end_hour.trim().parse::<u32>().unwrap_or(0).min(23);
                let limit = item.limit_kb.trim().parse::<u64>().unwrap_or(0);
                item.wraps = start >= end;
                item.summary =
                    SharedString::from(i18n::format_schedule_summary(start, end, limit, lang));
                vec_model.set_row_data(idx, item);
            });
        });
    }
}
