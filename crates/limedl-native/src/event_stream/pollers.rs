//! Periodic background pollers: the clipboard monitor, the BT runtime status
//! pills and the inspector (peers / trackers / piece map / files).

use std::rc::Rc;
use std::time::Duration;

use slint::{SharedString, VecModel};

use limedl_core::types::TaskId;

use crate::bridge::{
    ClipboardPayload, file_status_to_item, format_speed, generate_piece_map_image, parse_clipboard_download_text,
    peer_info_to_item, summary_to_inspector_info, tracker_info_to_item,
};
use crate::context::AppContext;
use crate::i18n;
use crate::platform_win;
use crate::toast::push_toast;
use crate::{PeerItem, TorrentFileItem, TrackerItem};

/// Watch the system clipboard and toast when it holds a downloadable link.
pub fn start_clipboard_monitor(ctx: &AppContext) {
    let ui_weak = ctx.ui_weak.clone();
    let toast_queue = ctx.toast_queue.clone();
    let store = ctx.store.clone();
    tokio::spawn(async move {
        let mut last_clipboard = String::new();
        if let Ok(mut clipboard) = arboard::Clipboard::new()
            && let Ok(text) = clipboard.get_text()
        {
            last_clipboard = text.trim().to_string();
        }

        let mut interval = tokio::time::interval(Duration::from_millis(1500));
        loop {
            interval.tick().await;
            let Ok(mut clipboard) = arboard::Clipboard::new() else {
                continue;
            };
            let Ok(text) = clipboard.get_text() else {
                continue;
            };
            let text = text.trim().to_string();
            if text.is_empty() || text == last_clipboard {
                continue;
            }
            last_clipboard = text.clone();

            match parse_clipboard_download_text(&text) {
                ClipboardPayload::SingleUrl(url) => {
                    let lang = store.lock().language();
                    let display_url = if url.len() > 50 {
                        format!("{}...", &url[..47])
                    } else {
                        url
                    };
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_detected_link(&display_url, lang),
                        "info",
                        Duration::from_secs(5),
                    );
                }
                ClipboardPayload::BatchUrls(urls) => {
                    let lang = store.lock().language();
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_detected_batch(urls.len(), lang),
                        "info",
                        Duration::from_secs(5),
                    );
                }
                ClipboardPayload::Empty => {}
            }
        }
    });
}

pub fn start_status_pollers(ctx: &AppContext) {
    spawn_bt_status_poller(ctx);
    spawn_inspector_poller(ctx);
}

/// BT runtime status pills (DHT nodes / upload speed / peers), matching the web
/// client's toolbar status strip. Polled slowly: the DHT node count only changes
/// gradually.
fn spawn_bt_status_poller(ctx: &AppContext) {
    let ui_weak = ctx.ui_weak.clone();
    let dispatcher = ctx.dispatcher.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            if !platform_win::is_window_visible() {
                continue;
            }
            let status = dispatcher.bt_runtime_status();
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = ui_weak.upgrade() else {
                    return;
                };
                match status {
                    Ok(status) if status.connected => {
                        ui.set_bt_status_visible(true);
                        ui.set_bt_dht_text(SharedString::from(match status.dht_nodes {
                            Some(nodes) if status.dht_enabled => nodes.to_string(),
                            _ => "-".to_string(),
                        }));
                        ui.set_bt_upload_speed_text(SharedString::from(format_speed(
                            status.upload_speed_bytes_per_second,
                        )));
                        ui.set_bt_peers_text(SharedString::from(status.peer_count.to_string()));
                    }
                    // No BT session yet (or the backend is gone): hide the pills
                    // exactly like the web client does with a null status payload.
                    _ => ui.set_bt_status_visible(false),
                }
            });
        }
    });
}

/// Inspector polling: peers, trackers, piece map and file list of the task the
/// inspector is currently showing.
fn spawn_inspector_poller(ctx: &AppContext) {
    let ui_weak = ctx.ui_weak.clone();
    let dispatcher = ctx.dispatcher.clone();
    let active_inspector_id = ctx.active_inspector_id.clone();
    let store = ctx.store.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1000));
        loop {
            interval.tick().await;
            if !platform_win::is_window_visible() {
                continue;
            }

            let current_id = active_inspector_id.lock().clone();
            let Some(task_id_str) = current_id else {
                continue;
            };
            let Ok(task_id) = TaskId::from_wire_string(&task_id_str) else {
                continue;
            };
            let summary = store.lock().get_summary(&task_id_str);
            let Some(summary) = summary else {
                continue;
            };

            let lang = store.lock().language();
            if summary.kind == limedl_core::types::TaskKind::Bt {
                let peers = dispatcher.bt_get_peers(&task_id).unwrap_or_default();
                let trackers = dispatcher.bt_get_trackers(&task_id).unwrap_or_default();
                let pieces = dispatcher.bt_get_pieces(&task_id).unwrap_or_default();
                let files = dispatcher.bt_get_files(&task_id).unwrap_or_default();

                let peer_items: Vec<PeerItem> = peers.iter().map(peer_info_to_item).collect();
                let tracker_items: Vec<TrackerItem> =
                    trackers.iter().map(tracker_info_to_item).collect();
                let file_items: Vec<TorrentFileItem> =
                    files.iter().map(file_status_to_item).collect();
                let inspector_info = summary_to_inspector_info(&summary, lang);

                let ui_weak = ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let (piece_map_image, piece_count_text) =
                            generate_piece_map_image(&pieces, lang);
                        ui.set_inspector_info(inspector_info);
                        ui.set_inspector_peers(Rc::new(VecModel::from(peer_items)).into());
                        ui.set_inspector_trackers(Rc::new(VecModel::from(tracker_items)).into());
                        ui.set_inspector_piece_map(piece_map_image);
                        ui.set_inspector_pieces_count_text(SharedString::from(piece_count_text));
                        ui.set_inspector_files(Rc::new(VecModel::from(file_items)).into());
                    }
                });
            } else {
                let inspector_info = summary_to_inspector_info(&summary, lang);
                let ui_weak = ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_inspector_info(inspector_info);
                    }
                });
            }
        }
    });
}
