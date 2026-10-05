#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

slint::include_modules!();

mod autostart;
mod bridge;
mod context;
mod crash;
mod event_stream;
mod handlers;
mod i18n;
mod paths;
mod platform_adapter;
mod platform_win;
mod power;
mod protocol;
mod renderer;
mod settings_sync;
mod single_instance;
mod task_ops;
mod toast;
mod tray;
mod ui_boot;
mod ui_sync;
#[cfg(test)]
mod ui_tests;
mod update;
mod url_utils;

use power::PowerGuard;
pub static POWER_GUARD: std::sync::LazyLock<PowerGuard> = std::sync::LazyLock::new(PowerGuard::new);

use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Context;
use parking_lot::Mutex;
use slint::ComponentHandle;
use tokio::sync::watch;
use tray_icon::TrayIconBuilder;

use limedl_core::aria2_rpc::Aria2RpcServer;
use limedl_core::bootstrap::bootstrap;
use limedl_core::event_bus::DownloadEvent;

use crate::i18n::Language;
use crate::tray::TRAY_INSTANCE;
use crate::ui_boot::UiState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(error) = run().await {
        // A GUI process has no console and, on Windows, no attached stderr: a
        // fatal startup error would otherwise leave the user with nothing at all.
        crash::report_startup_failure(&error);
        return Err(error);
    }
    Ok(())
}

