//! The contracts that only settle once the event loop runs: what the UI asks the
//! engine to do, and what a rejected save leaves behind.
//!
//! **All of them live in one test function.** Slint's event-loop proxy is a
//! process-global `OnceCell`, so `init_integration_test_with_mock_time` succeeds
//! for exactly one test per process; a second test would panic with "platform
//! already initialized". Each scenario therefore builds its own window through
//! [`new_window`] instead of relying on a fresh process.
//!
//! Those scenarios share the mocked clock, so they advance it in 20 ms steps;
//! keep them independent (own window, own seeded tasks, own recorded calls).

use limedl_core::types::DownloadState;

use super::*;

/// One test, many scenarios — see the module docs for why they cannot be
/// separate `#[test]` functions.
#[test]
fn event_loop_contracts() {
    with_ui_async(async |_| {
        batch_actions_reach_the_selection_only().await;
        batch_remove_never_deletes_files().await;
        batch_delete_files_purges().await;
        the_context_menu_keeps_remove_and_purge_apart().await;
        the_priority_menu_reports_the_chosen_priority().await;
        card_buttons_act_on_their_own_task().await;
        a_rejected_save_keeps_the_dialog_open().await;
        an_invalid_schedule_row_is_reported().await;
    });
}

async fn batch_actions_reach_the_selection_only() {
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

async fn batch_remove_never_deletes_files() {
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

async fn batch_delete_files_purges() {
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

async fn the_context_menu_keeps_remove_and_purge_apart() {
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

async fn the_priority_menu_reports_the_chosen_priority() {
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

async fn card_buttons_act_on_their_own_task() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
    ]);

    // Per-row buttons: the second card's folder icon must name the second task,
    // not the first row the query happens to return.
    ui.click_nth("TaskCard::card_explorer", 1);
    ui.pump_until("the explorer call", || !ui.core.explorer().is_empty())
        .await;
    assert_eq!(ui.core.explorer(), vec![http_wire(2)]);

    ui.core.clear();
    ui.click_nth("TaskCard::card_details", 1);
    assert_eq!(
        ui.ctx.active_inspector_id.lock().clone(),
        Some(http_wire(2))
    );
    // Let anything the click could have spawned settle before claiming it did
    // nothing else: the assertion is about the *absence* of a call.
    ui.pump(2).await;
    assert!(
        ui.core.explorer().is_empty(),
        "Details opens the panel, not a folder"
    );

    // The card mirrors the row state: one task is downloading (pause) and the
    // other paused (resume).
    assert_eq!(ui.find_all("TaskCard::card_pause").len(), 1);
    assert_eq!(ui.find_all("TaskCard::card_resume").len(), 1);
}

async fn a_rejected_save_keeps_the_dialog_open() {
    let ui = new_window();
    ui.click("MainWindow::ta_set");

    // Proxy set to "manual" with no URL: the form validator rejects it before the
    // core is even asked.
    let mut form = ui.window.get_settings_form();
    form.proxy_mode_idx = 2;
    form.proxy_manual_url = "".into();
    ui.window.set_settings_form(form);

    ui.window
        .invoke_save_settings(ui.window.get_settings_form());
    ui.pump_until("the rejected save to surface a toast", || {
        !ui.toasts().is_empty()
    })
    .await;

    assert_eq!(ui.toasts()[0].0, "error");
    assert!(
        ui.window.get_show_settings(),
        "a validation error must not close the dialog and lose the edits"
    );
}

async fn an_invalid_schedule_row_is_reported() {
    let ui = new_window();
    ui.click("MainWindow::ta_set");
    ui.window.invoke_schedule_set_enabled(true);
    ui.window
        .invoke_schedule_update(0, "start".into(), "not-a-hour".into());

    ui.window
        .invoke_save_settings(ui.window.get_settings_form());
    ui.pump_until("the schedule validation to surface a toast", || {
        !ui.toasts().is_empty()
    })
    .await;

    assert_eq!(ui.toasts()[0].0, "error");
    assert!(
        ui.window.get_show_settings(),
        "the row the user has to fix must stay reachable"
    );
}
