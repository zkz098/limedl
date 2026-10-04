//! Read-only query methods: tellStatus/tellActive/getFiles/getPeers/session listing.

use super::{BtPeerInfo, DownloadState, DownloadSummary, ERR_INTERNAL, ERR_INVALID_PARAMS, Id20, JsonRpcError, RpcContext, TaskId, TaskKind, Uuid, Value, bitfield_from_bits, bt_files_to_aria2, build_file_list, filter_status_keys, make_error, resolve_gid, summary_to_aria2_status};

/// Effective candidate URIs for a task: the primary URL plus any mirrors, in
/// order and de-duplicated, together with the URI currently in use. Reads the
/// live manifest (a summary only carries the primary URL) and falls back to the
/// summary for downloads evicted from memory.
async fn task_uri_state(
    ctx: &RpcContext,
    raw_id: &str,
) -> Option<(Vec<String>, String)> {
    let manifest = if let Some(dm) = ctx.http() {
        let downloads = dm.downloads.read().await;
        downloads
            .get(raw_id)
            .map(|managed| managed.lock_core().manifest.clone())
    } else {
        None
    };

    match manifest {
        Some(manifest) => {
            let current = manifest
                .mirror_url
                .clone()
                .unwrap_or_else(|| manifest.url.clone());
            let mut candidates: Vec<String> = Vec::new();
            for candidate in std::iter::once(manifest.url).chain(manifest.mirror_urls) {
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
            Some((candidates, current))
        }
        None => {
            let summary = summary_for_raw_id(ctx, raw_id).await?;
            Some((vec![summary.url.clone()], summary.url.clone()))
        }
    }
}

/// The `files[].path` list for HTTP mirrors is the save path; HTTP downloads
/// that are still in memory additionally expose their piece map. Returns the
/// piece length of the first chunk (all but the last share it) and one bool
/// per chunk, or `None` when the manifest is gone or has no chunks yet.
async fn http_pieces(ctx: &RpcContext, raw_id: &str) -> Option<(Option<u64>, Vec<bool>)> {
    let dm = ctx.http()?;
    let managed = dm.downloads.read().await.get(raw_id).cloned()?;
    let core = managed.lock_core();
    let chunks = &core.manifest.chunks;
    if chunks.is_empty() {
        return None;
    }
    let piece_length = chunks.first().map(|chunk| chunk.byte_len());
    Some((piece_length, chunks.iter().map(|chunk| chunk.completed).collect()))
}

/// Add the status fields that need a backend or manifest lookup.
///
/// `bitfield`/`numPieces`/`pieceLength` and the BitTorrent `bittorrent`
/// metadata object are only produced here (not in
/// [`summary_to_aria2_status`]) so the list-wide methods stay cheap. Every
/// lookup is best-effort: a backend error leaves the rest of the status intact.
async fn enrich_status(
    ctx: &RpcContext,
    task_id: &TaskId,
    summary: &DownloadSummary,
    status: &mut Value,
) {
    match task_id {
        TaskId::Http(uuid) => {
            if let Some((piece_length, bits)) = http_pieces(ctx, &uuid.to_string()).await
                && !bits.is_empty()
            {
                status["numPieces"] = Value::String(bits.len().to_string());
                status["bitfield"] = Value::String(bitfield_from_bits(&bits));
                if let Some(length) = piece_length {
                    status["pieceLength"] = Value::String(length.to_string());
                }
            }
        }
        TaskId::Bt(info_hash) => {
            let files = file_list_for(ctx, task_id, summary);
            let file_count = files.as_array().map_or(0, Vec::len);
            status["files"] = files;

            // No `bittorrent` object until the torrent metadata has arrived
            // (`files` is empty for a fresh magnet), mirroring aria2.
            if file_count == 0 {
                return;
            }

            let mut bittorrent = serde_json::Map::new();
            bittorrent.insert(
                "mode".to_string(),
                Value::String(
                    if file_count > 1 { "multi" } else { "single" }.to_string(),
                ),
            );
            // aria2's `bittorrent.info.name` is the torrent's display name.
            bittorrent.insert(
                "info".to_string(),
                serde_json::json!({ "name": summary.file_name }),
            );
            if let Some(backend) = ctx.bt() {
                if let Ok(trackers) = backend.get_trackers(*info_hash) {
                    let announce: Vec<Value> = trackers
                        .iter()
                        .map(|tracker| Value::String(tracker.url.clone()))
                        .collect();
                    if !announce.is_empty() {
                        // aria2 wraps tiers in an outer array; limedl has a
                        // flat tracker list, so it is one tier.
                        bittorrent.insert("announceList".to_string(), Value::Array(vec![Value::Array(announce)]));
                    }
                }
                if let Ok(pieces) = backend.get_pieces(*info_hash) {
                    let mut ordered = pieces;
                    ordered.sort_by_key(|piece| piece.index);
                    let bits: Vec<bool> = ordered.iter().map(|piece| piece.completed).collect();
                    if !bits.is_empty() {
                        status["numPieces"] = Value::String(bits.len().to_string());
                        status["bitfield"] = Value::String(bitfield_from_bits(&bits));
                    }
                }
            }
            status["bittorrent"] = Value::Object(bittorrent);
        }
    }
}

/// aria2 models per-file server lists. limedl's HTTP downloads are
/// single-file, so all mirror candidates are reported under file index 1; the
/// currently used candidate carries the task's live speed and the rest report 0.
fn build_http_server_list(summary: &DownloadSummary, candidates: &[String], current: &str) -> Value {
    let active_speed = summary
        .speed_bytes_per_second
        .map_or(0, |speed| speed as u64);
    let servers: Vec<Value> = candidates
        .iter()
        .map(|candidate| {
            let speed = if candidate == current { active_speed } else { 0 };
            serde_json::json!({
                "uri": candidate,
                "currentUri": candidate,
                "downloadSpeed": speed.to_string(),
            })
        })
        .collect();
    serde_json::json!([{ "index": "1", "servers": servers }])
}

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
    enrich_status(ctx, &task_id, &summary, &mut status).await;

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
    let mut active = Vec::new();
    for summary in all.iter().filter(|s| {
        matches!(
            s.state,
            DownloadState::Downloading | DownloadState::Retrying | DownloadState::Verifying
        )
    }) {
        // The active set is small, so the richer status (piece map, BT
        // metadata) is affordable here; tellWaiting/tellStopped stay cheap.
        let task_id = match summary.kind {
            TaskKind::Http => Uuid::parse_str(&summary.id).ok().map(TaskId::Http),
            TaskKind::Bt => Id20::from_hex(&summary.id).ok().map(TaskId::Bt),
        };
        let Some(task_id) = task_id else {
            continue;
        };
        let mut status = summary_to_aria2_status(summary);
        enrich_status(ctx, &task_id, summary, &mut status).await;
        active.push(match &keys {
            Some(keys) => filter_status_keys(status, keys),
            None => status,
        });
    }
    Ok(Value::Array(active))
}

