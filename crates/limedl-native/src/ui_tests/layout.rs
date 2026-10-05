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

use super::{DownloadState, Language, TestUi, http_task, http_wire, with_language, with_ui};

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

/// The declared `min-width`/`min-height` from `appwindow.slint`. The toolbar used
/// to clip its last button at 1000px, which is why the minimum is 1100px now.
const MIN_SIZE: (f32, f32) = (1100.0, 660.0);

/// The `preferred-width`/`preferred-height` from `appwindow.slint`.
const PREFERRED_SIZE: (f32, f32) = (1280.0, 800.0);

#[test]
fn the_chrome_fits_the_window_at_the_default_and_the_minimum_size() {
    with_ui(|ui| {
        ui.set_window_size(PREFERRED_SIZE.0, PREFERRED_SIZE.1);
        for id in CHROME {
            ui.assert_inside_window(id);
        }
        ui.assert_min_size(CHROME, 16.0);

        // Every control, including the last toolbar button: the minimum width was
        // raised to 1100px precisely because the toolbar clipped it at 1000px.
        ui.set_window_size(MIN_SIZE.0, MIN_SIZE.1);
        for id in CHROME {
            ui.assert_inside_window(id);
        }

        // Device pixel ratios: the assertions are logical, so this pins that the
        // layout never mixes the two units up. Both 100% and 200% are covered
        // explicitly because the host is not the same in CI as on a laptop.
        for scale in [1.0, 2.0] {
            ui.set_scale_factor(scale);
            ui.set_window_size(MIN_SIZE.0, MIN_SIZE.1);
            assert_eq!(
                ui.window_logical_size(),
                (MIN_SIZE.0, MIN_SIZE.1),
                "scale {scale}"
            );
            for id in CHROME {
                ui.assert_inside_window(id);
            }
        }
        ui.set_scale_factor(1.0);
    });
}

