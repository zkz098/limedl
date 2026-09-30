//! Task list: which shell the list area shows, and how the store's filters and
//! sort state reach the window.
//!
//! The list itself is a `for` loop, so a single element id matches every row.
//! Row-level interactions are therefore driven through the callbacks the rows
//! invoke (`invoke_select_category`, `invoke_search_changed`, …) rather than by
//! clicking a row: those callbacks *are* the wiring under test, and the real
//! control that triggers them is a `CategoryButton`/`SearchInput` covered by
//! `shell.rs`.

use limedl_core::types::DownloadState;

use super::*;

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

        ui.seed(vec![task(
            "http:one",
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
            task("http:a", "alpha.bin", DownloadState::Downloading, 10, 100),
            task("http:b", "bravo.bin", DownloadState::Downloading, 20, 100),
            task("http:c", "charlie.bin", DownloadState::Paused, 30, 100),
            task("http:d", "delta.bin", DownloadState::Completed, 100, 100),
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
            task("http:a", "alpha.bin", DownloadState::Downloading, 10, 100),
            task("http:b", "beta.bin", DownloadState::Downloading, 10, 100),
            task("http:c", "gamma.bin", DownloadState::Completed, 100, 100),
        ]);
        assert_eq!(ui.visible_rows(), 3);

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
