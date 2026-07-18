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
use tokio::time::sleep;

use download::event_bus::EventBus;
use download::CdnAccelerator;
use download::{
    cleanup_old_aria2_temp_files, AppState, Aria2RpcServer, DownloadManager,
    IrontideBtBackend, bt_get_peers, bt_get_pieces, bt_get_trackers,
    bt_preview_torrent, bt_runtime_status, bt_set_speed_limit, cdn_apply, cdn_cancel,
    cdn_candidates, cdn_clear, cdn_detail, cdn_fetch_ranges, cdn_status, cdn_test,
    download_cancel, download_list, download_open_in_explorer, download_pause, download_purge,
    download_remove, download_resume, download_start, download_status, get_bt_files,
    get_io_status, get_overclock_mode, init_logging, settings_fetch_tracker_list, settings_get,
    settings_save, toggle_game_mode, toggle_overclock_mode, update_bt_files,
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
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;

            // Hide window when launched via autostart
            let is_autostart = std::env::args().any(|a| a == "--hidden");
            if is_autostart
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.hide();
            }

            app.handle().plugin(tauri_plugin_autostart::Builder::new()
                .args(["--hidden"])
                .build())?;

            (|| -> anyhow::Result<()> {
                let state_dir = app
                    .path()
                    .app_local_data_dir()
                    .or_else(|_| app.path().app_data_dir())
                    .unwrap_or_else(|_| std::env::temp_dir().join("flareget"))
                    .join("downloads");

                std::fs::create_dir_all(&state_dir)
                    .with_context(|| format!("创建下载状态目录失败: {}", state_dir.display()))?;

                let rate_limiter = Arc::new(RateLimiter::default());

                let event_bus = Arc::new(EventBus::new(256));
                event_bus.set_app_handle(app.handle().clone());

                let download_manager = DownloadManager::new(
                    state_dir.clone(),
                    rate_limiter.clone(),
                    event_bus.clone(),
                )
                .with_context(|| format!("初始化下载管理器失败: {}", state_dir.display()))?;
                let download_manager = Arc::new(download_manager);
                download_manager.clone().start_scheduler_loop();

                let settings = download_manager.initial_settings();
                init_logging(&settings.logging, &state_dir).context("初始化日志系统失败")?;
                cleanup_old_aria2_temp_files();

                let bt_backend: Arc<IrontideBtBackend> = {
                    let own = tauri::async_runtime::block_on(IrontideBtBackend::new(
                        &settings,
                        state_dir.join("torrents"),
                        state_dir.join("bt_files"),
                        event_bus.clone(),
                    ))
                    .context("初始化 irontide BT 后端失败")?;
                    Arc::new(own)
                };

                // spawn_upload_policy_loop() calls tokio::spawn internally, which
                // requires an active Tokio runtime context. The setup closure runs
                // on the main thread outside any runtime, so enter Tauri's async
                // runtime context before invoking it.
                tauri::async_runtime::block_on(async {
                    bt_backend.clone().spawn_upload_policy_loop();
                });

                let cdn_accelerator = Arc::new(CdnAccelerator::new());
                download_manager.set_cdn_accelerator(cdn_accelerator.clone());

                // Restore CDN acceleration state from persisted settings so
                // downloads can use the previously-selected IP immediately on restart.
                {
                    let initial = download_manager.initial_settings();
                    tauri::async_runtime::block_on(cdn_accelerator.init_from_settings(&initial));
                }

                let app_handle = app.handle().clone();

                let rpc_shutdown = Arc::new(Mutex::new(None::<tokio::sync::watch::Sender<bool>>));

                // Create the backend registry that routes protocol methods to the correct backend.
                use download::types::TaskKind;
                use download::backend_registry::BackendRegistry;

                let mut registry = BackendRegistry::new();
                registry.register(TaskKind::Http, (*download_manager).clone());
                registry.register(TaskKind::Bt, (*bt_backend).clone());
                let registry = Arc::new(registry);

                // Start the alert bridge for the irontide backend.
                tauri::async_runtime::block_on(bt_backend.setup_alert_bridge());

                app.manage(AppState {
                    registry: registry.clone(),
                    event_bus: event_bus.clone(),
                    rate_limiter: rate_limiter.clone(),
                    cdn_accelerator: cdn_accelerator.clone(),
                    app_handle: app_handle.clone(),
                    rpc_shutdown: rpc_shutdown.clone(),
                });

                {
                    let state = AppState {
                        registry: registry.clone(),
                        event_bus: event_bus.clone(),
                        rate_limiter: rate_limiter.clone(),
                        cdn_accelerator: cdn_accelerator.clone(),
                        app_handle: app_handle.clone(),
                        rpc_shutdown: rpc_shutdown.clone(),
                    };
                    tauri::async_runtime::spawn(async move {
                        loop {
                            sleep(Duration::from_secs(300)).await;
                            state.emit_all_downloads().await;
                        }
                    });
                }

                if settings.aria2_rpc.enabled {
                    let (tx, rx) = tokio::sync::watch::channel(false);
                    let rpc_server = Aria2RpcServer::new(
                        registry.clone(),
                        &settings.aria2_rpc,
                        event_bus.clone(),
                    );
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = rpc_server.serve(rx).await {
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

                let tray = TrayIconBuilder::new()
                    .icon(app.default_window_icon().cloned().unwrap())
                    .tooltip("flareget")
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
                    })
                    .build(app)?;

                let show_item = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
                let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
                let menu = MenuBuilder::new(app)
                    .item(&show_item)
                    .item(&quit_item)
                    .build()?;
                tray.set_menu(Some(menu))?;

                let app_handle_menu = app.handle().clone();
                tray.on_menu_event(move |_tray, event| {
                    match event.id().as_ref() {
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
                tauri::async_runtime::spawn(async move {
                    for backend in registry.iter() {
                        let _ = backend.shutdown().await;
                    }
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
        eprintln!("[flareget] tauri runtime failed: {error}");
    }
}
