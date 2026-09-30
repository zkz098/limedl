//! Shared RPC context: token checks, gid resolution and event broadcast.

use super::*;

pub(crate) struct RpcContext {
    pub(crate) registry: Arc<BackendRegistry>,
    pub(crate) dispatcher: Dispatcher,
    pub(crate) secret: Option<String>,
    pub(crate) event_bus: Arc<EventBus>,
    pub(crate) gid_cache: Mutex<HashMap<String, TaskId>>,
    pub(crate) session_id: String,
}

impl RpcContext {
    pub(crate) fn settings_default_download_dir(&self) -> String {
        self.registry
            .get_typed::<DownloadManager>()
            .and_then(|dm| dm.settings_default_download_dir())
            .unwrap_or_else(|| dirs_next().unwrap_or_else(default_downloads_dir))
    }
}

pub(crate) fn check_token(ctx: &RpcContext, params: &[Value]) -> Result<(), JsonRpcError> {
    let Some(secret) = &ctx.secret else {
        return Ok(());
    };
    let expected = format!("token:{secret}");
    if params
        .first()
        .and_then(|v| v.as_str())
        .is_none_or(|s| s != expected)
    {
        return Err(make_error(1, "Unauthorized"));
    }
    Ok(())
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
