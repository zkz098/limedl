//! Shared RPC context: token checks, gid resolution and event broadcast.

use super::{Arc, Aria2AuthMode, Aria2RpcSettings, BackendRegistry, ClientToken, Dispatcher, DownloadEvent, DownloadManager, EventBus, HashMap, JsonRpcError, LazyBtBackend, Mutex, PathBuf, PerClientAuth, TaskId, Value, constant_time_eq, make_error};

/// Authentication scheme derived from [`Aria2RpcSettings`] at server start.
///
/// `Disabled` accepts every request; `Shared` is the legacy single-secret mode;
/// `PerClient` matches an Argon2-hashed token against the configured clients.
pub(crate) enum AuthConfig {
    Disabled,
    Shared { secret: String },
    PerClient(PerClientAuth),
}

impl AuthConfig {
    pub(crate) fn from_settings(settings: &Aria2RpcSettings) -> Self {
        match settings.auth_mode {
            Aria2AuthMode::Single => match settings.secret.clone().filter(|s| !s.is_empty()) {
                Some(secret) => AuthConfig::Shared { secret },
                None => AuthConfig::Disabled,
            },
            // An empty client list intentionally fails closed: enabling
            // per-client mode is an explicit opt-in, and silently falling back
            // to anonymous access would be the opposite of what it promises.
            Aria2AuthMode::PerClient => AuthConfig::PerClient(PerClientAuth::new(
                settings
                    .clients
                    .iter()
                    .map(|client| ClientToken {
                        id: client.id.clone(),
                        name: client.name.clone(),
                        token_hash: client.token_hash.clone(),
                    })
                    .collect(),
            )),
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        !matches!(self, AuthConfig::Disabled)
    }
}

pub(crate) struct RpcContext {
    pub(crate) registry: Arc<BackendRegistry>,
    pub(crate) dispatcher: Dispatcher,
    pub(crate) auth: AuthConfig,
    pub(crate) event_bus: Arc<EventBus>,
    pub(crate) gid_cache: Mutex<HashMap<String, TaskId>>,
    pub(crate) session_id: String,
}

impl RpcContext {
    /// The HTTP backend, if registered.
    ///
    /// This and [`RpcContext::bt`] are the **only** sanctioned `get_typed`
    /// downcasts in the aria2 layer: the RPC surface is inherently
    /// protocol-specific (`getOption` reads the HTTP manifest, `tellStatus.files`
    /// asks the BT engine), so the downcasts are centralized here instead of
    /// being scattered across the handlers.
    pub(crate) fn http(&self) -> Option<&DownloadManager> {
        self.registry.get_typed::<DownloadManager>()
    }

    /// The BT backend, if registered. See [`RpcContext::http`].
    pub(crate) fn bt(&self) -> Option<&LazyBtBackend> {
        self.registry.get_typed::<LazyBtBackend>()
    }

    pub(crate) fn settings_default_download_dir(&self) -> String {
        self.http()
            .and_then(|dm| dm.settings_default_download_dir())
            .unwrap_or_else(|| dirs_next().unwrap_or_else(default_downloads_dir))
    }
}

/// Enforce the configured authentication scheme.
///
/// Runs on every routed method (see `dispatch_method`), so a new handler is
/// protected automatically. Only the `token:`-prefixed first parameter is
/// accepted, matching aria2's wire format.
pub(crate) fn check_token(ctx: &RpcContext, params: &[Value]) -> Result<(), JsonRpcError> {
    if !ctx.auth.is_enabled() {
        return Ok(());
    }
    let Some(provided) = params.first().and_then(|v| v.as_str()) else {
        return Err(unauthorized());
    };
    let Some(token) = provided.strip_prefix("token:") else {
        return Err(unauthorized());
    };
    match &ctx.auth {
        AuthConfig::Disabled => Ok(()),
        AuthConfig::Shared { secret } => {
            if constant_time_eq(token, secret) {
                Ok(())
            } else {
                Err(unauthorized())
            }
        }
        AuthConfig::PerClient(auth) => match auth.verify(token) {
            Some(client) => {
                tracing::debug!(client = %client.name, id = %client.id, "aria2 rpc: per-client token accepted");
                Ok(())
            }
            None => Err(unauthorized()),
        },
    }
}

fn unauthorized() -> JsonRpcError {
    make_error(1, "Unauthorized")
}

pub(crate) fn strip_token(params: Vec<Value>) -> Vec<Value> {
    if params
        .first()
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.starts_with("token:"))
    {
        params.into_iter().skip(1).collect()
    } else {
        params
    }
}
pub(crate) fn broadcast_event(ctx: &RpcContext, method: &str, gid: &str) {
    ctx.event_bus.publish(DownloadEvent::Aria2Notification {
        event_name: method.to_string(),
        gid: gid.to_string(),
    });
}
pub(crate) fn dirs_next() -> Option<String> {
    let home = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").ok()
    } else {
        std::env::var("HOME").ok()
    };
    home.map(|p| {
        PathBuf::from(p)
            .join("Downloads")
            .to_string_lossy()
            .to_string()
    })
}

pub(crate) fn default_downloads_dir() -> String {
    dirs_next().unwrap_or_else(|| String::from("."))
}
