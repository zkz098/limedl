//! Headless limedl daemon.
//!
//! `limedl-server` runs the same engine as the desktop client (`bootstrap`) but
//! with no GUI, and exposes the Aria2 JSON-RPC API so a web console such as
//! AriaNg can drive it from another machine. The only frontend is the RPC
//! server: it is force-enabled regardless of `settings.json`, `aria2.shutdown`
//! stops the process, and a non-loopback listener is refused unless
//! authentication is configured.
//!
//! Deployment notes and a systemd example live in `docs/server-daemon.md`.

pub mod cli;
pub mod config;

use std::future::Future;
use std::path::Path;

use anyhow::Context as _;
use clap::Parser as _;
use limedl_core::aria2_rpc::Aria2RpcServer;
use limedl_core::bootstrap::bootstrap;
use limedl_core::init_logging;
use limedl_core::types::{Aria2AuthMode, LogSettings};

pub use cli::Cli;
pub use config::{Config, RpcOverrides, resolve_data_dir};

/// Parse `argv` and run until a termination signal or `aria2.shutdown`.
pub async fn main_entry() -> anyhow::Result<()> {
    let config = Config::from_cli(Cli::parse())?;
    run_with(config, shutdown_signal()).await
}

/// Run the daemon with an externally supplied shutdown future.
///
/// Split from [`main_entry`] so an integration test can start a real daemon and
/// drive it over JSON-RPC without raising a process signal.
pub async fn run_with(config: Config, shutdown: impl Future<Output = ()>) -> anyhow::Result<()> {
    let Config {
        data_dir,
        state_dir,
        rpc,
        download_dir,
        log_level,
    } = config;

    tokio::fs::create_dir_all(&state_dir)
        .await
        .with_context(|| format!("create the state directory {}", state_dir.display()))?;

    // Install logging before bootstrap so engine start-up messages are kept.
    let mut early_logging = LogSettings::default();
    if let Some(level) = log_level {
        early_logging.level = level;
    }
    init_logging(&early_logging, &state_dir).context("initialize logging")?;

    // One daemon per data directory: two would fight over the SQLite database,
    // the BitTorrent state and the RPC port. Held for the process lifetime.
    let _instance_lock = acquire_instance_lock(&data_dir)?;

    let core = bootstrap(state_dir.clone())
        .await
        .context("bootstrap the limedl engine")?;

    // `--download-dir` persists because some aria2 clients omit `dir`, and a
    // first headless run has no configured download directory.
    if let Some(dir) = &download_dir {
        core.dispatcher
            .save_settings_with(|settings| {
                settings.download.default_download_dir = dir.clone();
                Ok(())
            })
            .await
            .context("persist --download-dir")?;
    }

    // Re-apply logging with the on-disk settings; the CLI level wins.
    let mut logging = core.settings.logging.clone();
    if let Some(level) = log_level {
        logging.level = level;
    }
    init_logging(&logging, &state_dir).context("apply logging settings")?;

    let mut rpc_settings = core.settings.aria2_rpc.clone();
    // The RPC endpoint is the daemon's only interface: always on.
    rpc_settings.enabled = true;
    // aria2 semantics: `aria2.shutdown` stops the process. The desktop keeps
    // this off because it is a managed subsystem.
    rpc_settings.exit_on_shutdown = true;
    if let Some(host) = rpc.listen_host {
        rpc_settings.listen_address = host;
    }
    if let Some(port) = rpc.port {
        rpc_settings.port = port;
    }
    if let Some(secret) = rpc.secret {
        rpc_settings.auth_mode = Aria2AuthMode::Single;
        rpc_settings.secret = Some(secret);
    }
    rpc_settings.allow_any_origin |= rpc.allow_any_origin;
    rpc_settings.cors_allowed_origins.extend(rpc.extra_origins);

    let server = Aria2RpcServer::new(core.registry.clone(), &rpc_settings, core.event_bus.clone());
    let shutdown_notify = server.shutdown_notify();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    tracing::info!(bind = server.bind_addr(), "limedl-server starting");

    let mut serve_task = tokio::spawn(server.serve(shutdown_rx));
    let mut requested = false;
    // `serve` can also finish on its own (bind failure); treat that as the
    // result instead of waiting for a signal that will never come.
    let early_exit = tokio::select! {
        joined = &mut serve_task => Some(joined),
        _ = shutdown => {
            tracing::info!("shutdown signal received");
            requested = true;
            None
        }
        _ = shutdown_notify.notified() => {
            tracing::info!("aria2.shutdown requested");
            requested = true;
            None
        }
    };
    if requested {
        let _ = shutdown_tx.send(true);
    }

    let serve_result = match early_exit {
        Some(joined) => joined,
        None => serve_task.await,
    }
    .map_err(|error| anyhow::anyhow!("the RPC server task panicked: {error}"))?;

    // Flush manifests, checkpoint the WAL and release the BT session even when
    // the server failed.
    core.registry.shutdown_all().await;

    serve_result.context("serve the Aria2 RPC endpoint")
}

/// Take an exclusive advisory lock on `<data_dir>/limedl-server.lock`.
///
/// The lock is released by the OS when the process exits, so a crashed daemon
/// does not leave a stale lock behind.
fn acquire_instance_lock(data_dir: &Path) -> anyhow::Result<std::fs::File> {
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("create the data directory {}", data_dir.display()))?;
    let path = data_dir.join("limedl-server.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    file.try_lock().map_err(|_| {
        anyhow::anyhow!(
            "another limedl-server is already using {} (lock held on {})",
            data_dir.display(),
            path.display()
        )
    })?;
    Ok(file)
}

/// Resolve on SIGTERM or SIGINT, the signals a service manager sends.
#[cfg(unix)]
pub async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let (Ok(mut terminate), Ok(mut interrupt)) =
        (signal(SignalKind::terminate()), signal(SignalKind::interrupt()))
    else {
        tracing::warn!("could not install the SIGTERM/SIGINT handlers; falling back to Ctrl+C");
        let _ = tokio::signal::ctrl_c().await;
        return;
    };
    let received = tokio::select! {
        _ = terminate.recv() => "SIGTERM",
        _ = interrupt.recv() => "SIGINT",
    };
    tracing::info!("received {received}");
}

/// Windows has no SIGTERM to catch here; Ctrl+C is the console path.
#[cfg(not(unix))]
pub async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two daemons pointed at one data directory would race on the SQLite
    /// database and the RPC port; the advisory lock is the guard.
    #[test]
    fn instance_lock_is_exclusive_and_reusable() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = acquire_instance_lock(tmp.path()).expect("first lock");

        let second = acquire_instance_lock(tmp.path());
        assert!(
            second.is_err(),
            "a second daemon must not start on the same data directory"
        );

        drop(first);
        acquire_instance_lock(tmp.path()).expect("the lock must be reusable after release");
    }
}
