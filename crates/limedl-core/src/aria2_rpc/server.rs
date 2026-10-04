//! Aria2RpcServer: router assembly, bind address and lifecycle.

use super::{Arc, Aria2RpcSettings, AuthConfig, BackendRegistry, CorsLayer, Dispatcher, Duration, EventBus, HashMap, HeaderValue, Method, Mutex, Notify, Router, RpcContext, format_bind_addr, handle_jsonrpc_http, handle_websocket_upgrade, header, is_loopback_bind_address, post};

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

/// Return the reason a bind must be refused, or `None` when it is allowed.
///
/// A non-loopback listener is reachable from the network, where an empty secret
/// means anyone who can reach the port can drive downloads and write files (the
/// RPC `dir` option is not confined to a download root). The server therefore
/// fails closed instead of exposing an anonymous control endpoint.
///
/// Kept separate from [`Aria2RpcServer::serve`] so all four
/// loopback × auth combinations are unit-tested without opening a socket.
pub(crate) fn public_bind_rejection(bind_host: &str, auth: &AuthConfig) -> Option<String> {
    if is_loopback_bind_address(bind_host) || auth.is_enabled() {
        return None;
    }
    Some(format!(
        "refusing to bind the Aria2 RPC server to {bind_host} without authentication: \
         set a shared secret or per-client tokens, or bind to 127.0.0.1"
    ))
}

/// Build the CORS layer for the HTTP endpoint.
///
/// `allow_any` takes precedence and emits `Access-Control-Allow-Origin: *`,
/// which browsers reject together with `Access-Control-Allow-Credentials` — so
/// credentials are deliberately off in that mode.
fn build_cors_layer(allowed: &[String], allow_any: bool) -> CorsLayer {
    let methods = [Method::GET, Method::POST, Method::OPTIONS];
    let headers = [header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT];
    let max_age = Duration::from_secs(86400);

    if allow_any {
        return CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods(methods)
            .allow_headers(headers)
            .max_age(max_age);
    }

    let origins: Vec<HeaderValue> = allowed.iter().filter_map(|o| o.parse().ok()).collect();
    let origins = if origins.is_empty() {
        if !allowed.is_empty() {
            tracing::warn!(
                "All configured CORS origins failed to parse: {allowed:?}. Falling back to localhost."
            );
        }
        vec![
            "http://localhost".parse::<HeaderValue>().expect("valid header value"),
            "http://127.0.0.1".parse::<HeaderValue>().expect("valid header value"),
        ]
    } else {
        origins
    };

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(methods)
        .allow_headers(headers)
        .allow_credentials(true)
        .max_age(max_age)
}

pub struct Aria2RpcServer {
    ctx: Arc<RpcContext>,
    /// The resolved `host:port` to bind.
    addr: String,
    /// The configured host, kept so `serve` can decide whether the listener is
    /// network-reachable and must therefore require authentication.
    bind_host: String,
    cors_allowed_origins: Vec<String>,
    cors_allow_any_origin: bool,
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
            exit_on_shutdown: settings.exit_on_shutdown,
            shutdown_notify: Arc::new(Notify::new()),
        });

        Aria2RpcServer {
            ctx,
            addr: format_bind_addr(&settings.listen_address, settings.port),
            bind_host: settings.listen_address.clone(),
            cors_allowed_origins: settings.cors_allowed_origins.clone(),
            cors_allow_any_origin: settings.allow_any_origin,
        }
    }

    /// The address this server will try to bind. Useful for startup logs; the
    /// actual port is only known after `serve` binds (relevant for port `0`).
    pub fn bind_addr(&self) -> &str {
        &self.addr
    }

    /// A handle that resolves when a client calls `aria2.shutdown` *and*
    /// `Aria2RpcSettings::exit_on_shutdown` is set.
    ///
    /// A headless daemon clones this before `serve` consumes the server, then
    /// selects on it to shut the engine down cleanly. `notify_one` (not
    /// `notify_waiters`) is used so the signal is not lost if it arrives before
    /// the daemon starts awaiting.
    pub fn shutdown_notify(&self) -> Arc<Notify> {
        self.ctx.shutdown_notify.clone()
    }

    pub async fn serve(
        self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> anyhow::Result<()> {
        if let Some(reason) = public_bind_rejection(&self.bind_host, &self.ctx.auth) {
            anyhow::bail!(reason);
        }

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

        let cors = build_cors_layer(&self.cors_allowed_origins, self.cors_allow_any_origin);

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

    /// The security gate: only an unauthenticated *network* listener is refused.
    #[test]
    fn public_bind_rejection_blocks_only_unauthenticated_non_loopback() {
        let disabled = AuthConfig::Disabled;
        let shared = AuthConfig::Shared {
            secret: "s3cret".to_string(),
        };

        for loopback in ["127.0.0.1", "::1", "localhost"] {
            assert!(
                public_bind_rejection(loopback, &disabled).is_none(),
                "{loopback} is reachable only locally and needs no auth"
            );
        }
        for public in ["0.0.0.0", "::", "192.168.1.10", "nas.example.lan"] {
            let reason = public_bind_rejection(public, &disabled)
                .unwrap_or_else(|| panic!("{public} must be refused without auth"));
            assert!(
                reason.contains("without authentication"),
                "the refusal must name the cause: {reason}"
            );
            assert!(
                public_bind_rejection(public, &shared).is_none(),
                "a configured secret makes {public} acceptable"
            );
        }
    }

    /// A public listener with a secret must stay up (not be refused), and the
    /// loopback default must need no auth.
    #[tokio::test]
    async fn serve_keeps_running_for_an_authenticated_public_bind() {
        let registry = Arc::new(BackendRegistry::new());
        let event_bus = Arc::new(EventBus::new(16));
        let settings = Aria2RpcSettings {
            enabled: true,
            listen_address: "0.0.0.0".to_string(),
            port: 0,
            secret: Some("s3cret".to_string()),
            ..Aria2RpcSettings::default()
        };
        let server = Aria2RpcServer::new(registry, &settings, event_bus);
        let (tx, rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(server.serve(rx));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            !handle.is_finished(),
            "an authenticated public bind must keep serving instead of failing fast"
        );
        let _ = tx.send(true);
        let _ = handle.await;
    }
}