/// Everything `main` does, so a fatal error can be reported once instead of at
/// every `?`.
async fn run() -> anyhow::Result<()> {
    // CLI contract: `limedl-native [--hidden] [<url|magnet|path|limedl://…>]`
    let args: Vec<String> = std::env::args().skip(1).collect();
    let hidden_flag = args
        .iter()
        .any(|a| matches!(a.as_str(), "--hidden" | "-hidden" | "--minimized"));
    let cli_payload = args
        .iter()
        .find(|a| !matches!(a.as_str(), "--hidden" | "-hidden" | "--minimized"))
        .cloned();

    // Initialize core directories first: the single-instance claim and the
    // activation socket live in the data directory.
    let base_dir = paths::dirs_or_temp_dir();
    platform_win::set_base_dir(base_dir.clone());
    let state_dir = base_dir.join("downloads");
    update::clean_update_work_dir(&base_dir);
    tokio::fs::create_dir_all(&state_dir).await?;

    // Single-instance guard
    let instance_claim = single_instance::InstanceClaim::claim(&base_dir);
    if instance_claim.is_secondary() {
        if instance_claim.notify_primary(cli_payload.as_deref()) {
            return Ok(());
        }
        // Never exit with nothing on screen: something holds the instance claim
        // but did not answer, so report it through the startup-error path.
        anyhow::bail!(
            "another limedl instance is running but did not accept the activation request; \
             close it (or wait for it to finish starting) and try again"
        );
    }

    // Everything below can panic, and a GUI process has nowhere to print it:
    // install the crash reporter before the first thing that can fail.
    crash::install_panic_hook(&state_dir);

    // Logging comes up before the engine: bootstrap loads settings and migrates
    // the database, and a failure in either used to be reported to a subscriber
    // that did not exist yet — the one class of failure that most needs a log.
    // The configured settings are re-applied as soon as they are known.
    limedl_core::init_logging(&limedl_core::types::LogSettings::default(), &state_dir)
        .with_context(|| "初始化日志失败")?;

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
    tracing::info!(
        "启动 limedl Native 桌面客户端 ({})...",
        renderer::NAME
    );

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
        tokio::spawn(async move {
            if let Err(e) = rpc_server.serve(rx).await {
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
    // Only ask the core when the user has not pinned a directory: the lookup
    // walks the service layer and is skipped entirely in the configured case.
    let core_dir = if initial_settings.download.default_download_dir.is_empty() {
        core.dispatcher.default_download_dir().await
    } else {
        None
    };
    let default_download_dir =
        ui_boot::default_download_dir(&initial_settings, core_dir, &state_dir);

    let ui = ui_boot::build_ui(ui_boot::UiBootInputs {
        dispatcher: core.dispatcher.clone(),
        event_bus: core.event_bus.clone(),
        settings: initial_settings.clone(),
        language: initial_lang,
        default_download_dir,
        base_dir: base_dir.clone(),
        rpc_shutdown: rpc_shutdown.clone(),
        install_kind: update::detect_install_kind(),
    })?;
    let UiState { window: main_window, ctx } = ui;

    // Everything from here on needs the operating system: the real window
    // handle only exists once the event loop runs, and theme/geometry talk to
    // the windowing system. Keeping them out of `ui_boot::build_ui` is what
    // lets the UI tests build the very same window headlessly.
    platform_win::sync_window_theme(
        main_window.window(),
        initial_settings.appearance.color_mode == limedl_core::types::ColorMode::Dark,
    );

    // System Tray Icon
    //
    // The tray is an optional extra, not the primary UI: on Linux a missing
    // StatusNotifier host (GNOME without the AppIndicator extension, a bare
    // window manager) makes `build()` fail, and failing the whole launch there
    // would take the window down with it. Log the actionable hint and keep going.
    match TrayIconBuilder::new()
        .with_menu(Box::new(tray::build_tray_menu(
            initial_lang,
            ctx.tray_speed_limit_active.load(Ordering::Relaxed),
        )))
        .with_tooltip(i18n::get_tray_strings(initial_lang).tooltip)
        .with_icon(tray::create_default_tray_icon())
        .build()
    {
        Ok(tray_icon) => TRAY_INSTANCE.with(|cell| *cell.borrow_mut() = Some(tray_icon)),
        Err(error) => tracing::warn!("{}", tray::tray_init_failure_message(&error)),
    }

    // Load initial tasks from SQLite via Dispatcher
    if let Ok(initial_tasks) = core.dispatcher.list().await {
        let mut s = ctx.store.lock();
        s.replace_all(initial_tasks);
        ui_sync::refresh_ui(&main_window, &s);
    }

    // Platform hooks (drag-and-drop, WM_COPYDATA, protocol registration, activation)
    platform_adapter::setup_platform_integration(&ctx, &instance_claim);

    // Background listeners
    event_stream::start_clipboard_monitor(&ctx);
    event_stream::start_event_bus_listener(&ctx, core.event_bus.subscribe());
    event_stream::start_status_pollers(&ctx);
    event_stream::start_tray_event_loop(&ctx);

    // The BT engine starts in the background: `bootstrap()` no longer waits for
    // the irontide session, so the initial task list above ran before the
    // session existed and the torrents restored from resume data only become
    // visible once it is up. Re-publish them as `Updated` events (insert or
    // update, so a row added by a callback in the meantime is never dropped).
    //
    // `wait_for_warmup` (not `wait_ready`) is deliberate: in lightweight BT mode
    // the engine may legitimately stay unloaded, and a read path must not turn
    // that into a session startup. When it does stay down the persisted index
    // already supplied the rows in the initial list above.
    {
        let bt_backend = core.bt_backend.clone();
        let dispatcher = core.dispatcher.clone();
        let event_bus = core.event_bus.clone();
        tokio::spawn(async move {
            if !bt_backend.wait_for_warmup().await {
                return;
            }
            let Ok(downloads) = dispatcher.list().await else {
                return;
            };
            for summary in downloads {
                if !matches!(summary.kind, limedl_core::types::TaskKind::Bt) {
                    continue;
                }
                event_bus.publish(DownloadEvent::Updated {
                    summary: Box::new(summary),
                });
            }
        });
    }

    // Cold start with command line arguments
    if let Some(ref arg) = cli_payload {
        handlers::new_task::open_new_task_with_payload(
            arg,
            &ctx.ui_weak,
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
        ui_sync::schedule_window_placement_restore(&main_window, &base_dir);
        main_window.show()?;
    }
    install_signal_shutdown();
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

/// Ask the Slint event loop to quit.
///
/// Split out so the signal path below is a thin wrapper; also harmless when no
/// event loop is running yet (the call then fails and is ignored).
#[cfg(unix)]
fn request_event_loop_quit() {
    let _ = slint::invoke_from_event_loop(|| {
        let _ = slint::quit_event_loop();
    });
}

/// Turn SIGTERM/SIGINT into a normal quit.
///
/// Without this the process dies where it stands: no buffer flush, no manifest
/// persist, no WAL checkpoint — only the crash-recovery path would see the
/// download state. Logout, `systemctl stop` and a terminal Ctrl+C all take this
/// route.
#[cfg(unix)]
fn install_signal_shutdown() {
    use tokio::signal::unix::{SignalKind, signal};

    tokio::spawn(async move {
        let (Ok(mut terminate), Ok(mut interrupt)) =
            (signal(SignalKind::terminate()), signal(SignalKind::interrupt()))
        else {
            tracing::warn!("could not install the SIGTERM/SIGINT handlers");
            return;
        };
        let received = tokio::select! {
            _ = terminate.recv() => "SIGTERM",
            _ = interrupt.recv() => "SIGINT",
        };
        tracing::info!("received {received}; shutting the engine down cleanly");
        request_event_loop_quit();
    });
}

/// Windows has no signal to catch here (`windows_subsystem = "windows"` means no
/// console and therefore no Ctrl+C); the window close handler owns shutdown.
#[cfg(not(unix))]
fn install_signal_shutdown() {}

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

    /// The SIGTERM/SIGINT watcher calls this from a worker task, possibly before
    /// the event loop is up: it must fail quietly instead of panicking.
    #[cfg(unix)]
    #[test]
    fn quitting_without_an_event_loop_is_a_no_op() {
        request_event_loop_quit();
    }
}
