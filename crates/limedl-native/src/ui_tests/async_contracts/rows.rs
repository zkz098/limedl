//! Per-row controls: the card and table action buttons (each one has to act on
//! *its own* task) and the double-click behaviour configured in the settings.

use limedl_core::types::{DoubleClickOnCompleted, DoubleClickOnUncompleted, DownloadState};

use super::super::{http_task, http_wire, new_window};

pub(super) async fn card_buttons_act_on_their_own_task() {
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

/// The table duplicates the card's per-row actions in a second `for` loop, so the
/// same “which task did this button name?” check has to run there too — the
/// table's own buttons had no ids at all before, i.e. were untestable.
pub(super) async fn table_row_buttons_act_on_their_own_task() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
    ]);
    // The table's columns are wider than any supported window, so the action column
    // is pinned to the right edge and the data columns are clipped behind it. At
    // the declared minimum size the row buttons must be on screen — that is the
    // whole point of pinning them — and the header stays above its columns.
    ui.set_window_size(1100.0, 660.0);
    ui.click("MainWindow::btn_view_mode");
    ui.assert_inside_window("TaskTable::ta_explorer");
    ui.assert_inside_window("TaskTable::hdr_file");

    ui.click_nth("TaskTable::ta_explorer", 1);
    ui.pump_until("the explorer call", || !ui.core.explorer().is_empty())
        .await;
    assert_eq!(ui.core.explorer(), vec![http_wire(2)]);

    // Row 0 is downloading, row 1 is paused: each row offers the action its state
    // allows, and the button must name that row's task.
    assert_eq!(ui.find_all("TaskTable::ta_pause").len(), 1);
    assert_eq!(ui.find_all("TaskTable::ta_resume").len(), 1);
    ui.core.clear();
    ui.click_nth("TaskTable::ta_pause", 0);
    ui.pump_until("the pause to reach the engine", || {
        !ui.core.pauses().is_empty()
    })
    .await;
    assert_eq!(ui.core.pauses(), vec![http_wire(1)]);

    ui.core.clear();
    ui.click_nth("TaskTable::ta_resume", 0);
    ui.pump_until("the resume to reach the engine", || {
        !ui.core.resumes().is_empty()
    })
    .await;
    assert_eq!(
        ui.core.resumes(),
        vec![http_wire(2)],
        "the only resume button belongs to the paused row"
    );

    ui.core.clear();
    ui.click_nth("TaskTable::ta_details", 1);
    assert_eq!(
        ui.ctx.active_inspector_id.lock().clone(),
        Some(http_wire(2))
    );
    assert!(ui.window.get_show_inspector());
    ui.window.invoke_close_inspector();

    ui.core.clear();
    ui.click_nth("TaskTable::ta_remove", 0);
    assert_eq!(
        ui.visible_rows(),
        1,
        "the row is dropped before the backend answers"
    );
    ui.pump_until("the removal to reach the engine", || {
        !ui.core.removes().is_empty()
    })
    .await;
    assert_eq!(ui.core.removes(), vec![http_wire(1)]);
    assert!(
        ui.core.purges().is_empty(),
        "the trash button keeps the file"
    );
}

/// The card's own pause/resume buttons. `card_buttons_act_on_their_own_task`
/// asserts only that they render, never that they name their own task.
pub(super) async fn card_pause_and_resume_hit_their_own_row() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
    ]);

    ui.click_nth("TaskCard::card_pause", 0);
    ui.pump_until("the pause to reach the engine", || {
        !ui.core.pauses().is_empty()
    })
    .await;
    assert_eq!(ui.core.pauses(), vec![http_wire(1)]);

    ui.core.clear();
    ui.click_nth("TaskCard::card_resume", 0);
    ui.pump_until("the resume to reach the engine", || {
        !ui.core.resumes().is_empty()
    })
    .await;
    assert_eq!(ui.core.resumes(), vec![http_wire(2)]);
}

