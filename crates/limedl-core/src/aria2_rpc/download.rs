//! Download lifecycle methods: addUri/addTorrent/pause/unpause/remove/purge.

use base64::Engine;

use super::{DownloadManager, DownloadState, ERR_INTERNAL, ERR_INVALID_PARAMS, Id20, JsonRpcError, RpcContext, StartDownloadRequest, TaskId, TaskKind, Uuid, Value, broadcast_event, cleanup_old_aria2_temp_files, collect_request_headers, extract_gid, extract_option_str, extract_option_u32, extract_option_usize, get_all_summaries, internal_id_to_gid, make_error, option_is_true, parse_checksum_option, parse_select_file, resolve_gid};

/// Which backend `aria2.addUri` must route a URI to.
///
/// aria2 accepts HTTP(S) and BitTorrent magnet URIs there; a `.torrent` URL is
/// deliberately downloaded as a plain file (clients use `addTorrent` for the
/// parsed form), so it stays HTTP here even though the deeper
/// [`StartDownloadRequest::classify_kind`] heuristic maps that extension to BT.
pub(crate) fn classify_aria2_uri(url: &str) -> TaskKind {
    if url.trim().to_ascii_lowercase().starts_with("magnet:") {
        TaskKind::Bt
    } else {
        TaskKind::Http
    }
}

pub(crate) async fn handle_add_uri(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let uris: Vec<String> = params
        .first()
        .and_then(|v| v.as_array())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing uris array"))?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    if uris.is_empty() {
        return Err(make_error(ERR_INVALID_PARAMS, "No URIs provided"));
    }

    let options = params.get(1).and_then(|v| v.as_object());
    // aria2 treats `uris` as an ordered candidate list: the first entry is the
    // primary target and the remaining entries act as mirrors.
    let url = uris
        .first()
        .cloned()
        .ok_or_else(|| make_error(ERR_INTERNAL, "uris unexpectedly empty"))?;
    // aria2.addUri accepts HTTP(S) and BitTorrent magnet URIs. A `.torrent`
    // URL is deliberately *not* treated as BitTorrent here: aria2 downloads it
    // as a plain file, and clients use addTorrent for the parsed form.
    let kind = classify_aria2_uri(&url);
    let mirror_urls = (kind == TaskKind::Http && uris.len() > 1).then(|| uris.clone());
    let headers = collect_request_headers(options, &url);
    let (checksum, expected_checksum) = parse_checksum_option(options);
    let destination_dir = options
        .and_then(|o| o.get("dir"))
        .and_then(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
        .map(String::from)
        .unwrap_or_else(|| {
            // Use the configured default download directory from settings
            // instead of the process working directory, which is unreliable
            // (changes on restart, may be relative, etc.)
            ctx.settings_default_download_dir()
        });

    let request = StartDownloadRequest {
        kind: Some(kind),
        url,
        destination_dir,
        file_name: extract_option_str(options, "out"),
        user_agent: extract_option_str(options, "user-agent"),
        thread_mode: None,
        thread_count: extract_option_usize(options, "split"),
        max_retries: extract_option_u32(options, "max-tries"),
        checksum,
        expected_checksum,
        selected_file_indices: None,
        headers: (!headers.is_empty()).then_some(headers),
        start_paused: false,
        mirror_urls,
        priority: None,
    };

    // Dedup: if a non-terminal download for this URL already exists, return its GID.
    // Magnets are deduplicated by the BT session itself.
    if kind == TaskKind::Http {
        let dm = ctx
            .registry
            .get_typed::<DownloadManager>()
            .ok_or_else(|| make_error(ERR_INTERNAL, "HTTP backend not available"))?;
        if let Some(existing_id) = dm.find_active_by_url(&request.url).await {
            let gid = internal_id_to_gid(&existing_id);
            // Cache the GID so resolve_gid can find it without scanning.
            if let Ok(uuid) = Uuid::parse_str(&existing_id) {
                ctx.gid_cache
                    .lock()
                    .await
                    .insert(gid.clone(), TaskId::Http(uuid));
            }
            return Ok(Value::String(gid));
        }
    }
    // dm dropped — dispatcher.start will handle backend routing

    let task_id = ctx
        .dispatcher
        .start(request)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    let start_paused = option_is_true(options, "pause");
    if start_paused {
        let _ = ctx.dispatcher.pause(&task_id).await;
    }

    let gid = internal_id_to_gid(&task_id.raw_id());
    // Cache the GID so resolve_gid can find it without scanning.
    ctx.gid_cache.lock().await.insert(gid.clone(), task_id);
    // BT start/pause/resume notifications come from the BT alert bridge;
    // broadcasting here too would duplicate them.
    if kind == TaskKind::Http {
        // Emit initial Updated event so the frontend displays the task immediately.
        if let Ok(snapshot) = ctx.dispatcher.status(&task_id).await {
            ctx.dispatcher.emit_updated(&snapshot);
        }
        broadcast_event(ctx, "aria2.onDownloadStart", &gid);
    }
    Ok(Value::String(gid))
}
pub(crate) async fn handle_add_torrent(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let torrent_b64 = params
        .first()
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing torrent base64 data"))?;

    let torrent_bytes = base64::engine::general_purpose::STANDARD
        .decode(torrent_b64)
        .map_err(|_| make_error(ERR_INVALID_PARAMS, "Invalid base64 torrent data"))?;

    cleanup_old_aria2_temp_files();

    let temp_dir = std::env::temp_dir().join("limedl_aria2");
    tokio::fs::create_dir_all(&temp_dir).await.ok();
    let torrent_path = temp_dir.join(format!("{}.torrent", uuid::Uuid::new_v4()));
    tokio::fs::write(&torrent_path, &torrent_bytes)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, format!("Failed to write torrent file: {e}")))?;

    let options = params.get(1).and_then(|v| v.as_object());
    let destination_dir = options
        .and_then(|o| o.get("dir"))
        .and_then(|v| v.as_str())
        .filter(|dir| !dir.trim().is_empty())
        .map(String::from)
        .unwrap_or_else(|| ctx.settings_default_download_dir());
    let selected_file_indices = options
        .and_then(|o| o.get("select-file"))
        .map(parse_select_file)
        .transpose()?;

    let request = StartDownloadRequest {
        kind: Some(TaskKind::Bt),
        url: torrent_path.to_string_lossy().to_string(),
        destination_dir,
        file_name: extract_option_str(options, "out"),
        user_agent: None,
        thread_mode: None,
        thread_count: None,
        max_retries: None,
        checksum: None,
        expected_checksum: None,
        selected_file_indices,
        headers: None,
        start_paused: false,
        mirror_urls: None,
        priority: None,
    };

    let task_id = ctx
        .dispatcher
        .start(request)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    if option_is_true(options, "pause") {
        let _ = ctx.dispatcher.pause(&task_id).await;
    }

    // The BT backend's emit_pending_summary already emits Updated during
    // start(), and the alert bridge emits aria2.onDownloadStart for
    // TorrentAdded — broadcasting here would duplicate the notification.
    let gid = internal_id_to_gid(&task_id.raw_id());
    // Cache the GID so resolve_gid can find it without scanning.
    ctx.gid_cache.lock().await.insert(gid.clone(), task_id);
    Ok(Value::String(gid))
}
pub(crate) async fn handle_pause(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    ctx.dispatcher
        .pause(&task_id)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
    // BT pause notifications come from the alert bridge (TorrentPaused).
    if matches!(task_id, TaskId::Http(_)) {
        broadcast_event(ctx, "aria2.onDownloadPause", &gid);
    }
    Ok(Value::String(gid))
}

