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

use limedl_core::types::{ChecksumMode, DownloadState, TorrentFileEntry};

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
        delete_hotkey_removes_without_deleting_files().await;
        shift_delete_hotkey_purges_the_selection().await;
        space_hotkey_pauses_and_resumes_the_list().await;
        an_open_dialog_swallows_the_space_and_delete_hotkeys().await;
        a_rejected_removal_puts_the_row_back().await;
        submitting_a_url_passes_the_dialog_state_to_the_engine().await;
        submitting_a_torrent_leaves_out_the_unchecked_files().await;
        submitting_a_batch_starts_one_task_per_link().await;
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

/// `Delete` is the *non*-destructive shortcut: the row goes, the file stays.
///
/// The `.slint` key handler is what turns the key press into
/// `hotkey_delete(delete_files: false)`, so the `Ctrl+N` test in `shell.rs`
/// (which proves shortcuts reach the focus scope at all) cannot see this.
async fn delete_hotkey_removes_without_deleting_files() {
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
async fn shift_delete_hotkey_purges_the_selection() {
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
async fn space_hotkey_pauses_and_resumes_the_list() {
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
async fn an_open_dialog_swallows_the_space_and_delete_hotkeys() {
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

/// An optimistic delete has to be undone when the engine refuses it: the row is
/// dropped locally first, and a backend error must resynchronize the list from
/// the engine instead of leaving the UI out of sync with it.
///
/// [`RecordingBackend`] answers every mutation with `NotFound`, so this exercises
/// the `reload_tasks` path in `handlers/task/single.rs`.
async fn a_rejected_removal_puts_the_row_back() {
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

/// Seed the two caches the native torrent picker fills when it previews a file
/// list — the only way to reach the pre-selection without a file dialog.
fn seed_torrent_files(ui: &TestUi, included: &[bool]) {
    let entries = [
        (0, "movie/movie.mkv", 4_000_000_000_u64),
        (1, "movie/subs.srt", 40_000),
    ];
    *ui.ctx.new_task_torrent_entries.lock() = entries
        .iter()
        .map(|(index, path, size)| TorrentFileEntry {
            index: *index,
            path: (*path).to_string(),
            size: *size,
        })
        .collect();
    *ui.ctx.new_task_torrent_included.lock() = included.to_vec();
    ui.window.set_new_task_preview_state("ready".into());
}

/// Drain the clipboard prefill the dialog spawns on open.
///
/// It only fills an *empty* URL field, but what the host clipboard holds is not
/// this test's business: left pending it would race the assertions below.
async fn open_new_task_dialog_without_the_clipboard_race(ui: &TestUi) {
    ui.click("MainWindow::ta_new_task");
    ui.pump(2).await;
    ui.window.set_new_task_url("".into());
    assert!(ui.window.get_show_new_task_dialog());
}

/// Let the modal's height animation settle before clicking its footer.
///
/// `NewTaskDialog::modal` animates 470px → 600px when the torrent list appears,
/// and a click whose press and release straddle the moving footer is delivered to
/// two different positions and dropped.
async fn settle_dialog_animation(ui: &TestUi) {
    // 12 rounds x the 20ms the mock clock advances per round > the 200ms
    // `animate height` in `new_task_dialog.slint`.
    ui.pump(12).await;
}

/// The submit path is where the dialog's collected state becomes a
/// `StartDownloadRequest`. Nothing else sees it: `bridge/`'s unit tests stop at
/// the form and `new_task.rs` only covers the reset contract.
async fn submitting_a_url_passes_the_dialog_state_to_the_engine() {
    const SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    let ui = new_window();
    open_new_task_dialog_without_the_clipboard_race(&ui).await;

    // Type into the real fields: `submit_btn` is `enabled` on the URL property
    // the `TextInput` writes, so only the full path proves the wiring.
    ui.click("NewTaskDialog::nt_url_input");
    ui.type_text("https://example.invalid/archive.zip");
    assert_eq!(
        ui.window.get_new_task_url().as_str(),
        "https://example.invalid/archive.zip",
        "the URL field is bound to `new_task_url`, which gates the submit button"
    );
    ui.click("NewTaskDialog::nt_filename_input");
    ui.type_text("renamed.zip");
    assert_eq!(ui.window.get_new_task_filename().as_str(), "renamed.zip");
    // The dialog opens with the default directory already in the field, so it
    // has to be cleared for the typed value to be the only thing in the request.
    ui.window.set_new_task_dir("".into());
    ui.click("NewTaskDialog::nt_dir_input");
    ui.type_text("C:\\limedl-out");
    assert_eq!(ui.window.get_new_task_dir().as_str(), "C:\\limedl-out");

    // "Detect Checksum" performs a real HTTP probe, so the result is seeded the
    // way the prober's callback would leave it: the hash is only accepted
    // together with the state that says it belongs to the current URL.
    ui.window.set_new_task_probe_state("found".into());
    ui.window.set_new_task_probe_hash(SHA256.into());

    ui.click("NewTaskDialog::submit_btn");
    ui.pump_until("the single start request to reach the engine", || {
        !ui.core.starts().is_empty()
    })
    .await;

    let starts = ui.core.starts();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].url, "https://example.invalid/archive.zip");
    assert_eq!(
        starts[0].destination_dir, "C:\\limedl-out",
        "a typed directory must win over the default one"
    );
    assert_eq!(starts[0].file_name.as_deref(), Some("renamed.zip"));
    assert_eq!(starts[0].checksum, Some(ChecksumMode::Sha256));
    assert_eq!(starts[0].expected_checksum.as_deref(), Some(SHA256));
    assert_eq!(
        starts[0].selected_file_indices, None,
        "no torrent was previewed, so every file is included"
    );

    // The fixture answers `Ok`, which is what a successful add does: close the
    // dialog, clear the URL for the next visit and confirm with a toast.
    ui.pump_until("the dialog to close", || {
        !ui.window.get_show_new_task_dialog()
    })
    .await;
    assert_eq!(ui.window.get_new_task_url().as_str(), "");
    ui.assert_toast(
        "success",
        &crate::i18n::format_toast_task_added("renamed.zip", Language::ZhCn),
    );
}

/// The torrent pre-selection reaches the engine as indices, and a selection that
/// would download nothing is refused with an inline hint instead of a silent
/// no-op task.
///
/// The file list is driven by `preview_state` plus the picker's caches, not by the
/// URL: an `.torrent` / `magnet:` link would be classified as BT by
/// `Dispatcher::start` and routed to a backend this fixture does not register (a
/// routing fact covered by `limedl-core`'s own tests), so the URL here stays HTTP.
async fn submitting_a_torrent_leaves_out_the_unchecked_files() {
    let ui = new_window();
    // The expanded torrent list makes this dialog taller than the default 800x600
    // test window, and the dialog itself does not scroll: at that size the footer
    // (and with it the submit button a click has to reach) sits below the window
    // edge. `layout.rs` pins the same overflow as a characterization.
    ui.set_window_size(1280.0, 900.0);
    open_new_task_dialog_without_the_clipboard_race(&ui).await;
    ui.click("NewTaskDialog::nt_url_input");
    ui.type_text("https://example.invalid/pack.zip");

    seed_torrent_files(&ui, &[true, false]);
    assert_eq!(
        ui.window.get_new_task_url().as_str(),
        "https://example.invalid/pack.zip"
    );
    settle_dialog_animation(&ui).await;
    ui.assert_inside_window("NewTaskDialog::submit_btn");
    ui.click("NewTaskDialog::submit_btn");
    ui.pump_until("the torrent start request to reach the engine", || {
        !ui.core.starts().is_empty()
    })
    .await;

    let starts = ui.core.starts();
    assert_eq!(
        starts[0].selected_file_indices,
        Some(vec![0]),
        "only the checked file may be downloaded"
    );
    assert_eq!(starts[0].file_name, None, "no custom filename was typed");
    assert_eq!(starts[0].checksum, None, "no checksum was probed");
    assert_eq!(
        starts[0].destination_dir,
        ui.window.get_default_download_dir().as_str(),
        "an empty directory field falls back to the default"
    );

    // Second visit: every file unchecked. The dialog must stay open and say why.
    ui.core.clear();
    open_new_task_dialog_without_the_clipboard_race(&ui).await;
    ui.click("NewTaskDialog::nt_url_input");
    ui.type_text("https://example.invalid/pack.zip");
    seed_torrent_files(&ui, &[false, false]);
    settle_dialog_animation(&ui).await;

    ui.click("NewTaskDialog::submit_btn");
    ui.pump(2).await;
    assert!(
        ui.core.starts().is_empty(),
        "a task with no files selected must not be started: {:?}",
        ui.core.calls()
    );
    assert!(ui.window.get_show_new_task_dialog());
    assert_eq!(
        ui.window.get_new_task_preview_status_text().as_str(),
        crate::i18n::no_files_selected_text(Language::ZhCn)
    );
}

/// Batch mode parses the textarea and starts one task per link, each with its
/// own file name — the aggregate toast is the only feedback the user gets.
async fn submitting_a_batch_starts_one_task_per_link() {
    let ui = new_window();
    open_new_task_dialog_without_the_clipboard_race(&ui).await;

    ui.click("NewTaskDialog::batch_tab");
    assert!(
        ui.window.get_new_task_batch_mode(),
        "the Batch Mode tab must swap the dialog's content"
    );

    // The `edited` callback is what the `TextEdit` fires per keystroke; typing a
    // newline is not something `type_text` can do (a control character is not
    // inserted as text), so the text arrives the way a paste would.
    const TEXT: &str = "https://example.invalid/a.zip\nhttps://example.invalid/b.zip";
    ui.window.set_new_task_batch_text(TEXT.into());
    ui.window.invoke_new_task_batch_text_changed(TEXT.into());

    ui.click("NewTaskDialog::submit_batch_btn");
    ui.pump_until("both links to reach the engine", || {
        ui.core.starts().len() == 2
    })
    .await;

    let starts = ui.core.starts();
    let mut urls: Vec<String> = starts.iter().map(|start| start.url.clone()).collect();
    urls.sort();
    assert_eq!(
        urls,
        [
            "https://example.invalid/a.zip",
            "https://example.invalid/b.zip"
        ]
    );

    let mut names: Vec<Option<String>> = starts.iter().map(|start| start.file_name.clone()).collect();
    names.sort();
    assert_eq!(
        names,
        [Some("a.zip".to_string()), Some("b.zip".to_string())],
        "each link keeps its own file name"
    );

    let default_dir = ui.window.get_default_download_dir();
    assert!(
        starts
            .iter()
            .all(|start| start.destination_dir == default_dir.as_str()),
        "every link shares the dialog's directory"
    );
    assert!(
        starts
            .iter()
            .all(|start| start.checksum.is_none() && start.selected_file_indices.is_none()),
        "batch mode collects nothing but the links"
    );

    ui.pump_until("the dialog to close once every link finished", || {
        !ui.window.get_show_new_task_dialog()
    })
    .await;
    ui.assert_toast(
        "success",
        &crate::i18n::format_toast_batch_done(2, 2, Language::ZhCn),
    );
}
