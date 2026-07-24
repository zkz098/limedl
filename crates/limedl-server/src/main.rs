use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};

mod auth;
mod config;
mod rate_limiter;
mod rpc;
mod security;

use config::ServerConfig;
use rpc::RpcState;

use anyhow::Context;

#[derive(Parser)]
#[command(name = "limedl", about = "Fast multi-protocol download manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start daemon with WebSocket RPC + Web UI
    Daemon {
        /// Config file path (default: data_dir/config.json)
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Listen port (overrides config)
        #[arg(short, long)]
        port: Option<u16>,
        /// Auth username (overrides config)
        #[arg(long)]
        user: Option<String>,
        /// Auth password (overrides config)
        #[arg(long)]
        pass: Option<String>,
    },
    /// Quick single-file HTTP download
    Download {
        /// URL to download
        url: String,
        /// Output file or directory
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon {
            config: config_path,
            port,
            user,
            pass,
        } => run_daemon(config_path, port, user, pass).await,
        Commands::Download { url, output } => run_single_download(&url, output.as_ref()).await,
    }
}

async fn run_daemon(
    config_path: Option<PathBuf>,
    port: Option<u16>,
    user: Option<String>,
    pass: Option<String>,
) -> anyhow::Result<()> {
    use axum::{
        Router,
        body::Body,
        extract::{DefaultBodyLimit, Request},
        middleware::{self, Next},
        routing::get,
    };

    // Load config
    let data_dir = config::default_data_dir();
    let cfg_path = config_path.unwrap_or_else(|| data_dir.join("config.json"));
    let mut cfg = ServerConfig::load(&cfg_path)?;

    // CLI overrides
    cfg.apply_cli_overrides(port, user, pass);

    // ── Startup security check ──────────────────────────────────────────
    check_listen_safety(&cfg.host, cfg.port, cfg.auth.is_some())?;

    // ── TLS config validation ──────────────────────────────────────────────
    #[cfg(feature = "tls")]
    if cfg.tls.enabled {
        let cert_path =
            cfg.tls.cert_path.as_deref().ok_or_else(|| {
                anyhow::anyhow!("TLS is enabled but cert_path is not set in config")
            })?;
        let key_path =
            cfg.tls.key_path.as_deref().ok_or_else(|| {
                anyhow::anyhow!("TLS is enabled but key_path is not set in config")
            })?;
        if !std::path::Path::new(cert_path).exists() {
            anyhow::bail!("TLS certificate file not found: {cert_path}");
        }
        if !std::path::Path::new(key_path).exists() {
            anyhow::bail!("TLS key file not found: {key_path}");
        }
    }

    // Ensure data directory exists
    std::fs::create_dir_all(&cfg.data_dir)?;
    let state_dir = cfg.data_dir.join("downloads");

    // Initialize core subsystems via shared bootstrap
    let core = limedl_core::bootstrap::bootstrap(state_dir.clone()).await?;

    // Initialize logging
    let _ = limedl_core::init_logging(&core.settings.logging, &state_dir);

    // Initialize CDN service (same pattern as Tauri setup)
    let cdn_accelerator = core.cdn_service.accelerator().clone();
    core.download_manager.set_cdn_accelerator(cdn_accelerator);
    core.cdn_service.init_from_settings(&core.settings).await;

    // Build a shared HTTP client for one-off requests (tracker list fetch, etc.)
    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(concat!("limedl/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("创建 HTTP 客户端失败")?;

    // Build RPC state
    let rpc_state = Arc::new(RpcState {
        registry: core.registry.clone(),
        event_bus: core.event_bus.clone(),
        clients: Arc::new(parking_lot::Mutex::new(Vec::new())),
        rate_limiter: Arc::new(crate::rate_limiter::WsRateLimiter::new()),
        cdn_service: core.cdn_service.clone(),
        http_client,
    });

    // Build router with auth wrapping ALL routes (including static files)
    let auth_config = cfg.auth.clone();

    // Base API routes
    let api_routes = Router::new().route(
        "/ws",
        get(move |ws: axum::extract::WebSocketUpgrade| {
            let state = rpc_state.clone();
            async move { rpc::ws_handler(ws, state).await }
        }),
    );

    // Static file serving: embedded in release builds, filesystem in dev
    #[cfg(feature = "embed-frontend")]
    let app = {
        use rust_embed::RustEmbed;
        #[derive(RustEmbed)]
        #[folder = "../../dist"]
        struct Asset;

        let service = tower::service_fn(move |req: axum::extract::Request| {
            let uri = req.uri().clone();
            let path = uri.path().trim_start_matches('/');
            let path = if path.is_empty() { "index.html" } else { path };

            let (status, body, content_type) = match Asset::get(path) {
                Some(file) => {
                    let mime = guess_mime(path);
                    (axum::http::StatusCode::OK, file.data, mime)
                }
                None => match Asset::get("index.html") {
                    Some(index) => {
                        (axum::http::StatusCode::OK, index.data, "text/html")
                    }
                    None => {
                        (axum::http::StatusCode::NOT_FOUND, std::borrow::Cow::Borrowed(b"Not Found" as &[u8]), "text/plain")
                    }
                },
            };

            let response = axum::response::Response::builder()
                .status(status)
                .header(axum::http::header::CONTENT_TYPE, content_type)
                .body(axum::body::Body::from(body.into_owned()))
                .unwrap();
            std::future::ready(Ok::<_, std::convert::Infallible>(response))
        });

        api_routes.fallback_service(service)
    };

    #[cfg(not(feature = "embed-frontend"))]
    let app = if cfg.web_dir.exists() {
        api_routes.fallback_service(tower_http::services::ServeDir::new(&cfg.web_dir))
    } else {
        api_routes
    };

    // Apply middleware stack (outermost = first in list, applied last)
    let security_layers = security::security_headers_layers(
        &cfg.host,
        cfg.port,
        cfg.tls.enabled,
    );
    let app = app
        // 1. Default body limit (outermost layer)
        .layer(DefaultBodyLimit::max(256 * 1024))
        // 2. Security headers (CSP, X-Content-Type-Options, X-Frame-Options, Referrer-Policy)
        .layer(security_layers.3)
        .layer(security_layers.2)
        .layer(security_layers.1)
        .layer(security_layers.0)
        // 3. Auth middleware (wraps everything including static files)
        .layer(middleware::from_fn(
            move |mut req: Request<Body>, next: Next| {
                req.extensions_mut().insert(auth_config.clone());
                auth::basic_auth_middleware(req, next)
            },
        ));

    let addr = format!("{}:{}", cfg.host, cfg.port);
    tracing::info!("limedl daemon starting on {addr}");
    tracing::info!("Data directory: {}", cfg.data_dir.display());
    if cfg.auth.is_some() {
        tracing::info!("Basic Auth enabled");
    }

    if cfg.auth.is_some() {
        tracing::warn!(
            "WebSocket authentication active — clients will pass credentials via URL query parameter. \
             Credentials may appear in reverse-proxy access logs. \
             Consider using HTTPS/WSS with a reverse proxy for production deployments."
        );
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let registry = core.registry.clone();
    let shutdown_signal = async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Shutting down gracefully...");
        registry.shutdown_all().await;
    };
    #[cfg(feature = "tls")]
    {
        if cfg.tls.enabled {
            let cert = cfg
                .tls
                .cert_path
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("TLS cert_path is required when TLS is enabled"))?;
            let key = cfg
                .tls
                .key_path
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("TLS key_path is required when TLS is enabled"))?;
            let tls_config =
                axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
            let std_listener = listener.into_std()?;
            let handle = axum_server::Handle::new();
            let server =
                axum_server::from_tcp_rustls(std_listener, tls_config).handle(handle.clone());
            tracing::info!("HTTPS enabled");
            let serve_handle = tokio::spawn(server.serve(app.into_make_service()));
            shutdown_signal.await;
            handle.graceful_shutdown(None);
            serve_handle.await??;
        } else {
            tracing::info!(
                "HTTP only (set tls.enabled=true and provide cert_path/key_path for HTTPS)"
            );
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal)
                .await?;
        }
    }
    #[cfg(not(feature = "tls"))]
    {
        tracing::info!("HTTP only (compile with --features tls for HTTPS support)");
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await?;
    }
    tracing::info!("Server shut down");

    Ok(())
}

