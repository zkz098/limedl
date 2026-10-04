//! Command-line surface of `limedl-server`.
//!
//! Every flag overrides the matching `settings.json` field for this run only
//! (except `--download-dir`, which is persisted — see [`crate::run_with`]). With
//! no flags the daemon behaves like the desktop: settings in the data
//! directory, loopback RPC. A LAN deployment passes `--rpc-listen` and
//! `--rpc-secret` together.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use limedl_core::types::LogLevel;

#[derive(Debug, Parser)]
#[command(
    name = "limedl-server",
    version,
    about = "Headless limedl daemon: Aria2 JSON-RPC over the limedl engine (NAS / server)",
    long_about = "Runs the limedl download engine without a GUI and exposes the Aria2 \
                  JSON-RPC API, so AriaNg, Motrix and other aria2 clients can drive it.\n\n\
                  The listener defaults to 127.0.0.1. A non-loopback address requires \
                  authentication: pass --rpc-secret (or configure per-client tokens in \
                  settings.json), otherwise the daemon refuses to start."
)]
pub struct Cli {
    /// Data directory holding settings.json, downloads.db and torrent state.
    /// Falls back to $LIMEDL_DATA_DIR, then the platform data directory.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,

    /// Address to bind the Aria2 RPC listener to. Use `0.0.0.0` for LAN access.
    #[arg(long, value_name = "ADDR")]
    pub rpc_listen: Option<String>,

    /// TCP port for the Aria2 RPC listener.
    #[arg(long, value_name = "PORT")]
    pub rpc_port: Option<u16>,

    /// Shared secret required as `token:<secret>` on every request.
    /// Also read from $LIMEDL_RPC_SECRET when the flag is absent.
    #[arg(long, value_name = "TOKEN")]
    pub rpc_secret: Option<String>,

    /// Allow every CORS origin (`Access-Control-Allow-Origin: *`). Needed when
    /// AriaNg is served from a different host than the RPC endpoint.
    #[arg(long)]
    pub rpc_allow_origin_all: bool,

    /// Add one allowed CORS origin. Repeatable.
    #[arg(long = "rpc-allow-origin", value_name = "ORIGIN")]
    pub rpc_allowed_origins: Vec<String>,

    /// Default download directory. Overrides and persists the setting, because
    /// some aria2 clients omit `dir` and a headless first run has no Downloads
    /// folder to fall back to.
    #[arg(long, value_name = "PATH")]
    pub download_dir: Option<String>,

    /// Log level override (also controls the on-disk log file).
    #[arg(long, value_enum, value_name = "LEVEL")]
    pub log_level: Option<LogLevelArg>,
}

/// `clap`-friendly mirror of [`LogLevel`], which lives in `limedl-core` and does
/// not depend on `clap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevelArg {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevelArg> for LogLevel {
    fn from(level: LogLevelArg) -> Self {
        match level {
            LogLevelArg::Trace => LogLevel::Trace,
            LogLevelArg::Debug => LogLevel::Debug,
            LogLevelArg::Info => LogLevel::Info,
            LogLevelArg::Warn => LogLevel::Warn,
            LogLevelArg::Error => LogLevel::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_maps_to_every_core_variant() {
        for (arg, expected) in [
            (LogLevelArg::Trace, LogLevel::Trace),
            (LogLevelArg::Debug, LogLevel::Debug),
            (LogLevelArg::Info, LogLevel::Info),
            (LogLevelArg::Warn, LogLevel::Warn),
            (LogLevelArg::Error, LogLevel::Error),
        ] {
            assert_eq!(LogLevel::from(arg), expected);
        }
    }

    #[test]
    fn cli_accepts_the_documented_flags() {
        let cli = Cli::try_parse_from([
            "limedl-server",
            "--data-dir",
            "/srv/limedl",
            "--rpc-listen",
            "0.0.0.0",
            "--rpc-port",
            "6801",
            "--rpc-secret",
            "s3cret",
            "--rpc-allow-origin-all",
            "--rpc-allow-origin",
            "http://app.example",
            "--download-dir",
            "/srv/downloads",
            "--log-level",
            "debug",
        ])
        .expect("valid flags must parse");

        assert_eq!(cli.data_dir.as_deref(), Some(std::path::Path::new("/srv/limedl")));
        assert_eq!(cli.rpc_listen.as_deref(), Some("0.0.0.0"));
        assert_eq!(cli.rpc_port, Some(6801));
        assert_eq!(cli.rpc_secret.as_deref(), Some("s3cret"));
        assert!(cli.rpc_allow_origin_all);
        assert_eq!(cli.rpc_allowed_origins, vec!["http://app.example".to_string()]);
        assert_eq!(cli.download_dir.as_deref(), Some("/srv/downloads"));
        assert_eq!(cli.log_level, Some(LogLevelArg::Debug));
    }
}
