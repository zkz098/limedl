//! Aria2RpcServer: router assembly, bind address and lifecycle.

use super::{Arc, Aria2RpcSettings, BackendRegistry, CorsLayer, Dispatcher, Duration, EventBus, HashMap, HeaderValue, Method, Mutex, Router, RpcContext, handle_jsonrpc_http, handle_websocket_upgrade, header, post};

pub struct Aria2RpcServer {
    ctx: Arc<RpcContext>,
    addr: String,
}

impl Aria2RpcServer {
    pub fn new(
        registry: Arc<BackendRegistry>,
        settings: &Aria2RpcSettings,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let secret = settings.secret.clone().filter(|s| !s.is_empty());

        let dispatcher = Dispatcher::new(registry.clone(), event_bus.clone());
        let ctx = Arc::new(RpcContext {
            registry,
            secret,
            event_bus,
            dispatcher,
            gid_cache: Mutex::new(HashMap::default()),
            session_id: uuid::Uuid::new_v4().to_string(),
        });

        Aria2RpcServer {
            ctx,
            addr: format!("127.0.0.1:{}", settings.port),
        }
    }

    pub async fn serve(
        self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
        cors_allowed_origins: Vec<String>,
    ) -> anyhow::Result<()> {
        // Build CORS layer with configurable origins
        let cors = if cors_allowed_origins.is_empty() {
            // Default: localhost only
            CorsLayer::new()
                .allow_origin([
                    "http://localhost".parse::<HeaderValue>().unwrap(),
                    "http://127.0.0.1".parse::<HeaderValue>().unwrap(),
                ])
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
                .allow_credentials(true)
                .max_age(Duration::from_secs(86400))
        } else {
            // Use configured origins
            let origins: Vec<HeaderValue> = cors_allowed_origins
                .iter()
                .filter_map(|o| o.parse::<HeaderValue>().ok())
                .collect();

            if origins.is_empty() {
                // All configured origins failed to parse — warn and fall back to localhost
                tracing::warn!(
                    "All configured CORS origins failed to parse: {:?}. Falling back to localhost.",
                    cors_allowed_origins
                );
                CorsLayer::new()
                    .allow_origin([
                        "http://localhost".parse::<HeaderValue>().unwrap(),
                        "http://127.0.0.1".parse::<HeaderValue>().unwrap(),
                    ])
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
                    .allow_credentials(true)
                    .max_age(Duration::from_secs(86400))
            } else {
                CorsLayer::new()
                    .allow_origin(origins)
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
                    .allow_credentials(true)
                    .max_age(Duration::from_secs(86400))
            }
        };

        let app = Router::new()
            .route(
                "/jsonrpc",
                post(handle_jsonrpc_http).get(handle_websocket_upgrade),
            )
            .layer(cors)
            .with_state(self.ctx);

        tracing::info!("Aria2 RPC server listening on http://{}/jsonrpc", self.addr);

        let listener = tokio::net::TcpListener::bind(&self.addr).await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown.changed().await;
            })
            .await?;
        Ok(())
    }
}
