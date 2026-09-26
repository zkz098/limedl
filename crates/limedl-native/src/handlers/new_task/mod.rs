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
use limedl_core::types::TorrentFileEntry;

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
    ui.set_new_task_preview_state("none".into());
    ui.set_new_task_preview_status_text(SharedString::default());
    ui.set_new_task_preview_summary_text(SharedString::default());
    ui.set_new_task_torrent_files(ModelRc::default());
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

/// Switch the dialog into batch mode with `urls` pre-filled.
pub(super) fn fill_batch(ui: &MainWindow, urls: &[String], lang: Language) {
    ui.set_new_task_batch_text(SharedString::from(urls.join("\n")));
    ui.set_new_task_batch_mode(true);
    ui.set_new_task_batch_count_text(SharedString::from(i18n::format_batch_count(urls.len(), lang)));
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
                        ui.set_new_task_probe_hash(SharedString::from(hash));
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
