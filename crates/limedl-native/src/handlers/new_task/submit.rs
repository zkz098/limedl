//! Submitting new tasks: single URL (with checksum + BT file selection) and
//! batch mode.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use slint::SharedString;

use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::{ChecksumMode, StartDownloadRequest, TorrentFileEntry};

use crate::MainWindow;
use crate::bridge::TaskStore;
use crate::context::AppContext;
use crate::handlers::common::{read_ui, with_ui};
use crate::i18n;
use crate::toast::ToastQueue;
use crate::toast::push_toast;
use crate::url_utils::{extract_batch_file_name, parse_batch_urls};
use parking_lot::Mutex;

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;

    // Single mode: submit with the probed checksum and the BT file selection
    {
        let ui_weak = ctx.ui_weak.clone();
        let dispatcher = ctx.dispatcher.clone();
        let store = ctx.store.clone();
        let toast_queue = ctx.toast_queue.clone();
        let entries_cache = ctx.new_task_torrent_entries.clone();
        let included_cache = ctx.new_task_torrent_included.clone();
        ui.on_submit_new_task(move |url, dir, filename| {
            submit_single(
                ui_weak.clone(),
                dispatcher.clone(),
                store.clone(),
                toast_queue.clone(),
                entries_cache.clone(),
                included_cache.clone(),
                url,
                dir,
                filename,
            );
        });
    }

    // Batch mode: live link count under the textarea
    {
        let ui_weak = ctx.ui_weak.clone();
        let store = ctx.store.clone();
        ui.on_new_task_batch_text_changed(move |text| {
            update_batch_count(ui_weak.clone(), store.clone(), &text);
        });
    }

    // Batch mode: submit all parsed links concurrently
    {
        let ui_weak = ctx.ui_weak.clone();
        let dispatcher = ctx.dispatcher.clone();
        let store = ctx.store.clone();
        let toast_queue = ctx.toast_queue.clone();
        ui.on_submit_new_task_batch(move |text, dir| {
            submit_batch(
                ui_weak.clone(),
                dispatcher.clone(),
                store.clone(),
                toast_queue.clone(),
                &text,
                &dir,
            );
        });
    }
}