/// Check whether binding to the given host is safe without auth.
/// Returns an error if binding to a non-local address without auth configured.
fn check_listen_safety(host: &str, port: u16, has_auth: bool) -> anyhow::Result<()> {
    use std::net::Ipv4Addr;

    let is_localhost = matches!(
        host.parse::<IpAddr>(),
        Ok(ip) if ip.is_loopback()
    ) || host == "localhost";

    let is_private = host
        .parse::<Ipv4Addr>()
        .is_ok_and(|ip| ip.is_private() || ip.is_link_local());

    if is_localhost {
        // Localhost binding is always fine
        return Ok(());
    }

    if has_auth {
        tracing::info!(
            "Listening on {}:{} with Basic Auth — note: credentials sent in plaintext",
            host,
            port
        );
        return Ok(());
    }

    // No auth and not localhost — security risk
    if is_private {
        // Private network (10.x, 172.16-31.x, 192.168.x) — allow with opt-out
        if std::env::var("LIMEDL_ALLOW_NO_AUTH").is_ok() {
            tracing::warn!(
                "Listening on {}:{} with no auth on a private network — set auth config for security",
                host,
                port
            );
            return Ok(());
        }
        eprintln!(
            "\n⚠️  SECURITY WARNING: Listening on {}:{} with NO authentication on a private network.\n\
             Set auth credentials via --user/--pass or config file, or set LIMEDL_ALLOW_NO_AUTH=1 to bypass.\n",
            host, port
        );
        anyhow::bail!(
            "Refusing to start without auth on non-localhost ({}) — set auth or LIMEDL_ALLOW_NO_AUTH=1",
            host
        );
    }

    // Public IP — always refuse
    eprintln!(
        "\n🚫 SECURITY ERROR: Refusing to start on public address {}:{} without authentication.\n\
         This would expose the download manager to the internet with no password!\n\
         Configure auth via --user/--pass or config file.\n",
        host, port
    );
    anyhow::bail!(
        "Cannot bind {}:{} without auth on a public address",
        host,
        port
    );
}

