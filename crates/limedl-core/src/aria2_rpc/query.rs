//! Read-only query methods: tellStatus/tellActive/getFiles/getPeers/session listing.

use super::{BtPeerInfo, DownloadManager, DownloadState, DownloadSummary, ERR_INTERNAL, ERR_INVALID_PARAMS, JsonRpcError, LazyBtBackend, RpcContext, TaskId, Value, build_file_list, make_error, resolve_gid, summary_to_aria2_status};

pub(crate) async fn handle_tell_status(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let raw_id = task_id.raw_id();
    // O(1) lookup on DownloadManager first (covers all HTTP downloads).
    let summary = if let Some(dm) = ctx.registry.get_typed::<DownloadManager>() {
        if let Some(s) = dm.get_summary(&raw_id).await {
            s
        } else {
            let all = get_all_summaries(ctx).await?;
            all.into_iter()
                .find(|s| s.id == raw_id)
                .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?
        }
    } else {
        let all = get_all_summaries(ctx).await?;
        all.into_iter()
            .find(|s| s.id == raw_id)
            .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?
    };

    Ok(summary_to_aria2_status(&summary))
}

pub(crate) async fn handle_tell_active(
    ctx: &RpcContext,
    _params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;
    let active: Vec<Value> = all
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                DownloadState::Downloading | DownloadState::Retrying | DownloadState::Verifying
            )
        })
        .map(summary_to_aria2_status)
        .collect();
    Ok(Value::Array(active))
}

pub(crate) async fn handle_tell_waiting(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let offset: usize = parse_int_param(&params, 0).unwrap_or(0);
    let num: usize = parse_int_param(&params, 1).unwrap_or(1000);

    let all = get_all_summaries(ctx).await?;
    let waiting: Vec<Value> = all
        .iter()
        .filter(|s| matches!(s.state, DownloadState::Queued | DownloadState::Paused))
        .skip(offset)
        .take(num)
        .map(summary_to_aria2_status)
        .collect();
    Ok(Value::Array(waiting))
}

pub(crate) async fn handle_tell_stopped(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let offset: usize = parse_int_param(&params, 0).unwrap_or(0);
    let num: usize = parse_int_param(&params, 1).unwrap_or(1000);

    let all = get_all_summaries(ctx).await?;
    let stopped: Vec<Value> = all
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
            )
        })
        .skip(offset)
        .take(num)
        .map(summary_to_aria2_status)
        .collect();
    Ok(Value::Array(stopped))
}

pub(crate) async fn handle_global_stat(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;
    let num_active = all
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                DownloadState::Downloading | DownloadState::Retrying | DownloadState::Verifying
            )
        })
        .count();
    let num_waiting = all
        .iter()
        .filter(|s| matches!(s.state, DownloadState::Queued | DownloadState::Paused))
        .count();
    let num_stopped = all
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
            )
        })
        .count();

    let total_speed: u64 = all
        .iter()
        .filter_map(|s| s.speed_bytes_per_second.map(|v| v as u64))
        .sum();

    Ok(serde_json::json!({
        "downloadSpeed": total_speed.to_string(),
        "uploadSpeed": "0",
        "numActive": num_active.to_string(),
        "numWaiting": num_waiting.to_string(),
        "numStopped": num_stopped.to_string(),
        "numStoppedTotal": num_stopped.to_string(),
    }))
}

pub(crate) fn handle_version() -> Value {
    serde_json::json!({
        "version": "0.1.0",
        "enabledFeatures": [
            "Async DNS", "BitTorrent", "Firefox3 Cookie", "GZip",
            "HTTPS", "Message Digest", "XML-RPC"
        ]
    })
}

pub(crate) async fn handle_get_files(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let raw_id = task_id.raw_id();
    let summary = get_all_summaries(ctx)
        .await?
        .into_iter()
        .find(|s| s.id == raw_id)
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    Ok(build_file_list(&summary))
}

pub(crate) async fn handle_get_uris(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let raw_id = task_id.raw_id();
    let summary = get_all_summaries(ctx)
        .await?
        .into_iter()
        .find(|s| s.id == raw_id)
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    Ok(serde_json::json!([{
        "uri": summary.url,
        "status": "used"
    }]))
}

