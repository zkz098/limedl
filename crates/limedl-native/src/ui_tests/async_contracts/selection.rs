//! Multi-selection and the batch actions that hang off it.
//!
//! Every scenario here ends in a `Dispatcher` mutation, so the assertions read
//! the blast radius out of `TestUi::core` (which ids were named, and whether the
//! file was touched) instead of trusting the resulting row count.

use limedl_core::types::DownloadState;

use super::super::{Priority, http_task, http_wire, new_window};

pub(super) async fn batch_actions_reach_the_selection_only() {
    let ui = new_window();
    ui.seed(
        (1..=3)
            .map(|n| http_task(n, "alpha.bin", DownloadState::Downloading, 10, 100))
            .collect(),
    );
    ui.click("MainWindow::btn_view_mode");
    ui.click_nth("TaskTable::ta_row", 0);
    ui.click_nth("TaskTable::ta_row", 2);
    assert_eq!(
        ui.window.get_selected_count(),
        2,
        "first and third row are selected"
    );

    ui.click("MainWindow::batch_pause_btn");
    ui.pump_until("both selected tasks to be paused", || {
        ui.core.pauses().len() == 2
    })
    .await;

    // The selection is a set, so the engine sees the two ids in hash order:
    // compare sorted.
    let mut paused = ui.core.pauses();
    paused.sort();
    assert_eq!(paused, vec![http_wire(1), http_wire(3)]);
    assert!(ui.core.resumes().is_empty());
    assert!(
        ui.core.purges().is_empty(),
        "pausing must never touch the files"
    );
}

pub(super) async fn batch_remove_never_deletes_files() {
    let ui = new_window();
    ui.seed(
        (1..=2)
            .map(|n| http_task(n, "alpha.bin", DownloadState::Downloading, 10, 100))
            .collect(),
    );
    ui.click("MainWindow::btn_view_mode");
    ui.click("TaskTable::hdr_select_all");

    ui.click("MainWindow::batch_remove_btn");
    ui.pump_until("the record removal to reach the engine", || {
        ui.core.removes().len() == 2
    })
    .await;

    assert_eq!(ui.core.removes().len(), 2);
    assert!(
        ui.core.purges().is_empty(),
        "\"Remove Records\" must only drop the row, never the downloaded file"
    );
}

pub(super) async fn batch_delete_files_purges() {
    let ui = new_window();
    ui.seed(
        (1..=2)
            .map(|n| http_task(n, "alpha.bin", DownloadState::Downloading, 10, 100))
            .collect(),
    );
    ui.click("MainWindow::btn_view_mode");
    ui.click("TaskTable::hdr_select_all");

    ui.click("MainWindow::batch_purge_btn");
    ui.pump_until("the purge to reach the engine", || {
        ui.core.purges().len() == 2
    })
    .await;

    assert_eq!(ui.core.purges().len(), 2);
    assert!(ui.core.removes().is_empty());
}

pub(super) async fn the_context_menu_keeps_remove_and_purge_apart() {
    let ui = new_window();
    ui.seed(
        (1..=2)
            .map(|n| http_task(n, "alpha.bin", DownloadState::Downloading, 10, 100))
            .collect(),
    );
    ui.click("MainWindow::btn_view_mode");

    ui.right_click_nth("TaskTable::ta_row", 0);
    ui.click("ContextMenu::mi_remove");
    ui.pump_until("the removal to reach the engine", || {
        !ui.core.removes().is_empty()
    })
    .await;
    assert_eq!(ui.core.removes(), vec![http_wire(1)]);
    assert!(
        ui.core.purges().is_empty(),
        "dropping the record must keep the file"
    );

    ui.core.clear();
    ui.right_click_nth("TaskTable::ta_row", 1);
    ui.click("ContextMenu::mi_purge");
    ui.pump_until("the purge to reach the engine", || {
        !ui.core.purges().is_empty()
    })
    .await;
    assert_eq!(ui.core.purges(), vec![http_wire(2)]);
    assert!(
        ui.core.removes().is_empty(),
        "\"Delete File Permanently\" is the only entry point that may delete"
    );
}

