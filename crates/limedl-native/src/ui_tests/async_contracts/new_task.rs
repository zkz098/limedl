//! The new-task dialog: what reaches the engine for a single URL, a torrent file
//! list and a pasted batch, plus the shared entry point every external caller
//! (CLI argument, `limedl://` deep link, dropped file, clipboard monitor) uses.

use limedl_core::types::ChecksumMode;

use super::*;

/// The submit path is where the dialog's collected state becomes a
/// `StartDownloadRequest`. Nothing else sees it: `bridge/`'s unit tests stop at
/// the form and `new_task.rs` only covers the reset contract.
pub(super) async fn submitting_a_url_passes_the_dialog_state_to_the_engine() {
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
pub(super) async fn submitting_a_torrent_leaves_out_the_unchecked_files() {
    let ui = new_window();
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
pub(super) async fn submitting_a_batch_starts_one_task_per_link() {
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

    let mut names: Vec<Option<String>> =
        starts.iter().map(|start| start.file_name.clone()).collect();
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

/// Hand a raw payload to the shared entry point the way every external caller
/// does (CLI argument, `limedl://`, drop, clipboard monitor) and let the queued
/// `invoke_from_event_loop` land.
async fn deliver_payload(ui: &TestUi, payload: &str) {
    crate::handlers::new_task::open_new_task_with_payload(
        payload,
        &ui.ctx.ui_weak,
        &ui.ctx.dispatcher,
        &ui.ctx.store,
        &ui.ctx.new_task_torrent_entries,
        &ui.ctx.new_task_torrent_included,
    );
    ui.pump(2).await;
}

/// The `limedl://` deep link the browser protocol handler and the MSIX launcher
/// pass in: the URL is percent-encoded and the dialog must show it decoded.
///
/// The payload also kicks off the automatic checksum probe, which is a real HTTP
/// request (`Dispatcher::probe_checksum` owns the client, not the backend), so
/// its outcome is not asserted here — `new_task.rs` pins the probe's own state
/// contract instead.
pub(super) async fn a_deep_link_payload_reaches_the_dialog() {
    let ui = new_window();
    deliver_payload(
        &ui,
        "limedl://download?url=https%3A%2F%2Fexample.invalid%2Fdeep.zip",
    )
    .await;

    assert!(ui.window.get_show_new_task_dialog());
    assert_eq!(
        ui.window.get_new_task_url().as_str(),
        "https://example.invalid/deep.zip",
        "the percent-encoded target must arrive decoded"
    );
    assert!(!ui.window.get_new_task_batch_mode());
    assert!(
        ui.window.get_new_task_filename().is_empty(),
        "a deep link carries no file name"
    );
}

/// A magnet link is the one payload that carries its own display name, and the
/// dialog has to unpack `dn=` into the file-name field (the engine would
/// otherwise show the 40-character info hash).
pub(super) async fn a_magnet_payload_takes_its_name_from_dn() {
    let ui = new_window();
    let magnet = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=My%20File.iso";
    deliver_payload(&ui, magnet).await;

    assert!(ui.window.get_show_new_task_dialog());
    assert_eq!(ui.window.get_new_task_url().as_str(), magnet);
    assert_eq!(
        ui.window.get_new_task_filename().as_str(),
        "My File.iso",
        "`dn=` is percent-decoded into the file-name field"
    );
    assert!(
        !ui.window.get_new_task_batch_mode(),
        "one magnet is not a batch"
    );
}

/// A pasted list of links opens the dialog in batch mode, with the counter the
/// submit path will use.
pub(super) async fn a_pasted_batch_payload_opens_in_batch_mode() {
    let ui = new_window();
    deliver_payload(
        &ui,
        "https://example.invalid/one.zip\n\n  https://example.invalid/two.zip  ",
    )
    .await;

    assert!(ui.window.get_show_new_task_dialog());
    assert!(ui.window.get_new_task_batch_mode());
    assert_eq!(
        ui.window.get_new_task_batch_text().as_str(),
        "https://example.invalid/one.zip\nhttps://example.invalid/two.zip",
        "the blank line and the surrounding spaces are not links"
    );
    assert!(!ui.window.get_new_task_batch_count_text().is_empty());
    assert!(
        ui.window.get_new_task_url().is_empty(),
        "batch mode leaves the single-URL field alone"
    );
}

/// Text nothing can parse still opens the dialog with the text in the URL field:
/// the user sees what the app received (and why it will not work) instead of
/// getting a silent no-op after dropping a file.
pub(super) async fn a_payload_nothing_can_parse_still_opens_the_dialog() {
    let ui = new_window();
    deliver_payload(&ui, "  hello world  ").await;

    assert!(ui.window.get_show_new_task_dialog());
    assert_eq!(ui.window.get_new_task_url().as_str(), "hello world");
    assert!(!ui.window.get_new_task_batch_mode());
}

/// A dropped `.torrent` file is recognized by extension, its path (not a URL)
/// lands in the dialog and the preview starts. The parse itself needs a BT
/// session, so the fixture's answer is the failure branch — which is exactly the
/// branch that must leave the dialog usable.
pub(super) async fn a_dropped_torrent_file_previews_and_survives_a_failed_parse() {
    let ui = new_window();
    let path = ui.ctx.base_dir.join("Sample.torrent");
    tokio::fs::write(&path, b"d4:infod4:name4:teste")
        .await
        .expect("write the dropped file");

    deliver_payload(&ui, &path.to_string_lossy()).await;

    assert!(ui.window.get_show_new_task_dialog());
    assert_eq!(
        ui.window.get_new_task_url().as_str(),
        path.canonicalize()
            .unwrap_or(path.clone())
            .to_string_lossy()
            .as_ref(),
        "the dialog shows the resolved path it will hand to the BT backend"
    );
    assert_eq!(ui.window.get_new_task_filename().as_str(), "Sample.torrent");
    assert!(!ui.window.get_new_task_batch_mode());

    ui.pump_until("the torrent preview to report the failed parse", || {
        ui.window.get_new_task_preview_state().as_str() == "error"
    })
    .await;
    assert_eq!(
        ui.window.get_new_task_preview_state().as_str(),
        "error",
        "a preview that cannot be parsed reports it instead of hanging on 'loading'"
    );
    assert!(
        !ui.window.get_new_task_preview_status_text().is_empty(),
        "…with the reason in the status line"
    );
    assert_eq!(
        ui.window.get_new_task_torrent_files().row_count(),
        0,
        "no half-filled file list is left behind"
    );
}
