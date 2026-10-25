//! Server-wide methods: shutdown, listMethods and temp-file cleanup.
//!
//! `system.multicall` lives in `dispatch` instead: its handler has to re-enter
//! `dispatch_method` for every nested call, so keeping it here made `system` and
//! `dispatch` depend on each other.

use std::ffi::OsStr;

use super::context::RpcContext;
use super::jsonrpc::JsonRpcError;
use crate::event_bus::DownloadEvent;
use serde_json::Value;

/// Removes `.torrent` files in the aria2 temp directory that are older than 1 hour.
/// This is a best-effort cleanup — all errors are silently ignored.
pub fn cleanup_old_aria2_temp_files() {
    let temp_dir = std::env::temp_dir().join("limedl_aria2");
    let Ok(entries) = std::fs::read_dir(&temp_dir) else {
        return;
    };

    let now = std::time::SystemTime::now();
    let one_hour = std::time::Duration::from_secs(3600);
    let mut removed = 0u32;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(OsStr::to_str) != Some("torrent") {
            continue;
        }
        let age = match std::fs::metadata(&path).and_then(|m| m.modified().or_else(|_| m.created()))
        {
            Ok(created) => now.duration_since(created).unwrap_or_default(),
            Err(_) => continue,
        };
        if age >= one_hour && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }

    if removed > 0 {
        tracing::debug!("Cleaned up {removed} old aria2 torrent temp file(s)");
    }
}
pub async fn handle_shutdown(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    if ctx.exit_on_shutdown {
        tracing::info!(
            "aria2.shutdown requested — exit_on_shutdown is set, signalling the daemon to stop"
        );
        ctx.shutdown_notify.notify_one();
    } else {
        tracing::info!(
            "aria2.shutdown requested from aria2 client — limedl runs as a managed subsystem; use the application UI to exit"
        );
    }
    ctx.event_bus.publish(DownloadEvent::Warning {
        id: "system".into(),
        message: "Aria2 client 请求关闭程序。请使用应用界面退出。".into(),
    });
    Ok(Value::String(
        "Shutdown acknowledged. Use the application UI to exit.".to_string(),
    ))
}
