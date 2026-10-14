//! New-task dialog callbacks.
//!
//! `register` used to be a single ~550-line function plus a ~250-line payload
//! entry point; both are now split by concern, with the flows shared between
//! them (torrent preview, checksum probe, dialog reset) living here.

mod dialog;
mod payload;
mod submit;

use std::rc::Rc;
use std::sync::Arc;

use parking_lot::Mutex;
use slint::{ModelRc, SharedString, VecModel};

use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::{ChecksumMode, TorrentFileEntry};

use crate::aria2_cli::Aria2Command;
use crate::bridge::{TaskStore, torrent_entry_to_item};
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::i18n::{self, Language};
use crate::{MainWindow, NewTaskTorrentFileItem};

pub use payload::open_new_task_with_payload;

/// Wire every new-task callback onto the main window.
pub fn register(ctx: &AppContext) {
    dialog::register(ctx);
    submit::register(ctx);
}

/// Clear the transient probe / torrent-preview state. Every URL owns its own
/// detected hash, so this runs whenever the dialog is opened or its target
/// changes.
pub(super) fn reset_transient_state(ui: &MainWindow) {
    ui.set_new_task_probe_state("idle".into());
    ui.set_new_task_probe_status_text(SharedString::default());
    ui.set_new_task_probe_hash(SharedString::default());
    ui.set_new_task_checksum(SharedString::default());
    ui.set_new_task_headers(SharedString::default());
    ui.set_new_task_aria2_hint(SharedString::default());
    ui.set_new_task_preview_state("none".into());
    ui.set_new_task_preview_status_text(SharedString::default());
    ui.set_new_task_preview_summary_text(SharedString::default());
    ui.set_new_task_torrent_files(ModelRc::default());
}

/// Apply a parsed `aria2c` command to the dialog, if `text` is one.
///
/// Returns `false` when `text` is not an aria2 command (a bare URL, a magnet
/// link, an arbitrary paste), in which case the caller may still want to clear
/// the stale aria2-derived state.
pub(super) fn apply_aria2_input(ui: &MainWindow, text: &str, lang: Language) -> bool {
    match crate::aria2_cli::parse(text) {
        Some(command) => {
            apply_aria2_command(ui, &command, lang);
            true
        }
        None => false,
    }
}

/// Fill the dialog's visible fields from a parsed command.
///
/// The URL field keeps the raw command text (that is what the user pasted and
/// can keep editing); the target and mirrors are re-derived from it at submit
/// time by [`crate::handlers::new_task::submit`].
fn apply_aria2_command(ui: &MainWindow, command: &Aria2Command, lang: Language) {
    if let Some(out) = &command.out {
        ui.set_new_task_filename(SharedString::from(out.clone()));
    }
    if let Some(dir) = &command.dir {
        ui.set_new_task_dir(SharedString::from(dir.clone()));
    }
    if !command.headers.is_empty() {
        ui.set_new_task_headers(SharedString::from(command.headers.join("\n")));
    }
    if let Some((mode, hash)) = &command.checksum {
        ui.set_new_task_checksum(SharedString::from(checksum_field_value(*mode, hash)));
    }
    // A single-task dialog can only start one download, so several targets
    // become mirrors. `-Z` asked for separate downloads, which this field cannot
    // express; say so instead of silently changing the meaning.
    let mut ignored = command.unsupported.clone();
    if command.force_sequential && command.urls.len() > 1 {
        ignored.push("--force-sequential".to_string());
    }
    ui.set_new_task_aria2_hint(SharedString::from(i18n::format_aria2_hint(
        command.urls.len(),
        command.headers.len(),
        &ignored,
        lang,
    )));
}

/// Render a parsed digest the way [`crate::handlers::new_task::submit`] reads
/// it back (an explicit `algo:` prefix), so a 64-hex BLAKE3 is not mistaken for
/// a SHA-256.
fn checksum_field_value(mode: ChecksumMode, hash: &str) -> String {
    match mode {
        ChecksumMode::Sha256 => format!("sha256:{hash}"),
        ChecksumMode::Sha512 => format!("sha512:{hash}"),
        ChecksumMode::Blake3 => format!("blake3:{hash}"),
        ChecksumMode::None => hash.to_string(),
    }
}

/// Rebuild the torrent file-selection model from the cached entries.
pub(super) fn push_torrent_items(
    ui: &MainWindow,
    entries: &[TorrentFileEntry],
    included: &[bool],
) {
    let items: Vec<NewTaskTorrentFileItem> = entries
        .iter()
        .zip(included.iter())
        .map(|(entry, included)| torrent_entry_to_item(entry, *included))
        .collect();
    ui.set_new_task_torrent_files(Rc::new(VecModel::from(items)).into());
}

