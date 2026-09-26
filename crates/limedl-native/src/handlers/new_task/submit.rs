//! Submitting new tasks: single URL (with checksum + BT file selection) and
//! batch mode.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use slint::SharedString;

use limedl_core::types::{ChecksumMode, StartDownloadRequest};

use crate::context::AppContext;
use crate::handlers::common::{read_ui, with_ui};
use crate::i18n;
use crate::toast::push_toast;
use crate::url_utils::{extract_batch_file_name, parse_batch_urls};

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let toast_queue = ctx.toast_queue.clone();

    // Single mode: submit with the probed checksum and the BT file selection
    {
        let ui_weak = ui_weak.clone();
        let dispatcher = dispatcher.clone();
        let store = store.clone();
        let entries_cache = ctx.new_task_torrent_entries.clone();
        let included_cache = ctx.new_task_torrent_included.clone();
        let toast_queue = toast_queue.clone();
        ui.on_submit_new_task(move |url, dir, filename| {
            let Some(payload) = read_ui(&ui_weak, |ui| {
                let url = url.to_string();
                let dir = dir.to_string();
                let file_name = if filename.trim().is_empty() {
                    None
                } else {
                    Some(filename.trim().to_string())
                };

                // Detected checksum (cleared by the dialog whenever the URL
                // changes, so a `found` state here is always bound to the
                // current URL).
                let (checksum, expected_checksum) =
                    if ui.get_new_task_probe_state().as_str() == "found" {
                        let hash = ui.get_new_task_probe_hash().to_string();
                        (
                            (!hash.is_empty()).then_some(ChecksumMode::Sha256),
                            (!hash.is_empty()).then_some(hash),
                        )
                    } else {
                        (None, None)
                    };

                // BT file selection: None downloads everything; an empty
                // selection is rejected with a visible hint instead of a silent
                // no-op task.
                let selected_file_indices = {
                    let entries = entries_cache.lock();
                    let included = included_cache.lock();
                    if entries.is_empty() || ui.get_new_task_preview_state().as_str() != "ready" {
                        None
                    } else {
                        let chosen: Vec<usize> = entries
                            .iter()
                            .zip(included.iter())
                            .filter(|(_, included)| **included)
                            .map(|(entry, _)| entry.index)
                            .collect();
                        if chosen.len() == entries.len() {
                            None
                        } else {
                            Some(chosen)
                        }
                    }
                };
                if selected_file_indices.as_ref().is_some_and(Vec::is_empty) {
                    let lang = store.lock().language();
                    ui.set_new_task_preview_status_text(SharedString::from(
                        i18n::no_files_selected_text(lang),
                    ));
                    return None;
                }

                Some((url, dir, file_name, checksum, expected_checksum, selected_file_indices))
            })
            .flatten() else {
                return;
            };

            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            let store = store.clone();
            let toast_queue = toast_queue.clone();
            tokio::spawn(async move {
                let (url, destination_dir, file_name, checksum, expected_checksum, selected_file_indices) = payload;
                let request = StartDownloadRequest {
                    url,
                    destination_dir,
                    file_name,
                    checksum,
                    expected_checksum,
                    selected_file_indices,
                    ..Default::default()
                };

                let display_name = request.file_name.clone().unwrap_or_default();
                match dispatcher.start(request).await {
                    Ok(task_id) => {
                        tracing::info!("成功添加下载任务: {task_id}");
                        let lang = store.lock().language();
                        let display_name = if display_name.is_empty() {
                            i18n::format_unnamed_task(lang)
                        } else {
                            display_name.as_str()
                        };
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_task_added(display_name, lang),
                            "success",
                            Duration::from_secs(4),
                        );
                        let _ = slint::invoke_from_event_loop(move || {
                            with_ui(&ui_weak, |ui| {
                                ui.set_new_task_url(SharedString::default());
                                ui.set_new_task_filename(SharedString::default());
                                ui.set_show_new_task_dialog(false);
                            });
                        });
                    }
                    Err(err) => {
                        tracing::error!("添加下载任务失败: {err}");
                        let lang = store.lock().language();
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_task_add_failed(&format!("{err}"), lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    // Batch mode: live link count under the textarea
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        ui.on_new_task_batch_text_changed(move |text| {
            let count = parse_batch_urls(&text).len();
            with_ui(&ui_weak, |ui| {
                let lang = store.lock().language();
                ui.set_new_task_batch_count_text(SharedString::from(i18n::format_batch_count(
                    count, lang,
                )));
            });
        });
    }

    // Batch mode: submit all parsed links concurrently
    {
        let ui_weak = ui_weak.clone();
        let dispatcher = dispatcher.clone();
        let store = store.clone();
        let toast_queue = toast_queue.clone();
        ui.on_submit_new_task_batch(move |text, dir| {
            let Some((urls, dir, lang)) = read_ui(&ui_weak, |ui| {
                let urls = parse_batch_urls(&text);
                let dir = dir.to_string();
                let lang = store.lock().language();
                if urls.is_empty() {
                    ui.set_new_task_batch_status_text(SharedString::from(
                        i18n::format_batch_count(0, lang),
                    ));
                    return None;
                }
                ui.set_new_task_batch_submitting(true);
                ui.set_new_task_batch_status_text(SharedString::from(
                    i18n::format_batch_status(0, urls.len(), lang),
                ));
                Some((urls, dir, lang))
            })
            .flatten() else {
                return;
            };

            let ui_weak = ui_weak.clone();
            let dispatcher = dispatcher.clone();
            let toast_queue = toast_queue.clone();
            tokio::spawn(async move {
                let total = urls.len();
                let done = Arc::new(AtomicUsize::new(0));
                let ok_count = Arc::new(AtomicUsize::new(0));
                for entry_url in urls {
                    let dispatcher = dispatcher.clone();
                    let ui_weak = ui_weak.clone();
                    let toast_queue = toast_queue.clone();
                    let done = done.clone();
                    let ok_count = ok_count.clone();
                    let dir = dir.clone();
                    let file_name = extract_batch_file_name(&entry_url);
                    tokio::spawn(async move {
                        let request = StartDownloadRequest {
                            url: entry_url,
                            destination_dir: dir,
                            file_name,
                            ..Default::default()
                        };
                        if dispatcher.start(request).await.is_ok() {
                            ok_count.fetch_add(1, Ordering::Relaxed);
                        }
                        let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if finished >= total {
                            let succeeded = ok_count.load(Ordering::Relaxed);
                            push_toast(
                                &ui_weak,
                                &toast_queue,
                                i18n::format_toast_batch_done(succeeded, total, lang),
                                if succeeded == total {
                                    "success"
                                } else {
                                    "warning"
                                },
                                Duration::from_secs(5),
                            );
                        }
                        let _ = slint::invoke_from_event_loop(move || {
                            with_ui(&ui_weak, |ui| {
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
                            });
                        });
                    });
                }
            });
        });
    }
}
