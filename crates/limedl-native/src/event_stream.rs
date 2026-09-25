use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use notify_rust::Notification;
use slint::{ComponentHandle, SharedString, VecModel};
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

use limedl_core::event_bus::DownloadEvent;
use limedl_core::types::{DownloadProgress, DownloadState, DownloadSummary, TaskId};

use crate::context::AppContext;
use crate::bridge::{
    file_status_to_item, format_speed, generate_piece_map_image, peer_info_to_item,
    summary_to_inspector_info, tracker_info_to_item,
};
use crate::i18n::{self, Language};
use crate::platform_win;
use crate::task_ops::open_path_in_explorer;
use crate::toast::{WarningDedup, push_toast};
use crate::tray::{TRAY_SPEED_LIMIT_BPS, update_tray_menu_and_tooltip};
use crate::ui_sync::{refresh_ui, restore_and_show_window};
use crate::{PeerItem, TorrentFileItem, TrackerItem, POWER_GUARD};

pub fn start_clipboard_monitor(ctx: &AppContext) {
    let ui_weak = ctx.ui_weak.clone();
    let toast_queue_clone = ctx.toast_queue.clone();
    let store_clone = ctx.store.clone();
    tokio::spawn(async move {
        let mut last_clipboard = String::new();
        if let Ok(mut cb) = arboard::Clipboard::new()
            && let Ok(text) = cb.get_text()
        {
            last_clipboard = text.trim().to_string();
        }

        let mut interval = tokio::time::interval(Duration::from_millis(1500));
        loop {
            interval.tick().await;
            let Ok(mut cb) = arboard::Clipboard::new() else {
                continue;
            };
            let Ok(text) = cb.get_text() else {
                continue;
            };
            let text = text.trim().to_string();
            if text.is_empty() || text == last_clipboard {
                continue;
            }
            last_clipboard = text.clone();

            match crate::bridge::parse_clipboard_download_text(&text) {
                crate::bridge::ClipboardPayload::SingleUrl(url) => {
                    let lang = store_clone.lock().language();
                    let display_url = if url.len() > 50 {
                        format!("{}...", &url[..47])
                    } else {
                        url
                    };
                    let msg = i18n::format_detected_link(&display_url, lang);
                    push_toast(&ui_weak, &toast_queue_clone, msg, "info", Duration::from_secs(5));
                }
                crate::bridge::ClipboardPayload::BatchUrls(urls) => {
                    let lang = store_clone.lock().language();
                    let count = urls.len();
                    let msg = i18n::format_detected_batch(count, lang);
                    push_toast(&ui_weak, &toast_queue_clone, msg, "info", Duration::from_secs(5));
                }
                crate::bridge::ClipboardPayload::Empty => {}
            }
        }
    });
}

