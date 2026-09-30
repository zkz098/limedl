//! Server-wide methods: shutdown, multicall, listMethods and temp-file cleanup.

use super::*;

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
        if path.extension().and_then(|e| e.to_str()) != Some("torrent") {
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
    let params = strip_token(params);
    check_token(ctx, &params)?;

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

        match Box::pin(dispatch_method(ctx, method, call_params)).await {
            Ok(result) => results.push(Value::Array(vec![Value::Null, result])),
            Err(e) => results.push(Value::Array(vec![
                serde_json::json!({ "code": e.code, "message": e.message }),
                Value::Null,
            ])),
        }
    }
    Ok(Value::Array(results))
}
