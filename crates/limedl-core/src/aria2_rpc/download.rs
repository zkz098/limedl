//! Download lifecycle methods: addUri/addTorrent/pause/unpause/remove/purge.

use super::*;

pub(crate) async fn handle_add_uri(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;

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
    let mirror_urls = (uris.len() > 1).then(|| uris.clone());
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
        kind: Some(TaskKind::Http),
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

    // Dedup: if a non-terminal download for this URL already exists, return its GID
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
    // dm dropped — dispatcher.start will handle backend routing

    let task_id = ctx
        .dispatcher
        .start(request)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    let start_paused = options
        .and_then(|o| o.get("pause"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if start_paused {
        let _ = ctx.dispatcher.pause(&task_id).await;
    }

    // Emit initial Updated event so the frontend displays the task immediately.
    if let Ok(snapshot) = ctx.dispatcher.status(&task_id).await {
        ctx.dispatcher.emit_updated(&snapshot);
    }

    let gid = internal_id_to_gid(&task_id.raw_id());
    // Cache the GID so resolve_gid can find it without scanning.
    ctx.gid_cache.lock().await.insert(gid.clone(), task_id);
    broadcast_event(ctx, "aria2.onDownloadStart", &gid);
    Ok(Value::String(gid))
}
pub(crate) async fn handle_add_torrent(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;

    let torrent_b64 = params
        .first()
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing torrent base64 data"))?;

    let torrent_bytes = base64::engine::general_purpose::STANDARD
        .decode(torrent_b64)
        .map_err(|_| make_error(ERR_INVALID_PARAMS, "Invalid base64 torrent data"))?;

    cleanup_old_aria2_temp_files();

    let temp_dir = std::env::temp_dir().join("limedl_aria2");
    std::fs::create_dir_all(&temp_dir).ok();
    let torrent_path = temp_dir.join(format!("{}.torrent", uuid::Uuid::new_v4()));
    std::fs::write(&torrent_path, &torrent_bytes)
        .map_err(|e| make_error(ERR_INTERNAL, format!("Failed to write torrent file: {e}")))?;

    let options = params.get(1).and_then(|v| v.as_object());
    let destination_dir = options
        .and_then(|o| o.get("dir"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| String::from("."))
        });

    let request = StartDownloadRequest {
        kind: Some(TaskKind::Bt),
        url: torrent_path.to_string_lossy().to_string(),
        destination_dir,
        file_name: None,
        user_agent: None,
        thread_mode: None,
        thread_count: None,
        max_retries: None,
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
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

    // The BT backend's emit_pending_summary already emits Updated via the
    // event bus during start(), so no manual emit needed here.

    let gid = internal_id_to_gid(&task_id.raw_id());
    // Cache the GID so resolve_gid can find it without scanning.
    ctx.gid_cache.lock().await.insert(gid.clone(), task_id);
    broadcast_event(ctx, "aria2.onDownloadStart", &gid);
    Ok(Value::String(gid))
}
pub(crate) async fn handle_pause(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    ctx.dispatcher
        .pause(&task_id)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
    broadcast_event(ctx, "aria2.onDownloadPause", &gid);
    Ok(Value::String(gid))
}

pub(crate) async fn handle_unpause(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    ctx.dispatcher
        .resume(&task_id)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
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
        let _ = ctx.dispatcher.pause(&task_id).await;
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
        let _ = ctx.dispatcher.resume(&task_id).await;
    }
    Ok(Value::String("OK".to_string()))
}

pub(crate) async fn handle_remove(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    ctx.dispatcher
        .remove(&task_id)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
    broadcast_event(ctx, "aria2.onDownloadStop", &gid);
    Ok(Value::String(gid))
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

    let purged_count = terminal.len();

    // Remove each terminal download (keep files on disk)
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

    tracing::info!("Purged {purged_count} completed/error/removed downloads");
    Ok(Value::String("OK".to_string()))
}
