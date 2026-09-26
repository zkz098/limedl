//! New-task dialog lifecycle: open / reset, checksum probe, torrent file
//! selection and the native file pickers.

use std::sync::Arc;

use parking_lot::Mutex;
use slint::SharedString;

use limedl_core::types::TorrentFileEntry;

use crate::bridge::{self, ClipboardPayload};
use crate::context::AppContext;
use crate::handlers::common::{read_ui, with_ui};
use crate::handlers::new_task::{
    preview_torrent, push_torrent_items, reset_transient_state, spawn_probe,
};
use crate::i18n;
use crate::MainWindow;

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let entries_cache = ctx.new_task_torrent_entries.clone();
    let included_cache = ctx.new_task_torrent_included.clone();

    // Open New Task Dialog
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_open_new_task_dialog(move || {
            with_ui(&ui_weak, |ui| {
                // Reset transient dialog state (probe / torrent preview / batch)
                let lang = store.lock().language();
                reset_transient_state(&ui);
                ui.set_new_task_batch_mode(false);
                ui.set_new_task_batch_text(SharedString::default());
                ui.set_new_task_batch_count_text(SharedString::from(i18n::format_batch_count(
                    0, lang,
                )));
                ui.set_new_task_batch_submitting(false);
                ui.set_new_task_batch_status_text(SharedString::default());
                ui.set_show_new_task_dialog(true);

                // Clipboard auto-fill: if the URL field is still empty, read
                // the system clipboard and prefill single link or switch to
                // batch mode.
                if ui.get_new_task_url().trim().is_empty() {
                    let ui_weak = ui_weak.clone();
                    let store = store.clone();
                    tokio::spawn(async move {
                        let Ok(mut clipboard) = arboard::Clipboard::new() else {
                            return;
                        };
                        let Ok(text) = clipboard.get_text() else {
                            return;
                        };
                        match bridge::parse_clipboard_download_text(&text) {
                            ClipboardPayload::SingleUrl(url) => {
                                let _ = slint::invoke_from_event_loop(move || {
                                    with_ui(&ui_weak, |ui| {
                                        if ui.get_new_task_url().trim().is_empty() {
                                            ui.set_new_task_url(SharedString::from(url));
                                            ui.set_new_task_batch_mode(false);
                                        }
                                    });
                                });
                            }
                            ClipboardPayload::BatchUrls(urls) => {
                                let _ = slint::invoke_from_event_loop(move || {
                                    with_ui(&ui_weak, |ui| {
                                        if ui.get_new_task_url().trim().is_empty() {
                                            let lang = store.lock().language();
                                            crate::handlers::new_task::fill_batch(&ui, &urls, lang);
                                        }
                                    });
                                });
                            }
                            ClipboardPayload::Empty => {}
                        }
                    });
                }
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        ui.on_close_new_task_dialog(move || {
            with_ui(&ui_weak, |ui| ui.set_show_new_task_dialog(false));
        });
    }

    // Reset the checksum probe: the dialog calls this whenever the URL is
    // edited, because a detected hash is only valid for the URL it was probed
    // against.
    {
        let ui_weak = ui_weak.clone();
        ui.on_reset_new_task_probe(move || {
            with_ui(&ui_weak, |ui| {
                if ui.get_new_task_probe_state().as_str() != "idle" {
                    ui.set_new_task_probe_state("idle".into());
                    ui.set_new_task_probe_status_text(SharedString::default());
                    ui.set_new_task_probe_hash(SharedString::default());
                }
            });
        });
    }

    // Manual probe of `.sha256` / `SHA256SUMS` for the entered HTTP link
    {
        let ui_weak = ui_weak.clone();
        let dispatcher = dispatcher.clone();
        let store = store.clone();
        ui.on_probe_new_task_checksum(move || {
            let Some((url, file_name)) = read_ui(&ui_weak, |ui| {
                let url = ui.get_new_task_url().trim().to_string();
                if url.is_empty() {
                    return None;
                }
                let custom_name = ui.get_new_task_filename().trim().to_string();
                let file_name = (!custom_name.is_empty()).then_some(custom_name);
                let lang = store.lock().language();
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    ui.set_new_task_probe_state("not_http".into());
                    ui.set_new_task_probe_status_text(SharedString::from(
                        i18n::format_probe_status("not_http", "", lang),
                    ));
                    return None;
                }
                ui.set_new_task_probe_state("probing".into());
                ui.set_new_task_probe_status_text(SharedString::from(
                    i18n::format_probe_status("probing", "", lang),
                ));
                Some((url, file_name))
            })
            .flatten() else {
                return;
            };
            spawn_probe(&ui_weak, &dispatcher, &store, url, file_name);
        });
    }

    register_file_selection(ui, &ui_weak, &entries_cache, &included_cache);

    // Pick Torrent File (Native Dialog) + start file pre-selection preview
    {
        let ui_weak = ui_weak.clone();
        let dispatcher = dispatcher.clone();
        let store = store.clone();
        let entries_cache = entries_cache.clone();
        let included_cache = included_cache.clone();
        ui.on_pick_torrent_file(move || {
            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            let store = store.clone();
            let entries_cache = entries_cache.clone();
            let included_cache = included_cache.clone();
            tokio::spawn(async move {
                let lang = store.lock().language();
                let file = rfd::AsyncFileDialog::new()
                    .add_filter(i18n::pick_torrent_filter(lang), &["torrent", "TORRENT"])
                    .set_title(i18n::pick_torrent_title(lang))
                    .pick_file()
                    .await;

                let Some(handle) = file else {
                    return;
                };
                let path = handle.path().to_string_lossy().to_string();
                let file_name = handle.file_name();
                {
                    let ui_weak = ui_weak.clone();
                    let path_for_ui = path.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        with_ui(&ui_weak, |ui| {
                            ui.set_new_task_url(SharedString::from(&path_for_ui));
                            if ui.get_new_task_filename().trim().is_empty() {
                                ui.set_new_task_filename(SharedString::from(&file_name));
                            }
                        });
                    });
                }

                preview_torrent(
                    &ui_weak,
                    &dispatcher,
                    &store,
                    &entries_cache,
                    &included_cache,
                    path,
                );
            });
        });
    }

    // Pick Save Folder (Native Dialog)
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_pick_save_folder(move || {
            let ui_weak = ui_weak.clone();
            let store = store.clone();
            tokio::spawn(async move {
                let lang = store.lock().language();
                let folder = rfd::AsyncFileDialog::new()
                    .set_title(i18n::pick_download_dir_title(lang))
                    .pick_folder()
                    .await;

                let Some(handle) = folder else {
                    return;
                };
                let path = handle.path().to_string_lossy().to_string();
                let _ = slint::invoke_from_event_loop(move || {
                    with_ui(&ui_weak, |ui| {
                        ui.set_new_task_dir(SharedString::from(&path));
                    });
                });
            });
        });
    }
}

