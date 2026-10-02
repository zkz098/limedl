//! Per-directory media override rows.
//!
//! The rows live in the UI model until Save, exactly like the speed-limit
//! schedule: the dialog's model is the authoritative text state, and
//! `handlers::settings::dialog` turns it into settings when saving.

use std::sync::Arc;

use parking_lot::Mutex;
use slint::{Model, ModelRc, VecModel};

use crate::bridge::{DiskTypeOverrideText, TaskStore, disk_override_rows_to_slint, index_is_hdd};
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::i18n::Language;
use crate::ui_sync::read_disk_override_rows;
use crate::{DiskTypeOverrideItem, MainWindow};

/// Push override rows into the window model.
pub fn push_disk_override_rows(ui: &MainWindow, rows: &[DiskTypeOverrideText], lang: Language) {
    ui.set_disk_type_overrides(ModelRc::new(VecModel::from(disk_override_rows_to_slint(
        rows, lang,
    ))));
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_disk_override_add(move || {
            let lang = store.lock().language();
            with_ui(&ui_weak, |ui| {
                let mut rows = read_disk_override_rows(&ui);
                rows.push(DiskTypeOverrideText::default());
                push_disk_override_rows(&ui, &rows, lang);
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_disk_override_remove(move |index| {
            let lang = store.lock().language();
            with_ui(&ui_weak, |ui| {
                let mut rows = read_disk_override_rows(&ui);
                let index = index.max(0) as usize;
                if index < rows.len() {
                    rows.remove(index);
                }
                push_disk_override_rows(&ui, &rows, lang);
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_disk_override_path_edited(move |index, path| {
            edit_override_row(ui_weak.clone(), store.clone(), index, |row| {
                row.path = path.to_string();
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_disk_override_media_selected(move |index, media| {
            edit_override_row(ui_weak.clone(), store.clone(), index, |row| {
                row.is_hdd = index_is_hdd(media);
            });
        });
    }
}

/// Mutate one row in place and refresh its derived text.
///
/// The model row is updated through `set_row_data` rather than by rebuilding the
/// whole model, so an in-progress text edit keeps its caret and selection —
/// the same reason the schedule editor works this way. `path` is deliberately
/// copied back verbatim: the derived hint uses the trimmed path, but the input
/// keeps what the user actually typed.
fn edit_override_row(
    ui_weak: slint::Weak<MainWindow>,
    store: Arc<Mutex<TaskStore>>,
    index: i32,
    edit: impl FnOnce(&mut DiskTypeOverrideText),
) {
    let lang = store.lock().language();
    with_ui(&ui_weak, |ui| {
        let model = ui.get_disk_type_overrides();
        let Some(vec_model) = model
            .as_any()
            .downcast_ref::<VecModel<DiskTypeOverrideItem>>()
        else {
            return;
        };
        let index = index.max(0) as usize;
        // Out-of-range indices are ignored: the model and the store can briefly
        // disagree while the list is rebuilt.
        let Some(item) = vec_model.row_data(index) else {
            return;
        };

        let mut row = DiskTypeOverrideText {
            path: item.path.to_string(),
            is_hdd: index_is_hdd(item.media_idx),
        };
        edit(&mut row);

        // Rebuild only the derived fields; `path` carries the typed text through
        // verbatim (untrimmed) so the focused input does not lose its caret.
        let Some(refreshed) = disk_override_rows_to_slint(std::slice::from_ref(&row), lang)
            .into_iter()
            .next()
        else {
            return;
        };
        vec_model.set_row_data(index, refreshed);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::SharedString;

    fn item(path: &str, media_idx: i32, detected: &str) -> DiskTypeOverrideItem {
        DiskTypeOverrideItem {
            path: SharedString::from(path),
            media_idx,
            detected_text: SharedString::from(detected),
        }
    }

    /// The row state a callback starts from is what the model shows, so an edit
    /// cannot resurrect a value the model already replaced.
    #[test]
    fn row_state_is_read_back_from_the_model() {
        let model = VecModel::from(vec![item(r"D:\Downloads", 1, "Detected: HDD")]);
        let item = model.row_data(0).expect("row 0");
        let mut row = DiskTypeOverrideText {
            path: item.path.to_string(),
            is_hdd: index_is_hdd(item.media_idx),
        };
        assert_eq!(row.path, r"D:\Downloads");
        assert!(row.is_hdd);

        row.is_hdd = index_is_hdd(0);
        assert!(!row.is_hdd);
        // The refresh keeps the typed text and only recomputes the hint.
        let refreshed = disk_override_rows_to_slint(std::slice::from_ref(&row), Language::EnUs);
        assert_eq!(refreshed[0].path.as_str(), r"D:\Downloads");
        assert_eq!(refreshed[0].media_idx, 0);
    }
}