async fn run_single_download(url: &str, output: Option<&PathBuf>) -> anyhow::Result<()> {
    use std::io::Write;

    let temp_dir = std::env::temp_dir().join("limedl-cli");
    let state_dir = temp_dir.join("downloads");
    std::fs::create_dir_all(&state_dir)?;

    let rate_limiter = Arc::new(limedl_core::RateLimiter::default());
    let event_bus = Arc::new(limedl_core::EventBus::new(8192));

    let download_manager = Arc::new(limedl_core::DownloadManager::new(
        state_dir.clone(),
        rate_limiter.clone(),
        event_bus.clone(),
    )?);

    let output_path = output.cloned().unwrap_or_else(|| {
        let filename = url.split('/').next_back().unwrap_or("download");
        PathBuf::from(filename)
    });

    let dest_dir = output_path
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .to_string_lossy()
        .to_string();
    let file_name = output_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string());

    let request = limedl_core::types::StartDownloadRequest {
        url: url.to_string(),
        destination_dir: dest_dir,
        file_name,
        kind: None,
        thread_mode: None,
        thread_count: None,
        max_retries: None,
        checksum: None,
        start_paused: false,
        mirror_urls: None,
        // These fields exist in the struct but are not used by run_single_download
        user_agent: None,
        expected_checksum: None,
        selected_file_indices: None,
        priority: None,
    };

    // Print progress
    {
        let mut rx = event_bus.subscribe();
        let url_owned = url.to_string();
        tokio::spawn(async move {
            download_manager.start(request).await.ok();
        });

        loop {
            match rx.recv().await {
                Ok(limedl_core::DownloadEvent::Progress {
                    id: _,
                    progress_json,
                }) => {
                    let _ = writeln!(std::io::stderr(), "Progress: {}", progress_json);
                }
                Ok(limedl_core::DownloadEvent::Updated {
                    id: _,
                    summary_json,
                }) => {
                    // Check if download completed
                    if let Some(state) = summary_json.get("state").and_then(|s| s.as_str()) {
                        if state == "completed" {
                            println!("Download complete: {url_owned}");
                            break;
                        } else if state == "error" {
                            eprintln!("Download failed: {url_owned}");
                            break;
                        }
                    }
                }
                Ok(_) => {
                    // Ignore other event types (Aria2Notification, CdnProgress, CdnComplete)
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            }
        }
    }

    // Cleanup temp dir
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(())
}

