use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};

mod auth;
mod config;
mod rpc;

use config::ServerConfig;
use rpc::RpcState;

#[derive(Parser)]
#[command(name = "flareget", about = "Fast multi-protocol download manager")]
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
        Commands::Download { url, output } => {
            run_single_download(&url, output.as_ref()).await
        }
    }
}

async fn run_daemon(
    config_path: Option<PathBuf>,
    port: Option<u16>,
    user: Option<String>,
    pass: Option<String>,
) -> anyhow::Result<()> {
    use axum::{middleware, routing::get, Router};

    // Load config
    let data_dir = config::default_data_dir();
    let cfg_path = config_path.unwrap_or_else(|| data_dir.join("config.json"));
    let mut cfg = ServerConfig::load(&cfg_path)?;

    // CLI overrides
    if let Some(p) = port {
        cfg.port = p;
    }
    if let (Some(u), Some(p)) = (user, pass) {
        cfg.auth = Some(config::AuthConfig {
            username: u,
            password: p,
        });
    }

    // Ensure data directory exists
    std::fs::create_dir_all(&cfg.data_dir)?;
    let state_dir = cfg.data_dir.join("downloads");

    // Initialize core subsystems via shared bootstrap
    let core = flareget_core::bootstrap::bootstrap(state_dir.clone()).await?;

    // Initialize logging
    let _ = flareget_core::init_logging(&core.settings.logging, &state_dir);

    // Build RPC state
    let rpc_state = Arc::new(RpcState {
        registry: core.registry.clone(),
        event_bus: core.event_bus.clone(),
        clients: Arc::new(parking_lot::Mutex::new(Vec::new())),
    });

    // Build router
    let auth_config = cfg.auth.clone();
    let app = Router::new()
        .route(
            "/ws",
            get(move |ws: axum::extract::WebSocketUpgrade| {
                let state = rpc_state.clone();
                async move { rpc::ws_handler(ws, state).await }
            }),
        )
        .layer(middleware::from_fn(
            move |mut req: axum::http::Request<axum::body::Body>,
                  next: middleware::Next| {
                req.extensions_mut().insert(auth_config.clone());
                auth::basic_auth_middleware(req, next)
            },
        ));

    // Add static file serving if web_dir exists
    let app = if cfg.web_dir.exists() {
        app.fallback_service(tower_http::services::ServeDir::new(&cfg.web_dir))
    } else {
        app
    };

    let addr = format!("{}:{}", cfg.host, cfg.port);
    tracing::info!("flareget daemon starting on {addr}");
    tracing::info!("Data directory: {}", cfg.data_dir.display());
    if cfg.auth.is_some() {
        tracing::info!("Basic Auth enabled");
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn run_single_download(url: &str, output: Option<&PathBuf>) -> anyhow::Result<()> {
    use std::io::Write;

    let temp_dir = std::env::temp_dir().join("flareget-cli");
    let state_dir = temp_dir.join("downloads");
    std::fs::create_dir_all(&state_dir)?;

    let rate_limiter = Arc::new(flareget_core::RateLimiter::default());
    let event_bus = Arc::new(flareget_core::EventBus::new(1024));

    let download_manager = Arc::new(flareget_core::DownloadManager::new(
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

    let request = flareget_core::types::StartDownloadRequest {
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
                Ok(flareget_core::DownloadEvent::Progress {
                    id: _,
                    progress_json,
                }) => {
                    let _ = writeln!(std::io::stderr(), "Progress: {}", progress_json);
                }
                Ok(flareget_core::DownloadEvent::Updated {
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