pub(crate) async fn handle_unpause(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    ctx.dispatcher
        .resume(&task_id)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
    // BT resume notifications come from the alert bridge (TorrentResumed).
    if matches!(task_id, TaskId::Http(_)) {
        broadcast_event(ctx, "aria2.onDownloadStart", &gid);
    }
    Ok(Value::String(gid))
}

pub(crate) async fn handle_pause_all(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;
    for s in &all {
        let task_id = match s.kind {
            TaskKind::Http => match Uuid::parse_str(&s.id) {
                Ok(uuid) => TaskId::Http(uuid),
                Err(_) => continue,
            },
            TaskKind::Bt => match Id20::from_hex(&s.id) {
                Ok(ih) => TaskId::Bt(ih),
                Err(_) => continue,
            },
        };
        if ctx.dispatcher.pause(&task_id).await.is_ok() && matches!(task_id, TaskId::Http(_)) {
            broadcast_event(ctx, "aria2.onDownloadPause", &internal_id_to_gid(&s.id));
        }
    }
    Ok(Value::String("OK".to_string()))
}

pub(crate) async fn handle_unpause_all(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;
    for s in &all {
        let task_id = match s.kind {
            TaskKind::Http => match Uuid::parse_str(&s.id) {
                Ok(uuid) => TaskId::Http(uuid),
                Err(_) => continue,
            },
            TaskKind::Bt => match Id20::from_hex(&s.id) {
                Ok(ih) => TaskId::Bt(ih),
                Err(_) => continue,
            },
        };
        if ctx.dispatcher.resume(&task_id).await.is_ok() && matches!(task_id, TaskId::Http(_)) {
            broadcast_event(ctx, "aria2.onDownloadStart", &internal_id_to_gid(&s.id));
        }
    }
    Ok(Value::String("OK".to_string()))
}

