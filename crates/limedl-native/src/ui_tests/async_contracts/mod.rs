//! The contracts that only settle once the event loop runs: what the UI asks the
//! engine to do, what the engine pushes back, and what a rejected save leaves
//! behind.
//!
//! **All of them live in one test function.** Slint's event-loop proxy is a
//! process-global `OnceCell`, so `init_integration_test_with_mock_time` succeeds
//! for exactly one test per process; a second test would panic with "platform
//! already initialized". Each scenario therefore builds its own window through
//! [`new_window`] instead of relying on a fresh process.
//!
//! Those scenarios share the mocked clock, so they advance it in 20 ms steps;
//! keep them independent (own window, own seeded tasks, own recorded calls).
//!
//! The scenarios are grouped by the surface they drive — the split is for
//! readers, not for the runner: every group is called from the one `#[test]`
//! below, so a scenario is `pub(super)` and is *not* a test of its own.
//!
//! * [`bus`] — engine → window: the `DownloadEvent` subscriber and the periodic
//!   list resync.
//! * [`selection`] — multi-selection, batch actions, remove vs purge.
//! * [`hotkeys`] — `Space` / `Delete` / `Shift+Delete` and the modal guard.
//! * [`rows`] — per-row card/table buttons and the double-click behaviour.
//! * [`new_task`] — the submit payloads and the shared external entry point.
//! * [`toolbar`] — the global list actions (pause all / resume all / clear).
//! * [`dialogs`] — settings, labs and wizard save paths, including rejections.

use limedl_core::types::TorrentFileEntry;

use super::{TestUi, with_ui_async};

mod bus;
mod dialogs;
mod hotkeys;
mod new_task;
mod rows;
mod selection;
mod toolbar;

/// One test, many scenarios — see the module docs for why they cannot be
/// separate `#[test]` functions.
#[test]
fn event_loop_contracts() {
    with_ui_async(async |_| {
        // Engine → window: the only paths that move state without a user action.
        bus::a_progress_tick_repaints_only_the_row_it_names().await;
        bus::a_terminal_state_event_toasts_and_repaints_the_row().await;
        bus::an_event_for_a_task_the_backend_forgot_drops_the_row_and_closes_the_inspector().await;
        bus::a_lagged_subscriber_gets_the_whole_list_back().await;
        bus::a_warning_becomes_a_toast_and_duplicates_are_collapsed().await;
        bus::the_cdn_speedtest_reports_its_progress_and_outcome().await;
        bus::a_background_refresh_resyncs_the_whole_list().await;

        // List actions and the blast radius of each one.
        selection::batch_actions_reach_the_selection_only().await;
        selection::batch_remove_never_deletes_files().await;
        selection::batch_delete_files_purges().await;
        selection::the_context_menu_keeps_remove_and_purge_apart().await;
        selection::the_priority_menu_reports_the_chosen_priority().await;
        selection::a_rejected_removal_puts_the_row_back().await;
        selection::select_all_takes_the_filtered_rows_and_the_selection_survives_a_switch().await;

        hotkeys::delete_hotkey_removes_without_deleting_files().await;
        hotkeys::shift_delete_hotkey_purges_the_selection().await;
        hotkeys::space_hotkey_pauses_and_resumes_the_list().await;
        hotkeys::an_open_dialog_swallows_the_space_and_delete_hotkeys().await;

        rows::card_buttons_act_on_their_own_task().await;
        rows::table_row_buttons_act_on_their_own_task().await;
        rows::card_pause_and_resume_hit_their_own_row().await;
        rows::double_click_follows_the_configured_behavior().await;

        new_task::submitting_a_url_passes_the_dialog_state_to_the_engine().await;
        new_task::submitting_a_torrent_leaves_out_the_unchecked_files().await;
        new_task::submitting_a_batch_starts_one_task_per_link().await;
        new_task::a_deep_link_payload_reaches_the_dialog().await;
        new_task::a_magnet_payload_takes_its_name_from_dn().await;
        new_task::a_pasted_batch_payload_opens_in_batch_mode().await;
        new_task::a_payload_nothing_can_parse_still_opens_the_dialog().await;
        new_task::a_dropped_torrent_file_previews_and_survives_a_failed_parse().await;

        toolbar::pause_all_and_resume_all_follow_the_state_not_the_selection().await;
        toolbar::clear_completed_reports_nothing_to_do_and_surfaces_a_failed_clear().await;

        dialogs::a_rejected_save_keeps_the_dialog_open().await;
        dialogs::an_invalid_schedule_row_is_reported().await;
        dialogs::an_invalid_media_override_row_is_reported().await;
        dialogs::saving_settings_persists_the_edited_form().await;
        dialogs::saving_media_overrides_persists_the_rows().await;
        dialogs::saving_aria2_clients_persists_hashes_not_plaintext().await;
        dialogs::duplicate_aria2_client_names_are_rejected().await;
        dialogs::saving_labs_persists_the_rules().await;
        dialogs::the_setup_wizard_persists_its_form_and_remembers_where_it_was().await;
        dialogs::the_factory_reset_gate_disarms_when_the_dialog_closes().await;
    });
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