pub fn start_event_bus_listener(
    ctx: &AppContext,
    mut rx: tokio::sync::broadcast::Receiver<DownloadEvent>,
) {
    let ui_weak = ctx.ui_weak.clone();
    let store_clone = ctx.store.clone();
    let active_inspector_id_clone = ctx.active_inspector_id.clone();
    let toast_queue_clone = ctx.toast_queue.clone();
    let current_settings_clone = ctx.current_settings.clone();
    let dispatcher = ctx.dispatcher.clone();

    tokio::spawn(async move {
            let mut warning_dedup = WarningDedup::new();
            while let Ok(event) = rx.recv().await {
                let store = store_clone.clone();
                let ui_weak = ui_weak.clone();
                let active_inspector_id = active_inspector_id_clone.clone();
                let toast_queue = toast_queue_clone.clone();
                let current_settings = current_settings_clone.clone();
                let dispatcher = dispatcher.clone();

                match event {
                    DownloadEvent::Updated { id, summary_json } => {
                        // If task no longer exists in backend, it was removed.
                        if let Ok(task_id) = TaskId::from_wire_string(&id)
                            && dispatcher.status(&task_id).await.is_err()
                        {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    let mut s = store.lock();
                                    s.remove(&id);
                                    refresh_ui(&ui, &s);

                                    if let Some(ref current_id) = *active_inspector_id.lock()
                                        && current_id == &id
                                    {
                                        ui.set_show_inspector(false);
                                    }
                                }
                            });
                            continue;
                        }

                        if let Ok(summary) =
                            serde_json::from_value::<DownloadSummary>(summary_json)
                        {
                            let current_lang = store.lock().language();
                            let notif_enabled = current_settings.lock().notifications.enabled;
                            // OS notification + in-app toast on completion or failure
                            if matches!(summary.state, DownloadState::Completed) {
                                if notif_enabled {
                                    let (title, body) = i18n::format_notification_completed(
                                        &summary.file_name,
                                        current_lang,
                                    );
                                    let mut notif = Notification::new();
                                    notif.appname("limedl")
                                        .summary(&title)
                                        .body(&body);
                                    #[cfg(windows)]
                                    {
                                        notif.app_id("limedl");
                                    }
                                    let _ = notif.show();
                                }
                                push_toast(
                                    &ui_weak,
                                    &toast_queue,
                                    i18n::format_toast_state(&summary.file_name, &summary.state, current_lang),
                                    "success",
                                    Duration::from_secs(5),
                                );
                            } else if matches!(summary.state, DownloadState::Failed) {
                                if notif_enabled {
                                    let (title, body) = i18n::format_notification_failed(
                                        &summary.file_name,
                                        summary.error.as_deref(),
                                        current_lang,
                                    );
                                    let mut notif = Notification::new();
                                    notif.appname("limedl")
                                        .summary(&title)
                                        .body(&body);
                                    #[cfg(windows)]
                                    {
                                        notif.app_id("limedl");
                                    }
                                    let _ = notif.show();
                                }
                                push_toast(
                                    &ui_weak,
                                    &toast_queue,
                                    i18n::format_toast_state(&summary.file_name, &summary.state, current_lang),
                                    "error",
                                    Duration::from_secs(6),
                                );
                            }

                            let summary_clone = summary.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    let mut s = store.lock();
                                    let lang = s.language();
                                    s.insert_or_update(summary_clone.clone());
                                    let (_, downloading, _, _, _) = s.counts();
                                    POWER_GUARD.update(downloading);
                                    if !platform_win::is_window_visible() {
                                        return;
                                    }
                                    refresh_ui(&ui, &s);

                                    // Refresh Inspector if this task is currently viewed
                                    if let Some(ref current_id) = *active_inspector_id.lock()
                                        && current_id == &summary_clone.id
                                    {
                                        ui.set_inspector_info(summary_to_inspector_info(&summary_clone, lang));
                                    }
                                }
                            });
                        }
                    }
                    DownloadEvent::Progress { progress_json, .. } => {
                        if let Ok(progress) =
                            serde_json::from_value::<DownloadProgress>(progress_json)
                        {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    let mut s = store.lock();
                                    let lang = s.language();
                                    s.update_progress(&progress);
                                    let (_, downloading, _, _, _) = s.counts();
                                    POWER_GUARD.update(downloading);
                                    if !platform_win::is_window_visible() {
                                        return;
                                    }
                                    refresh_ui(&ui, &s);

                                    // Refresh Inspector progress if active
                                    if let Some(ref current_id) = *active_inspector_id.lock()
                                        && current_id == &progress.id
                                        && let Some(summary) = s.get_summary(current_id)
                                    {
                                        ui.set_inspector_info(summary_to_inspector_info(&summary, lang));
                                    }
                                }
                            });
                        }
                    }
                    DownloadEvent::FullState { downloads } => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut s = store.lock();
                                s.replace_all(downloads);
                                let (_, downloading, _, _, _) = s.counts();
                                POWER_GUARD.update(downloading);
                                if !platform_win::is_window_visible() {
                                    return;
                                }
                                refresh_ui(&ui, &s);
                            }
                        });
                    }
                    DownloadEvent::CdnProgress { phase, current, total } => {
                        let current_lang = store_clone.lock().language();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_is_testing = true;
                                form.cdn_status_type = SharedString::from("testing");
                                form.cdn_status_label = SharedString::from(match current_lang {
                                    Language::ZhCn => "测速中",
                                    Language::ZhTw => "測速中",
                                    Language::EnUs => "Testing",
                                });
                                form.cdn_phase_label = SharedString::from(match phase.as_str() {
                                    "fetchingRanges" => match current_lang {
                                        Language::ZhCn => "获取网段",
                                        Language::ZhTw => "獲取網段",
                                        Language::EnUs => "Fetching IP ranges",
                                    },
                                    "screening" => match current_lang {
                                        Language::ZhCn => "延迟初筛",
                                        Language::ZhTw => "延遲初篩",
                                        Language::EnUs => "Screening latency",
                                    },
                                    "measuringThroughput" => match current_lang {
                                        Language::ZhCn => "带宽测速",
                                        Language::ZhTw => "頻寬測速",
                                        Language::EnUs => "Measuring bandwidth",
                                    },
                                    other => other,
                                });
                                if total > 0 {
                                    form.cdn_progress_percent = (current as f32 / total as f32 * 100.0).clamp(0.0, 100.0);
                                    form.cdn_progress_label = SharedString::from(format!("{current} / {total}"));
                                }
                                ui.set_labs_form(form);
                            }
                        });
                    }
                    DownloadEvent::CdnComplete { state, active_ip, active_speed_mbps } => {
                        let ui_weak_evt = ui_weak.clone();
                        let state_evt = state.clone();
                        let ip_evt = active_ip.clone();
                        let current_lang = store_clone.lock().language();
                        let is_ready = state_evt == "Ready" || state_evt == "ready";
                        let is_error = state_evt.starts_with("Error") || state_evt == "error";
                        let err_msg = if is_error {
                            state_evt.strip_prefix("Error: ").unwrap_or(&state_evt).to_string()
                        } else {
                            String::new()
                        };
                        let err_msg_ui = err_msg.clone();

                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_evt.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_is_testing = false;
                                let (st, sl) = if is_ready {
                                    ("ready", match current_lang {
                                        Language::ZhCn => "准备就绪",
                                        Language::ZhTw => "準備就緒",
                                        Language::EnUs => "Ready",
                                    })
                                } else if is_error {
                                    ("error", match current_lang {
                                        Language::ZhCn => "测速失败",
                                        Language::ZhTw => "測速失敗",
                                        Language::EnUs => "Failed",
                                    })
                                } else {
                                    ("idle", match current_lang {
                                        Language::ZhCn => "未配置",
                                        Language::ZhTw => "未配置",
                                        Language::EnUs => "Not Configured",
                                    })
                                };
                                form.cdn_status_type = SharedString::from(st);
                                form.cdn_status_label = SharedString::from(sl);
                                if is_ready {
                                    form.cdn_last_error = SharedString::default();
                                } else if is_error {
                                    form.cdn_last_error = SharedString::from(&err_msg_ui);
                                }
                                if let Some(ip) = ip_evt {
                                    form.cdn_active_ip = SharedString::from(ip);
                                }
                                if let Some(spd) = active_speed_mbps {
                                    form.cdn_active_speed_text = SharedString::from(format!("{spd:.2} MB/s"));
                                }
                                ui.set_labs_form(form);
                            }
                        });
                        // In-app toast for the async test result.
                        if is_ready {
                            let ip = active_ip.as_deref();
                            push_toast(
                                &ui_weak,
                                &toast_queue_clone,
                                i18n::format_toast_cdn_test_done(ip, current_lang),
                                "success",
                                Duration::from_secs(5),
                            );
                        } else if is_error {
                            let msg = if err_msg.is_empty() {
                                i18n::cdn_test_failed_label(current_lang)
                            } else {
                                &err_msg
                            };
                            push_toast(
                                &ui_weak,
                                &toast_queue_clone,
                                i18n::format_toast_cdn_test_failed(msg, current_lang),
                                "error",
                                Duration::from_secs(6),
                            );
                        }
                    }
                    DownloadEvent::Warning { id, message } => {
                        // Warnings originate in core (mirror failover, disk full,
                        // anti-leech bans, …). Surface them the same way the web
                        // client does: an in-app warning toast. Duplicate messages
                        // are collapsed within a short window because anti-leech
                        // bans arrive one per peer.
                        if !warning_dedup.should_show(&id, &message) {
                            continue;
                        }
                        let file_name = store.lock().get_summary(&id).map(|s| s.file_name);
                        let text = match file_name {
                            Some(name) if !name.is_empty() => {
                                i18n::format_warning_with_file(&name, &message)
                            }
                            _ => message,
                        };
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            text,
                            "warning",
                            Duration::from_secs(6),
                        );
                    }
                    DownloadEvent::Aria2Notification { .. } => {
                        // Aria2 compatibility notifications are pushed by the
                        // Aria2 RPC server straight to connected aria2 clients
                        // (AriaNg / Motrix); the native UI has no surface for
                        // them, so nothing to do here.
                    }
                }
            }
        });
    }

