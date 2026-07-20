mod download;

#[cfg(any(test, feature = "test-utils"))]
pub use download::aimd;

#[cfg(any(test, feature = "test-utils"))]
pub use download::buffer_pool;

#[cfg(any(test, feature = "test-utils"))]
pub use download::test_harness;

pub use download::RateLimiter;

use std::sync::Arc;

use parking_lot::Mutex;
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
    cleanup_old_aria2_temp_files, download_cancel, download_list, download_open_in_explorer,
    download_pause, download_purge, download_remove, download_resume, download_start,
    download_status, get_bt_files, get_io_status, get_overclock_mode, init_logging,
    settings_fetch_tracker_list, settings_get, settings_save, toggle_game_mode,
    toggle_overclock_mode, update_bt_files,
};

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
                    };
                    tauri::async_runtime::spawn(async move {
                        loop {
                            match rx.recv().await {
                                Ok(event) => match &event {
                                    DownloadEvent::Updated {
                                        id: _,
                                        summary_json,
                                    } => {
                                        let _ =
                                            app_handle_tx.emit("download-updated", summary_json);
                                    }
                                    DownloadEvent::Progress {
                                        id: _,
                                        progress_json,
                                    } => {
                                        let _ =
                                            app_handle_tx.emit("download-progress", progress_json);
                                    }
                                    DownloadEvent::Aria2Notification { event_name, gid } => {
                                        let _ = app_handle_tx.emit(event_name, gid);
                                    }
                                    DownloadEvent::CdnProgress {
                                        phase,
                                        current,
                                        total,
                                    } => {
                                        let _ = app_handle_tx.emit(
                                            "cdn-test-progress",
                                            serde_json::json!({
                                                "phase": phase,
                                                "current": current,
                                                "total": total,
                                            }),
                                        );
                                    }
                                    DownloadEvent::CdnComplete {
                                        state,
                                        active_ip,
                                        active_speed_mbps,
                                    } => {
                                        let _ = app_handle_tx.emit(
                                            "cdn-test-complete",
                                            serde_json::json!({
                                                "state": state,
                                                "activeIp": active_ip,
                                                "activeSpeedMbps": active_speed_mbps,
                                            }),
                                        );
                                    }
                                    DownloadEvent::Warning { id, message } => {
                                        let _ = app_handle_tx.emit(
                                            "download-warning",
                                            serde_json::json!({
                                                "id": id,
                                                "message": message,
                                            }),
                                        );
                                    }
                                },
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

                let mut tray_builder = TrayIconBuilder::new()
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

                let show_item = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
                let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
                let menu = MenuBuilder::new(app)
                    .item(&show_item)
                    .item(&quit_item)
                    .build()?;
                tray.set_menu(Some(menu))?;

                let app_handle_menu = app.handle().clone();
                tray.on_menu_event(move |_tray, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app_handle_menu.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app_handle_menu.exit(0);
                    }
                    _ => {}
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
                tauri::async_runtime::spawn(async move {
                    // Shutdown all backends (cancels scheduler + worker tokens, drains buffer pool)
                    registry.shutdown_all().await;
                    handle.exit(0);
                });
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
        ])
        .run(tauri::generate_context!());

    if let Err(error) = run_result {
        eprintln!("[limedl] tauri runtime failed: {error}");
    }
}
