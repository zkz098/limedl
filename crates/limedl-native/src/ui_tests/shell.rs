//! Window shell: toolbar controls, dialog visibility and the view-mode toggle.
//!
//! These are the tests that catch the failure modes nothing else can see — a
//! toolbar button wired to the wrong callback, a dialog whose `is_open` property
//! nobody sets, a close button that calls the wrong handler.

use super::*;

#[test]
fn settings_dialog_opens_from_the_toolbar_and_closes_from_its_header() {
    with_ui(|ui| {
        assert!(!ui.window.get_show_settings());
        // Closed dialogs are `visible: is_open || opacity > 0.01`, and element
        // queries prune invisible subtrees, so absence here means "not on screen".
        assert!(!ui.has("SettingsDialog::close_btn"));

        ui.click("MainWindow::ta_set");
        assert!(
            ui.window.get_show_settings(),
            "gear button must open settings"
        );
        assert!(
            ui.has("SettingsDialog::close_btn"),
            "settings dialog must be on screen after clicking the gear button"
        );

        ui.click("SettingsDialog::close_btn");
        assert!(
            !ui.window.get_show_settings(),
            "close button must shut settings"
        );
        assert!(!ui.has("SettingsDialog::close_btn"));
    });
}

#[test]
fn labs_dialog_opens_from_the_toolbar_and_closes_from_its_header() {
    with_ui(|ui| {
        assert!(!ui.window.get_show_labs());

        ui.click("MainWindow::ta_lab");
        assert!(
            ui.window.get_show_labs(),
            "labs button must open the labs dialog"
        );
        assert!(ui.has("LabsDialog::close_btn"));

        ui.click("LabsDialog::close_btn");
        assert!(
            !ui.window.get_show_labs(),
            "close button must shut the labs dialog"
        );
    });
}

#[test]
fn new_task_dialog_reopens_without_the_previous_batch_state() {
    with_ui(|ui| {
        ui.click("MainWindow::ta_new_task");
        assert!(ui.window.get_show_new_task_dialog());

        // Pretend the previous session left the dialog in batch mode with text
        // typed into it: reopening must reset both, otherwise the next URL the
        // user enters is parsed as a batch list.
        ui.window.set_new_task_batch_mode(true);
        ui.window
            .set_new_task_batch_text("https://example.invalid/leftover".into());

        ui.click("NewTaskDialog::close_btn");
        assert!(!ui.window.get_show_new_task_dialog());

        ui.click("MainWindow::ta_new_task");
        assert!(ui.window.get_show_new_task_dialog());
        assert!(
            !ui.window.get_new_task_batch_mode(),
            "reopen must leave batch mode"
        );
        assert_eq!(
            ui.window.get_new_task_batch_text().as_str(),
            "",
            "reopen must clear the batch text"
        );
    });
}

#[test]
fn view_mode_toggle_switches_between_cards_and_table() {
    with_ui(|ui| {
        assert_eq!(ui.window.get_view_mode(), 0, "cards is the default view");

        ui.click("MainWindow::btn_view_mode");
        assert_eq!(
            ui.window.get_view_mode(),
            1,
            "first click switches to the table"
        );

        ui.click("MainWindow::btn_view_mode");
        assert_eq!(
            ui.window.get_view_mode(),
            0,
            "second click switches back to cards"
        );
    });
}

#[test]
fn startup_applies_the_persisted_view_preferences() {
    let mut settings = AppSettings::default();
    settings.appearance.compact_view = true;
    settings.appearance.visible_columns = vec!["size".into(), "progress".into()];
    settings.download.default_download_dir = "D:\\from-settings".into();

    with_settings(settings, |ui| {
        assert!(
            ui.window.get_compact_view(),
            "compact view flag must reach the UI"
        );
        assert_eq!(
            ui.window.get_view_mode(),
            0,
            "cards stays the default view mode"
        );
        // `file` is always shown regardless of the persisted list.
        assert!(
            ui.window.get_column_file(),
            "the file column is always visible"
        );
        assert!(ui.window.get_column_size());
        assert!(ui.window.get_column_progress());
        assert!(
            !ui.window.get_column_status(),
            "status is not in visible_columns"
        );
        assert!(!ui.window.get_column_eta(), "eta is not in visible_columns");
        assert_eq!(
            ui.window.get_default_download_dir().as_str(),
            "D:\\from-settings",
            "a non-empty configured directory must win over the OS default"
        );
    });
}

#[test]
fn an_unset_download_directory_still_leaves_the_dialog_somewhere_to_put_files() {
    let mut settings = AppSettings::default();
    settings.download.default_download_dir = String::new();

    // Which fallback wins (OS download folder vs. the data directory) depends on
    // the host; the precedence itself is pinned in `ui_boot`'s unit tests. What
    // this asserts is the wiring: whatever was resolved reaches the window, and
    // the new-task dialog starts out pointing at it.
    with_settings(settings, |ui| {
        assert!(
            !ui.window.get_default_download_dir().is_empty(),
            "the new-task dialog must never open without a destination"
        );
        assert_eq!(
            ui.window.get_new_task_dir(),
            ui.window.get_default_download_dir(),
            "the new-task directory starts out at the default download directory"
        );
    });
}
