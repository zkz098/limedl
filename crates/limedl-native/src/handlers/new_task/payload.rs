//! Entry point for "open with" payloads: CLI argument, `limedl://` deep link,
//! `magnet:` link, dropped `.torrent` file or a pasted list of URLs.

use std::sync::Arc;

use parking_lot::Mutex;
use slint::SharedString;

use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::TorrentFileEntry;

use crate::bridge::{self, ClipboardPayload, TaskStore};
use crate::handlers::common::with_ui;
use crate::handlers::new_task::{fill_batch, preview_torrent, reset_transient_state, spawn_probe};
use crate::ui_sync::restore_and_show_window;
use crate::MainWindow;

pub fn open_new_task_with_payload(
    raw_payload: &str,
    ui_weak: &slint::Weak<MainWindow>,
    dispatcher: &Arc<Dispatcher>,
    store: &Arc<Mutex<TaskStore>>,
    entries_cache: &Arc<Mutex<Vec<TorrentFileEntry>>>,
    included_cache: &Arc<Mutex<Vec<bool>>>,
) {
    let payload = raw_payload.trim().trim_matches('"').trim_matches('\'').trim();
    if payload.is_empty() {
        return;
    }

    // Check limedl:// deep link scheme
    let normalized = if let Some(stripped) = payload.strip_prefix("limedl://") {
        let after = stripped.trim_start_matches('/');
        if let Some(url_part) = after.strip_prefix("download?url=") {
            percent_encoding::percent_decode_str(url_part)
                .decode_utf8_lossy()
                .to_string()
        } else if let Some(magnet_part) = after.strip_prefix("magnet:") {
            format!("magnet:{magnet_part}")
        } else {
            after.to_string()
        }
    } else {
        payload.to_string()
    };

    let normalized = normalized.trim().to_string();

    // Check if it's a local .torrent file
    let path_candidate = if let Some(file_url) = normalized.strip_prefix("file:///") {
        file_url.to_string()
    } else if let Some(file_url) = normalized.strip_prefix("file://") {
        file_url.to_string()
    } else {
        normalized.clone()
    };

    let path = std::path::Path::new(&path_candidate);
    let is_torrent_file = path.is_file()
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("torrent"))
            .unwrap_or(false);

    if is_torrent_file {
        let full_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string();
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();

        {
            let ui_weak = ui_weak.clone();
            let store = store.clone();
            let path_for_ui = full_path.clone();
            let _ = slint::invoke_from_event_loop(move || {
                with_ui(&ui_weak, |ui| {
                    restore_and_show_window(&ui, Some(&store.lock()));
                    ui.set_new_task_url(SharedString::from(&path_for_ui));
                    if !file_name.is_empty() {
                        ui.set_new_task_filename(SharedString::from(&file_name));
                    }
                    ui.set_new_task_batch_mode(false);
                    ui.set_show_new_task_dialog(true);
                });
            });
        }

        preview_torrent(
            ui_weak,
            dispatcher,
            store,
            entries_cache,
            included_cache,
            full_path,
        );
        return;
    }

    // Check magnet link
    if normalized.starts_with("magnet:?") {
        let magnet_url = normalized.clone();
        let display_name = magnet_url
            .find("dn=")
            .map(|start| {
                let raw = magnet_url[start + 3..].split('&').next().unwrap_or("");
                percent_encoding::percent_decode_str(raw)
                    .decode_utf8_lossy()
                    .to_string()
            })
            .unwrap_or_default();

        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui_weak, |ui| {
                restore_and_show_window(&ui, Some(&store.lock()));
                ui.set_new_task_url(SharedString::from(&magnet_url));
                if !display_name.is_empty() {
                    ui.set_new_task_filename(SharedString::from(&display_name));
                }
                ui.set_new_task_batch_mode(false);
                reset_transient_state(&ui);
                ui.set_show_new_task_dialog(true);
            });
        });
        return;
    }

    // Check multiple lines or URLs via clipboard parser
    match bridge::parse_clipboard_download_text(&normalized) {
        ClipboardPayload::BatchUrls(urls) => {
            let ui_weak = ui_weak.clone();
            let store = store.clone();
            let _ = slint::invoke_from_event_loop(move || {
                with_ui(&ui_weak, |ui| {
                    restore_and_show_window(&ui, Some(&store.lock()));
                    let lang = store.lock().language();
                    fill_batch(&ui, &urls, lang);
                    ui.set_new_task_batch_submitting(false);
                    ui.set_new_task_batch_status_text(SharedString::default());
                    ui.set_show_new_task_dialog(true);
                });
            });
        }
        ClipboardPayload::SingleUrl(url) => {
            {
                let ui_weak = ui_weak.clone();
                let url = url.clone();
                let store = store.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    with_ui(&ui_weak, |ui| {
                        restore_and_show_window(&ui, Some(&store.lock()));
                        ui.set_new_task_url(SharedString::from(&url));
                        ui.set_new_task_batch_mode(false);
                        reset_transient_state(&ui);
                        ui.set_show_new_task_dialog(true);
                    });
                });
            }

            // HTTP links get their checksum probed automatically.
            if url.starts_with("http://") || url.starts_with("https://") {
                spawn_probe(ui_weak, dispatcher, store, url, None);
            }
        }
        ClipboardPayload::Empty => {
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                with_ui(&ui_weak, |ui| {
                    ui.set_new_task_url(SharedString::from(&normalized));
                    ui.set_new_task_batch_mode(false);
                    ui.set_show_new_task_dialog(true);
                });
            });
        }
    }
}