#[cfg(feature = "embed-frontend")]
fn guess_mime(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "application/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes env-var-mutating tests. `cargo test` runs tests in parallel
    /// by default, and `LIMEDL_ALLOW_NO_AUTH` is process-global state — without
    /// this lock, a sibling test could `set_var` between this test's
    /// `remove_var` and its `check_listen_safety` call, flipping the result.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Guard that removes `LIMEDL_ALLOW_NO_AUTH` on drop, ensuring tests that
    /// set this env var don't pollute sibling tests.
    struct AllowNoAuthGuard;
    impl Drop for AllowNoAuthGuard {
        fn drop(&mut self) {
            // SAFETY: test-only env var mutation, isolated by #[cfg(test)]
            unsafe {
                std::env::remove_var("LIMEDL_ALLOW_NO_AUTH");
            }
        }
    }

    #[test]
    fn localhost_ipv4_always_allowed_without_auth() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // SAFETY: test-only env var mutation, isolated by #[cfg(test)]
        unsafe {
            std::env::remove_var("LIMEDL_ALLOW_NO_AUTH");
        }
        assert!(check_listen_safety("127.0.0.1", 9090, false).is_ok());
    }

    #[test]
    fn localhost_ipv6_always_allowed_without_auth() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // SAFETY: test-only env var mutation, isolated by #[cfg(test)]
        unsafe {
            std::env::remove_var("LIMEDL_ALLOW_NO_AUTH");
        }
        assert!(check_listen_safety("::1", 9090, false).is_ok());
    }

    #[test]
    fn localhost_string_hostname_always_allowed_without_auth() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // SAFETY: test-only env var mutation, isolated by #[cfg(test)]
        unsafe {
            std::env::remove_var("LIMEDL_ALLOW_NO_AUTH");
        }
        assert!(check_listen_safety("localhost", 9090, false).is_ok());
    }

    #[test]
    fn auth_enabled_allows_non_localhost_binding() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // SAFETY: test-only env var mutation, isolated by #[cfg(test)]
        unsafe {
            std::env::remove_var("LIMEDL_ALLOW_NO_AUTH");
        }
        assert!(check_listen_safety("8.8.8.8", 9090, true).is_ok());
    }

    #[test]
    fn private_network_without_auth_rejects_without_opt_out_env() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // SAFETY: test-only env var mutation, isolated by #[cfg(test)]
        unsafe {
            std::env::remove_var("LIMEDL_ALLOW_NO_AUTH");
        }
        let err = check_listen_safety("10.0.0.1", 9090, false).unwrap_err();
        assert!(
            err.to_string().contains("LIMEDL_ALLOW_NO_AUTH"),
            "expected error to mention LIMEDL_ALLOW_NO_AUTH, got: {}",
            err
        );
    }

    #[test]
    fn private_network_without_auth_allowed_with_opt_out_env() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // SAFETY: test-only env var mutation, isolated by #[cfg(test)]
        unsafe {
            std::env::remove_var("LIMEDL_ALLOW_NO_AUTH");
        }
        let _guard = AllowNoAuthGuard;
        // SAFETY: test-only env var mutation, Dropped by AllowNoAuthGuard
        unsafe {
            std::env::set_var("LIMEDL_ALLOW_NO_AUTH", "1");
        }
        assert!(check_listen_safety("192.168.1.1", 9090, false).is_ok());
    }

    #[test]
    fn public_ip_without_auth_always_rejects() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // SAFETY: test-only env var mutation, isolated by #[cfg(test)]
        unsafe {
            std::env::remove_var("LIMEDL_ALLOW_NO_AUTH");
        }
        let err = check_listen_safety("8.8.8.8", 9090, false).unwrap_err();
        assert!(
            err.to_string().contains("public address"),
            "expected error to mention 'public address', got: {}",
            err
        );
    }

    #[test]
    fn public_ip_with_auth_allowed() {
        // SAFETY: test-only env var mutation, isolated by #[cfg(test)]
        unsafe { std::env::remove_var("LIMEDL_ALLOW_NO_AUTH"); }
        assert!(check_listen_safety("1.1.1.1", 9090, true).is_ok());
    }
}
