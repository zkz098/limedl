//! New-task dialog: the state that must be reset between visits, the batch
//! link counter, and the torrent file pre-selection.

use limedl_core::types::TorrentFileEntry;
use slint::Model;

use super::{Language, TestUi, with_ui};

fn torrent_rows(ui: &TestUi) -> Vec<crate::NewTaskTorrentFileItem> {
    let model = ui.window.get_new_task_torrent_files();
    (0..model.row_count())
        .filter_map(|i| model.row_data(i))
        .collect()
}

#[test]
fn reopening_the_dialog_resets_batch_mode_and_the_checksum_probe() {
    with_ui(|ui| {
        ui.click("MainWindow::ta_new_task");
        ui.window.set_new_task_batch_mode(true);
        ui.window
            .set_new_task_batch_text("https://example.invalid/leftover".into());
        ui.window.set_new_task_checksum("deadbeef".into());
        ui.window.set_new_task_probe_state("ready".into());
        ui.window
            .set_new_task_probe_status_text("sha256 detected".into());
        ui.window.set_new_task_probe_hash("deadbeef".into());
        ui.window.set_new_task_preview_state("ready".into());

        ui.click("NewTaskDialog::close_btn");
        ui.click("MainWindow::ta_new_task");

        assert!(
            !ui.window.get_new_task_batch_mode(),
            "batch mode must not survive a reopen"
        );
        assert!(ui.window.get_new_task_batch_text().is_empty());
        assert!(ui.window.get_new_task_checksum().is_empty());
        assert_eq!(ui.window.get_new_task_probe_state().as_str(), "idle");
        assert!(ui.window.get_new_task_probe_hash().is_empty());
        assert!(ui.window.get_new_task_probe_status_text().is_empty());
        assert_ne!(
            ui.window.get_new_task_preview_state().as_str(),
            "ready",
            "a torrent preview from the previous visit must be gone"
        );
    });
}

#[test]
fn editing_the_url_drops_a_checksum_that_belonged_to_the_old_one() {
    with_ui(|ui| {
        ui.click("MainWindow::ta_new_task");
        ui.window.set_new_task_probe_state("ready".into());
        ui.window.set_new_task_probe_hash("deadbeef".into());
        ui.window.set_new_task_checksum("deadbeef".into());
        ui.window
            .set_new_task_probe_status_text("sha256 detected".into());

        // The dialog calls this on every URL edit: a hash is only valid for the
        // URL it was probed against.
        ui.window.invoke_reset_new_task_probe();

        assert_eq!(ui.window.get_new_task_probe_state().as_str(), "idle");
        assert!(ui.window.get_new_task_probe_hash().is_empty());
        assert!(ui.window.get_new_task_checksum().is_empty());
        assert!(ui.window.get_new_task_probe_status_text().is_empty());

        // Idempotent: resetting an idle dialog is a no-op, not a model churn.
        ui.window.invoke_reset_new_task_probe();
        assert_eq!(ui.window.get_new_task_probe_state().as_str(), "idle");
    });
}

#[test]
fn the_batch_counter_tracks_the_parser() {
    with_ui(|ui| {
        ui.click("MainWindow::ta_new_task");
        // The link counter reports what the *parser* counts, so the parser is the
        // oracle here (its own rules are covered by `url_utils`'s tests); what
        // this pins is that the label follows it, including its leniency about
        // bare text.
        for text in [
            "https://example.invalid/a.zip\nhttps://example.invalid/b.zip\n\nnot a url",
            "https://example.invalid/a.zip",
        ] {
            let expected = crate::url_utils::parse_batch_entries(text).len();
            ui.window.invoke_new_task_batch_text_changed(text.into());
            let label = ui.window.get_new_task_batch_count_text();
            assert!(
                label.contains(&expected.to_string()),
                "the counter must say {expected} for {text:?}, got {label:?}"
            );
        }

        ui.window.invoke_new_task_batch_text_changed("".into());
        assert_eq!(
            ui.window.get_new_task_batch_count_text().as_str(),
            crate::i18n::format_batch_count(0, Language::ZhCn),
            "an empty textarea gets the dedicated \"nothing to add\" message, not a 0 link count"
        );
    });
}

#[test]
fn torrent_rows_come_from_the_pending_entries_and_can_be_toggled_in_bulk() {
    with_ui(|ui| {
        ui.click("MainWindow::ta_new_task");
        // The picker seeds these two caches; a test seeds them directly, which
        // is the only way to reach the pre-selection list without a file dialog.
        *ui.ctx.new_task_torrent_entries.lock() = vec![
            TorrentFileEntry {
                index: 0,
                path: "movie/movie.mkv".into(),
                size: 4_000_000_000,
            },
            TorrentFileEntry {
                index: 1,
                path: "movie/subs.srt".into(),
                size: 40_000,
            },
        ];
        *ui.ctx.new_task_torrent_included.lock() = vec![true, true];

        ui.window.invoke_toggle_new_task_file(1);
        let rows = torrent_rows(ui);
        assert_eq!(rows.len(), 2, "toggling renders the whole list");
        assert!(rows[0].included);
        assert!(!rows[1].included);
        assert_eq!(
            ui.ctx.new_task_torrent_included.lock().clone(),
            vec![true, false],
            "the cache and the rendered rows must agree"
        );

        ui.window.invoke_set_all_new_task_files(false);
        assert_eq!(
            torrent_rows(ui).iter().filter(|row| row.included).count(),
            0
        );
        ui.window.invoke_set_all_new_task_files(true);
        assert_eq!(
            torrent_rows(ui).iter().filter(|row| row.included).count(),
            2
        );

        // With nothing picked there is nothing to toggle: the early return must
        // not turn into an index panic. It also does not clear what is already
        // rendered — only the picker rebuilds the list.
        *ui.ctx.new_task_torrent_entries.lock() = Vec::new();
        *ui.ctx.new_task_torrent_included.lock() = Vec::new();
        ui.window.invoke_set_all_new_task_files(true);
        ui.window.invoke_toggle_new_task_file(0);
        assert_eq!(
            torrent_rows(ui).len(),
            2,
            "an empty cache leaves the rendered rows alone (the picker rebuilds them)"
        );
    });
}

