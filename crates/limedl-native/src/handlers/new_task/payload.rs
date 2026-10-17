//! Entry point for "open with" payloads: CLI argument, `limedl://` deep link,
//! `magnet:` link, dropped `.torrent` file or a pasted list of URLs.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use slint::SharedString;

use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::TorrentFileEntry;

use crate::bridge::{self, ClipboardPayload, TaskStore};
use crate::handlers::common::with_ui;
use crate::handlers::new_task::{
    apply_aria2_input, fill_batch, preview_torrent, reset_transient_state, spawn_probe,
};
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

    let normalized = normalize_deep_link(payload);
    let normalized = normalized.trim();

    // A local path payload may arrive as a `file://` URL.
    let path_candidate = strip_file_url(normalized);
    let path = Path::new(&path_candidate);

    if has_extension(path, &["torrent"]) {
        open_torrent_file_payload(
            ui_weak,
            dispatcher,
            store,
            entries_cache,
            included_cache,
            path,
        );
        return;
    }

    // A metalink that fails to parse is not handled, so keep looking for another
    // interpretation of the payload.
    if has_extension(path, &["metalink", "meta4"]) && try_open_metalink(path, ui_weak, store) {
        return;
    }

    if normalized.starts_with("magnet:?") {
        open_magnet_dialog(normalized, ui_weak, store);
        return;
    }

    match bridge::parse_clipboard_download_text(normalized) {
        ClipboardPayload::BatchUrls(urls) => open_batch_dialog(&urls, ui_weak, store),
        ClipboardPayload::SingleUrl(url) => open_single_dialog(&url, ui_weak, dispatcher, store),
        ClipboardPayload::Empty => open_unnamed_dialog(normalized, ui_weak),
    }
}

/// Resolve the `limedl://` deep-link wrapper around a payload.
fn normalize_deep_link(payload: &str) -> String {
    let Some(stripped) = payload.strip_prefix("limedl://") else {
        return payload.to_string();
    };
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
}

/// Strip a `file://` / `file:///` prefix, leaving other payloads untouched.
fn strip_file_url(payload: &str) -> String {
    match payload
        .strip_prefix("file:///")
        .or_else(|| payload.strip_prefix("file://"))
    {
        Some(rest) => rest.to_string(),
        None => payload.to_string(),
    }
}

/// True when `path` is an existing file whose extension is one of `extensions`
/// (compared ASCII-case-insensitively).
fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|ext| {
                extensions
                    .iter()
                    .any(|wanted| ext.eq_ignore_ascii_case(wanted))
            })
}

/// Pre-fill the dialog from the first mirror list of a `.metalink`/`.meta4` file.
/// Returns `false` when the payload was not handled.
fn try_open_metalink(
    path: &Path,
    ui_weak: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(doc) = limedl_core::metalink::parse_metalink_xml(&content) else {
        return false;
    };
    let Some(file) = doc.files.into_iter().next() else {
        return false;
    };
    let Some(primary_url) = file.sorted_mirror_urls().first().cloned() else {
        return false;
    };
    let file_name = file.name;
    let ui_weak = ui_weak.clone();
    let store = store.clone();
    let _ = slint::invoke_from_event_loop(move || {
        with_ui(&ui_weak, |ui| {
            restore_and_show_window(&ui, Some(&store.lock()));
            ui.set_new_task_url(SharedString::from(&primary_url));
            ui.set_new_task_filename(SharedString::from(&file_name));
            ui.set_new_task_batch_mode(false);
            reset_transient_state(&ui);
            ui.set_show_new_task_dialog(true);
        });
    });
    true
}

/// Pre-fill the dialog for a magnet link, taking the display name from `dn=`.
fn open_magnet_dialog(
    normalized: &str,
    ui_weak: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
) {
    let magnet_url = normalized.to_string();
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
}

/// Fill the dialog with a pasted batch of URLs.
fn open_batch_dialog(
    urls: &[String],
    ui_weak: &slint::Weak<MainWindow>,
    store: &Arc<Mutex<TaskStore>>,
) {
    let ui_weak = ui_weak.clone();
    let store = store.clone();
    let urls = urls.to_vec();
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

/// Fill the dialog with a single pasted URL — unpacking an `aria2c` command line
/// into URL plus `--out` / `--dir` / `--header` rather than showing it verbatim —
/// and probe HTTP links for a checksum.
fn open_single_dialog(
    url: &str,
    ui_weak: &slint::Weak<MainWindow>,
    dispatcher: &Arc<Dispatcher>,
    store: &Arc<Mutex<TaskStore>>,
) {
    {
        let ui_weak = ui_weak.clone();
        let url = url.to_string();
        let store = store.clone();
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui_weak, |ui| {
                restore_and_show_window(&ui, Some(&store.lock()));
                ui.set_new_task_batch_mode(false);
                reset_transient_state(&ui);
                // A pasted aria2c command is unpacked into URL plus
                // `--out` / `--dir` / `--header` instead of being shown
                // verbatim as an unusable URL.
                let lang = store.lock().language();
                if !apply_aria2_input(&ui, &url, lang) {
                    ui.set_new_task_url(SharedString::from(&url));
                }
                ui.set_show_new_task_dialog(true);
            });
        });
    }

    // HTTP links get their checksum probed automatically.
    if url.starts_with("http://") || url.starts_with("https://") {
        spawn_probe(ui_weak, dispatcher, store, url.to_string(), None);
    }
}

/// Show the empty dialog with the raw payload as the URL field.
fn open_unnamed_dialog(normalized: &str, ui_weak: &slint::Weak<MainWindow>) {
    let payload = normalized.to_string();
    let ui_weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        with_ui(&ui_weak, |ui| {
            ui.set_new_task_url(SharedString::from(&payload));
            ui.set_new_task_batch_mode(false);
            ui.set_show_new_task_dialog(true);
        });
    });
}

/// Open the new-task dialog for a local `.torrent` payload and kick off the
/// torrent preview. Split out of [`open_new_task_with_payload`] to keep that
/// function's branching manageable.
fn open_torrent_file_payload(
    ui_weak: &slint::Weak<MainWindow>,
    dispatcher: &Arc<Dispatcher>,
    store: &Arc<Mutex<TaskStore>>,
    entries_cache: &Arc<Mutex<Vec<TorrentFileEntry>>>,
    included_cache: &Arc<Mutex<Vec<bool>>>,
    path: &std::path::Path,
) {
    let full_path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    let ui_for_ui = ui_weak.clone();
    let store_for_ui = store.clone();
    let path_for_ui = full_path.clone();
    let _ = slint::invoke_from_event_loop(move || {
        with_ui(&ui_for_ui, |ui| {
            restore_and_show_window(&ui, Some(&store_for_ui.lock()));
            ui.set_new_task_url(SharedString::from(&path_for_ui));
            if !file_name.is_empty() {
                ui.set_new_task_filename(SharedString::from(&file_name));
            }
            ui.set_new_task_batch_mode(false);
            ui.set_show_new_task_dialog(true);
        });
    });

    preview_torrent(
        ui_weak,
        dispatcher,
        store,
        entries_cache,
        included_cache,
        full_path,
    );
}
