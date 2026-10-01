//! Window shell: toolbar controls, dialog visibility, the view-mode toggle and
//! the Escape unwinding order.
//!
//! These are the tests that catch the failure modes nothing else can see — a
//! toolbar button wired to the wrong callback, a dialog whose `is_open` property
//! nobody sets, a close button that calls the wrong handler.

use limedl_core::types::DownloadState;

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

/// Open every overlay that `handle_key_escape` knows about, bottom-up. The
/// toolbar buttons cannot do this one at a time — the first dialog covers them —
/// so the window callbacks stand in for the *openers*; the Escape path itself is
/// the thing under test.
fn open_every_overlay(ui: &TestUi) {
    ui.seed(vec![http_task(
        1,
        "alpha.bin",
        DownloadState::Downloading,
        10,
        100,
    )]);
    ui.window.invoke_open_new_task_dialog();
    ui.window.invoke_open_settings();
    ui.window.invoke_open_labs();
    ui.window.invoke_open_inspector(http_wire(1).into());
    ui.window.invoke_open_speed_limit_dialog();
    ui.window.set_priority_menu_visible(true);
    let mut menu = ui.window.get_context_menu_state();
    menu.visible = true;
    menu.task_id = http_wire(1).into();
    ui.window.set_context_menu_state(menu);
}

/// One overlay of the Escape chain: its name (for the assertion message) and a
/// probe for whether it is currently open.
type LayerProbe = (&'static str, fn(&TestUi) -> bool);

#[test]
fn escape_unwinds_the_overlays_one_layer_at_a_time() {
    with_ui(|ui| {
        open_every_overlay(ui);

        // Ordered exactly like the chain in `handle_key_escape`: the topmost
        // overlay closes and everything below it stays open. A chain that closes
        // two layers at once (or the wrong one) reads to the user as "Escape is
        // broken" — and there is no other test for it, because the chain lives in
        // `.slint`.
        let layers: [LayerProbe; 7] = [
            ("new task dialog", |ui| ui.window.get_show_new_task_dialog()),
            ("settings", |ui| ui.window.get_show_settings()),
            ("labs", |ui| ui.window.get_show_labs()),
            ("inspector", |ui| ui.window.get_show_inspector()),
            ("speed limit dialog", |ui| {
                ui.window.get_show_speed_limit_dialog()
            }),
            ("priority menu", |ui| ui.window.get_priority_menu_visible()),
            ("context menu", |ui| {
                ui.window.get_context_menu_state().visible
            }),
        ];

        for (index, (name, is_open)) in layers.iter().enumerate() {
            for (still_name, still_open) in &layers[index..] {
                assert!(
                    still_open(ui),
                    "{still_name} should be open before {name} is closed"
                );
            }
            ui.window.invoke_handle_key_escape();
            assert!(!is_open(ui), "Escape must have closed {name}");
            for (untouched_name, untouched) in &layers[index + 1..] {
                assert!(untouched(ui), "Escape also closed {untouched_name}");
            }
        }

        // Nothing left to close: Escape must be a no-op, not a window close.
        ui.window.invoke_handle_key_escape();
        assert!(
            ui.has("MainWindow::ta_set"),
            "the main window is still there"
        );
    });
}

#[test]
fn escape_leaves_the_first_run_wizard_alone() {
    with_ui(|ui| {
        ui.window.set_show_setup_wizard(true);
        ui.window.invoke_handle_key_escape();
        assert!(
            ui.window.get_show_setup_wizard(),
            "the wizard is modal until it is finished: Escape must not dismiss it"
        );
    });
}

#[test]
fn keyboard_shortcuts_reach_the_window_and_are_swallowed_by_an_open_dialog() {
    with_ui(|ui| {
        // Shortcuts live in the root `FocusScope`, which the window forwards
        // focus to. Clicking a text field first would move focus there and the
        // field would consume the key — so this test deliberately does not click
        // anything before pressing the shortcut.
        ui.press_keys(&[Key::Control.into(), 'n']);
        assert!(
            ui.window.get_show_new_task_dialog(),
            "Ctrl+N must open the new-task dialog"
        );
        ui.window.invoke_close_new_task_dialog();

        // The chain rejects keys while a dialog is open, before reaching the
        // shortcut branch: a global shortcut must not stack a second modal.
        ui.window.invoke_open_settings();
        ui.press_keys(&[Key::Control.into(), 'n']);
        assert!(
            !ui.window.get_show_new_task_dialog(),
            "an open dialog must swallow the new-task shortcut"
        );
        assert!(ui.window.get_show_settings());
    });
}

#[test]
fn ctrl_a_and_ctrl_f_reach_the_list_and_the_search_box() {
    with_ui(|ui| {
        ui.seed(vec![
            http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
            http_task(2, "beta.bin", DownloadState::Downloading, 10, 100),
        ]);

        // Neither shortcut may be preceded by a click: it would hand the focus to
        // a field and the key would never reach the root `FocusScope` (see the
        // module docs).
        ui.press_keys(&[Key::Control.into(), 'a']);
        assert_eq!(
            ui.window.get_selected_count(),
            2,
            "Ctrl+A must select the rows the current filter shows"
        );

        ui.window.invoke_clear_selection();
        assert_eq!(ui.window.get_selected_count(), 0);

        // Ctrl+F focuses the search box; what is typed next has to land there and
        // go through `search_changed`, or the list would silently ignore it.
        ui.press_keys(&[Key::Control.into(), 'f']);
        ui.type_text("beta");
        assert_eq!(ui.window.get_search_query().as_str(), "beta");
        assert_eq!(ui.visible_rows(), 1, "the focused search box filters");
    });
}
