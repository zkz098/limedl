mod download;

#[cfg(feature = "test-utils")]
pub use download::aimd;

#[cfg(feature = "test-utils")]
pub use download::buffer_pool;

#[cfg(feature = "test-utils")]
pub use download::test_harness;

pub use download::RateLimiter;

use std::sync::Arc;

use parking_lot::Mutex;
use parking_lot::RwLock as ParkingRwLock;
use std::time::Duration;

use anyhow::Context;
use tauri::Emitter;
use tauri::Manager;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tokio::sync::broadcast;
use tokio::time::sleep;

use download::event_bus::DownloadEvent;
use download::{
    AppState, Aria2RpcServer, bootstrap, bt_get_peers, bt_get_pieces,
    bt_get_trackers, bt_preview_torrent, bt_runtime_status, bt_set_speed_limit, cdn_apply,
    cdn_cancel, cdn_candidates, cdn_clear, cdn_detail, cdn_fetch_ranges, cdn_status, cdn_test,
    cleanup_old_aria2_temp_files, detect_disk_type, download_cancel, download_list,
    download_open_in_explorer,
    download_pause, download_purge, download_remove, download_resume, download_set_priority,
    download_start, download_status, get_bt_files, get_io_status, get_overclock_mode, init_logging,
    settings_fetch_tracker_list, settings_get, settings_save, toggle_game_mode,
    toggle_overclock_mode, update_bt_files, CloseBehavior,
};

/// Maps a [`DownloadEvent`] to a Tauri event name and JSON payload.
///
/// Pure function — no side effects. This is the single source of truth for
/// the shape of every event emitted to the Tauri frontend.
fn map_event_to_emit(event: &DownloadEvent) -> (&str, serde_json::Value) {
    match event {
        DownloadEvent::Updated { summary_json, .. } => {
            ("download-updated", summary_json.clone())
        }
        DownloadEvent::Progress { progress_json, .. } => {
            ("download-progress", progress_json.clone())
        }
        DownloadEvent::Aria2Notification { event_name, gid } => {
            (event_name.as_str(), serde_json::json!(gid))
        }
        DownloadEvent::CdnProgress {
            phase,
            current,
            total,
        } => (
            "cdn-test-progress",
            serde_json::json!({
                "phase": phase,
                "current": current,
                "total": total,
            }),
        ),
        DownloadEvent::CdnComplete {
            state,
            active_ip,
            active_speed_mbps,
        } => (
            "cdn-test-complete",
            serde_json::json!({
                "state": state,
                "activeIp": active_ip,
                "activeSpeedMbps": active_speed_mbps,
            }),
        ),
        DownloadEvent::Warning { id, message } => {
            ("download-warning", serde_json::json!({ "id": id, "message": message }))
        }
    }
}

