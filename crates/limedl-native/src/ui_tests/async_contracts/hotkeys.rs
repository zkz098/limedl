//! Global list shortcuts (`Space`, `Delete`, `Shift+Delete`) and the modal
//! guard that has to swallow them while a dialog is open.

use limedl_core::types::DownloadState;

use super::*;

/// `Delete` is the *non*-destructive shortcut: the row goes, the file stays.
///
/// The `.slint` key handler is what turns the key press into
/// `hotkey_delete(delete_files: false)`, so the `Ctrl+N` test in `shell.rs`
/// (which proves shortcuts reach the focus scope at all) cannot see this.
pub(super) async fn delete_hotkey_removes_without_deleting_files() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Downloading, 10, 100),
    ]);
    // Select through the callback the row's checkbox invokes: clicking the row
    // first would hand the focus to the list and swallow the key.
    ui.window.invoke_toggle_select_task(http_wire(2).into());
    assert_eq!(ui.window.get_selected_count(), 1);

    ui.press_keys(&[Key::Delete.into()]);

    assert_eq!(
        ui.visible_rows(),
        1,
        "the selected row disappears before the backend confirms"
    );

    ui.pump_until("the removal to reach the engine", || {
        !ui.core.removes().is_empty()
    })
    .await;
    assert_eq!(ui.core.removes(), vec![http_wire(2)]);
    assert!(
        ui.core.purges().is_empty(),
        "plain Delete must never delete the file on disk"
    );
}

/// The one keyboard path that deletes files. `Shift` arrives as its own key
/// event, which is exactly what `event.modifiers.shift` in the `.slint` handler
/// reads.
pub(super) async fn shift_delete_hotkey_purges_the_selection() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Downloading, 10, 100),
    ]);
    ui.window.invoke_select_all();

    ui.press_keys(&[Key::Shift.into(), Key::Delete.into()]);
    assert_eq!(ui.visible_rows(), 0, "the whole selection goes at once");

    ui.pump_until("the purge to reach the engine", || {
        ui.core.purges().len() == 2
    })
    .await;
    // The selection is a set, so the engine sees the ids in hash order.
    let mut purged = ui.core.purges();
    purged.sort();
    assert_eq!(purged, vec![http_wire(1), http_wire(2)]);
    assert!(
        ui.core.removes().is_empty(),
        "Shift+Delete is the shortcut that deletes files"
    );
}

/// `Space` toggles pause/resume for the selection, or for the whole filtered
/// list when nothing is selected — and it is the list state that decides which
/// of the two actions it applies.
pub(super) async fn space_hotkey_pauses_and_resumes_the_list() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
    ]);

    // No selection, and something is downloading: everything gets paused, not
    // just the downloading row.
    ui.press_keys(&[' ']);
    ui.pump_until("the pause to reach the engine", || {
        ui.core.pauses().len() == 2
    })
    .await;
    let mut paused = ui.core.pauses();
    paused.sort();
    assert_eq!(paused, vec![http_wire(1), http_wire(2)]);
    assert!(ui.core.resumes().is_empty());

    // With a paused row selected there is nothing left to pause, so the same key
    // resumes it — the branch the "toggle" name promises.
    ui.core.clear();
    ui.window.invoke_toggle_select_task(http_wire(2).into());
    ui.press_keys(&[' ']);
    ui.pump_until("the resume to reach the engine", || {
        !ui.core.resumes().is_empty()
    })
    .await;
    assert_eq!(ui.core.resumes(), vec![http_wire(2)]);
    assert!(ui.core.pauses().is_empty());
}

/// The `.slint` key chain rejects `Space` / `Delete` while a modal is open.
/// Before that branch joined the `else if` chain it was a bare `if … { reject }`
/// whose value was discarded, so the chain still ran and `Delete` kept removing
/// the rows *behind* the dialog (see the comment in `appwindow.slint`).
pub(super) async fn an_open_dialog_swallows_the_space_and_delete_hotkeys() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Downloading, 10, 100),
    ]);
    ui.window.invoke_select_all();
    ui.window.invoke_open_settings();

    ui.press_keys(&[' ']);
    ui.press_keys(&[Key::Delete.into()]);
    ui.press_keys(&[Key::Shift.into(), Key::Delete.into()]);

    // The assertion is about the *absence* of engine calls, so let whatever the
    // keys could have spawned settle first (`pump` alone: there is nothing to
    // wait for, so a `pump_until` predicate would be vacuous).
    ui.pump(2).await;
    assert!(
        ui.core.pauses().is_empty() && ui.core.resumes().is_empty(),
        "the list behind the modal must not react to Space: {:?}",
        ui.core.calls()
    );
    assert!(
        ui.core.removes().is_empty() && ui.core.purges().is_empty(),
        "Delete / Shift+Delete must not reach the rows behind the modal: {:?}",
        ui.core.calls()
    );
    assert_eq!(
        ui.window.get_selected_count(),
        2,
        "the selection behind the modal is untouched"
    );
}