/// The waiting-queue order shared with the scheduler and `changePosition`:
/// priority first, then creation time, then id for determinism. Used by
/// `tellWaiting` so a client sees the same order the queue will run in.
pub(crate) async fn waiting_order(ctx: &RpcContext) -> Result<Vec<DownloadSummary>, JsonRpcError> {
    let mut waiting: Vec<DownloadSummary> = get_all_summaries(ctx)
        .await?
        .into_iter()
        .filter(|s| matches!(s.state, DownloadState::Queued | DownloadState::Paused))
        .collect();
    waiting.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.created_at_ms.cmp(&b.created_at_ms))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(waiting)
}

pub(crate) async fn handle_tell_waiting(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let offset: usize = parse_int_param(&params, 0).unwrap_or(0);
    let num: usize = parse_int_param(&params, 1).unwrap_or(1000);
    let keys = parse_keys(&params, 2);

    let waiting: Vec<Value> = waiting_order(ctx)
        .await?
        .into_iter()
        .skip(offset)
        .take(num)
        .map(|summary| summary_to_aria2_status(&summary))
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
    let total_upload_speed: u64 = all
        .iter()
        .filter_map(|s| s.upload_speed_bytes_per_second.map(|v| v as u64))
        .sum();

    Ok(serde_json::json!({
        "downloadSpeed": total_speed.to_string(),
        "uploadSpeed": total_upload_speed.to_string(),
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

    let (candidates, current) = task_uri_state(ctx, &raw_id)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let uris: Vec<Value> = candidates
        .iter()
        .map(|candidate| {
            let status = if candidate == &current {
                "used"
            } else {
                "waiting"
            };
            serde_json::json!({"uri": candidate, "status": status})
        })
        .collect();
    Ok(Value::Array(uris))
}

/// `aria2.getServers` — the HTTP(S)/FTP servers backing a download, or the
/// tracker servers of a BitTorrent download. The response is one struct per
/// file index; limedl's HTTP downloads are single-file, so there is one entry.
pub(crate) async fn handle_get_servers(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;
    let raw_id = task_id.raw_id();

    if let TaskId::Bt(info_hash) = &task_id {
        let trackers = ctx
            .bt()
            .ok_or_else(|| make_error(ERR_INTERNAL, "BT backend not available"))?
            .get_trackers(*info_hash)
            .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
        let servers: Vec<Value> = trackers
            .iter()
            .map(|tracker| {
                serde_json::json!({
                    "uri": tracker.url,
                    "currentUri": tracker.url,
                    "downloadSpeed": "0",
                })
            })
            .collect();
        return Ok(serde_json::json!([{ "index": "1", "servers": servers }]));
    }

    let summary = summary_for_raw_id(ctx, &raw_id)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;
    let (candidates, current) = task_uri_state(ctx, &raw_id)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;
    Ok(build_http_server_list(&summary, &candidates, &current))
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
            "aria2.changePosition",
            "aria2.changeUri",
            "aria2.getFiles",
            "aria2.getGlobalOption",
            "aria2.getGlobalStat",
            "aria2.getOption",
            "aria2.getPeers",
            "aria2.getServers",
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
            "aria2.forceShutdown",
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