pub(crate) async fn handle_get_peers(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    // HTTP downloads don't have BitTorrent peers — return empty array.
    let TaskId::Bt(info_hash) = &task_id else {
        return Ok(Value::Array(vec![]));
    };

    let peers = ctx
        .registry
        .get_typed::<LazyBtBackend>()
        .ok_or_else(|| make_error(ERR_INTERNAL, "BT backend not available"))?
        .get_peers(*info_hash)
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    let aria2_peers: Vec<Value> = peers.iter().map(peer_info_to_aria2_peer).collect();

    Ok(Value::Array(aria2_peers))
}

pub(crate) fn peer_info_to_aria2_peer(p: &BtPeerInfo) -> Value {
    let (ip, port) = match p.address.rsplit_once(':') {
        Some((ip, port_str)) => (ip.to_string(), port_str.parse::<u16>().unwrap_or(0)),
        None => (p.address.clone(), 0),
    };

    // Decode 'am_choking' from flags ('c' character)
    let am_choking = p.flags.contains('c');
    // seeder if progress >= 1.0 (100% complete)
    let seeder = p.progress >= 1.0;

    serde_json::json!({
        "peerId": "",
        "ip": ip,
        "port": port,
        "bitfield": "",
        "amChoking": if am_choking { "true" } else { "false" },
        "peerChoking": "false",
        "downloadSpeed": p.download_speed.to_string(),
        "uploadSpeed": p.upload_speed.to_string(),
        "seeder": if seeder { "true" } else { "false" },
    })
}
pub(crate) fn handle_get_session_info(ctx: &RpcContext) -> Value {
    serde_json::json!({ "sessionId": ctx.session_id })
}

pub(crate) fn handle_save_session() -> Value {
    // limedl auto-persists to SQLite on every state change; no explicit save needed
    Value::String("OK".to_string())
}

pub(crate) fn handle_list_notifications() -> Value {
    Value::Array(
        [
            "aria2.onDownloadStart",
            "aria2.onDownloadPause",
            "aria2.onDownloadStop",
            "aria2.onDownloadComplete",
            "aria2.onBtDownloadComplete",
            "aria2.onDownloadError",
        ]
        .iter()
        .map(|&s| Value::String(s.to_string()))
        .collect(),
    )
}

pub(crate) fn handle_list_methods() -> Value {
    Value::Array(
        [
            "aria2.addTorrent",
            "aria2.addUri",
            "aria2.changeGlobalOption",
            "aria2.getFiles",
            "aria2.getGlobalOption",
            "aria2.getGlobalStat",
            "aria2.getOption",
            "aria2.getPeers",
            "aria2.getSessionInfo",
            "aria2.getUris",
            "aria2.getVersion",
            "aria2.multicall",
            "aria2.pause",
            "aria2.forcePause",
            "aria2.pauseAll",
            "aria2.forcePauseAll",
            "aria2.purgeDownloadResult",
            "aria2.remove",
            "aria2.forceRemove",
            "aria2.saveSession",
            "aria2.shutdown",
            "aria2.tellActive",
            "aria2.tellStatus",
            "aria2.tellStopped",
            "aria2.tellWaiting",
            "aria2.unpause",
            "aria2.unpauseAll",
            "system.listMethods",
            "system.listNotifications",
            "system.multicall",
        ]
        .iter()
        .map(|&s| Value::String(s.to_string()))
        .collect(),
    )
}

pub(crate) fn extract_gid(params: &[Value]) -> Result<String, JsonRpcError> {
    params
        .first()
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing GID parameter"))
}

pub(crate) fn parse_int_param(params: &[Value], index: usize) -> Option<usize> {
    params.get(index).and_then(|v| {
        v.as_str()
            .and_then(|s| s.parse::<usize>().ok())
            .or_else(|| v.as_u64().map(|n| n as usize))
    })
}

pub(crate) async fn get_all_summaries(
    ctx: &RpcContext,
) -> Result<Vec<DownloadSummary>, JsonRpcError> {
    let mut all = Vec::new();
    for backend in ctx.registry.iter() {
        match backend.list().await {
            Ok(summaries) => all.extend(summaries),
            Err(e) => tracing::warn!("get_all_summaries: backend list failed: {e}"),
        }
    }
    Ok(all)
}