/// Collect the dialog state for a single submit and start the task.
#[allow(clippy::too_many_arguments)]
fn submit_single(
    ui_weak: slint::Weak<MainWindow>,
    dispatcher: Arc<Dispatcher>,
    store: Arc<Mutex<TaskStore>>,
    toast_queue: ToastQueue,
    entries_cache: Arc<Mutex<Vec<TorrentFileEntry>>>,
    included_cache: Arc<Mutex<Vec<bool>>>,
    url: SharedString,
    dir: SharedString,
    filename: SharedString,
) {
    let Some(payload) = read_ui(&ui_weak, |ui| {
        let url = url.to_string();
        let dir = dir.to_string();
        let file_name = if filename.trim().is_empty() {
            None
        } else {
            Some(filename.trim().to_string())
        };

        // Checksum: prefer manual user input (if valid), fallback to detected probe hash.
        let manual = ui.get_new_task_checksum().trim().to_string();
        let (checksum, expected_checksum) = if !manual.is_empty() {
            match parse_user_checksum(&manual) {
                Some((mode, hash)) => (Some(mode), Some(hash)),
                None => {
                    let lang = store.lock().language();
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_invalid_checksum(lang),
                        "error",
                        Duration::from_secs(4),
                    );
                    return None;
                }
            }
        } else if ui.get_new_task_probe_state().as_str() == "found" {
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
            ui.set_new_task_preview_status_text(SharedString::from(i18n::no_files_selected_text(
                lang,
            )));
            return None;
        }

        Some((
            url,
            dir,
            file_name,
            checksum,
            expected_checksum,
            selected_file_indices,
        ))
    })
    .flatten() else {
        return;
    };

    let ui_weak = ui_weak.clone();
    let dispatcher = dispatcher.clone();
    let store = store.clone();
    let toast_queue = toast_queue.clone();
    tokio::spawn(async move {
        let (url, destination_dir, file_name, checksum, expected_checksum, selected_file_indices) =
            payload;
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
                        ui.set_new_task_checksum(SharedString::default());
                        ui.set_new_task_probe_hash(SharedString::default());
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
}

/// Refresh the parsed-link counter shown under the batch textarea.
fn update_batch_count(
    ui_weak: slint::Weak<MainWindow>,
    store: Arc<Mutex<TaskStore>>,
    text: &SharedString,
) {
    let count = parse_batch_urls(text).len();
    with_ui(&ui_weak, |ui| {
        let lang = store.lock().language();
        ui.set_new_task_batch_count_text(SharedString::from(i18n::format_batch_count(count, lang)));
    });
}

/// Submit every parsed batch link concurrently and report aggregate progress.
fn submit_batch(
    ui_weak: slint::Weak<MainWindow>,
    dispatcher: Arc<Dispatcher>,
    store: Arc<Mutex<TaskStore>>,
    toast_queue: ToastQueue,
    text: &str,
    dir: &SharedString,
) {
    let Some((urls, dir, lang)) = read_ui(&ui_weak, |ui| {
        let urls = parse_batch_urls(text);
        let dir = dir.to_string();
        let lang = store.lock().language();
        if urls.is_empty() {
            ui.set_new_task_batch_status_text(SharedString::from(i18n::format_batch_count(
                0, lang,
            )));
            return None;
        }
        ui.set_new_task_batch_submitting(true);
        ui.set_new_task_batch_status_text(SharedString::from(i18n::format_batch_status(
            0,
            urls.len(),
            lang,
        )));
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
}

/// Parse a user-entered checksum string into an explicit mode and normalized lowercase hex digest.
///
/// Supports:
/// - Bare hex: 64 hex chars (default: SHA-256), 128 hex chars (SHA-512)
/// - Prefixes: `sha256:`, `sha-256:`, `sha512:`, `sha-512:`, `blake3:`, `blake-3:`, `b3:`
/// - BSD format: `SHA256 (...) = <hash>`, `SHA512 (...) = <hash>`, `BLAKE3 (...) = <hash>`
/// - GNU format: `<hash>  <filename>` or `<hash> *<filename>`
/// - Strips surrounding quotes and whitespace.
pub fn parse_user_checksum(raw: &str) -> Option<(ChecksumMode, String)> {
    let mut s = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if s.is_empty() {
        return None;
    }

    let mut explicit_mode: Option<ChecksumMode> = None;

    // Check BSD format: ALGO (...) = HASH or ALGO = HASH
    if let Some((prefix, rest)) = s.split_once('=') {
        let prefix_lower = prefix.trim().to_ascii_lowercase();
        if prefix_lower.starts_with("sha256") || prefix_lower.starts_with("sha-256") {
            explicit_mode = Some(ChecksumMode::Sha256);
            s = rest.trim();
        } else if prefix_lower.starts_with("sha512") || prefix_lower.starts_with("sha-512") {
            explicit_mode = Some(ChecksumMode::Sha512);
            s = rest.trim();
        } else if prefix_lower.starts_with("blake3")
            || prefix_lower.starts_with("blake-3")
            || prefix_lower.starts_with("b3")
        {
            explicit_mode = Some(ChecksumMode::Blake3);
            s = rest.trim();
        }
    }

    // Check prefix like "sha256:", "sha-256:", "sha512:", "sha-512:", "blake3:", "blake-3:", "b3:"
    let s_lower = s.to_ascii_lowercase();
    if let Some(rest) = s_lower
        .strip_prefix("sha256:")
        .or_else(|| s_lower.strip_prefix("sha-256:"))
    {
        explicit_mode = Some(ChecksumMode::Sha256);
        s = s[s.len() - rest.len()..].trim();
    } else if let Some(rest) = s_lower
        .strip_prefix("sha512:")
        .or_else(|| s_lower.strip_prefix("sha-512:"))
    {
        explicit_mode = Some(ChecksumMode::Sha512);
        s = s[s.len() - rest.len()..].trim();
    } else if let Some(rest) = s_lower
        .strip_prefix("blake3:")
        .or_else(|| s_lower.strip_prefix("blake-3:"))
        .or_else(|| s_lower.strip_prefix("b3:"))
    {
        explicit_mode = Some(ChecksumMode::Blake3);
        s = s[s.len() - rest.len()..].trim();
    }

    // Strip quotes again if they were after the prefix
    let s = s.trim_matches('"').trim_matches('\'').trim();

    // Take the first whitespace-separated token in case GNU format was pasted: `<hash>  <filename>`
    let hash_token = s.split_whitespace().next().unwrap_or(s);

    if hash_token.is_empty() || !hash_token.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let hash_lower = hash_token.to_ascii_lowercase();
    match (explicit_mode, hash_lower.len()) {
        (Some(ChecksumMode::Sha256), 64) => Some((ChecksumMode::Sha256, hash_lower)),
        (Some(ChecksumMode::Blake3), 64) => Some((ChecksumMode::Blake3, hash_lower)),
        (Some(ChecksumMode::Sha512), 128) => Some((ChecksumMode::Sha512, hash_lower)),
        (None, 64) => Some((ChecksumMode::Sha256, hash_lower)),
        (None, 128) => Some((ChecksumMode::Sha512, hash_lower)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_64: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const HASH_128: &str = "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e";

    #[test]
    fn parse_bare_hex() {
        assert_eq!(
            parse_user_checksum(HASH_64),
            Some((ChecksumMode::Sha256, HASH_64.to_string()))
        );
        assert_eq!(
            parse_user_checksum(&HASH_64.to_ascii_uppercase()),
            Some((ChecksumMode::Sha256, HASH_64.to_string()))
        );
        assert_eq!(
            parse_user_checksum(HASH_128),
            Some((ChecksumMode::Sha512, HASH_128.to_string()))
        );
    }

    #[test]
    fn parse_with_prefixes() {
        assert_eq!(
            parse_user_checksum(&format!("sha256:{HASH_64}")),
            Some((ChecksumMode::Sha256, HASH_64.to_string()))
        );
        assert_eq!(
            parse_user_checksum(&format!("SHA-256: {HASH_64}")),
            Some((ChecksumMode::Sha256, HASH_64.to_string()))
        );
        assert_eq!(
            parse_user_checksum(&format!("sha512:{HASH_128}")),
            Some((ChecksumMode::Sha512, HASH_128.to_string()))
        );
        assert_eq!(
            parse_user_checksum(&format!("blake3:{HASH_64}")),
            Some((ChecksumMode::Blake3, HASH_64.to_string()))
        );
        assert_eq!(
            parse_user_checksum(&format!("b3: {HASH_64}")),
            Some((ChecksumMode::Blake3, HASH_64.to_string()))
        );
    }

    #[test]
    fn parse_gnu_and_bsd_formats() {
        assert_eq!(
            parse_user_checksum(&format!("{HASH_64}  myfile.tar.gz")),
            Some((ChecksumMode::Sha256, HASH_64.to_string()))
        );
        assert_eq!(
            parse_user_checksum(&format!("{HASH_64} *myfile.zip")),
            Some((ChecksumMode::Sha256, HASH_64.to_string()))
        );
        assert_eq!(
            parse_user_checksum(&format!("SHA256 (myfile.iso) = {HASH_64}")),
            Some((ChecksumMode::Sha256, HASH_64.to_string()))
        );
        assert_eq!(
            parse_user_checksum(&format!("BLAKE3 (archive.tar) = {HASH_64}")),
            Some((ChecksumMode::Blake3, HASH_64.to_string()))
        );
        assert_eq!(
            parse_user_checksum(&format!("sha256 = {HASH_64}")),
            Some((ChecksumMode::Sha256, HASH_64.to_string()))
        );
    }

    #[test]
    fn parse_whitespace_and_quotes() {
        assert_eq!(
            parse_user_checksum(&format!(" \"{HASH_64}\" ")),
            Some((ChecksumMode::Sha256, HASH_64.to_string()))
        );
        assert_eq!(
            parse_user_checksum(&format!("'sha256:{HASH_64}'")),
            Some((ChecksumMode::Sha256, HASH_64.to_string()))
        );
    }

    #[test]
    fn reject_invalid_checksums() {
        assert_eq!(parse_user_checksum(""), None);
        assert_eq!(parse_user_checksum("   "), None);
        // MD5 (32 chars) unsupported
        assert_eq!(parse_user_checksum("d41d8cd98f00b204e9800998ecf8427e"), None);
        // SHA-1 (40 chars) unsupported
        assert_eq!(
            parse_user_checksum("da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            None
        );
        // Mismatched length with prefix
        assert_eq!(parse_user_checksum(&format!("sha512:{HASH_64}")), None);
        assert_eq!(parse_user_checksum(&format!("sha256:{HASH_128}")), None);
        // Invalid hex characters
        assert_eq!(
            parse_user_checksum(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b8zz"
            ),
            None
        );
    }
}
