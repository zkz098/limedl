use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use parking_lot::Mutex;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::{ChecksumMode, StartDownloadRequest, TorrentFileEntry};

use crate::bridge::{self, TaskStore, torrent_entry_to_item};
use crate::context::AppContext;
use crate::i18n;
use crate::toast::push_toast;
use crate::ui_sync::restore_and_show_window;
use crate::url_utils::{extract_batch_file_name, parse_batch_urls};
use crate::{MainWindow, NewTaskTorrentFileItem};

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

    let p = std::path::Path::new(&path_candidate);
    let is_torrent_file = p.is_file()
        && p.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("torrent"))
            .unwrap_or(false);

    if is_torrent_file {
        let full_path = p
            .canonicalize()
            .unwrap_or_else(|_| p.to_path_buf())
            .to_string_lossy()
            .to_string();
        let file_name = p
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        let ui_weak_cl = ui_weak.clone();
        let lang = store.lock().language();
        let path_for_ui = full_path.clone();
        let store_open = store.clone();

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak_cl.upgrade() {
                restore_and_show_window(&ui, Some(&store_open.lock()));
                ui.set_new_task_url(SharedString::from(&path_for_ui));
                if !file_name.is_empty() {
                    ui.set_new_task_filename(SharedString::from(&file_name));
                }
                ui.set_new_task_batch_mode(false);
                ui.set_new_task_preview_state("loading".into());
                ui.set_new_task_preview_status_text(SharedString::from(
                    i18n::format_preview_status("loading", "", lang),
                ));
                ui.set_new_task_preview_summary_text(SharedString::default());
                ui.set_new_task_torrent_files(ModelRc::default());
                ui.set_show_new_task_dialog(true);
            }
        });

        let dispatcher = dispatcher.clone();
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let entries_cache = entries_cache.clone();
        let included_cache = included_cache.clone();

        tokio::spawn(async move {
            let preview = dispatcher.bt_preview_torrent(&full_path).await;
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = ui_weak.upgrade() else {
                    return;
                };
                let lang = store.lock().language();
                match preview {
                    Ok(entries) => {
                        let total_bytes: u64 = entries.iter().map(|e| e.size).sum();
                        *entries_cache.lock() = entries.clone();
                        *included_cache.lock() = vec![true; entries.len()];
                        let items: Vec<NewTaskTorrentFileItem> = entries
                            .iter()
                            .map(|e| torrent_entry_to_item(e, true))
                            .collect();
                        ui.set_new_task_torrent_files(Rc::new(VecModel::from(items)).into());
                        ui.set_new_task_preview_state("ready".into());
                        ui.set_new_task_preview_status_text(SharedString::default());
                        ui.set_new_task_preview_summary_text(SharedString::from(
                            i18n::format_preview_summary(
                                entries.len(),
                                &bridge::format_bytes(total_bytes),
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
        return;
    }

    // Check magnet link
    if normalized.starts_with("magnet:?") {
        let magnet_url = normalized.clone();
        let mut display_name = String::new();
        if let Some(dn_start) = magnet_url.find("dn=") {
            let slice = &magnet_url[dn_start + 3..];
            let dn_raw = slice.split('&').next().unwrap_or("");
            display_name = percent_encoding::percent_decode_str(dn_raw)
                .decode_utf8_lossy()
                .to_string();
        }

        let ui_weak = ui_weak.clone();
        let store_magnet = store.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                restore_and_show_window(&ui, Some(&store_magnet.lock()));
                ui.set_new_task_url(SharedString::from(&magnet_url));
                if !display_name.is_empty() {
                    ui.set_new_task_filename(SharedString::from(&display_name));
                }
                ui.set_new_task_batch_mode(false);
                ui.set_new_task_probe_state("idle".into());
                ui.set_new_task_probe_status_text(SharedString::default());
                ui.set_new_task_probe_hash(SharedString::default());
                ui.set_new_task_preview_state("none".into());
                ui.set_new_task_preview_status_text(SharedString::default());
                ui.set_new_task_preview_summary_text(SharedString::default());
                ui.set_new_task_torrent_files(ModelRc::default());
                ui.set_show_new_task_dialog(true);
            }
        });
        return;
    }

    // Check multiple lines or URLs via clipboard parser
    match crate::bridge::parse_clipboard_download_text(&normalized) {
        crate::bridge::ClipboardPayload::BatchUrls(urls) => {
            let joined = urls.join("\n");
            let count = urls.len();
            let ui_weak = ui_weak.clone();
            let store = store.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    restore_and_show_window(&ui, Some(&store.lock()));
                    let lang = store.lock().language();
                    ui.set_new_task_batch_text(SharedString::from(&joined));
                    ui.set_new_task_batch_mode(true);
                    ui.set_new_task_batch_count_text(SharedString::from(
                        i18n::format_batch_count(count, lang),
                    ));
                    ui.set_new_task_batch_submitting(false);
                    ui.set_new_task_batch_status_text(SharedString::default());
                    ui.set_show_new_task_dialog(true);
                }
            });
        }
        crate::bridge::ClipboardPayload::SingleUrl(url) => {
            let ui_weak_cl = ui_weak.clone();
            let url_cl = url.clone();
            let store_single = store.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_cl.upgrade() {
                    restore_and_show_window(&ui, Some(&store_single.lock()));
                    ui.set_new_task_url(SharedString::from(&url_cl));
                    ui.set_new_task_batch_mode(false);
                    ui.set_new_task_probe_state("idle".into());
                    ui.set_new_task_probe_status_text(SharedString::default());
                    ui.set_new_task_probe_hash(SharedString::default());
                    ui.set_new_task_preview_state("none".into());
                    ui.set_new_task_preview_status_text(SharedString::default());
                    ui.set_new_task_preview_summary_text(SharedString::default());
                    ui.set_new_task_torrent_files(ModelRc::default());
                    ui.set_show_new_task_dialog(true);
                }
            });

            // If auto-detect sha256 or HTTP, probe checksum
            if url.starts_with("http://") || url.starts_with("https://") {
                let dispatcher = dispatcher.clone();
                let ui_weak = ui_weak.clone();
                let store = store.clone();
                tokio::spawn(async move {
                    let detected = dispatcher
                        .probe_checksum(&url, None)
                        .await
                        .ok()
                        .flatten();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
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
                        }
                    });
                });
            }
        }
        crate::bridge::ClipboardPayload::Empty => {
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_new_task_url(SharedString::from(&normalized));
                    ui.set_new_task_batch_mode(false);
                    ui.set_show_new_task_dialog(true);
                }
            });
        }
    }
}