/// Switch the dialog into batch mode with `urls` pre-filled. Each entry may be
/// a plain link or an `aria2c` command, so the task count is the number of
/// parsed entries rather than the number of lines.
pub(super) fn fill_batch(ui: &MainWindow, urls: &[String], lang: Language) {
    let text = urls.join("\n");
    let count = crate::url_utils::parse_batch_entries(&text).len();
    ui.set_new_task_batch_text(SharedString::from(text));
    ui.set_new_task_batch_mode(true);
    ui.set_new_task_batch_count_text(SharedString::from(i18n::format_batch_count(count, lang)));
}

/// Parse a `.torrent` file and push its file list into the dialog.
///
/// Shows the loading state first so the dialog never keeps a stale list while
/// the (potentially slow) parse runs, then replaces it with the result.
pub(super) fn preview_torrent(
    ui: &slint::Weak<MainWindow>,
    dispatcher: &Arc<Dispatcher>,
    store: &Arc<Mutex<TaskStore>>,
    entries_cache: &Arc<Mutex<Vec<TorrentFileEntry>>>,
    included_cache: &Arc<Mutex<Vec<bool>>>,
    path: String,
) {
    let lang = store.lock().language();
    {
        let ui = ui.clone();
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui, |ui| {
                ui.set_new_task_preview_state("loading".into());
                ui.set_new_task_preview_status_text(SharedString::from(
                    i18n::format_preview_status("loading", "", lang),
                ));
                ui.set_new_task_preview_summary_text(SharedString::default());
                ui.set_new_task_torrent_files(ModelRc::default());
            });
        });
    }

    let ui = ui.clone();
    let dispatcher = dispatcher.clone();
    let store = store.clone();
    let entries_cache = entries_cache.clone();
    let included_cache = included_cache.clone();
    tokio::spawn(async move {
        let preview = dispatcher.bt_preview_torrent(&path).await;
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui, |ui| {
                let lang = store.lock().language();
                match preview {
                    Ok(entries) => {
                        let total_bytes: u64 = entries.iter().map(|entry| entry.size).sum();
                        *entries_cache.lock() = entries.clone();
                        *included_cache.lock() = vec![true; entries.len()];
                        push_torrent_items(&ui, &entries, &vec![true; entries.len()]);
                        ui.set_new_task_preview_state("ready".into());
                        ui.set_new_task_preview_status_text(SharedString::default());
                        ui.set_new_task_preview_summary_text(SharedString::from(
                            i18n::format_preview_summary(
                                entries.len(),
                                &crate::bridge::format_bytes(total_bytes),
                                lang,
                            ),
                        ));
                    }
                    Err(err) => {
                        entries_cache.lock().clear();
                        included_cache.lock().clear();
                        ui.set_new_task_torrent_files(ModelRc::default());
                        ui.set_new_task_preview_state("error".into());
                        ui.set_new_task_preview_status_text(SharedString::from(
                            i18n::format_preview_status("error", &err.to_string(), lang),
                        ));
                    }
                }
            });
        });
    });
}

/// Probe a `.sha256` / `SHA256SUMS` companion for `url` and push the outcome
/// into the dialog.
pub(super) fn spawn_probe(
    ui: &slint::Weak<MainWindow>,
    dispatcher: &Arc<Dispatcher>,
    store: &Arc<Mutex<TaskStore>>,
    url: String,
    file_name: Option<String>,
) {
    let ui = ui.clone();
    let dispatcher = dispatcher.clone();
    let store = store.clone();
    tokio::spawn(async move {
        let detected = dispatcher
            .probe_checksum(&url, file_name.as_deref())
            .await
            .ok()
            .flatten();
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui, |ui| {
                let lang = store.lock().language();
                match detected {
                    Some(hash) => {
                        ui.set_new_task_probe_state("found".into());
                        ui.set_new_task_probe_status_text(SharedString::from(
                            i18n::format_probe_status("found", &hash, lang),
                        ));
                        ui.set_new_task_probe_hash(SharedString::from(&hash));
                        ui.set_new_task_checksum(SharedString::from(hash));
                    }
                    None => {
                        ui.set_new_task_probe_state("missing".into());
                        ui.set_new_task_probe_status_text(SharedString::from(
                            i18n::format_probe_status("missing", "", lang),
                        ));
                    }
                }
            });
        });
    });
}
