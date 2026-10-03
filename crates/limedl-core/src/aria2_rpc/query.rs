//! Read-only query methods: tellStatus/tellActive/getFiles/getPeers/session listing.

use super::{BtPeerInfo, DownloadState, DownloadSummary, ERR_INTERNAL, ERR_INVALID_PARAMS, JsonRpcError, RpcContext, TaskId, Value, bt_files_to_aria2, build_file_list, filter_status_keys, make_error, resolve_gid, summary_to_aria2_status};

fn is_terminal(state: DownloadState) -> bool {
    matches!(
        state,
        DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
    )
}

/// aria2's optional `keys` parameter: a list of field names to keep.
fn parse_keys(params: &[Value], index: usize) -> Option<Vec<String>> {
    params.get(index).and_then(|v| v.as_array()).map(|keys| {
        keys.iter()
            .filter_map(|key| key.as_str().map(String::from))
            .collect()
    })
}

/// File list for a resolved task: real aria2 entries once torrent metadata is
/// known, the single synthetic entry for HTTP downloads. A magnet whose
/// metadata has not arrived yet reports no files rather than a fake one.
fn file_list_for(ctx: &RpcContext, task_id: &TaskId, summary: &DownloadSummary) -> Value {
    let TaskId::Bt(info_hash) = task_id else {
        return build_file_list(summary);
    };
    match ctx.bt() {
        Some(backend) => match backend.get_torrent_files(*info_hash) {
            Ok(files) if !files.is_empty() => bt_files_to_aria2(&files),
            Ok(_) => Value::Array(Vec::new()),
            Err(_) => build_file_list(summary),
        },
        None => build_file_list(summary),
    }
}

/// Summary lookup that also finds terminal downloads evicted from the
/// in-memory map by `max_in_memory_downloads`: aria2 keeps stopped results
/// queryable until `purgeDownloadResult`/`removeDownloadResult`.
async fn summary_for_raw_id(ctx: &RpcContext, raw_id: &str) -> Option<DownloadSummary> {
    if let Some(dm) = ctx.http() {
        if let Some(summary) = dm.get_summary(raw_id).await {
            return Some(summary);
        }
        let db = dm.db.clone();
        let id = raw_id.to_string();
        if let Ok(Ok(Some(manifest))) = tokio::task::spawn_blocking(move || db.get_download_header(&id)).await
            && is_terminal(manifest.state)
        {
            return Some(DownloadSummary::from(&crate::manifest::snapshot_from_manifest(
                &manifest,
            )));
        }
    }
    get_all_summaries(ctx).await.ok()?.into_iter().find(|s| s.id == raw_id)
}

pub(crate) async fn handle_tell_status(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let keys = parse_keys(&params, 1);
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let raw_id = task_id.raw_id();
    let summary = summary_for_raw_id(ctx, &raw_id)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let mut status = summary_to_aria2_status(&summary);
    if matches!(task_id, TaskId::Bt(_)) {
        status["files"] = file_list_for(ctx, &task_id, &summary);
    }

    Ok(match keys {
        Some(keys) => filter_status_keys(status, &keys),
        None => status,
    })
}

pub(crate) async fn handle_tell_active(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let keys = parse_keys(&params, 0);
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
        .map(|status| match &keys {
            Some(keys) => filter_status_keys(status, keys),
            None => status,
        })
        .collect();
    Ok(Value::Array(active))
}

pub(crate) async fn handle_tell_waiting(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let offset: usize = parse_int_param(&params, 0).unwrap_or(0);
    let num: usize = parse_int_param(&params, 1).unwrap_or(1000);
    let keys = parse_keys(&params, 2);

    let all = get_all_summaries(ctx).await?;
    let waiting: Vec<Value> = all
        .iter()
        .filter(|s| matches!(s.state, DownloadState::Queued | DownloadState::Paused))
        .skip(offset)
        .take(num)
        .map(summary_to_aria2_status)
        .map(|status| match &keys {
            Some(keys) => filter_status_keys(status, keys),
            None => status,
        })
        .collect();
    Ok(Value::Array(waiting))
}

