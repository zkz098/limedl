//! Resolve the CLI into a concrete [`Config`]: data directory, RPC overrides and
//! logging level.
//!
//! The data-directory precedence deliberately matches the desktop
//! (`crates/limedl-native/src/paths.rs`): an explicit flag, then
//! `$LIMEDL_DATA_DIR`, then the platform local data dir, then a temp fallback.
//! That way a user can point the daemon at an existing desktop profile (after
//! stopping the desktop, which shares the SQLite database and BT state).

use std::path::{Path, PathBuf};

use anyhow::bail;
use limedl_core::types::LogLevel;

use crate::cli::Cli;

/// RPC fields a flag can override for this run. `None` means "use settings.json".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcOverrides {
    pub listen_host: Option<String>,
    pub port: Option<u16>,
    pub secret: Option<String>,
    pub allow_any_origin: bool,
    pub extra_origins: Vec<String>,
}

/// Fully resolved daemon configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Holds `settings.json` and the instance lock.
    pub data_dir: PathBuf,
    /// Holds `downloads.db`, the BT state and the log directory. Always
    /// `data_dir/downloads`, matching `SystemContext`'s settings-path rule.
    pub state_dir: PathBuf,
    pub rpc: RpcOverrides,
    pub download_dir: Option<String>,
    pub log_level: Option<LogLevel>,
}

impl Config {
    pub fn from_cli(cli: Cli) -> anyhow::Result<Self> {
        let data_dir = resolve_data_dir(cli.data_dir.as_deref());
        let state_dir = data_dir.join("downloads");

        let download_dir = match cli.download_dir.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(dir) => {
                if !Path::new(dir).is_absolute() {
                    bail!("--download-dir must be an absolute path: {dir}");
                }
                Some(dir.to_string())
            }
        };

        // `LIMEDL_RPC_SECRET` keeps the token out of the process list.
        let secret = cli
            .rpc_secret
            .or_else(|| std::env::var("LIMEDL_RPC_SECRET").ok())
            .map(|secret| secret.trim().to_string())
            .filter(|secret| !secret.is_empty());

        let listen_host = cli
            .rpc_listen
            .map(|host| host.trim().to_string())
            .filter(|host| !host.is_empty());

        Ok(Config {
            data_dir,
            state_dir,
            rpc: RpcOverrides {
                listen_host,
                port: cli.rpc_port,
                secret,
                allow_any_origin: cli.rpc_allow_origin_all,
                extra_origins: cli
                    .rpc_allowed_origins
                    .into_iter()
                    .map(|origin| origin.trim().to_string())
                    .filter(|origin| !origin.is_empty())
                    .collect(),
            },
            download_dir,
            log_level: cli.log_level.map(LogLevel::from),
        })
    }
}

/// Resolve the base data directory with the documented precedence.
pub fn resolve_data_dir(override_dir: Option<&Path>) -> PathBuf {
    if let Some(dir) = override_dir {
        return dir.to_path_buf();
    }
    if let Some(dir) = std::env::var_os("LIMEDL_DATA_DIR") {
        return PathBuf::from(dir);
    }
    platform_data_dir()
        .map(|base| base.join("limedl"))
        .unwrap_or_else(|| std::env::temp_dir().join("limedl"))
}

fn platform_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn no_flags_leave_settings_in_charge() {
        let cli = Cli::try_parse_from(["limedl-server"]).expect("parse");
        let config = Config::from_cli(cli).expect("config");

        assert!(config.rpc.listen_host.is_none());
        assert!(config.rpc.port.is_none());
        assert!(!config.rpc.allow_any_origin);
        assert!(config.rpc.extra_origins.is_empty());
        assert!(config.download_dir.is_none());
        assert!(config.log_level.is_none());
        // The lock and settings.json live one level above the engine state.
        assert_eq!(config.state_dir, config.data_dir.join("downloads"));
    }

    #[test]
    fn flags_override_and_are_trimmed() {
        let cli = Cli::try_parse_from([
            "limedl-server",
            "--data-dir",
            "/srv/limedl",
            "--rpc-listen",
            "  0.0.0.0  ",
            "--rpc-port",
            "6801",
            "--rpc-secret",
            "  s3cret  ",
            "--rpc-allow-origin-all",
            "--rpc-allow-origin",
            "  http://app.example  ",
            "--rpc-allow-origin",
            "   ",
            "--download-dir",
            "/srv/downloads",
            "--log-level",
            "debug",
        ])
        .expect("parse");
        let config = Config::from_cli(cli).expect("config");

        assert_eq!(config.data_dir, PathBuf::from("/srv/limedl"));
        assert_eq!(config.rpc.listen_host.as_deref(), Some("0.0.0.0"));
        assert_eq!(config.rpc.port, Some(6801));
        assert_eq!(config.rpc.secret.as_deref(), Some("s3cret"));
        assert!(config.rpc.allow_any_origin);
        assert_eq!(
            config.rpc.extra_origins,
            vec!["http://app.example".to_string()],
            "blank origins must be dropped"
        );
        assert_eq!(config.download_dir.as_deref(), Some("/srv/downloads"));
        assert_eq!(config.log_level, Some(LogLevel::Debug));
    }

    #[test]
    fn relative_download_dir_is_rejected() {
        let cli =
            Cli::try_parse_from(["limedl-server", "--download-dir", "relative/dir"]).expect("parse");
        let error = Config::from_cli(cli).expect_err("relative path must fail");
        assert!(error.to_string().contains("absolute path"), "{error}");
    }

    #[test]
    fn resolve_data_dir_prefers_the_explicit_override() {
        assert_eq!(
            resolve_data_dir(Some(Path::new("/tmp/limedl-explicit"))),
            PathBuf::from("/tmp/limedl-explicit")
        );
    }
}