pub(crate) async fn handle_remove(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    ctx.dispatcher
        .remove(&task_id)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
    // Drop the cache entry with the task. `addUri` and the scan in `resolve_gid`
    // insert entries that nothing else removes except `purgeDownloadResult`, so
    // without this the map grows for the whole lifetime of the RPC server.
    ctx.gid_cache.lock().await.remove(&gid);
    broadcast_event(ctx, "aria2.onDownloadStop", &gid);
    Ok(Value::String(gid))
}

/// `aria2.removeDownloadResult` — drop a single stopped result.
///
/// aria2 keeps completed/error/removed entries until they are individually
/// removed or purged; AriaNg uses this for the per-row delete button on the
/// stopped list.
pub(crate) async fn handle_remove_download_result(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let gid = extract_gid(&params)?;

    if let Some(task_id) = resolve_gid(ctx, &gid).await {
        // A cached GID can still point at a task that was evicted from memory;
        // only treat a successful status as authoritative.
        if let Ok(snapshot) = ctx.dispatcher.status(&task_id).await {
            if !matches!(
                snapshot.state,
                DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
            ) {
                return Err(make_error(
                    1,
                    format!("GID {gid} is not a stopped download result"),
                ));
            }
            ctx.dispatcher
                .remove(&task_id)
                .await
                .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
            ctx.gid_cache.lock().await.remove(&gid);
            return Ok(Value::String("OK".to_string()));
        }
    }

    // A stopped result may have been evicted from memory by
    // `max_in_memory_downloads`; remove the database row directly.
    let Some(dm) = ctx.registry.get_typed::<DownloadManager>() else {
        return Err(make_error(1, format!("GID not found: {gid}")));
    };
    let db = dm.db.clone();
    let wanted = gid.clone();
    let found = tokio::task::spawn_blocking(move || {
        db.list_download_headers().map(|manifests| {
            manifests
                .into_iter()
                .find(|manifest| internal_id_to_gid(&manifest.id) == wanted)
        })
    })
    .await
    .ok()
    .and_then(std::result::Result::ok)
    .flatten();

    let Some(manifest) = found else {
        return Err(make_error(1, format!("GID not found: {gid}")));
    };
    if !matches!(
        manifest.state,
        DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
    ) {
        return Err(make_error(
            1,
            format!("GID {gid} is not a stopped download result"),
        ));
    }
    let db = dm.db.clone();
    let id = manifest.id;
    tokio::task::spawn_blocking(move || db.delete_download(&id))
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
    ctx.gid_cache.lock().await.remove(&gid);
    Ok(Value::String("OK".to_string()))
}

pub(crate) async fn handle_purge_download_result(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;

    // Collect TaskIds for terminal downloads
    let terminal: Vec<(TaskId, String)> = all
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
            )
        })
        .filter_map(|s| {
            let task_id = match s.kind {
                TaskKind::Http => Uuid::parse_str(&s.id).ok().map(TaskId::Http),
                TaskKind::Bt => Id20::from_hex(&s.id).ok().map(TaskId::Bt),
            };
            task_id.map(|tid| (tid, s.id.clone()))
        })
        .collect();

    let mut purged_count = terminal.len();

    // Remove each terminal download from memory (keep files on disk)
    for (task_id, _id) in &terminal {
        let _ = ctx.dispatcher.remove(task_id).await;
    }

    // Clean up gid_cache entries for purged downloads
    {
        let mut cache = ctx.gid_cache.lock().await;
        for (_task_id, raw_id) in &terminal {
            let gid = internal_id_to_gid(raw_id);
            cache.remove(&gid);
        }
    }

    // Terminal rows evicted from memory are not in `all`; purge them too,
    // otherwise they would keep showing up in `tellStopped`.
    if let Some(dm) = ctx.registry.get_typed::<DownloadManager>() {
        let in_memory: std::collections::HashSet<String> =
            terminal.iter().map(|(_, raw_id)| raw_id.clone()).collect();
        let db = dm.db.clone();
        let removed = tokio::task::spawn_blocking(move || -> usize {
            let mut removed = 0;
            if let Ok(manifests) = db.list_download_headers() {
                for manifest in manifests {
                    if matches!(
                        manifest.state,
                        DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
                    ) && !in_memory.contains(&manifest.id)
                        && db.delete_download(&manifest.id).is_ok()
                    {
                        removed += 1;
                    }
                }
            }
            removed
        })
        .await
        .unwrap_or(0);
        purged_count += removed;
    }

    tracing::info!("Purged {purged_count} completed/error/removed downloads");
    Ok(Value::String("OK".to_string()))
}
