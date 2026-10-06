//! Task list: which shell the list area shows, how the store's filters reach the
//! window, and how the two views route a click — including the destructive
//! actions, which is where a wiring mistake costs data.
//!
//! The list is a `for` loop, so a single id matches every row:
//! `click_nth`/`right_click_nth` address them in model order. Callbacks the
//! view's own widgets emit (category buttons, the search box, the columns) are
//! driven both ways — by clicking the real control where it is unique, and by
//! `invoke_*` where the value under test is the Rust-side bookkeeping.

use limedl_core::types::DownloadState;

use slint::Model;

use super::{AppSettings, TestUi, http_task, http_wire, with_settings, with_ui};

#[test]
fn the_list_area_swaps_between_the_empty_state_and_the_populated_shell() {
    with_ui(|ui| {
        assert_eq!(ui.visible_rows(), 0);
        assert!(
            ui.has("MainWindow::empty_menu_ta"),
            "an empty list shows the empty state (background menu target)"
        );
        assert!(
            !ui.has("MainWindow::list_menu_ta"),
            "the populated list's background menu target must not exist while empty"
        );
        assert!(
            ui.has("MainWindow::ta_create_task"),
            "the empty state offers its own create button"
        );

        ui.seed(vec![http_task(
            1,
            "one.bin",
            DownloadState::Downloading,
            512,
            1024,
        )]);

        assert_eq!(ui.visible_rows(), 1);
        assert!(
            !ui.has("MainWindow::empty_menu_ta"),
            "the empty state must disappear as soon as a task exists"
        );
        assert!(
            ui.has("MainWindow::list_menu_ta"),
            "the populated list must expose its background menu target"
        );
        assert!(
            !ui.has("MainWindow::ta_create_task"),
            "the in-list create button belongs to the empty state only"
        );
        assert!(
            ui.has("MainWindow::ta_new_task"),
            "the toolbar create button stays available in every state"
        );
    });
}

#[test]
fn category_counts_and_the_visible_rows_follow_the_task_states() {
    with_ui(|ui| {
        ui.seed(vec![
            http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
            http_task(2, "bravo.bin", DownloadState::Downloading, 20, 100),
            http_task(3, "charlie.bin", DownloadState::Paused, 30, 100),
            http_task(4, "delta.bin", DownloadState::Completed, 100, 100),
        ]);

        assert_eq!(ui.window.get_count_all().as_str(), "4");
        assert_eq!(ui.window.get_count_downloading().as_str(), "2");
        assert_eq!(ui.window.get_count_paused().as_str(), "1");
        assert_eq!(ui.window.get_count_completed().as_str(), "1");
        assert_eq!(ui.window.get_count_failed().as_str(), "0");

        // Clicking a real sidebar entry both selects the category and refilters
        // the rows; the pair is the point — a `select_category` that forgets to
        // refresh the list would still pass a property-only assertion.
        ui.click("MainWindow::cat_downloading");
        assert_eq!(ui.window.get_active_category(), 1);
        assert_eq!(
            ui.visible_rows(),
            2,
            "only the downloading rows survive the filter"
        );

        ui.click("MainWindow::cat_completed");
        assert_eq!(ui.window.get_active_category(), 3);
        assert_eq!(ui.visible_rows(), 1);

        ui.click("MainWindow::cat_all");
        assert_eq!(ui.window.get_active_category(), 0);
        assert_eq!(ui.visible_rows(), 4, "All Tasks must restore every row");
    });
}

#[test]
fn the_search_query_filters_rows_and_the_empty_state_returns_when_nothing_matches() {
    with_ui(|ui| {
        ui.seed(vec![
            http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
            http_task(2, "beta.bin", DownloadState::Downloading, 10, 100),
            http_task(3, "gamma.bin", DownloadState::Completed, 100, 100),
        ]);
        assert_eq!(ui.visible_rows(), 3);

        // Typed into the real field: the widget owns `text` and the callback
        // consumes it, and the empty state's create button reads the property,
        // so only the full path proves the wiring.
        ui.search("beta");
        assert_eq!(ui.visible_rows(), 1, "the query must narrow the rows");

        ui.search("nothing-matches-this");
        assert_eq!(ui.visible_rows(), 0);
        assert!(
            ui.has("MainWindow::empty_menu_ta"),
            "a filter that matches nothing falls back to the empty state"
        );
        assert!(
            !ui.has("MainWindow::ta_create_task"),
            "the create button is hidden while a query is active, so the empty \
             state does not offer to create from a filtered view"
        );

        ui.search("");
        assert_eq!(
            ui.visible_rows(),
            3,
            "clearing the query restores every row"
        );
    });
}