/// Double click follows the configured behavior: unfinished tasks toggle
/// pause/resume in the direction their state allows, completed ones run the
/// configured open action (`task_ops::handle_task_double_click`, covered nowhere
/// else).
///
/// The gesture is delivered through the window callback rather than
/// `ElementHandle::double_click`, which awaits Slint's click timer — a timer only
/// fires while the mock clock advances, and inside an awaited helper there is no
/// pump left to advance it.
pub(super) async fn double_click_follows_the_configured_behavior() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
        http_task(3, "charlie.bin", DownloadState::Completed, 100, 100),
    ]);
    ui.ctx
        .dispatcher
        .save_settings_with(|settings| {
            settings.double_click.on_uncompleted = DoubleClickOnUncompleted::TogglePauseResume;
            Ok(())
        })
        .await
        .expect("persist double-click behavior");

    // Unfinished: pause for a running task, resume for a paused one.
    ui.window.invoke_task_double_clicked(http_wire(1).into());
    ui.pump_until("the pause to reach the engine", || {
        !ui.core.pauses().is_empty()
    })
    .await;
    assert_eq!(ui.core.pauses(), vec![http_wire(1)]);
    assert!(ui.core.resumes().is_empty());

    ui.core.clear();
    ui.window.invoke_task_double_clicked(http_wire(2).into());
    ui.pump_until("the resume to reach the engine", || {
        !ui.core.resumes().is_empty()
    })
    .await;
    assert_eq!(ui.core.resumes(), vec![http_wire(2)]);
    assert!(ui.core.pauses().is_empty());

    // Completed: the three open actions are three different backend calls, so the
    // setting has to select between them rather than always fall back to one.
    ui.core.clear();
    ui.ctx
        .dispatcher
        .save_settings_with(|settings| {
            settings.double_click.on_completed = DoubleClickOnCompleted::OpenFile;
            Ok(())
        })
        .await
        .expect("persist double-click behavior");
    ui.window.invoke_task_double_clicked(http_wire(3).into());
    ui.pump_until("the file open", || !ui.core.files_opened().is_empty())
        .await;
    assert_eq!(ui.core.files_opened(), vec![http_wire(3)]);
    assert!(ui.core.dirs_opened().is_empty() && ui.core.explorer().is_empty());

    ui.core.clear();
    ui.ctx
        .dispatcher
        .save_settings_with(|settings| {
            settings.double_click.on_completed = DoubleClickOnCompleted::OpenInExplorer;
            Ok(())
        })
        .await
        .expect("persist double-click behavior");
    ui.window.invoke_task_double_clicked(http_wire(3).into());
    ui.pump_until("the explorer open", || !ui.core.explorer().is_empty())
        .await;
    assert_eq!(ui.core.explorer(), vec![http_wire(3)]);
    assert!(ui.core.files_opened().is_empty() && ui.core.dirs_opened().is_empty());

    ui.core.clear();
    ui.ctx
        .dispatcher
        .save_settings_with(|settings| {
            settings.double_click.on_completed = DoubleClickOnCompleted::OpenDownloadDir;
            Ok(())
        })
        .await
        .expect("persist double-click behavior");
    ui.window.invoke_task_double_clicked(http_wire(3).into());
    ui.pump_until("the directory open", || !ui.core.dirs_opened().is_empty())
        .await;
    assert_eq!(ui.core.dirs_opened(), vec![http_wire(3)]);
    assert!(ui.core.files_opened().is_empty() && ui.core.explorer().is_empty());

    // “Do nothing” must stay silent for both kinds of task.
    ui.ctx
        .dispatcher
        .save_settings_with(|settings| {
            settings.double_click.on_completed = DoubleClickOnCompleted::None;
            settings.double_click.on_uncompleted = DoubleClickOnUncompleted::None;
            Ok(())
        })
        .await
        .expect("persist double-click behavior");
    ui.core.clear();
    ui.window.invoke_task_double_clicked(http_wire(1).into());
    ui.window.invoke_task_double_clicked(http_wire(3).into());
    ui.pump(2).await;
    assert!(
        ui.core.calls().is_empty(),
        "the disabled behavior must not touch the engine: {:?}",
        ui.core.calls()
    );
}
