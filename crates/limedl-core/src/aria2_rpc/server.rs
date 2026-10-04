//! Aria2RpcServer: router assembly, bind address and lifecycle.

use super::{Arc, Aria2RpcSettings, AuthConfig, BackendRegistry, CorsLayer, Dispatcher, Duration, EventBus, HashMap, HeaderValue, Method, Mutex, Router, RpcContext, handle_jsonrpc_http, handle_websocket_upgrade, header, post};

/// How long [`Aria2RpcServer::serve`] keeps retrying `bind` while a previous
/// instance of the server releases the port during a settings hot-reload.
///
/// The old accept loop drops its listener promptly once the shutdown signal
/// propagates, but that happens on a different task, so the replacement can
/// still observe `AddrInUse` if it binds first. Retrying for a short window
/// closes that race instead of losing the RPC endpoint until the next restart.
const BIND_RETRY_WINDOW: Duration = Duration::from_secs(5);
const BIND_RETRY_INTERVAL: Duration = Duration::from_millis(25);

/// Bind `addr`, retrying `AddrInUse` for `window`.
///
/// `AddrInUse` is the expected state while the hot-reloaded predecessor is
/// still shutting down; any other error (permission denied, bad address) is
/// returned immediately. If the port is genuinely held by another process the
/// retries give up and the caller surfaces the original error.
async fn bind_with_retry(addr: &str, window: Duration) -> std::io::Result<tokio::net::TcpListener> {
    let deadline = tokio::time::Instant::now() + window;
    let mut announced = false;
    loop {
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => return Ok(listener),
            Err(err)
                if err.kind() == std::io::ErrorKind::AddrInUse
                    && tokio::time::Instant::now() < deadline =>
            {
                if !announced {
                    tracing::info!("端口 {addr} 仍被上一个 Aria2 RPC 实例占用，等待其释放...");
                    announced = true;
                }
                tokio::time::sleep(BIND_RETRY_INTERVAL).await;
            }
            Err(err) => return Err(err),
        }
    }
}

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
        let auth = AuthConfig::from_settings(settings);

        let dispatcher = Dispatcher::new(registry.clone(), event_bus.clone());
        let ctx = Arc::new(RpcContext {
            registry,
            auth,
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
        // Warn once, before the context is handed to the router: with an empty
        // secret the endpoint answers anonymously, so any local process (and any
        // page the default CORS policy admits) can drive it.
        if !self.ctx.auth.is_enabled() {
            tracing::warn!(
                "Aria2 RPC is serving without authentication on {} — any local process can \
                 control downloads. Set a secret or switch to per-client tokens to lock it \
                 down, or disable the service in Settings.",
                self.addr
            );
        }

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

        let listener = bind_with_retry(&self.addr, BIND_RETRY_WINDOW).await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown.changed().await;
            })
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact race a settings hot-reload hits: the predecessor still owns
    /// the port when the replacement starts binding, and releases it on a
    /// different task shortly after. The retry must win the port instead of
    /// failing the whole server restart.
    #[tokio::test]
    async fn bind_with_retry_waits_for_the_predecessor_to_release_the_port() {
        let first = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = first.local_addr().unwrap().to_string();
        let release = tokio::spawn(async move {
            tokio::time::sleep(BIND_RETRY_INTERVAL * 3).await;
            drop(first);
        });

        let second = bind_with_retry(&addr, BIND_RETRY_WINDOW)
            .await
            .expect("the retry must win the port once the predecessor releases it");
        release.await.unwrap();
        assert_eq!(second.local_addr().unwrap().to_string(), addr);
    }

    /// A port occupied for the whole window is a real conflict: the retry
    /// gives up rather than hanging the caller forever.
    #[tokio::test]
    async fn bind_with_retry_gives_up_after_the_window() {
        let held = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = held.local_addr().unwrap().to_string();

        let err = bind_with_retry(&addr, BIND_RETRY_INTERVAL * 3)
            .await
            .expect_err("a permanently occupied port must surface an error");
        assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
    }
}