#[test]
fn the_sort_order_toggle_reaches_both_the_store_and_the_window() {
    with_ui(|ui| {
        let initial = ui.window.get_sort_asc();

        ui.click("MainWindow::btn_sort_order");
        assert_eq!(
            ui.window.get_sort_asc(),
            !initial,
            "the toolbar button must flip the sort order shown by the header"
        );
        assert_eq!(
            ui.ctx.store.lock().sort_asc(),
            ui.window.get_sort_asc(),
            "the store and the window must not disagree about the sort order"
        );

        ui.click("MainWindow::btn_sort_order");
        assert_eq!(ui.window.get_sort_asc(), initial);
        // Persisting the change happens on a spawned task which this layer never
        // polls — see the module docs in `ui_tests/mod.rs`.
    });
}

#[test]
fn a_card_body_click_does_not_select_but_its_checkbox_does() {
    with_ui(|ui| {
        ui.seed(vec![
            http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
            http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
        ]);

        // The card body only handles the shift-click range gesture: a plain
        // click on a card is *not* a selection. Batch actions hang off the
        // selection, so a card that selected on any click would make an
        // accidental click dangerous.
        ui.click_nth("TaskCard::ta", 0);
        assert_eq!(
            ui.window.get_selected_count(),
            0,
            "a card body click must not select"
        );
        assert!(
            !ui.has("MainWindow::batch_bar"),
            "…and must not raise the batch bar"
        );

        ui.click_nth("TaskCard::card_check", 0);
        assert_eq!(
            ui.window.get_selected_count(),
            1,
            "the card checkbox selects"
        );
        assert!(ui.has("MainWindow::batch_bar"));

        ui.click_nth("TaskCard::card_check", 0);
        assert_eq!(ui.window.get_selected_count(), 0);
    });
}

#[test]
fn a_table_row_click_toggles_the_selection() {
    with_ui(|ui| {
        ui.seed(
            (1..=3)
                .map(|n| http_task(n, "alpha.bin", DownloadState::Downloading, 10, 100))
                .collect(),
        );
        ui.click("MainWindow::btn_view_mode");

        ui.click_nth("TaskTable::ta_row", 1);
        assert_eq!(ui.window.get_selected_count(), 1);
        assert_eq!(
            ui.ctx.store.lock().selected_ids(),
            vec![http_wire(2)],
            "the clicked row is the selected one, not the first"
        );

        ui.click_nth("TaskTable::ta_row", 1);
        assert_eq!(
            ui.window.get_selected_count(),
            0,
            "clicking the row again deselects"
        );
    });
}

#[test]
fn the_table_header_checkbox_reflects_the_selection_state() {
    with_ui(|ui| {
        ui.seed(
            (1..=3)
                .map(|n| http_task(n, "alpha.bin", DownloadState::Downloading, 10, 100))
                .collect(),
        );
        ui.click("MainWindow::btn_view_mode");

        assert_eq!(
            ui.find("TaskTable::hdr_select_all").accessible_checked(),
            Some(false),
            "nothing selected yet"
        );

        ui.click_nth("TaskTable::ta_row", 0);
        assert_eq!(
            ui.find("TaskTable::hdr_select_all").accessible_checked(),
            Some(false),
            "a partial selection must not look like select-all"
        );

        ui.click("TaskTable::hdr_select_all");
        assert_eq!(
            ui.window.get_selected_count(),
            3,
            "partial selection → select all"
        );
        assert_eq!(
            ui.find("TaskTable::hdr_select_all").accessible_checked(),
            Some(true)
        );

        ui.click("TaskTable::hdr_select_all");
        assert_eq!(ui.window.get_selected_count(), 0, "select-all → clear");
    });
}

#[test]
fn column_visibility_reaches_the_table_headers() {
    let mut settings = AppSettings::default();
    settings.appearance.visible_columns = vec!["size".into()];

    with_settings(settings, |ui| {
        ui.seed(vec![http_task(
            1,
            "alpha.bin",
            DownloadState::Downloading,
            10,
            100,
        )]);
        ui.click("MainWindow::btn_view_mode");

        // The file name column is documented as always visible.
        assert!(ui.has("TaskTable::hdr_file"));
        assert!(ui.has("TaskTable::hdr_size"));
        assert!(!ui.has("TaskTable::hdr_status"));
        assert!(!ui.has("TaskTable::hdr_progress"));
        assert!(!ui.has("TaskTable::hdr_speed"));
        assert!(!ui.has("TaskTable::hdr_eta"));
        assert!(!ui.has("TaskTable::hdr_priority_col"));
    });
}

#[test]
fn sorting_from_a_column_header_applies_immediately_and_toggles_on_repeat() {
    with_ui(|ui| {
        ui.seed(
            (1..=3)
                .map(|n| http_task(n, "alpha.bin", DownloadState::Downloading, 10, 100))
                .collect(),
        );
        ui.click("MainWindow::btn_view_mode");

        // A new column sorts ascending; the same column again flips the order.
        ui.click("TaskTable::hdr_size");
        assert_eq!(
            ui.window.get_sort_field(),
            1,
            "the size column sorts by size"
        );
        assert!(ui.window.get_sort_asc());

        ui.click("TaskTable::hdr_size");
        assert!(!ui.window.get_sort_asc());

        ui.click("TaskTable::hdr_file");
        assert_eq!(ui.window.get_sort_field(), 4);
        assert!(
            ui.window.get_sort_asc(),
            "switching columns restarts ascending"
        );
        assert_eq!(
            ui.ctx.store.lock().sort_field(),
            ui.window.get_sort_field(),
            "the store follows the header click"
        );
    });
}