pub(crate) async fn handle_tell_stopped(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let offset: usize = parse_int_param(&params, 0).unwrap_or(0);
    let num: usize = parse_int_param(&params, 1).unwrap_or(1000);
    let keys = parse_keys(&params, 2);

    let all = get_all_summaries(ctx).await?;
    let mut stopped: Vec<DownloadSummary> = all
        .into_iter()
        .filter(|s| is_terminal(s.state))
        .collect();
    let mut seen: std::collections::HashSet<String> =
        stopped.iter().map(|s| s.id.clone()).collect();

    // Terminal downloads evicted from memory still exist in the database and
    // must stay visible until they are purged or individually removed.
    if let Some(dm) = ctx.http() {
        let db = dm.db.clone();
        if let Ok(Ok(manifests)) = tokio::task::spawn_blocking(move || db.list_download_headers()).await {
            for manifest in manifests {
                if is_terminal(manifest.state) && seen.insert(manifest.id.clone()) {
                    stopped.push(DownloadSummary::from(&crate::manifest::snapshot_from_manifest(
                        &manifest,
                    )));
                }
            }
        }
    }

    // Deterministic order (creation ascending, id tie-break) so offset/num
    // paging is stable, unlike the previous HashMap iteration order.
    stopped.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms).then_with(|| a.id.cmp(&b.id)));

    let page: Vec<Value> = stopped
        .into_iter()
        .skip(offset)
        .take(num)
        .map(|summary| summary_to_aria2_status(&summary))
        .map(|status| match &keys {
            Some(keys) => filter_status_keys(status, keys),
            None => status,
        })
        .collect();
    Ok(Value::Array(page))
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
    // Count stopped results in the database: terminal downloads may have been
    // evicted from memory, but aria2 clients still see them in `tellStopped`.
    // (`numStoppedTotal` mirrors the current count — we do not keep a lifetime
    // cumulative counter, unlike aria2.)
    let mut num_stopped = all.iter().filter(|s| is_terminal(s.state)).count();
    if let Some(dm) = ctx.http() {
        let db = dm.db.clone();
        if let Ok(Ok(count)) =
            tokio::task::spawn_blocking(move || db.count_terminal_downloads()).await
        {
            num_stopped = count;
        }
    }

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
    let summary = summary_for_raw_id(ctx, &raw_id)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    Ok(file_list_for(ctx, &task_id, &summary))
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

    // The manifest holds the full candidate list (primary + mirrors); the
    // summary only carries the primary URL.
    let manifest = if let Some(dm) = ctx.http() {
        let downloads = dm.downloads.read().await;
        downloads
            .get(&raw_id)
            .map(|managed| managed.lock_core().manifest.clone())
    } else {
        None
    };

    let (primary, mirrors, current) = match manifest {
        Some(manifest) => {
            let current = manifest
                .mirror_url
                .clone()
                .unwrap_or_else(|| manifest.url.clone());
            (manifest.url, manifest.mirror_urls, current)
        }
        None => {
            let summary = summary_for_raw_id(ctx, &raw_id)
                .await
                .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;
            let url = summary.url.clone();
            (url.clone(), Vec::new(), url)
        }
    };

    let mut uris: Vec<Value> = Vec::new();
    for candidate in std::iter::once(primary).chain(mirrors) {
        if uris
            .iter()
            .any(|entry| entry["uri"].as_str() == Some(candidate.as_str()))
        {
            continue;
        }
        let status = if candidate == current {
            "used"
        } else {
            "waiting"
        };
        uris.push(serde_json::json!({"uri": candidate, "status": status}));
    }
    Ok(Value::Array(uris))
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

    let peers = ctx.bt()
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
            "aria2.changeOption",
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
            "aria2.removeDownloadResult",
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
