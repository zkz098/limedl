#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

slint::include_modules!();

mod autostart;
mod bridge;
mod context;
mod event_stream;
mod handlers;
mod i18n;
mod migrate;
mod paths;
mod platform_adapter;
mod platform_win;
mod power;
mod protocol;
mod single_instance;
mod task_ops;
mod toast;
mod tray;
mod ui_sync;
mod update;
mod url_utils;

use power::PowerGuard;
pub static POWER_GUARD: std::sync::LazyLock<PowerGuard> = std::sync::LazyLock::new(PowerGuard::new);

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Context;
use parking_lot::Mutex;
use slint::{ComponentHandle, SharedString};
use tokio::sync::watch;
use tray_icon::TrayIconBuilder;

use limedl_core::aria2_rpc::Aria2RpcServer;
use limedl_core::bootstrap::bootstrap;
use limedl_core::types::SortDirection;

use crate::bridge::TaskStore;
use crate::context::AppContext;
use crate::i18n::Language;
use crate::tray::TRAY_INSTANCE;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // CLI contract: `limedl-native [--hidden] [<url|magnet|path|limedl://…>]`
    let args: Vec<String> = std::env::args().skip(1).collect();
    let hidden_flag = args
        .iter()
        .any(|a| matches!(a.as_str(), "--hidden" | "-hidden" | "--minimized"));
    let cli_payload = args
        .iter()
        .find(|a| !matches!(a.as_str(), "--hidden" | "-hidden" | "--minimized"))
        .cloned();

    // Single-instance guard
    let instance_claim = single_instance::InstanceClaim::claim();
    if instance_claim.is_secondary() {
        instance_claim.notify_primary(cli_payload.as_deref());
        return Ok(());
    }

    // Initialize core directories
    let base_dir = paths::dirs_or_temp_dir();
    platform_win::set_base_dir(base_dir.clone());
    let state_dir = base_dir.join("downloads");
    update::clean_update_work_dir(&base_dir);
    std::fs::create_dir_all(&state_dir)?;

    // First Native run: migrate data from Tauri
    let migration_report = migrate::migrate_tauri_data_if_needed(&base_dir, &state_dir);

    // Bootstrap download core
    let core = bootstrap(state_dir.clone())
        .await
        .with_context(|| "初始化核心引擎失败")?;

    let initial_settings = core
        .dispatcher
        .get_settings()
        .await
        .unwrap_or_default();

    limedl_core::init_logging(&initial_settings.logging, &state_dir)
        .with_context(|| "初始化日志失败")?;
    tracing::info!("启动 limedl Native 桌面客户端 (Skia)...");

    if let Some(report) = migration_report.as_ref() {
        tracing::info!(
            "Tauri 数据迁移完成：{} 个文件 / {} 字节（来源 {}）",
            report.copied_files,
            report.copied_bytes,
            report.source.display()
        );
    }

    let current_settings = Arc::new(Mutex::new(initial_settings.clone()));
    autostart::sync_from_settings(initial_settings.autostart);

    // Aria2 RPC
    let rpc_shutdown: Arc<Mutex<Option<watch::Sender<bool>>>> = Arc::new(Mutex::new(None));
    if initial_settings.aria2_rpc.enabled {
        let (tx, rx) = watch::channel(false);
        let rpc_server = Aria2RpcServer::new(
            core.registry.clone(),
            &initial_settings.aria2_rpc,
            core.event_bus.clone(),
        );
        let cors = initial_settings.aria2_rpc.cors_allowed_origins.clone();
        tokio::spawn(async move {
            if let Err(e) = rpc_server.serve(rx, cors).await {
                tracing::error!("Aria2 RPC server stopped: {e:#}");
            }
        });
        *rpc_shutdown.lock() = Some(tx);
    }

    let initial_lang = match initial_settings.appearance.language.as_str() {
        "en-US" => Language::EnUs,
        "zh-TW" => Language::ZhTw,
        _ => Language::ZhCn,
    };
    let store = Arc::new(Mutex::new(TaskStore::with_language(initial_lang)));
    {
        let mut s = store.lock();
        s.apply_sort(
            bridge::sort_key_to_field(initial_settings.appearance.sort_key),
            matches!(initial_settings.appearance.sort_direction, SortDirection::Asc),
        );
    }

    let default_download_dir = if !initial_settings.download.default_download_dir.is_empty() {
        initial_settings.download.default_download_dir.clone()
    } else if let Some(dir) = core.dispatcher.default_download_dir().await {
        dir
    } else if let Some(dir) = paths::dirs_download_dir() {
        dir.to_string_lossy().to_string()
    } else {
        state_dir.to_string_lossy().to_string()
    };

    let main_window = MainWindow::new()?;
    i18n::apply_translation(initial_lang);
    let ui_weak = main_window.as_weak();

    let install_kind = update::detect_install_kind();
    main_window.set_update_state(UpdateState {
        phase: "idle".into(),
        latest_version: "".into(),
        notes: "".into(),
        progress_percent: 0.0,
        progress_label: "".into(),
        error_text: "".into(),
        install_kind: match install_kind {
            update::InstallKind::Store => "store".into(),
            update::InstallKind::Installer => "installer".into(),
            update::InstallKind::Portable => "portable".into(),
        },
    });

    let is_dark = initial_settings.appearance.color_mode == limedl_core::types::ColorMode::Dark;
    ui_sync::apply_appearance(
        &main_window,
        initial_settings.appearance.color_mode.clone(),
        initial_settings.appearance.theme_color.clone(),
    );
    main_window.set_default_download_dir(SharedString::from(&default_download_dir));
    main_window.set_new_task_dir(SharedString::from(&default_download_dir));

    platform_win::sync_window_theme(main_window.window(), is_dark);
    ui_sync::schedule_window_placement_restore(&main_window, &base_dir);
    ui_sync::apply_view_preferences(&main_window, &initial_settings);

    let toast_queue = Arc::new(Mutex::new(Vec::new()));
    if let Some(report) = migration_report.as_ref() {
        ui_sync::announce_migration(report, &ui_weak, &toast_queue, initial_lang);
    }

    let game_mode_active = Arc::new(Mutex::new(core.dispatcher.game_mode()));
    let is_overclock_mode = Arc::new(Mutex::new(core.dispatcher.get_overclock_mode()));
    let tray_speed_limit_active = Arc::new(AtomicBool::new(
        initial_settings.global_speed_limit_bps > 0,
    ));

    // System Tray Icon
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray::build_tray_menu(
            initial_lang,
            tray_speed_limit_active.load(Ordering::Relaxed),
        )))
        .with_tooltip(i18n::get_tray_strings(initial_lang).tooltip)
        .with_icon(tray::create_default_tray_icon())
        .build()
        .map_err(|e| anyhow::anyhow!(tray::tray_init_failure_message(&e)))?;
    TRAY_INSTANCE.with(|cell| *cell.borrow_mut() = Some(tray_icon));

    // Load initial tasks from SQLite via Dispatcher
    if let Ok(initial_tasks) = core.dispatcher.list().await {
        let mut s = store.lock();
        s.replace_all(initial_tasks);
        ui_sync::refresh_ui(&main_window, &s);
    }

    let ctx = AppContext {
        ui: main_window.clone_strong(),
        ui_weak: ui_weak.clone(),
        dispatcher: core.dispatcher.clone(),
        event_bus: core.event_bus.clone(),
        store: store.clone(),
        current_settings: current_settings.clone(),
        toast_queue: toast_queue.clone(),
        rpc_shutdown: rpc_shutdown.clone(),
        new_task_torrent_entries: Arc::new(Mutex::new(Vec::new())),
        new_task_torrent_included: Arc::new(Mutex::new(Vec::new())),
        active_inspector_id: Arc::new(Mutex::new(None)),
        labs_expanded_ids: Arc::new(Mutex::new(HashSet::new())),
        labs_candidates: Arc::new(Mutex::new(Vec::new())),
        rewrite_rules: Arc::new(Mutex::new(initial_settings.url_rewrite.rules.clone())),
        sandbox_test_url: Arc::new(Mutex::new(
            "https://raw.github.com/user/repo/master/README.md".to_string(),
        )),
        game_mode_active: game_mode_active.clone(),
        is_overclock_mode: is_overclock_mode.clone(),
        tray_speed_limit_active: tray_speed_limit_active.clone(),
        base_dir: base_dir.clone(),
    };

    // Platform hooks (drag-and-drop, WM_COPYDATA, protocol registration, activation)
    platform_adapter::setup_platform_integration(&ctx, &instance_claim);

    // Register all UI callbacks
    handlers::register_all(&ctx);

    // Background listeners
    event_stream::start_clipboard_monitor(&ctx);
    event_stream::start_event_bus_listener(&ctx, core.event_bus.subscribe());
    event_stream::start_status_pollers(&ctx);
    event_stream::start_tray_event_loop(&ctx);

    // Cold start with command line arguments
    if let Some(ref arg) = cli_payload {
        handlers::new_task::open_new_task_with_payload(
            arg,
            &ui_weak,
            &ctx.dispatcher,
            &ctx.store,
            &ctx.new_task_torrent_entries,
            &ctx.new_task_torrent_included,
        );
    }

    // Startup visibility decision
    let msix_login_launch = !hidden_flag
        && cli_payload.is_none()
        && initial_settings.autostart
        && update::has_package_identity()
        && platform_win::launched_at_logon(ui_sync::MSIX_LOGIN_LAUNCH_WINDOW);
    let start_hidden = ui_sync::should_start_hidden(
        hidden_flag,
        msix_login_launch,
        initial_settings.setup_completed,
    );
    if start_hidden {
        tracing::info!(
            "以静默模式启动（仅托盘；来源：{}）",
            if hidden_flag { "--hidden" } else { "MSIX 登录启动" }
        );
        platform_win::set_window_visible(false);
        platform_win::trim_working_set();
    }

    if !start_hidden {
        main_window.show()?;
    }
    slint::run_event_loop_until_quit()?;

    // Graceful shutdown
    platform_win::save_current_window_geometry(main_window.window(), &base_dir);
    POWER_GUARD.release();
    TRAY_INSTANCE.with(|cell| {
        *cell.borrow_mut() = None;
    });
    tracing::info!("Native UI 正在退出，关闭核心引擎...");
    if let Some(tx) = rpc_shutdown.lock().take() {
        let _ = tx.send(true);
    }
    core.registry.shutdown_all().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_hidden_requires_a_reason_and_a_finished_setup() {
        assert!(!ui_sync::should_start_hidden(true, false, false));
        assert!(!ui_sync::should_start_hidden(false, true, false));
        assert!(ui_sync::should_start_hidden(true, false, true));
        assert!(ui_sync::should_start_hidden(false, true, true));
        assert!(!ui_sync::should_start_hidden(false, false, true));
    }
}