pub(super) async fn the_priority_menu_reports_the_chosen_priority() {
    let ui = new_window();
    ui.seed(vec![http_task(
        1,
        "alpha.bin",
        DownloadState::Downloading,
        10,
        100,
    )]);
    ui.click("MainWindow::btn_view_mode");

    ui.right_click_nth("TaskTable::ta_row", 0);
    ui.click("ContextMenu::mi_priority");
    assert!(!ui.window.get_context_menu_state().visible);
    assert!(ui.window.get_priority_menu_visible());
    assert_eq!(ui.window.get_priority_menu_task_id().as_str(), http_wire(1));

    ui.click("PriorityMenu::prio_high");
    ui.pump_until("the priority change to reach the engine", || {
        !ui.core.priorities().is_empty()
    })
    .await;

    assert_eq!(ui.core.priorities(), vec![(http_wire(1), Priority::High)]);
    assert!(
        !ui.window.get_priority_menu_visible(),
        "picking closes the popup"
    );
}

/// An optimistic delete has to be undone when the engine refuses it: the row is
/// dropped locally first, and a backend error must resynchronize the list from
/// the engine instead of leaving the UI out of sync with it.
///
/// [`RecordingBackend`] answers every mutation with `NotFound`, so this exercises
/// the `reload_tasks` path in `handlers/task/single.rs`.
pub(super) async fn a_rejected_removal_puts_the_row_back() {
    let ui = new_window();
    ui.seed(vec![http_task(
        1,
        "alpha.bin",
        DownloadState::Downloading,
        10,
        100,
    )]);
    ui.click("MainWindow::btn_view_mode");

    ui.right_click_nth("TaskTable::ta_row", 0);
    ui.click("ContextMenu::mi_remove");
    assert_eq!(
        ui.visible_rows(),
        0,
        "the row is dropped before the backend answers"
    );

    ui.pump_until("the failed removal to restore the row", || {
        ui.visible_rows() == 1
    })
    .await;
    assert!(
        !ui.core.removes().is_empty(),
        "the backend was asked to drop the record"
    );
    assert!(ui.core.purges().is_empty(), "and only the record");
}

/// “Select all” is filter-aware (`TaskStore::select_all` walks the *filtered*
/// rows) and the selection then outlives a category switch. Both halves are
/// load-bearing for the destructive batch actions: a select-all that ignored the
/// filter would hand the engine every task id while the user sees two rows — and
/// `Delete Files` would delete files they never selected.
pub(super) async fn select_all_takes_the_filtered_rows_and_the_selection_survives_a_switch() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Downloading, 20, 100),
        http_task(3, "charlie.bin", DownloadState::Paused, 30, 100),
        http_task(4, "delta.bin", DownloadState::Completed, 100, 100),
    ]);

    // Only the table header offers “select all” before anything is selected.
    ui.click("MainWindow::cat_downloading");
    assert_eq!(ui.visible_rows(), 2);
    ui.click("MainWindow::btn_view_mode");
    ui.click("TaskTable::hdr_select_all");
    assert_eq!(
        ui.window.get_selected_count(),
        2,
        "select-all must take the two visible rows, not the whole list"
    );

    // The selection is deliberately not scoped to the category: switching away
    // keeps it (and the batch bar) alive.
    ui.click("MainWindow::cat_completed");
    assert_eq!(ui.visible_rows(), 1);
    assert_eq!(
        ui.window.get_selected_count(),
        2,
        "the selection survives a category switch"
    );
    assert!(ui.has("MainWindow::batch_bar"));

    // …and the destructive action still names exactly those two rows.
    ui.click("MainWindow::cat_downloading");
    ui.click("MainWindow::batch_purge_btn");
    ui.pump_until("the purge to reach the engine", || {
        ui.core.purges().len() == 2
    })
    .await;
    let mut purged = ui.core.purges();
    purged.sort();
    assert_eq!(purged, vec![http_wire(1), http_wire(2)]);
    assert!(
        ui.core.removes().is_empty(),
        "Delete Files is the only entry point that purges"
    );
}