#[tauri::command]
async fn update_tray_language(app: tauri::AppHandle, language: String) -> Result<(), String> {
    use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
    use download::manager::DownloadManager;
    use download::types::DownloadState;

    let state = app.state::<AppState>();

    // Check download state for Pause/Resume All
    let has_active = if let Some(dm) = state.registry.get_typed::<DownloadManager>() {
        if let Ok(list) = dm.list().await {
            list.iter().any(|s| matches!(s.state, DownloadState::Downloading))
        } else {
            false
        }
    } else {
        false
    };

    // Read current settings
    let settings = state.settings.read();
    let speed_limit_active = settings.global_speed_limit_bps > 0;
    let game_mode = if let Some(dm) = state.registry.get_typed::<DownloadManager>() {
        dm.game_mode()
    } else {
        false
    };
    drop(settings);

    let is_zh = language.as_str() == "zh-CN";

    // Build menu items
    let show_text = if is_zh { "显示窗口" } else { "Show Window" };
    let show = MenuItemBuilder::with_id("show", show_text)
        .build(&app)
        .map_err(|e| e.to_string())?;

    let sep1 = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;

    let (pause_text, pause_id) = if has_active {
        (
            if is_zh { "暂停全部下载" } else { "Pause All" },
            "pause_all",
        )
    } else {
        (
            if is_zh { "恢复全部下载" } else { "Resume All" },
            "resume_all",
        )
    };
    let pause = MenuItemBuilder::with_id(pause_id, pause_text)
        .build(&app)
        .map_err(|e| e.to_string())?;

    let speed_text = if is_zh { "限速模式" } else { "Speed Limit" };
    let speed = CheckMenuItemBuilder::with_id("speed_limit", speed_text)
        .checked(speed_limit_active)
        .build(&app)
        .map_err(|e| e.to_string())?;

    let open_text = if is_zh { "打开下载目录" } else { "Open Download Dir" };
    let open = MenuItemBuilder::with_id("open_dir", open_text)
        .build(&app)
        .map_err(|e| e.to_string())?;

    let game_text = if is_zh { "游戏模式" } else { "Game Mode" };
    let game = CheckMenuItemBuilder::with_id("game_mode", game_text)
        .checked(game_mode)
        .build(&app)
        .map_err(|e| e.to_string())?;

    let sep2 = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;

    let quit_text = if is_zh { "退出" } else { "Quit" };
    let quit = MenuItemBuilder::with_id("quit", quit_text)
        .build(&app)
        .map_err(|e| e.to_string())?;

    let menu = MenuBuilder::new(&app)
        .item(&show)
        .item(&sep1)
        .item(&pause)
        .item(&speed)
        .item(&open)
        .item(&game)
        .item(&sep2)
        .item(&quit)
        .build()
        .map_err(|e| e.to_string())?;

    if let Some(tray) = app.tray_by_id("main-tray") {
        tray.set_menu(Some(menu))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app.emit("single-instance", ());
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                let _ = window.show();
            }
        }));
    }

    let run_result = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            app.handle()
                .plugin(tauri_plugin_clipboard_manager::init())?;

            // Hide window when launched via autostart
            let is_autostart = std::env::args().any(|a| a == "--hidden");
            if is_autostart && let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }

            app.handle().plugin(
                tauri_plugin_autostart::Builder::new()
                    .args(["--hidden"])
                    .build(),
            )?;

            (|| -> anyhow::Result<()> {
                let state_dir = app
                    .path()
                    .app_local_data_dir()
                    .or_else(|_| app.path().app_data_dir())
                    .unwrap_or_else(|_| std::env::temp_dir().join("limedl"))
                    .join("downloads");

                let core = tauri::async_runtime::block_on(bootstrap::bootstrap(state_dir.clone()))
                    .with_context(|| "初始化核心子系统失败")?;

                init_logging(&core.settings.logging, &state_dir).context("初始化日志系统失败")?;
                cleanup_old_aria2_temp_files();

                // CDN accelerator setup
                let cdn_accelerator = core.cdn_service.accelerator().clone();
                core.download_manager
                    .set_cdn_accelerator(cdn_accelerator);
                tauri::async_runtime::block_on(
                    core.cdn_service.init_from_settings(&core.settings),
                );

                let rpc_shutdown = Arc::new(Mutex::new(None::<tokio::sync::watch::Sender<bool>>));

                app.manage(AppState {
                    registry: core.registry.clone(),
                    event_bus: core.event_bus.clone(),
                    cdn_service: core.cdn_service.clone(),
                    rpc_shutdown: rpc_shutdown.clone(),
                    settings: Arc::new(ParkingRwLock::new(core.settings.clone())),
                });

                // Subscribe to EventBus and forward events to Tauri frontend
                {
                    let mut rx = core.event_bus.subscribe();
                    let app_handle_tx = app.handle().clone();
                    let state = AppState {
                        registry: core.registry.clone(),
                        event_bus: core.event_bus.clone(),
                        cdn_service: core.cdn_service.clone(),
                        rpc_shutdown: rpc_shutdown.clone(),
                        settings: Arc::new(ParkingRwLock::new(core.settings.clone())),
                    };
                    tauri::async_runtime::spawn(async move {
                        loop {
                            match rx.recv().await {
                                Ok(event) => {
                                    let (event_name, payload) = map_event_to_emit(&event);
                                    let _ = app_handle_tx.emit(event_name, payload);
                                }
                                Err(broadcast::error::RecvError::Lagged(n)) => {
                                    tracing::warn!("EventBus subscriber lagged by {n} messages");
                                    state.emit_all_downloads().await;
                                }
                                Err(broadcast::error::RecvError::Closed) => break,
                            }
                        }
                    });
                }

                // Periodic emit task
                {
                    let state = AppState {
                        registry: core.registry.clone(),
                        event_bus: core.event_bus.clone(),
                        cdn_service: core.cdn_service.clone(),
                        rpc_shutdown: rpc_shutdown.clone(),
                        settings: Arc::new(ParkingRwLock::new(core.settings.clone())),
                    };
                    tauri::async_runtime::spawn(async move {
                        loop {
                            sleep(Duration::from_secs(30)).await;
                            state.emit_all_downloads().await;
                        }
                    });
                }

                // Aria2 RPC startup
                if core.settings.aria2_rpc.enabled {
                    let (tx, rx) = tokio::sync::watch::channel(false);
                    let rpc_server = Aria2RpcServer::new(
                        core.registry.clone(),
                        &core.settings.aria2_rpc,
                        core.event_bus.clone(),
                    );
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = rpc_server.serve(rx, vec![]).await {
                            tracing::error!("Aria2 RPC server stopped: {error}");
                        }
                    });
                    *rpc_shutdown.lock() = Some(tx);
                    tracing::info!("Aria2 RPC 服务器已启动");
                }

                Ok(())
            })()
            .map_err(|error| -> Box<dyn std::error::Error> {
                Box::new(std::io::Error::other(error.to_string()))
            })?;

            // Build system tray (desktop only)
            #[cfg(desktop)]
            {
                use tauri::menu::{MenuBuilder, MenuItemBuilder};

                let mut tray_builder = TrayIconBuilder::with_id("main-tray")
                    .tooltip("limedl")
                    .show_menu_on_left_click(false)
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    });

                if let Some(icon) = app.default_window_icon().cloned() {
                    tray_builder = tray_builder.icon(icon);
                } else {
                    tracing::warn!("No default window icon available for system tray");
                }

                let tray = tray_builder.build(app)?;

                let show_text = "Show Window";
                let quit_text = "Quit";
                let show_item = MenuItemBuilder::with_id("show", show_text).build(app)?;
                let quit_item = MenuItemBuilder::with_id("quit", quit_text).build(app)?;
                let menu = MenuBuilder::new(app)
                    .item(&show_item)
                    .item(&quit_item)
                    .build()?;
                tray.set_menu(Some(menu))?;

                let app_handle_menu = app.handle().clone();
                tray.on_menu_event(move |_tray, event| {
                    let app = app_handle_menu.clone();
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "pause_all" | "resume_all" => {
                            tauri::async_runtime::spawn(async move {
                                use download::{Dispatcher, types::TaskId, types::DownloadState};

                                let state = app.state::<AppState>();
                                let dispatcher = Dispatcher::new(
                                    state.registry.clone(),
                                    state.event_bus.clone(),
                                );
                                if let Ok(list) = dispatcher.list().await {
                                    let has_active = list
                                        .iter()
                                        .any(|s| matches!(s.state, DownloadState::Downloading));
                                    for s in &list {
                                        if let Ok(task_id) =
                                            TaskId::from_legacy_string(&s.id)
                                        {
                                            if has_active
                                                && matches!(s.state, DownloadState::Downloading)
                                            {
                                                let _ = dispatcher.pause(&task_id).await;
                                            } else if !has_active
                                                && matches!(s.state, DownloadState::Paused)
                                            {
                                                let _ = dispatcher.resume(&task_id).await;
                                            }
                                        }
                                    }
                                }
                            });
                        }
                        "speed_limit" => {
                            tauri::async_runtime::spawn(async move {
                                use download::manager::DownloadManager;

                                let state = app.state::<AppState>();
                                let new_limit = {
                                    let mut settings = state.settings.write();
                                    if settings.global_speed_limit_bps > 0 {
                                        settings.global_speed_limit_bps = 0;
                                        0
                                    } else {
                                        settings.global_speed_limit_bps = 1_048_576;
                                        1_048_576
                                    }
                                };
                                if new_limit > 0
                                    && let Some(dm) =
                                        state.registry.get_typed::<DownloadManager>()
                                {
                                    let s = state.settings.read().clone();
                                    let _ = dm.apply_settings(s).await;
                                }
                            });
                        }
                        "open_dir" => {
                            let state = app.state::<AppState>();
                            let dir = state
                                .settings
                                .read()
                                .download
                                .default_download_dir
                                .clone();
                            if !dir.is_empty() {
                                let _ = tauri_plugin_opener::open_path(dir, None::<&str>);
                            }
                        }
                        "game_mode" => {
                            tauri::async_runtime::spawn(async move {
                                use download::manager::DownloadManager;

                                let state = app.state::<AppState>();
                                if let Some(dm) = state.registry.get_typed::<DownloadManager>() {
                                    let current = dm.game_mode();
                                    dm.set_game_mode(!current);
                                }
                            });
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    }
                });
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let handle = window.app_handle().clone();
                let state = window.state::<AppState>();
                let registry = state.registry.clone();
                let settings = state.settings.read().clone();

                if settings.appearance.close_behavior == CloseBehavior::MinimizeToTray {
                    // Minimize to tray instead of exiting
                    if let Some(win) = handle.get_webview_window("main") {
                        let _ = win.hide();
                    }
                } else {
                    // Exit completely
                    tauri::async_runtime::spawn(async move {
                        registry.shutdown_all().await;
                        handle.exit(0);
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            download_start,
            download_pause,
            download_resume,
            download_cancel,
            download_remove,
            download_purge,
            download_open_in_explorer,
            download_status,
            download_list,
            download_set_priority,
            bt_runtime_status,
            bt_set_speed_limit,
            bt_preview_torrent,
            bt_get_peers,
            bt_get_trackers,
            bt_get_pieces,
            get_bt_files,
            update_bt_files,
            settings_fetch_tracker_list,
            settings_get,
            settings_save,
            cdn_fetch_ranges,
            cdn_test,
            cdn_apply,
            cdn_clear,
            cdn_status,
            cdn_cancel,
            cdn_detail,
            cdn_candidates,
            toggle_game_mode,
            get_io_status,
            toggle_overclock_mode,
            get_overclock_mode,
            detect_disk_type,
            update_tray_language,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = run_result {
        eprintln!("[limedl] tauri runtime failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use download::event_bus::DownloadEvent;

    // ── Sample event helpers ──────────────────────────────────────────────

    fn ev_updated() -> DownloadEvent {
        DownloadEvent::Updated {
            id: "t1".into(),
            summary_json: serde_json::json!({
                "id": "t1",
                "status": "completed",
                "progress": 1.0,
            }),
        }
    }

    fn ev_progress() -> DownloadEvent {
        DownloadEvent::Progress {
            id: "t1".into(),
            progress_json: serde_json::json!({
                "id": "t1",
                "downloaded": 1024,
                "speedBps": 512000,
            }),
        }
    }

    fn ev_aria2() -> DownloadEvent {
        DownloadEvent::Aria2Notification {
            event_name: "bt-on-download-complete".into(),
            gid: "gid-abc123".into(),
        }
    }

    fn ev_cdn_progress() -> DownloadEvent {
        DownloadEvent::CdnProgress {
            phase: "probing".into(),
            current: 3,
            total: 10,
        }
    }

    fn ev_cdn_complete_some() -> DownloadEvent {
        DownloadEvent::CdnComplete {
            state: "completed".into(),
            active_ip: Some("1.2.3.4".into()),
            active_speed_mbps: Some(50.0),
        }
    }

    fn ev_cdn_complete_none() -> DownloadEvent {
        DownloadEvent::CdnComplete {
            state: "failed".into(),
            active_ip: None,
            active_speed_mbps: None,
        }
    }

    fn ev_warning() -> DownloadEvent {
        DownloadEvent::Warning {
            id: "t1".into(),
            message: "Connection reset by peer".into(),
        }
    }

    // ── map_event_to_emit tests ──────────────────────────────────────────

    #[test]
    fn map_updated_returns_correct_event_name_and_passthrough_payload() {
        let event = ev_updated();
        let (name, payload) = map_event_to_emit(&event);
        assert_eq!(name, "download-updated");
        assert_eq!(payload["id"], "t1");
        assert_eq!(payload["status"], "completed");
        assert_eq!(payload["progress"], 1.0);
    }

    #[test]
    fn map_progress_returns_correct_event_name_and_passthrough_payload() {
        let event = ev_progress();
        let (name, payload) = map_event_to_emit(&event);
        assert_eq!(name, "download-progress");
        assert_eq!(payload["id"], "t1");
        assert_eq!(payload["downloaded"], 1024);
        assert_eq!(payload["speedBps"], 512000);
    }

    #[test]
    fn map_aria2_notification_preserves_dynamic_event_name() {
        let event = ev_aria2();
        let (name, payload) = map_event_to_emit(&event);
        assert_eq!(name, "bt-on-download-complete");
        // gid is emitted as a raw JSON string value
        assert_eq!(payload, serde_json::json!("gid-abc123"));
    }

    #[test]
    fn map_cdn_progress_returns_correct_event_name_and_fields() {
        let event = ev_cdn_progress();
        let (name, payload) = map_event_to_emit(&event);
        assert_eq!(name, "cdn-test-progress");
        assert_eq!(payload["phase"], "probing");
        assert_eq!(payload["current"], 3);
        assert_eq!(payload["total"], 10);
    }

    #[test]
    fn map_cdn_complete_with_values_uses_camel_case_fields() {
        let event = ev_cdn_complete_some();
        let (name, payload) = map_event_to_emit(&event);
        assert_eq!(name, "cdn-test-complete");
        assert_eq!(payload["state"], "completed");
        assert_eq!(payload["activeIp"], "1.2.3.4");
        assert_eq!(payload["activeSpeedMbps"], 50.0);
    }

    #[test]
    fn map_cdn_complete_with_nulls_emits_null_fields() {
        let event = ev_cdn_complete_none();
        let (name, payload) = map_event_to_emit(&event);
        assert_eq!(name, "cdn-test-complete");
        assert_eq!(payload["state"], "failed");
        assert!(payload["activeIp"].is_null());
        assert!(payload["activeSpeedMbps"].is_null());
    }

    #[test]
    fn map_warning_returns_correct_event_name_and_fields() {
        let event = ev_warning();
        let (name, payload) = map_event_to_emit(&event);
        assert_eq!(name, "download-warning");
        assert_eq!(payload["id"], "t1");
        assert_eq!(payload["message"], "Connection reset by peer");
    }

    #[test]
    fn map_updated_null_payload_round_trips() {
        let event = DownloadEvent::Updated {
            id: "t1".into(),
            summary_json: serde_json::Value::Null,
        };
        let (name, payload) = map_event_to_emit(&event);
        assert_eq!(name, "download-updated");
        assert!(payload.is_null());
    }

    #[test]
    fn map_all_variants_have_unique_event_names() {
        let events: [DownloadEvent; 8] = [
            ev_updated(),
            ev_progress(),
            ev_aria2(),
            ev_cdn_progress(),
            ev_cdn_complete_some(),
            ev_cdn_complete_none(),
            ev_warning(),
            // Another Aria2Notification with a different dynamic event name
            DownloadEvent::Aria2Notification {
                event_name: "bt-on-download-start".into(),
                gid: "gid-xyz".into(),
            },
        ];
        let names: Vec<&str> = events
            .iter()
            .map(|e| map_event_to_emit(e).0)
            .collect();
        // The first 6 should be unique (Aria2Notification is dynamic so duplicates possible)
        assert_eq!(names[0], "download-updated");
        assert_eq!(names[1], "download-progress");
        assert_eq!(names[3], "cdn-test-progress");
        assert_eq!(names[4], "cdn-test-complete");
        assert_eq!(names[5], "cdn-test-complete");
        assert_eq!(names[6], "download-warning");
        // Dynamic event names preserved
        assert_eq!(names[2], "bt-on-download-complete");
        assert_eq!(names[7], "bt-on-download-start");
    }
}