pub fn register(ctx: &AppContext) {
    let main_window = &ctx.ui;
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let toast_queue = ctx.toast_queue.clone();
    let new_task_torrent_entries = ctx.new_task_torrent_entries.clone();
    let new_task_torrent_included = ctx.new_task_torrent_included.clone();

    // Open New Task Dialog
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_open_new_task_dialog(move || {
            if let Some(ui) = ui_weak.upgrade() {
                // Reset transient dialog state (probe / torrent preview / batch)
                let lang = store_clone.lock().language();
                ui.set_new_task_probe_state("idle".into());
                ui.set_new_task_probe_status_text(SharedString::default());
                ui.set_new_task_probe_hash(SharedString::default());
                ui.set_new_task_preview_state("none".into());
                ui.set_new_task_preview_status_text(SharedString::default());
                ui.set_new_task_preview_summary_text(SharedString::default());
                ui.set_new_task_torrent_files(ModelRc::default());
                ui.set_new_task_batch_mode(false);
                ui.set_new_task_batch_text(SharedString::default());
                ui.set_new_task_batch_count_text(
                    SharedString::from(i18n::format_batch_count(0, lang)),
                );
                ui.set_new_task_batch_submitting(false);
                ui.set_new_task_batch_status_text(SharedString::default());
                ui.set_show_new_task_dialog(true);

                // Clipboard auto-fill: if the URL field is still empty, read
                // the system clipboard and prefill single link or switch to batch mode.
                if ui.get_new_task_url().trim().is_empty() {
                    let ui_weak = ui_weak.clone();
                    let store_clone = store_clone.clone();
                    tokio::spawn(async move {
                        let Ok(mut clipboard) = arboard::Clipboard::new() else {
                            return;
                        };
                        let Ok(text) = clipboard.get_text() else {
                            return;
                        };
                        match crate::bridge::parse_clipboard_download_text(&text) {
                            crate::bridge::ClipboardPayload::SingleUrl(url) => {
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_weak.upgrade()
                                        && ui.get_new_task_url().trim().is_empty()
                                    {
                                        ui.set_new_task_url(SharedString::from(url));
                                        ui.set_new_task_batch_mode(false);
                                    }
                                });
                            }
                            crate::bridge::ClipboardPayload::BatchUrls(urls) => {
                                let joined = urls.join("\n");
                                let count = urls.len();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_weak.upgrade()
                                        && ui.get_new_task_url().trim().is_empty()
                                    {
                                        ui.set_new_task_batch_text(SharedString::from(&joined));
                                        ui.set_new_task_batch_mode(true);
                                        let lang = store_clone.lock().language();
                                        ui.set_new_task_batch_count_text(
                                            SharedString::from(i18n::format_batch_count(count, lang)),
                                        );
                                    }
                                });
                            }
                            crate::bridge::ClipboardPayload::Empty => {}
                        }
                    });
                }
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_close_new_task_dialog(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_new_task_dialog(false);
            }
        });
    }

    // New Task: reset checksum probe (dialog calls this whenever the URL is edited,
    // because a detected hash is only valid for the URL it was probed against)
    {
        let ui_weak = main_window.as_weak();
        main_window.on_reset_new_task_probe(move || {
            if let Some(ui) = ui_weak.upgrade()
                && ui.get_new_task_probe_state().as_str() != "idle"
            {
                ui.set_new_task_probe_state("idle".into());
                ui.set_new_task_probe_status_text(SharedString::default());
                ui.set_new_task_probe_hash(SharedString::default());
            }
        });
    }

    // New Task: probe .sha256 / SHA256SUMS for the entered HTTP link
    {
        let dispatcher = dispatcher.clone();
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_probe_new_task_checksum(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let url = ui.get_new_task_url().trim().to_string();
            if url.is_empty() {
                return;
            }
            let custom_name = ui.get_new_task_filename().trim().to_string();
            let file_name = (!custom_name.is_empty()).then_some(custom_name);
            let lang = store_clone.lock().language();
            if !url.starts_with("http://") && !url.starts_with("https://") {
                ui.set_new_task_probe_state("not_http".into());
                ui.set_new_task_probe_status_text(
                    SharedString::from(i18n::format_probe_status("not_http", "", lang)),
                );
                return;
            }
            ui.set_new_task_probe_state("probing".into());
            ui.set_new_task_probe_status_text(
                SharedString::from(i18n::format_probe_status("probing", "", lang)),
            );
            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            tokio::spawn(async move {
                let detected = dispatcher
                    .probe_checksum(&url, file_name.as_deref())
                    .await
                    .ok()
                    .flatten();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
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
                    }
                });
            });
        });
    }

    // New Task: torrent file pre-selection toggles
    {
        let ui_weak = main_window.as_weak();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();
        main_window.on_toggle_new_task_file(move |idx| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let mut included = included_cache.lock();
            let pos = idx.max(0) as usize;
            if let Some(flag) = included.get_mut(pos) {
                *flag = !*flag;
                let entries = entries_cache.lock();
                let items: Vec<NewTaskTorrentFileItem> = entries
                    .iter()
                    .zip(included.iter())
                    .map(|(e, inc)| torrent_entry_to_item(e, *inc))
                    .collect();
                drop(entries);
                drop(included);
                ui.set_new_task_torrent_files(Rc::new(VecModel::from(items)).into());
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();
        main_window.on_set_all_new_task_files(move |all| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let mut included = included_cache.lock();
            if included.is_empty() {
                return;
            }
            included.iter_mut().for_each(|flag| *flag = all);
            let entries = entries_cache.lock();
            let items: Vec<NewTaskTorrentFileItem> = entries
                .iter()
                .zip(included.iter())
                .map(|(e, inc)| torrent_entry_to_item(e, *inc))
                .collect();
            drop(entries);
            drop(included);
            ui.set_new_task_torrent_files(Rc::new(VecModel::from(items)).into());
        });
    }

    // Submit New Task (single mode: with checksum probe result and BT file selection)
    {
        let dispatcher = dispatcher.clone();
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();
        let toast_queue_clone = toast_queue.clone();
        main_window.on_submit_new_task(move |url, dir, filename| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let url_str = url.to_string();
            let dir_str = dir.to_string();
            let filename_opt = if filename.trim().is_empty() {
                None
            } else {
                Some(filename.trim().to_string())
            };

            // Detected checksum (cleared by the dialog whenever the URL changes,
            // so a `found` state here is always bound to the current URL)
            let (checksum, expected_checksum) = if ui.get_new_task_probe_state().as_str() == "found"
            {
                let hash = ui.get_new_task_probe_hash().to_string();
                (
                    (!hash.is_empty()).then_some(ChecksumMode::Sha256),
                    (!hash.is_empty()).then_some(hash),
                )
            } else {
                (None, None)
            };

            // BT file selection: None downloads everything; an empty selection
            // is rejected with a visible hint instead of a silent no-op task.
            let selected_file_indices = {
                let entries = entries_cache.lock();
                let included = included_cache.lock();
                if entries.is_empty() || ui.get_new_task_preview_state().as_str() != "ready" {
                    None
                } else {
                    let chosen: Vec<usize> = entries
                        .iter()
                        .zip(included.iter())
                        .filter(|(_, inc)| **inc)
                        .map(|(e, _)| e.index)
                        .collect();
                    if chosen.len() == entries.len() {
                        None
                    } else {
                        Some(chosen)
                    }
                }
            };
            if selected_file_indices.as_ref().is_some_and(Vec::is_empty) {
                let lang = store_clone.lock().language();
                ui.set_new_task_preview_status_text(SharedString::from(
                    i18n::no_files_selected_text(lang),
                ));
                return;
            }

            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let toast_queue_clone = toast_queue_clone.clone();
            tokio::spawn(async move {
                let req = StartDownloadRequest {
                    url: url_str,
                    destination_dir: dir_str,
                    file_name: filename_opt,
                    checksum,
                    expected_checksum,
                    selected_file_indices,
                    ..Default::default()
                };

                let file_name = req.file_name.clone().unwrap_or_default();
                match dispatcher.start(req).await {
                    Ok(task_id) => {
                        tracing::info!("成功添加下载任务: {task_id}");
                        let lang = store_clone.lock().language();
                        let display_name = if file_name.is_empty() {
                            i18n::format_unnamed_task(lang)
                        } else {
                            file_name.as_str()
                        };
                        push_toast(
                            &ui_weak,
                            &toast_queue_clone,
                            i18n::format_toast_task_added(display_name, lang),
                            "success",
                            Duration::from_secs(4),
                        );
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_new_task_url(SharedString::default());
                                ui.set_new_task_filename(SharedString::default());
                                ui.set_show_new_task_dialog(false);
                            }
                        });
                    }
                    Err(err) => {
                        tracing::error!("添加下载任务失败: {err}");
                        let lang = store_clone.lock().language();
                        push_toast(
                            &ui_weak,
                            &toast_queue_clone,
                            i18n::format_toast_task_add_failed(&format!("{err}"), lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    // New Task: batch mode — live link count under the textarea
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_new_task_batch_text_changed(move |text| {
            if let Some(ui) = ui_weak.upgrade() {
                let count = parse_batch_urls(&text).len();
                let lang = store_clone.lock().language();
                ui.set_new_task_batch_count_text(SharedString::from(i18n::format_batch_count(
                    count, lang,
                )));
            }
        });
    }

    // New Task: batch mode — submit all parsed links concurrently
    {
        let dispatcher = dispatcher.clone();
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        main_window.on_submit_new_task_batch(move |text, dir| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let urls = parse_batch_urls(&text);
            let dir_str = dir.to_string();
            let lang = store_clone.lock().language();
            if urls.is_empty() {
                ui.set_new_task_batch_status_text(SharedString::from(i18n::format_batch_count(
                    0, lang,
                )));
                return;
            }
            ui.set_new_task_batch_submitting(true);
            ui.set_new_task_batch_status_text(SharedString::from(i18n::format_batch_status(
                0,
                urls.len(),
                lang,
            )));

            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            let toast_queue_clone = toast_queue_clone.clone();
            tokio::spawn(async move {
                let total = urls.len();
                let done = Arc::new(AtomicUsize::new(0));
                let ok_count = Arc::new(AtomicUsize::new(0));
                for entry_url in urls {
                    let dispatcher = dispatcher.clone();
                    let ui_weak = ui_weak.clone();
                    let toast_queue = toast_queue_clone.clone();
                    let done = done.clone();
                    let ok_count = ok_count.clone();
                    let dir_str = dir_str.clone();
                    let file_name = extract_batch_file_name(&entry_url);
                    tokio::spawn(async move {
                        let req = StartDownloadRequest {
                            url: entry_url,
                            destination_dir: dir_str,
                            file_name,
                            ..Default::default()
                        };
                        if dispatcher.start(req).await.is_ok() {
                            ok_count.fetch_add(1, Ordering::Relaxed);
                        }
                        let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if finished >= total {
                            let succeeded = ok_count.load(Ordering::Relaxed);
                            // In-app toast on the final result (best effort).
                            push_toast(
                                &ui_weak,
                                &toast_queue,
                                i18n::format_toast_batch_done(succeeded, total, lang),
                                if succeeded == total { "success" } else { "warning" },
                                Duration::from_secs(5),
                            );
                        }
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                if finished < total {
                                    ui.set_new_task_batch_status_text(SharedString::from(
                                        i18n::format_batch_status(finished, total, lang),
                                    ));
                                } else {
                                    let succeeded = ok_count.load(Ordering::Relaxed);
                                    ui.set_new_task_batch_submitting(false);
                                    ui.set_new_task_batch_status_text(SharedString::from(
                                        i18n::format_batch_status(succeeded, total, lang),
                                    ));
                                    if succeeded == total {
                                        ui.set_show_new_task_dialog(false);
                                    }
                                }
                            }
                        });
                    });
                }
            });
        });
    }

    // Pick Torrent File (Native Dialog) + start file pre-selection preview
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();
        main_window.on_pick_torrent_file(move || {
            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let entries_cache = entries_cache.clone();
            let included_cache = included_cache.clone();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                let file = rfd::AsyncFileDialog::new()
                    .add_filter(i18n::pick_torrent_filter(lang), &["torrent", "TORRENT"])
                    .set_title(i18n::pick_torrent_title(lang))
                    .pick_file()
                    .await;

                if let Some(handle) = file {
                    let path = handle.path().to_string_lossy().to_string();
                    let file_name = handle.file_name();
                    let lang = store_clone.lock().language();
                    let path_for_ui = path.clone();
                    let ui_weak_first = ui_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak_first.upgrade() {
                            ui.set_new_task_url(SharedString::from(&path_for_ui));
                            if ui.get_new_task_filename().trim().is_empty() {
                                ui.set_new_task_filename(SharedString::from(&file_name));
                            }
                            // Torrent pre-selection: parse the file list now
                            ui.set_new_task_preview_state("loading".into());
                            ui.set_new_task_preview_status_text(SharedString::from(
                                i18n::format_preview_status("loading", "", lang),
                            ));
                            ui.set_new_task_preview_summary_text(SharedString::default());
                            ui.set_new_task_torrent_files(ModelRc::default());
                        }
                    });

                    let preview = dispatcher.bt_preview_torrent(&path).await;
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_weak.upgrade() else {
                            return;
                        };
                        match preview {
                            Ok(entries) => {
                                let total_bytes: u64 = entries.iter().map(|e| e.size).sum();
                                *entries_cache.lock() = entries.clone();
                                *included_cache.lock() = vec![true; entries.len()];
                                let items: Vec<NewTaskTorrentFileItem> = entries
                                    .iter()
                                    .map(|e| torrent_entry_to_item(e, true))
                                    .collect();
                                ui.set_new_task_torrent_files(
                                    Rc::new(VecModel::from(items)).into(),
                                );
                                ui.set_new_task_preview_state("ready".into());
                                ui.set_new_task_preview_status_text(SharedString::default());
                                ui.set_new_task_preview_summary_text(SharedString::from(
                                    i18n::format_preview_summary(
                                        entries.len(),
                                        &bridge::format_bytes(total_bytes),
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
                }
            });
        });
    }

    // Pick Save Folder (Native Dialog)
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_pick_save_folder(move || {
            let ui_weak = ui_weak.clone();
            let store_clone = store_clone.clone();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                let folder = rfd::AsyncFileDialog::new()
                    .set_title(i18n::pick_download_dir_title(lang))
                    .pick_folder()
                    .await;

                if let Some(handle) = folder {
                    let path = handle.path().to_string_lossy().to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_new_task_dir(SharedString::from(&path));
                        }
                    });
                }
            });
        });
    }
}
