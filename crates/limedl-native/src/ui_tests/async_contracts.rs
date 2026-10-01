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

use limedl_core::types::{
    ChecksumMode, DownloadState, DoubleClickOnCompleted, DoubleClickOnUncompleted,
    TorrentFileEntry,
};

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
        select_all_takes_the_filtered_rows_and_the_selection_survives_a_switch().await;
        table_row_buttons_act_on_their_own_task().await;
        pause_all_and_resume_all_follow_the_state_not_the_selection().await;
        clear_completed_reports_nothing_to_do_and_surfaces_a_failed_clear().await;
        card_pause_and_resume_hit_their_own_row().await;
        double_click_follows_the_configured_behavior().await;
        saving_settings_persists_the_edited_form().await;
        saving_labs_persists_the_rules().await;
        the_setup_wizard_persists_its_form_and_remembers_where_it_was().await;
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

/// “Select all” is filter-aware (`TaskStore::select_all` walks the *filtered*
/// rows) and the selection then outlives a category switch. Both halves are
/// load-bearing for the destructive batch actions: a select-all that ignored the
/// filter would hand the engine every task id while the user sees two rows — and
/// `Delete Files` would delete files they never selected.
async fn select_all_takes_the_filtered_rows_and_the_selection_survives_a_switch() {
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

/// The table duplicates the card's per-row actions in a second `for` loop, so the
/// same “which task did this button name?” check has to run there too — the
/// table's own buttons had no ids at all before, i.e. were untestable.
async fn table_row_buttons_act_on_their_own_task() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
    ]);
    // The table is wider than the default 800x600 test window, and without one
    // the action column is clipped away (see the characterization in
    // `layout.rs`), i.e. a click could not reach it.
    ui.set_window_size(1600.0, 900.0);
    ui.click("MainWindow::btn_view_mode");

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
    assert_eq!(ui.ctx.active_inspector_id.lock().clone(), Some(http_wire(2)));
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
    assert!(ui.core.purges().is_empty(), "the trash button keeps the file");
}

/// The two toolbar actions walk the *list state*, not the selection: Pause All
/// only names downloading tasks, Resume All only paused ones — a selection must
/// not change that (unlike the batch bar, which acts on the selection alone).
async fn pause_all_and_resume_all_follow_the_state_not_the_selection() {
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
    assert_eq!(ui.core.pauses(), vec![http_wire(1)], "only the downloading task");
    assert!(ui.core.resumes().is_empty());

    ui.core.clear();
    ui.click("MainWindow::btn_resume_all");
    ui.pump_until("the resume-all to reach the engine", || {
        !ui.core.resumes().is_empty()
    })
    .await;
    assert_eq!(ui.core.resumes(), vec![http_wire(2)], "only the paused task");
    assert!(ui.core.pauses().is_empty());
}

