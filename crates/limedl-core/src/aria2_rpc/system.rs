//! Server-wide methods: shutdown, multicall, listMethods and temp-file cleanup.

use std::ffi::OsStr;

use super::{DownloadEvent, ERR_INVALID_PARAMS, JsonRpcError, RpcContext, Value, dispatch_method, make_error};

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
pub(crate) async fn handle_shutdown(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    tracing::info!(
        "aria2.shutdown requested from aria2 client — limedl runs as a managed subsystem; use the application UI to exit"
    );
    ctx.event_bus.publish(DownloadEvent::Warning {
        id: "system".into(),
        message: "Aria2 client 请求关闭程序。请使用应用界面退出。".into(),
    });
    Ok(Value::String(
        "Shutdown acknowledged. Use the application UI to exit.".to_string(),
    ))
}

pub(crate) async fn handle_multicall(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    // `dispatch_method` already stripped any outer token (aria2's multicall
    // carries none), so each nested call is authenticated on its own: it goes
    // back through `dispatch_method`, which checks and strips the per-call
    // `token:` element. An unauthenticated nested call fails with its own
    // Unauthorized entry instead of executing.
    let calls = params
        .first()
        .and_then(|v| v.as_array())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing methods array"))?;

    let mut results = Vec::with_capacity(calls.len());
    for call in calls {
        let method = call
            .get("methodName")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let call_params = call
            .get("params")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // aria2's response shape is a single-element array per call:
        // `[value]` on success, `[{"code":…,"message":…}]` on failure.
        // AriaNg/Motrix index into element 0, so a two-element
        // `[null, value]` wrapper would read as a failed call.
        match Box::pin(dispatch_method(ctx, method, call_params)).await {
            Ok(result) => results.push(Value::Array(vec![result])),
            Err(e) => results.push(Value::Array(vec![serde_json::json!({
                "code": e.code,
                "message": e.message,
            })])),
        }
    }
    Ok(Value::Array(results))
}
