//! The inspector: the BT file list's "keep at least one" rule and the panel's
//! own state.
//!
//! `inspector_files` is normally filled by the status poller from the running BT
//! session; here the test sets it directly, which is the only way to reach the
//! toggle without a live torrent. The invariant is what matters: a task whose
//! files are all unchecked would download nothing.

use limedl_core::types::DownloadState;
use slint::{Model, ModelRc, VecModel};

use super::{Language, TestUi, http_task, http_wire, with_ui};

fn inspector_file(index: i32, path: &str, included: bool) -> crate::TorrentFileItem {
    crate::TorrentFileItem {
        index,
        path: path.into(),
        size_text: "1 MB".into(),
        downloaded_text: "0 B".into(),
        progress: 0.0,
        included,
    }
}

fn seed_inspector_files(ui: &TestUi, files: Vec<crate::TorrentFileItem>) {
    ui.window
        .set_inspector_files(ModelRc::new(VecModel::from(files)));
}

#[test]
fn unchecking_the_last_torrent_file_is_refused_with_a_warning() {
    with_ui(|ui| {
        ui.seed(vec![http_task(
            1,
            "alpha.torrent",
            DownloadState::Downloading,
            10,
            100,
        )]);
        ui.window.invoke_open_inspector(http_wire(1).into());
        seed_inspector_files(ui, vec![inspector_file(0, "movie.mkv", true)]);

        ui.window.invoke_toggle_inspector_file(0, true);

        ui.assert_toast(
            "warning",
            crate::i18n::format_toast_bt_files_keep_one(Language::ZhCn),
        );
        assert!(
            ui.window
                .get_inspector_files()
                .row_data(0)
                .unwrap()
                .included,
            "the last selected file must stay selected"
        );
    });
}

#[test]
fn toggling_a_torrent_file_is_left_to_the_poller_and_only_warns_on_the_last_one() {
    with_ui(|ui| {
        ui.seed(vec![http_task(
            1,
            "alpha.torrent",
            DownloadState::Downloading,
            10,
            100,
        )]);
        ui.window.invoke_open_inspector(http_wire(1).into());
        seed_inspector_files(
            ui,
            vec![
                inspector_file(0, "movie.mkv", true),
                inspector_file(1, "subs.srt", true),
            ],
        );

        // Dropping one of two files is not a warning…
        ui.window.invoke_toggle_inspector_file(1, true);
        assert!(
            ui.toasts().is_empty(),
            "dropping a non-last file is not a warning"
        );
        // …and the handler does not repaint: `inspector_files` belongs to the
        // status poller (only a live BT session knows the real selection), so the
        // list here legitimately still shows the old state. The engine call it
        // spawns is covered by the event-loop scenarios.
        assert!(
            ui.window
                .get_inspector_files()
                .row_data(1)
                .unwrap()
                .included
        );

        // An index that is not in the list must not be mistaken for "dropping the
        // last file" (nor panic): the poller can rebuild the list underneath.
        ui.window.invoke_toggle_inspector_file(9, true);
        assert!(ui.toasts().is_empty());
    });
}

#[test]
fn the_inspector_closes_from_its_own_button_and_clears_the_active_task() {
    with_ui(|ui| {
        ui.seed(vec![http_task(
            1,
            "alpha.bin",
            DownloadState::Downloading,
            10,
            100,
        )]);
        ui.window.invoke_open_inspector(http_wire(1).into());
        assert!(ui.window.get_show_inspector());
        assert_eq!(
            ui.ctx.active_inspector_id.lock().clone(),
            Some(http_wire(1))
        );

        ui.window.invoke_set_inspector_tab(2);
        assert_eq!(ui.window.get_inspector_tab(), 2);

        ui.click("TaskInspector::close_btn");
        assert!(
            !ui.window.get_show_inspector(),
            "the panel's own close button works"
        );
        assert!(
            ui.ctx.active_inspector_id.lock().is_none(),
            "closing the inspector must forget the task it was showing"
        );
    });
}