/// `Clear Completed` drops finished *records* (the files stay) and reports what
/// it did.
///
/// Only two of its three branches are reachable here: the fixture rejects
/// mutations on purpose (the rollback scenario needs that), so the “cleared N”
/// success toast cannot be produced without a backend that confirms the removal.
async fn clear_completed_reports_nothing_to_do_and_surfaces_a_failed_clear() {
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

/// The card's own pause/resume buttons. `card_buttons_act_on_their_own_task`
/// asserts only that they render, never that they name their own task.
async fn card_pause_and_resume_hit_their_own_row() {
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
async fn double_click_follows_the_configured_behavior() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
        http_task(3, "charlie.bin", DownloadState::Completed, 100, 100),
    ]);
    let settings = &ui.ctx.current_settings;
    settings.lock().double_click.on_uncompleted = DoubleClickOnUncompleted::TogglePauseResume;

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
    settings.lock().double_click.on_completed = DoubleClickOnCompleted::OpenFile;
    ui.window.invoke_task_double_clicked(http_wire(3).into());
    ui.pump_until("the file open", || !ui.core.files_opened().is_empty())
        .await;
    assert_eq!(ui.core.files_opened(), vec![http_wire(3)]);
    assert!(ui.core.dirs_opened().is_empty() && ui.core.explorer().is_empty());

    ui.core.clear();
    settings.lock().double_click.on_completed = DoubleClickOnCompleted::OpenInExplorer;
    ui.window.invoke_task_double_clicked(http_wire(3).into());
    ui.pump_until("the explorer open", || !ui.core.explorer().is_empty())
        .await;
    assert_eq!(ui.core.explorer(), vec![http_wire(3)]);
    assert!(ui.core.files_opened().is_empty() && ui.core.dirs_opened().is_empty());

    ui.core.clear();
    settings.lock().double_click.on_completed = DoubleClickOnCompleted::OpenDownloadDir;
    ui.window.invoke_task_double_clicked(http_wire(3).into());
    ui.pump_until("the directory open", || !ui.core.dirs_opened().is_empty())
        .await;
    assert_eq!(ui.core.dirs_opened(), vec![http_wire(3)]);
    assert!(ui.core.files_opened().is_empty() && ui.core.explorer().is_empty());

    // “Do nothing” must stay silent for both kinds of task.
    ui.core.clear();
    settings.lock().double_click.on_completed = DoubleClickOnCompleted::None;
    settings.lock().double_click.on_uncompleted = DoubleClickOnUncompleted::None;
    ui.window.invoke_task_double_clicked(http_wire(1).into());
    ui.window.invoke_task_double_clicked(http_wire(3).into());
    ui.pump(2).await;
    assert!(
        ui.core.calls().is_empty(),
        "the disabled behavior must not touch the engine: {:?}",
        ui.core.calls()
    );
}

/// Saving the settings dialog: the edited form has to reach `AppSettings`, the
/// engine (one `update_settings` per backend) and the user (dialog closed +
/// confirmation). Only the *rejected* save had a test before, which never
/// exercised the success path at all.
async fn saving_settings_persists_the_edited_form() {
    let ui = new_window();
    ui.click("MainWindow::ta_set");
    assert!(ui.window.get_show_settings());

    // One field, edited the way the dialog edits it. The download directory is
    // deliberate: an aria2/autostart change would fire the side effects the real
    // save configures (service restart, registry write) and this test is about
    // the form → settings → engine path.
    let mut form = ui.window.get_settings_form();
    form.default_download_dir = "C:\\limedl-saved".into();
    ui.window.set_settings_form(form);

    ui.click("SettingsDialog::save_btn");
    ui.pump_until("the save to reach the engine", || {
        ui.core.settings_pushes() == 1
    })
    .await;
    assert_eq!(
        ui.ctx.current_settings.lock().download.default_download_dir,
        "C:\\limedl-saved",
        "the edited field must land in the shared settings"
    );

    ui.pump_until("the dialog to close", || !ui.window.get_show_settings())
        .await;
    ui.assert_toast(
        "success",
        crate::i18n::format_toast_settings_saved(Language::ZhCn),
    );
}

/// Saving the labs dialog: the rule editor's Rust-side list is what gets
/// persisted (the widget owns the field text until Save), so this is the only
/// place where the two sources of truth are merged.
async fn saving_labs_persists_the_rules() {
    let ui = new_window();
    ui.window.invoke_open_labs();
    ui.window.invoke_import_rewrite_preset("github".into());
    assert_eq!(ui.ctx.rewrite_rules.lock().len(), 1);

    ui.click("LabsDialog::save_labs_btn");
    ui.pump_until("the labs save to reach the engine", || {
        ui.core.settings_pushes() == 1
    })
    .await;
    assert_eq!(
        ui.ctx.current_settings.lock().url_rewrite.rules.len(),
        1,
        "the imported rule must be persisted with the settings"
    );

    ui.pump_until("the dialog to close", || !ui.window.get_show_labs())
        .await;
    ui.assert_toast(
        "success",
        crate::i18n::format_toast_labs_saved(Language::ZhCn),
    );
}

