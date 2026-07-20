use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Listen host, default "0.0.0.0"
    #[serde(default = "default_host")]
    pub host: String,
    /// Listen port, default 9090
    #[serde(default = "default_port")]
    pub port: u16,
    /// Data directory for downloads + database
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    /// Optional Basic Auth credentials
    pub auth: Option<AuthConfig>,
    /// Path to web UI static files (Vue dist/), default "./dist"
    #[serde(default = "default_web_dir")]
    pub web_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub username: String,
    pub password: String,
}

fn default_host() -> String {
    "0.0.0.0".into()
}
fn default_port() -> u16 {
    9090
}
pub fn default_data_dir() -> PathBuf {
    dirs_data_dir().join("limedl")
}
fn default_web_dir() -> PathBuf {
    PathBuf::from("./dist")
}

impl ServerConfig {
    /// Load from config file, falling back to defaults if file doesn't exist.
    /// Config file path: data_dir/config.json
    pub fn load(config_path: &PathBuf) -> anyhow::Result<Self> {
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(ServerConfig::default())
        }
    }

}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            data_dir: default_data_dir(),
            auth: None,
            web_dir: default_web_dir(),
        }
    }
}

/// Cross-platform user data directory
fn dirs_data_dir() -> PathBuf {
    // Use standard dirs or env var
    if let Ok(dir) = std::env::var("LIMEDL_DATA_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(dir).join("limedl");
        }
    }
    let home = dirs_home();
    #[cfg(target_os = "linux")]
    {
        home.join(".local/share/limedl")
    }
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/limedl")
    }
    #[cfg(target_os = "windows")]
    {
        home.join("AppData/Local/limedl")
    }
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}