/// The same invariants under English, which is where the labels actually grow.
///
/// Every other test runs under the fixture's zh-CN, so the wider English catalogs
/// — the reason this file pins geometry at all ("the settings tab row already
/// overflowed once") — were never laid out. `select_bundled_translation` is
/// process-global, so the language is put back before returning for the sibling
/// tests that share the process.
#[test]
fn the_layout_survives_the_english_labels() {
    with_language(Language::EnUs, |ui| {
        ui.set_window_size(PREFERRED_SIZE.0, PREFERRED_SIZE.1);
        for id in CHROME {
            ui.assert_inside_window(id);
        }
        ui.assert_min_size(CHROME, 16.0);

        // The reason the minimum width is 1100px: the English toolbar labels are the
        // widest ones, and at the minimum size they still have to fit.
        ui.set_window_size(MIN_SIZE.0, MIN_SIZE.1);
        for id in CHROME {
            ui.assert_inside_window(id);
        }
        ui.set_window_size(PREFERRED_SIZE.0, PREFERRED_SIZE.1);

        ui.click("MainWindow::ta_set");
        assert_dialog_fits(ui, "SettingsDialog::modal", "SettingsDialog::save_btn");
        ui.click("SettingsDialog::close_btn");

        ui.click("MainWindow::ta_lab");
        assert_dialog_fits(ui, "LabsDialog::modal", "LabsDialog::save_labs_btn");
        ui.click("LabsDialog::close_btn");

        ui.click("MainWindow::ta_new_task");
        assert_dialog_fits(ui, "NewTaskDialog::modal", "NewTaskDialog::submit_btn");
        ui.click("NewTaskDialog::close_btn");

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

        // The table's columns are label-driven too, and a longer English header
        // must not push a column past the window edge.
        ui.set_window_size(1600.0, 900.0);
        ui.click("MainWindow::btn_view_mode");
        for id in [
            "TaskTable::hdr_file",
            "TaskTable::hdr_size",
            "TaskTable::hdr_status",
            "TaskTable::hdr_progress",
            "TaskTable::hdr_speed",
            "TaskTable::hdr_eta",
        ] {
            ui.assert_inside_window(id);
        }

        crate::i18n::apply_translation(Language::ZhCn);
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
        ui.set_window_size(MIN_SIZE.0, MIN_SIZE.1);

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

        // With the torrent file list expanded the dialog needs more room than its
        // fixed modal height; the list is what gives way (`preferred-height` /
        // `min-height` in `new_task_dialog.slint`), so the footer stays reachable.
        ui.click("MainWindow::ta_new_task");
        ui.window.set_new_task_preview_state("ready".into());
        assert_dialog_fits(ui, "NewTaskDialog::modal", "NewTaskDialog::submit_btn");
        ui.click("NewTaskDialog::close_btn");

        // The wizard's three language cards share their row (`horizontal-stretch`),
        // so all three — including the `en-US` one that switches the UI to English
        // — are inside the modal and clickable.
        ui.window.set_setup_start_step(1);
        ui.window.set_show_setup_wizard(true);
        let english = ui
            .find_all("LanguageCard::ta")
            .pop()
            .expect("three language cards");
        let card_left = english.absolute_position().x;
        let card_width = english.size().width;
        let (modal_x, _, modal_width, _) = ui.bounds("SetupWizard::modal");
        assert!(
            card_width > 0.0 && card_left + card_width <= modal_x + modal_width + 0.5,
            "the en-US card must sit inside the wizard modal (left {card_left}, width {card_width}, \
             modal {}..{})",
            modal_x,
            modal_x + modal_width
        );
        ui.window.set_show_setup_wizard(false);
        ui.window.set_setup_start_step(0);
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
        ui.set_window_size(MIN_SIZE.0, MIN_SIZE.1);
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

#[test]
fn view_mode_button_shows_text_only_on_wide_viewports() {
    with_ui(|ui| {
        // At default size (1280px) and minimum size (1100px), it is icon-only (<= 32px width).
        ui.set_window_size(MIN_SIZE.0, MIN_SIZE.1);
        let (_, _, min_w, _) = ui.bounds("MainWindow::btn_view_mode");
        assert!(min_w <= 32.0, "icon-only button at min width, got {min_w}");

        ui.set_window_size(PREFERRED_SIZE.0, PREFERRED_SIZE.1);
        let (_, _, pref_w, _) = ui.bounds("MainWindow::btn_view_mode");
        assert!(pref_w <= 32.0, "icon-only button at preferred width, got {pref_w}");

        // At wide viewport (>= 1360px), it expands to show the text label.
        ui.set_window_size(1400.0, 800.0);
        let (_, _, wide_w, _) = ui.bounds("MainWindow::btn_view_mode");
        assert!(wide_w >= 60.0, "expanded button with text at 1400px, got {wide_w}");
        ui.assert_inside_window("MainWindow::btn_view_mode");
        ui.assert_inside_window("MainWindow::ta_new_task");
    });
}

#[test]
fn drop_hint_pill_icon_and_text_are_vertically_centered() {
    with_ui(|ui| {
        let (_, pill_y, _, pill_h) = ui.bounds("MainWindow::drop_hint_pill");
        let (_, icon_y, _, icon_h) = ui.bounds("MainWindow::drop_hint_icon");
        let (_, text_y, _, text_h) = ui.bounds("MainWindow::drop_hint_text");

        let pill_center_y = pill_y + pill_h / 2.0;
        let icon_center_y = icon_y + icon_h / 2.0;
        let text_center_y = text_y + text_h / 2.0;

        assert!(
            (icon_center_y - pill_center_y).abs() <= 1.0,
            "icon center ({icon_center_y}) should align with pill center ({pill_center_y})"
        );
        assert!(
            (text_center_y - pill_center_y).abs() <= 1.0,
            "text center ({text_center_y}) should align with pill center ({pill_center_y})"
        );
        assert!(
            (icon_center_y - text_center_y).abs() <= 1.0,
            "icon center ({icon_center_y}) should align with text center ({text_center_y})"
        );
    });
}

