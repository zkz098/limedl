//! The global list actions on the toolbar / batch bar, which read the *state* of
//! the list rather than the selection.

use limedl_core::types::DownloadState;

use super::super::{Language, http_task, http_wire, new_window};

/// The two toolbar actions walk the *list state*, not the selection: Pause All
/// only names downloading tasks, Resume All only paused ones — a selection must
/// not change that (unlike the batch bar, which acts on the selection alone).
pub(super) async fn pause_all_and_resume_all_follow_the_state_not_the_selection() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
        http_task(3, "charlie.bin", DownloadState::Completed, 100, 100),
    ]);
    ui.window.invoke_toggle_select_task(http_wire(3).into());

    ui.click("MainWindow::btn_pause_all");
    ui.pump_until("the pause-all to reach the engine", || {
        !ui.core.pauses().is_empty()
    })
    .await;
    assert_eq!(
        ui.core.pauses(),
        vec![http_wire(1)],
        "only the downloading task"
    );
    assert!(ui.core.resumes().is_empty());

    ui.core.clear();
    ui.click("MainWindow::btn_resume_all");
    ui.pump_until("the resume-all to reach the engine", || {
        !ui.core.resumes().is_empty()
    })
    .await;
    assert_eq!(
        ui.core.resumes(),
        vec![http_wire(2)],
        "only the paused task"
    );
    assert!(ui.core.pauses().is_empty());
}

/// `Clear Completed` drops finished *records* (the files stay) and reports what
/// it did.
///
/// Only two of its three branches are reachable here: the fixture rejects
/// mutations on purpose (the rollback scenario needs that), so the “cleared N”
/// success toast cannot be produced without a backend that confirms the removal.
pub(super) async fn clear_completed_reports_nothing_to_do_and_surfaces_a_failed_clear() {
    let ui = new_window();
    ui.seed(vec![http_task(
        1,
        "alpha.bin",
        DownloadState::Downloading,
        10,
        100,
    )]);

    // Nothing finished yet: an info toast, no engine call, and the row stays.
    ui.click("MainWindow::btn_clear_completed");
    ui.pump_until("the empty-clear toast", || !ui.toasts().is_empty())
        .await;
    ui.assert_toast(
        "info",
        crate::i18n::format_toast_clear_completed_none(Language::ZhCn),
    );
    assert!(ui.core.removes().is_empty() && ui.core.purges().is_empty());
    assert_eq!(ui.visible_rows(), 1);

    // With a finished task the row goes first and the backend rejection is what
    // the user ends up seeing (not a silent success).
    ui.core.clear();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Completed, 100, 100),
    ]);
    ui.click("MainWindow::btn_clear_completed");
    assert_eq!(ui.visible_rows(), 1, "the finished row goes first");
    assert!(
        ui.core.purges().is_empty(),
        "clearing records must never delete files"
    );
    ui.pump_until("the failed clear to be reported", || {
        ui.toasts().iter().any(|(kind, _)| kind == "error")
    })
    .await;
    assert!(ui.core.removes().contains(&http_wire(2)));
}