/// A pasted `aria2c` command line fills the visible fields while the raw
/// command stays in the URL box, and switching back to a plain URL drops the
/// parsed Cookie.
#[test]
fn an_aria2c_command_fills_the_visible_fields_and_a_plain_url_clears_them() {
    with_ui(|ui| {
        ui.click("MainWindow::ta_new_task");
        ui.window.set_new_task_url("".into());
        ui.window.set_new_task_filename("".into());
        ui.window.set_new_task_headers("".into());

        const COMMAND: &str =
            "aria2c \"https://example.invalid/a.ass\" --out \"a.ass\" --header \"Cookie: k=v\"";
        ui.click("NewTaskDialog::nt_url_input");
        ui.type_text(COMMAND);

        assert_eq!(
            ui.window.get_new_task_url().as_str(),
            COMMAND,
            "the raw command has to stay editable in the URL field"
        );
        assert_eq!(ui.window.get_new_task_filename().as_str(), "a.ass");
        assert_eq!(ui.window.get_new_task_headers().as_str(), "Cookie: k=v");
        assert!(
            !ui.window.get_new_task_aria2_hint().is_empty(),
            "the dialog must say that the command was recognized"
        );

        // Editing the target to something that is not an aria2 command must not
        // keep the Cookie that belonged to the old one.
        ui.window
            .invoke_new_task_url_changed("https://example.invalid/plain.zip".into());
        assert!(ui.window.get_new_task_headers().is_empty());
        assert!(ui.window.get_new_task_aria2_hint().is_empty());
    });
}

/// An ultra-long URL or aria2c command line must not cause the dialog or
/// its inputs to expand horizontally or produce a horizontal scrollbar.
/// Buttons like "Detect Checksum" and "Browse..." must remain visible and
/// properly positioned inside the modal.
#[test]
fn test_long_url_does_not_break_dialog_layout() {
    with_ui(|ui| {
        ui.click("MainWindow::ta-new-task");
        let long_url = format!("https://example.com/{}", "a".repeat(500));
        ui.window.set_new_task_url(long_url.into());

        ui.assert_inside_window("NewTaskDialog::modal");
        ui.assert_inside_window("NewTaskDialog::nt-url-input");
        ui.assert_inside_window("NewTaskDialog::nt-probe-btn");
        ui.assert_inside_window("NewTaskDialog::submit-btn");

        let (modal_x, _, modal_w, _) = ui.bounds("NewTaskDialog::modal");
        let (url_x, _, url_w, _) = ui.bounds("NewTaskDialog::nt-url-input");
        let (probe_x, _, probe_w, _) = ui.bounds("NewTaskDialog::nt-probe-btn");

        assert!(
            url_x >= modal_x && url_x + url_w <= modal_x + modal_w,
            "url input must stay inside modal: url_x={url_x}, url_w={url_w}, modal_x={modal_x}, modal_w={modal_w}"
        );
        assert!(
            probe_x >= modal_x && probe_x + probe_w <= modal_x + modal_w,
            "probe button must stay inside modal: probe_x={probe_x}, probe_w={probe_w}, modal_x={modal_x}, modal_w={modal_w}"
        );
        assert!(
            !ui.has("ScrollView::horizontal-bar"),
            "horizontal scrollbar must not appear on long url input"
        );

        // Also test a long aria2c command line with header options
        let long_cmd = format!(
            "aria2c \"https://example.com/{}\" --header \"Cookie: {}\"",
            "b".repeat(400),
            "c".repeat(400)
        );
        ui.window.invoke_new_task_url_changed(long_cmd.clone().into());
        ui.window.set_new_task_url(long_cmd.into());

        let (url2_x, _, url2_w, _) = ui.bounds("NewTaskDialog::nt-url-input");
        let (probe2_x, _, probe2_w, _) = ui.bounds("NewTaskDialog::nt-probe-btn");
        assert!(
            url2_x >= modal_x && url2_x + url2_w <= modal_x + modal_w,
            "long aria2c command must keep url input within modal"
        );
        assert!(
            probe2_x >= modal_x && probe2_x + probe2_w <= modal_x + modal_w,
            "probe button must stay inside modal with long aria2c command"
        );
        assert!(
            !ui.has("ScrollView::horizontal-bar"),
            "horizontal scrollbar must not appear on long aria2c command"
        );
    });
}