pub fn start_status_pollers(ctx: &AppContext) {
    let main_window = &ctx.ui;
    let dispatcher = ctx.dispatcher.clone();
    let active_inspector_id = ctx.active_inspector_id.clone();
    let store = ctx.store.clone();

    // BT runtime status pills (DHT nodes / upload speed / peers), matching the
    // web client's toolbar status strip. Polled on a slow interval because the
    // DHT node count only changes gradually.
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = dispatcher.clone();
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
                        Ok(s) if s.connected => {
                            ui.set_bt_status_visible(true);
                            ui.set_bt_dht_text(SharedString::from(match s.dht_nodes {
                                Some(n) if s.dht_enabled => n.to_string(),
                                _ => "-".to_string(),
                            }));
                            ui.set_bt_upload_speed_text(SharedString::from(format_speed(
                                s.upload_speed_bytes_per_second,
                            )));
                            ui.set_bt_peers_text(SharedString::from(s.peer_count.to_string()));
                        }
                        // No BT session yet (or the backend is gone): hide the
                        // pills exactly like the web client does with a null
                        // status payload.
                        _ => ui.set_bt_status_visible(false),
                    }
                });
            }
        });
    }

    // Phase 3: Periodic Inspector Polling (Peers, Trackers, Piece Map, Files)
    {
        let ui_weak = main_window.as_weak();
        let dispatcher = dispatcher.clone();
        let active_inspector_id_clone = active_inspector_id.clone();
        let store_clone = store.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(1000));
            loop {
                interval.tick().await;
                if !platform_win::is_window_visible() {
                    continue;
                }

                let current_id = {
                    let lock = active_inspector_id_clone.lock();
                    lock.clone()
                };

                if let Some(task_id_str) = current_id
                    && let Ok(task_id) = TaskId::from_wire_string(&task_id_str)
                {
                    let summary_opt = {
                        let s = store_clone.lock();
                        s.get_summary(&task_id_str)
                    };

                    if let Some(summary) = summary_opt {
                        let lang = store_clone.lock().language();
                        if summary.kind == limedl_core::types::TaskKind::Bt {
                            let peers = dispatcher.bt_get_peers(&task_id).unwrap_or_default();
                            let trackers = dispatcher.bt_get_trackers(&task_id).unwrap_or_default();
                            let pieces = dispatcher.bt_get_pieces(&task_id).unwrap_or_default();
                            let files = dispatcher.bt_get_files(&task_id).unwrap_or_default();

                            let peer_items: Vec<PeerItem> = peers.iter().map(peer_info_to_item).collect();
                            let tracker_items: Vec<TrackerItem> = trackers.iter().map(tracker_info_to_item).collect();
                            let file_items: Vec<TorrentFileItem> = files.iter().map(file_status_to_item).collect();
                            let insp_info = summary_to_inspector_info(&summary, lang);

                            let ui_weak = ui_weak.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    let (piece_map_img, piece_count_text) = generate_piece_map_image(&pieces, lang);
                                    ui.set_inspector_info(insp_info);
                                    ui.set_inspector_peers(Rc::new(VecModel::from(peer_items)).into());
                                    ui.set_inspector_trackers(Rc::new(VecModel::from(tracker_items)).into());
                                    ui.set_inspector_piece_map(piece_map_img);
                                    ui.set_inspector_pieces_count_text(SharedString::from(piece_count_text));
                                    ui.set_inspector_files(Rc::new(VecModel::from(file_items)).into());
                                }
                            });
                        } else {
                            let insp_info = summary_to_inspector_info(&summary, lang);
                            let ui_weak = ui_weak.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak.upgrade() {
                                    ui.set_inspector_info(insp_info);
                                }
                            });
                        }
                    }
                }
            }
        });
    }
}

