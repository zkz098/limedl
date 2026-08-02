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
    /// Optional TLS configuration
    #[serde(default)]
    pub tls: TlsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TlsConfig {
    /// Enable HTTPS. Requires cert_path and key_path.
    #[serde(default)]
    pub enabled: bool,
    /// Path to PEM-encoded TLS certificate.
    #[serde(default)]
    pub cert_path: Option<String>,
    /// Path to PEM-encoded TLS private key.
    #[serde(default)]
    pub key_path: Option<String>,
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

    /// Apply CLI overrides on top of a loaded config.
    ///
    /// `--port` overrides the config-file port when `Some`.
    /// `--user` and `--pass` together replace/enable auth.
    /// A lone `--user` or `--pass` (without the other) is ignored.
    pub fn apply_cli_overrides(
        &mut self,
        port: Option<u16>,
        user: Option<String>,
        pass: Option<String>,
    ) {
        if let Some(p) = port {
            self.port = p;
        }
        if let (Some(u), Some(p)) = (user, pass) {
            self.auth = Some(AuthConfig {
                username: u,
                password: p,
            });
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
            tls: TlsConfig::default(),
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

pub(crate) fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    // ── ServerConfig::load() tests ─────────────────────────────────────

    #[test]
    fn load_missing_file_returns_defaults() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.json");
        assert!(!path.exists(), "precondition: file must not exist");

        let cfg = ServerConfig::load(&path).unwrap();
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 9090);
        assert!(cfg.auth.is_none());
        assert_eq!(cfg.web_dir, PathBuf::from("./dist"));
        assert!(!cfg.tls.enabled);
    }

    #[test]
    fn load_full_valid_config() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");
        let config_data = json!({
            "host": "127.0.0.1",
            "port": 8080,
            "data_dir": tmp.path().join("data"),
            "auth": { "username": "admin", "password": "secret" },
            "web_dir": tmp.path().join("web"),
            "tls": { "enabled": true, "certPath": "/certs/cert.pem", "keyPath": "/certs/key.pem" }
        });
        std::fs::write(&path, config_data.to_string()).unwrap();

        let cfg = ServerConfig::load(&path).unwrap();
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.data_dir, tmp.path().join("data"));
        let auth = cfg.auth.expect("auth should be present");
        assert_eq!(auth.username, "admin");
        assert_eq!(auth.password, "secret");
        assert_eq!(cfg.web_dir, tmp.path().join("web"));
        assert!(cfg.tls.enabled);
        assert_eq!(cfg.tls.cert_path.as_deref(), Some("/certs/cert.pem"));
        assert_eq!(cfg.tls.key_path.as_deref(), Some("/certs/key.pem"));
    }

    #[test]
    fn load_partial_config_fills_defaults() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");
        // Only set port — everything else should default
        let config_data = json!({ "port": 8080 });
        std::fs::write(&path, config_data.to_string()).unwrap();

        let cfg = ServerConfig::load(&path).unwrap();
        assert_eq!(cfg.port, 8080, "port from config");
        assert_eq!(cfg.host, "0.0.0.0", "host should default");
        assert!(cfg.auth.is_none(), "auth should default to None");
        assert_eq!(cfg.web_dir, PathBuf::from("./dist"), "web_dir should default");
        assert!(!cfg.tls.enabled, "tls should default to disabled");
    }

    #[test]
    fn load_invalid_json_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, "this is not json").unwrap();

        let err = ServerConfig::load(&path).unwrap_err();
        assert!(
            err.to_string().contains("expected") || err.to_string().contains("invalid"),
            "error should mention parsing failure, got: {err}"
        );
    }

    #[test]
    fn load_rejects_wrong_types() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");
        // port is a string, should be a number
        let config_data = json!({ "port": "not-a-port" });
        std::fs::write(&path, config_data.to_string()).unwrap();

        let err = ServerConfig::load(&path).unwrap_err();
        assert!(
            err.to_string().contains("invalid type") || err.to_string().contains("expected"),
            "error should mention type mismatch, got: {err}"
        );
    }

    // ── ServerConfig::apply_cli_overrides() tests ──────────────────────

    #[test]
    fn cli_port_overrides_config_port() {
        let mut cfg = ServerConfig::default();
        assert_eq!(cfg.port, 9090);

        cfg.apply_cli_overrides(Some(3000), None, None);
        assert_eq!(cfg.port, 3000);
    }

    #[test]
    fn cli_port_none_keeps_config_port() {
        let mut cfg = ServerConfig {
            port: 8080,
            ..ServerConfig::default()
        };

        cfg.apply_cli_overrides(None, None, None);
        assert_eq!(cfg.port, 8080);
    }

    #[test]
    fn cli_user_pass_creates_auth() {
        let mut cfg = ServerConfig::default();
        assert!(cfg.auth.is_none());

        cfg.apply_cli_overrides(
            None,
            Some("alice".into()),
            Some("p@ss".into()),
        );
        let auth = cfg.auth.expect("auth should be set");
        assert_eq!(auth.username, "alice");
        assert_eq!(auth.password, "p@ss");
    }

    #[test]
    fn cli_user_only_does_not_set_auth() {
        let mut cfg = ServerConfig::default();
        cfg.apply_cli_overrides(None, Some("alice".into()), None);
        assert!(cfg.auth.is_none(), "auth should not be set with user alone");
    }

    #[test]
    fn cli_pass_only_does_not_set_auth() {
        let mut cfg = ServerConfig::default();
        cfg.apply_cli_overrides(None, None, Some("secret".into()));
        assert!(cfg.auth.is_none(), "auth should not be set with pass alone");
    }

    #[test]
    fn cli_overrides_combine_with_file_values() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");
        // File says port=8080, no auth
        let config_data = json!({ "port": 8080 });
        std::fs::write(&path, config_data.to_string()).unwrap();

        let mut cfg = ServerConfig::load(&path).unwrap();
        assert_eq!(cfg.port, 8080);
        assert!(cfg.auth.is_none());

        // CLI overrides port and adds auth
        cfg.apply_cli_overrides(Some(9090), Some("admin".into()), Some("hunter2".into()));

        assert_eq!(cfg.port, 9090, "CLI port overrides file port");
        let auth = cfg.auth.expect("CLI auth should be set");
        assert_eq!(auth.username, "admin");
        assert_eq!(auth.password, "hunter2");
    }
}