/// Torrent file checkboxes inside the dialog.
fn register_file_selection(
    ui: &MainWindow,
    ui_weak: &slint::Weak<MainWindow>,
    entries_cache: &Arc<Mutex<Vec<TorrentFileEntry>>>,
    included_cache: &Arc<Mutex<Vec<bool>>>,
) {
    {
        let ui_weak = ui_weak.clone();
        let entries_cache = entries_cache.clone();
        let included_cache = included_cache.clone();
        ui.on_toggle_new_task_file(move |idx| {
            with_ui(&ui_weak, |ui| {
                let mut included = included_cache.lock();
                let pos = idx.max(0) as usize;
                let Some(flag) = included.get_mut(pos) else {
                    return;
                };
                *flag = !*flag;
                let entries = entries_cache.lock();
                push_torrent_items(&ui, &entries, &included);
            });
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let entries_cache = entries_cache.clone();
        let included_cache = included_cache.clone();
        ui.on_set_all_new_task_files(move |all| {
            with_ui(&ui_weak, |ui| {
                let mut included = included_cache.lock();
                if included.is_empty() {
                    return;
                }
                included.iter_mut().for_each(|flag| *flag = all);
                let entries = entries_cache.lock();
                push_torrent_items(&ui, &entries, &included);
            });
        });
    }
}