pub fn start_tray_event_loop(ctx: &AppContext) {
    let ui_weak = ctx.ui_weak.clone();
    let dispatcher = ctx.dispatcher.clone();
    let default_dir = ctx.current_settings.lock().download.default_download_dir.clone();
    let game_mode_active_clone = ctx.game_mode_active.clone();
    let current_settings_tray = ctx.current_settings.clone();
    let tray_speed_limit_active_clone = ctx.tray_speed_limit_active.clone();
    let store_tray = ctx.store.clone();
    let toast_queue_tray = ctx.toast_queue.clone();

        let (menu_tx, mut menu_rx) = tokio::sync::mpsc::unbounded_channel::<muda::MenuEvent>();
        let (tray_tx, mut tray_rx) = tokio::sync::mpsc::unbounded_channel::<TrayIconEvent>();
        std::thread::Builder::new()
            .name("tray-menu-events".into())
            .spawn(move || {
                let receiver = muda::MenuEvent::receiver();
                while let Ok(event) = receiver.recv() {
                    if menu_tx.send(event).is_err() {
                        break;
                    }
                }
            })
            .expect("failed to spawn tray menu event thread");
        std::thread::Builder::new()
            .name("tray-icon-events".into())
            .spawn(move || {
                let receiver = TrayIconEvent::receiver();
                while let Ok(event) = receiver.recv() {
                    if tray_tx.send(event).is_err() {
                        break;
                    }
                }
            })
            .expect("failed to spawn tray icon event thread");

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(event) = menu_rx.recv() => {
                        match event.id.as_ref() {
                            "show" => {
                                let _ = slint::invoke_from_event_loop({
                                    let ui_weak = ui_weak.clone();
                                    let store = store_tray.clone();
                                    move || {
                                        if let Some(ui) = ui_weak.upgrade() {
                                            restore_and_show_window(&ui, Some(&store.lock()));
                                        }
                                    }
                                });
                            }
                            "pause_all" => {
                                if let Ok(list) = dispatcher.list().await {
                                    for item in list {
                                        if matches!(item.state, DownloadState::Downloading)
                                            && let Ok(task_id) = TaskId::from_wire_string(&item.id)
                                        {
                                            let _ = dispatcher.pause(&task_id).await;
                                        }
                                    }
                                }
                            }
                            "resume_all" => {
                                if let Ok(list) = dispatcher.list().await {
                                    for item in list {
                                        if matches!(item.state, DownloadState::Paused)
                                            && let Ok(task_id) = TaskId::from_wire_string(&item.id)
                                        {
                                            let _ = dispatcher.resume(&task_id).await;
                                        }
                                    }
                                }
                            }
                            "game_mode" => {
                                if let Ok(new_val) = dispatcher.toggle_game_mode(None) {
                                    *game_mode_active_clone.lock() = new_val;
                                    let ui_weak = ui_weak.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = ui_weak.upgrade() {
                                            ui.set_game_mode_active(new_val);
                                        }
                                    });
                                }
                            }
                            "speed_limit" => {
                                // Quick global speed limit shortcut: unlimited ↔ 1 MB/s.
                                let mut settings = current_settings_tray.lock().clone();
                                let enabling = settings.global_speed_limit_bps == 0;
                                settings.global_speed_limit_bps = if enabling {
                                    TRAY_SPEED_LIMIT_BPS
                                } else {
                                    0
                                };
                                let lang = store_tray.lock().language();
                                match dispatcher.save_settings(&settings).await {
                                    Ok(saved) => {
                                        *current_settings_tray.lock() = saved.clone();
                                        tray_speed_limit_active_clone.store(
                                            saved.global_speed_limit_bps > 0,
                                            Ordering::Relaxed,
                                        );
                                        push_toast(
                                            &ui_weak,
                                            &toast_queue_tray,
                                            i18n::format_toast_speed_limit(enabling, lang),
                                            "info",
                                            Duration::from_secs(4),
                                        );
                                        let limit_kb =
                                            (saved.global_speed_limit_bps / 1024).to_string();
                                        let ui_weak = ui_weak.clone();
                                        let lang_code = Language::from_code(&saved.appearance.language);
                                        let spd_active = saved.global_speed_limit_bps > 0;
                                        let _ = slint::invoke_from_event_loop(move || {
                                            update_tray_menu_and_tooltip(lang_code, spd_active);
                                            if let Some(ui) = ui_weak.upgrade() {
                                                let mut form = ui.get_settings_form();
                                                form.global_speed_limit_kb =
                                                    SharedString::from(limit_kb);
                                                ui.set_settings_form(form);
                                            }
                                        });
                                    }
                                    Err(e) => tracing::warn!("托盘限速切换失败: {e:#}"),
                                }
                            }
                            "open_dir" => {
                                let _ = open_path_in_explorer(&default_dir);
                            }
                            "quit" => {
                                POWER_GUARD.release();
                                // Quit through the Slint event loop so the
                                // runtime teardown (engine shutdown, registry
                                // shutdown_all) runs; fall back to a hard exit
                                // only if the event loop is already gone.
                                if slint::quit_event_loop().is_err() {
                                    std::process::exit(0);
                                }
                                break;
                            }
                            _ => {}
                        }
                    }
                    Some(event) = tray_rx.recv() => {
                        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                            let _ = slint::invoke_from_event_loop({
                                let ui_weak = ui_weak.clone();
                                let store = store_tray.clone();
                                move || {
                                    if let Some(ui) = ui_weak.upgrade() {
                                        restore_and_show_window(&ui, Some(&store.lock()));
                                    }
                                }
                            });
                        }
                    }
                    else => break,
                }
            }
        });

}
