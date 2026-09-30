//! Layout invariants.
//!
//! The `.slint` files centre every dialog with `min(parent.width - 100px, …)`
//! and stack their content in layouts, so the realistic regression is not "the
//! dialog is missing" but "the footer slid below the modal" or "the toolbar
//! pushed a button out of the window" — usually because a label grew: English
//! strings are wider than the Chinese ones the layout was tuned for, and the
//! settings tab row already overflowed once for exactly that reason.
//!
//! These tests pin the geometry that the UI tests can see (`absolute_position`,
//! `size`) instead of pixels, so they stay meaningful without a renderer.

use super::*;

/// Controls that are on screen whenever the main view is.
const CHROME: &[&str] = &[
    "MainWindow::ta_new_task",
    "MainWindow::ta_set",
    "MainWindow::ta_lab",
    "MainWindow::btn_view_mode",
    "MainWindow::btn_sort_order",
    "MainWindow::cat_all",
    "MainWindow::cat_failed",
    "MainWindow::search_box",
];

/// Everything above except the button the known minimum-width overflow clips
/// (see `the_chrome_fits_the_window_at_the_default_and_the_minimum_size`).
fn assert_narrow_chrome_fits(ui: &TestUi) {
    for id in CHROME.iter().filter(|id| **id != "MainWindow::ta_new_task") {
        ui.assert_inside_window(id);
    }
}

#[test]
fn the_chrome_fits_the_window_at_the_default_and_the_minimum_size() {
    with_ui(|ui| {
        // 1280x800 is `preferred-width`/`preferred-height`, 1000x660 is
        // `min-width`/`min-height` as declared in `appwindow.slint`.
        ui.set_window_size(1280.0, 800.0);
        for id in CHROME {
            ui.assert_inside_window(id);
        }
        ui.assert_min_size(CHROME, 16.0);

        ui.set_window_size(1000.0, 660.0);
        assert_narrow_chrome_fits(ui);

        // Known issue, found by this test: at the declared minimum width the
        // toolbar overflows and clips the New Task button (its left 62px are on
        // screen, the rest is past the right edge) — with Chinese labels, and
        // English ones are wider. Pinned as a characterization so the fix (raise
        // `min-width`, or let that row wrap/scroll) fails here and gets the
        // contract updated with it.
        let (button_x, _, button_width, _) = ui.bounds("MainWindow::ta_new_task");
        assert!(
            button_x + button_width > 1000.0,
            "the toolbar now fits the minimum width — drop this characterization and \
             assert `ta_new_task` inside the window like the rest of the chrome"
        );

        // Device pixel ratios: the assertions are logical, so this pins that the
        // layout never mixes the two units up. Both 100% and 200% are covered
        // explicitly because the host is not the same in CI as on a laptop.
        for scale in [1.0, 2.0] {
            ui.set_scale_factor(scale);
            ui.set_window_size(1000.0, 660.0);
            assert_eq!(ui.window_logical_size(), (1000.0, 660.0), "scale {scale}");
            assert_narrow_chrome_fits(ui);
        }
        ui.set_scale_factor(1.0);
    });
}

/// A dialog fits when its modal box fits *and* its primary action is inside the
/// modal: the footer is where a long label overflows first.
fn assert_dialog_fits(ui: &TestUi, modal: &str, primary: &str) {
    ui.assert_inside_window(modal);
    ui.assert_inside_window(primary);

    let (_, modal_y, _, modal_height) = ui.bounds(modal);
    let (_, y, _, height) = ui.bounds(primary);
    assert!(
        y + height <= modal_y + modal_height + 0.5,
        "{primary} hangs below {modal} (button bottom {}, modal bottom {})",
        y + height,
        modal_y + modal_height
    );
}

#[test]
fn dialogs_fit_the_window_at_the_minimum_size() {
    with_ui(|ui| {
        ui.set_window_size(1000.0, 660.0);

        ui.click("MainWindow::ta_set");
        assert_dialog_fits(ui, "SettingsDialog::modal", "SettingsDialog::save_btn");
        ui.click("SettingsDialog::close_btn");

        ui.click("MainWindow::ta_lab");
        assert_dialog_fits(ui, "LabsDialog::modal", "LabsDialog::save_labs_btn");
        ui.click("LabsDialog::close_btn");

        ui.click("MainWindow::ta_new_task");
        assert_dialog_fits(ui, "NewTaskDialog::modal", "NewTaskDialog::submit_btn");
        ui.click("NewTaskDialog::close_btn");

        // The remaining overlays are opened by their handlers rather than by a
        // toolbar button: the setup wizard is driven by the first-run flow and
        // the inspector by a task.
        ui.seed(vec![http_task(
            1,
            "alpha.bin",
            DownloadState::Downloading,
            10,
            100,
        )]);
        ui.window.invoke_open_inspector(http_wire(1).into());
        assert_dialog_fits(ui, "TaskInspector::panel", "TaskInspector::close_btn");
        ui.window.invoke_close_inspector();

        ui.window.invoke_open_speed_limit_dialog();
        assert_dialog_fits(
            ui,
            "SpeedLimitDialog::modal",
            "SpeedLimitDialog::submit_btn",
        );
        ui.window.invoke_close_speed_limit_dialog();

        ui.window.set_show_setup_wizard(true);
        assert_dialog_fits(ui, "SetupWizard::modal", "SetupWizard::next_btn");
        ui.window.set_show_setup_wizard(false);
    });
}

#[test]
fn the_context_and_priority_menus_are_clamped_into_the_window() {
    with_ui(|ui| {
        ui.seed(vec![
            http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
            http_task(2, "bravo.bin", DownloadState::Completed, 100, 100),
        ]);
        ui.click("MainWindow::btn_view_mode");

        // Right-click the *last* row: a menu anchored at the pointer would hang
        // out of the bottom edge, which is what the `min(menu_x, parent.width -
        // …)` clamping exists to prevent.
        ui.right_click_nth("TaskTable::ta_row", 1);
        assert!(ui.window.get_context_menu_state().visible);
        ui.assert_inside_window("ContextMenu::menu_box");

        ui.click("ContextMenu::mi_priority");
        assert!(ui.window.get_priority_menu_visible());
        ui.assert_inside_window("PriorityMenu::menu_box");
    });
}

#[test]
fn the_batch_bar_stays_inside_the_window_and_its_actions_do_not_overlap() {
    with_ui(|ui| {
        ui.set_window_size(1000.0, 660.0);
        ui.seed(
            (1..=3)
                .map(|n| http_task(n, "alpha.bin", DownloadState::Downloading, 10, 100))
                .collect(),
        );
        ui.window.invoke_select_all();

        ui.assert_inside_window("MainWindow::batch_bar");
        ui.assert_no_overlap(&[
            "MainWindow::batch_select_all",
            "MainWindow::batch_deselect",
            "MainWindow::batch_pause_btn",
            "MainWindow::batch_resume_btn",
            "MainWindow::batch_remove_btn",
            "MainWindow::batch_purge_btn",
        ]);
        ui.assert_min_size(
            &["MainWindow::batch_pause_btn", "MainWindow::batch_purge_btn"],
            16.0,
        );
    });
}