#[test]
fn right_clicking_the_empty_list_opens_the_background_menu() {
    with_ui(|ui| {
        assert!(
            ui.has("MainWindow::empty_menu_ta"),
            "the empty state covers the list area"
        );
        ui.right_click("MainWindow::empty_menu_ta");
        assert!(
            ui.window.get_background_menu_visible(),
            "right-click opens the menu"
        );

        ui.click("BackgroundMenu::bg_new_task");
        assert!(
            !ui.window.get_background_menu_visible(),
            "picking an item closes it"
        );
        assert!(ui.window.get_show_new_task_dialog());
    });
}

/// The selection as the store holds it, sorted (a `HashSet` has no order).
fn selection(ui: &TestUi) -> Vec<String> {
    let mut ids = ui.ctx.store.lock().selected_ids();
    ids.sort();
    ids
}

/// Shift-click selects the range between the last clicked row and the clicked
/// one. The gesture is `.slint`-only (`pointer-event` → `shift_click` →
/// `range_select`); the store sees just the resulting id, which is why the
/// `select_range` unit test cannot cover it.
#[test]
fn a_shift_click_selects_the_range_between_the_anchor_and_the_row() {
    with_ui(|ui| {
        ui.seed(vec![
            http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
            http_task(2, "bravo.bin", DownloadState::Downloading, 10, 100),
            http_task(3, "alpha.bin", DownloadState::Downloading, 10, 100),
            http_task(4, "bravo.bin", DownloadState::Downloading, 10, 100),
        ]);
        ui.click("MainWindow::btn_view_mode");

        // A plain click is the anchor; the shift-click extends from it.
        ui.click_nth("TaskTable::ta_row", 0);
        ui.shift_click_nth("TaskTable::ta_row", 2);
        assert_eq!(ui.window.get_selected_count(), 3, "rows 0..=2");
        assert_eq!(
            selection(ui),
            vec![http_wire(1), http_wire(2), http_wire(3)],
            "the range has to cover the rows between the anchor and the click"
        );
        for row in 0..3 {
            assert!(
                ui.window.get_tasks().row_data(row).unwrap().selected,
                "row {row} must be painted as selected, not just counted"
            );
        }

        // The range is measured in the *visible* order: with a filter on, the
        // rows in between are the filtered ones. A range taken from the raw list
        // would drag `bravo.bin` in here.
        ui.window.invoke_clear_selection();
        ui.search("alpha");
        assert_eq!(ui.visible_rows(), 2, "only the two alpha rows are shown");
        ui.click_nth("TaskTable::ta_row", 0);
        ui.shift_click_nth("TaskTable::ta_row", 1);
        assert_eq!(
            selection(ui),
            vec![http_wire(1), http_wire(3)],
            "the range follows the filtered list, not the unfiltered one"
        );
    });
}

#[test]
fn tasks_model_reconciles_in_place_preserving_instance_and_updating_data() {
    with_ui(|ui| {
        ui.seed(vec![
            http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
            http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
        ]);

        let initial_model = ui.window.get_tasks();
        assert_eq!(initial_model.row_count(), 2);

        // Mutate progress of task 1 in the store and refresh
        {
            let mut store = ui.ctx.store.lock();
            let mut summary = store.get_summary(&http_wire(1)).unwrap();
            summary.downloaded_bytes = 50;
            summary.speed_bytes_per_second = Some(1024.0 * 1024.0);
            store.insert_or_update(summary);
            crate::ui_sync::refresh_ui(&ui.window, &store);
        }

        let after_progress_model = ui.window.get_tasks();
        let initial_vec = initial_model
            .as_any()
            .downcast_ref::<slint::VecModel<crate::TaskItem>>()
            .unwrap();
        let after_vec = after_progress_model
            .as_any()
            .downcast_ref::<slint::VecModel<crate::TaskItem>>()
            .unwrap();
        assert!(
            std::ptr::eq(initial_vec, after_vec),
            "reconciliation must preserve the in-place VecModel instance pointer"
        );
        let row0 = after_progress_model.row_data(0).unwrap();
        assert_eq!(row0.id, http_wire(1));
        assert_eq!(row0.downloaded_text, "50 B");

        // Remove task 2 and ensure model shrinks in place
        {
            let mut store = ui.ctx.store.lock();
            store.remove(&http_wire(2));
            crate::ui_sync::refresh_ui(&ui.window, &store);
        }

        assert_eq!(ui.window.get_tasks().row_count(), 1);
        let after_remove_model = ui.window.get_tasks();
        let after_remove_vec = after_remove_model
            .as_any()
            .downcast_ref::<slint::VecModel<crate::TaskItem>>()
            .unwrap();
        assert!(std::ptr::eq(initial_vec, after_remove_vec));
    });
}