/// The first-run wizard, in three phases: finishing persists the form and marks
/// the setup done, the header close button only remembers the step (and syncs the
/// language the cards switched live), and “re-run setup” resets both flags.
///
/// `factory_reset` is deliberately out of reach here — it shuts the backends
/// down, wipes the data directory and relaunches the process.
async fn the_setup_wizard_persists_its_form_and_remembers_where_it_was() {
    // Phase 1: finish from the last step.
    let ui = new_window();
    ui.window.set_setup_start_step(8);
    ui.window.set_show_setup_wizard(true);
    assert!(ui.has("SetupWizard::finish_btn"));

    ui.click("SetupWizard::finish_btn");
    ui.pump_until("the wizard save to reach the engine", || {
        ui.core.settings_pushes() == 1
    })
    .await;
    {
        let settings = ui.ctx.current_settings.lock();
        assert!(settings.setup_completed, "finishing marks the setup as done");
        assert_eq!(settings.last_setup_step, Some(8));
    }
    ui.pump_until("the wizard to close", || {
        !ui.window.get_show_setup_wizard()
    })
    .await;
    // `assert_toast` (exactly one) would be wrong here: finishing can legitimately
    // emit side-effect toasts on the way, e.g. `sync_aria2_rpc` stopping a
    // service the default form switches off. What the wizard owes the user is the
    // success line.
    assert!(
        ui.toasts().iter().any(|(kind, message)| kind == "success"
            && message == crate::i18n::format_toast_setup_finished(Language::ZhCn)),
        "finishing must confirm: {:?}",
        ui.toasts()
    );

    // Phase 2: the language cards and the header close button.
    let ui = new_window();
    ui.window.set_setup_start_step(1);
    ui.window.set_show_setup_wizard(true);
    // The three language cards are three 50%-wide cards in one row — 150% of the
    // modal before the gaps — so the `en-US` card always overflows the modal and
    // cannot be clicked (pinned in `layout.rs`): no window size moves it back in,
    // because its left edge already sits at the modal's right edge. That leaves
    // the handler the card calls as the testable contract.
    assert_eq!(
        ui.find_all("LanguageCard::ta").len(),
        3,
        "all three languages render"
    );
    ui.window.invoke_setup_set_language(2); // zh-CN / zh-TW / en-US, in that order
    assert_eq!(ui.window.get_setup_form().language_idx, 2);
    // `select_bundled_translation` is process-global, so put it back before the
    // assertions below — the fixture's other windows expect zh-CN (`cargo test`
    // runs them in one process, nextest does not).
    crate::i18n::apply_translation(Language::ZhCn);

    ui.click("SetupWizard::wizard_close_btn");
    ui.pump_until("the step to be persisted", || {
        ui.ctx.current_settings.lock().last_setup_step == Some(1)
    })
    .await;
    assert_eq!(
        ui.ctx.store.lock().language(),
        Language::EnUs,
        "closing the wizard hands its live language to the task store"
    );
    ui.pump_until("the wizard to close", || {
        !ui.window.get_show_setup_wizard()
    })
    .await;
    assert!(
        !ui.ctx.current_settings.lock().setup_completed,
        "closing the wizard is not finishing it"
    );

    // Phase 3: “Re-run Setup Wizard” (the About tab's button calls exactly this
    // callback) resets both wizard flags and reopens the first step.
    let ui = new_window();
    {
        let mut settings = ui.ctx.current_settings.lock();
        settings.setup_completed = true;
        settings.last_setup_step = Some(5);
    }
    ui.window.set_show_settings(true);

    ui.window.invoke_restart_setup();
    ui.pump_until("the restart to reach the engine", || {
        ui.core.settings_pushes() == 1
    })
    .await;
    {
        let settings = ui.ctx.current_settings.lock();
        assert!(!settings.setup_completed, "the wizard has to run again");
        assert_eq!(settings.last_setup_step, None, "…from the first step");
    }
    ui.pump_until("the wizard to come back", || {
        ui.window.get_show_setup_wizard()
    })
    .await;
    assert!(!ui.window.get_show_settings());
    assert_eq!(ui.window.get_setup_start_step(), 0);
}
