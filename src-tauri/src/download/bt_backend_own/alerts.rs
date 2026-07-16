use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use irontide::core::Id20;
use parking_lot::Mutex;
use tauri::Emitter;
use tokio::sync::broadcast;

use super::OwnBtBackend;
use super::snapshot::{map_state, estimate_eta, StateHelpers};
use super::super::types::DownloadState;
use super::super::lock;

impl OwnBtBackend {
    /// Spawn the alert bridge that listens for irontide alerts and forwards
    /// relevant events to the frontend / Aria2 RPC channel.
    pub async fn setup_alert_bridge(self: &Arc<Self>) {
        let session = self.session.clone();
        let event_tx = self.event_tx.clone();
        let task_map = self.task_map.clone();
        let app_handle = self.app_handle.clone();

        let handle = tokio::spawn(async move {
            alert_bridge_loop(session, event_tx, task_map, app_handle).await;
        });

        *lock(&self.alert_task) = Some(handle);
    }
}

// ---------------------------------------------------------------------------
//  Alert bridge — forwards irontide events to frontend / Aria2 RPC
// ---------------------------------------------------------------------------

/// Extract the `Id20` info hash from an `AlertKind`, if the variant carries one.
pub(crate) fn extract_info_hash<'a>(kind: &'a irontide::session::AlertKind) -> Option<&'a Id20> {
    use irontide::session::AlertKind::*;
    match kind {
        TorrentAdded { info_hash, .. }
        | TorrentRemoved { info_hash }
        | TorrentPaused { info_hash }
        | TorrentResumed { info_hash }
        | TorrentFinished { info_hash }
        | StateChanged { info_hash, .. }
        | MetadataReceived { info_hash, .. }
        | MetadataFailed { info_hash }
        | TorrentChecked { info_hash, .. }
        | CheckingProgress { info_hash, .. }
        | PieceFinished { info_hash, .. }
        | BlockFinished { info_hash, .. }
        | HashFailed { info_hash, .. }
        | PeerConnected { info_hash, .. }
        | PeerDisconnected { info_hash, .. }
        | PeerBanned { info_hash, .. }
        | TrackerReply { info_hash, .. }
        | TrackerWarning { info_hash, .. }
        | TrackerError { info_hash, .. }
        | ScrapeReply { info_hash, .. }
        | ScrapeError { info_hash, .. }
        | DhtGetPeers { info_hash, .. }
        | FileCompleted { info_hash, .. }
        | FileRenamed { info_hash, .. }
        | StorageMoved { info_hash, .. }
        | FileError { info_hash, .. }
        | ResumeDataSaved { info_hash }
        | TorrentError { info_hash, .. }
        | PerformanceWarning { info_hash, .. }
        | TorrentQueuePositionChanged { info_hash, .. }
        | TorrentAutoManaged { info_hash, .. }
        | WebSeedBanned { info_hash, .. }
        | HolepunchSucceeded { info_hash, .. }
        | HolepunchFailed { info_hash, .. }
        | PeerTurnover { info_hash, .. }
        | SslTorrentError { info_hash, .. }
        | InconsistentHashes { info_hash, .. } => Some(info_hash),
        _ => None,
    }
}

/// Background loop that subscribes to irontide alerts and emits events,
/// with periodic progress emission every 2 seconds for all active torrents.
async fn alert_bridge_loop(
    session: irontide::session::SessionHandle,
    event_tx: Arc<Mutex<Option<broadcast::Sender<String>>>>,
    task_map: Arc<DashMap<String, Id20>>,
    app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
) {
    use irontide::session::AlertKind;

    let mut rx = session.subscribe();
    let mut progress_timer = tokio::time::interval(Duration::from_secs(2));
    progress_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    tracing::info!("irontide alert bridge started");

    loop {
        tokio::select! {
            alert = rx.recv() => {
                let alert = match alert {
                    Ok(a) => a,
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("irontide alert bridge stopped (channel closed)");
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("irontide alert bridge lagged by {n} messages");
                        continue;
                    }
                };

                // Try to extract info_hash — many AlertKind variants carry it.
                let info_hash = extract_info_hash(&alert.kind);
                let Some(info_hash) = info_hash else {
                    continue;
                };

                let task_id = format!("{}{}", super::BT_PREFIX, info_hash.to_hex());

                match &alert.kind {
                    AlertKind::TorrentAdded { .. } => {
                        if !task_map.contains_key(&task_id) {
                            task_map.insert(task_id.clone(), *info_hash);
                        }
                        emit_alert_event(&event_tx, "aria2.onDownloadStart", &task_id);
                    }
                    AlertKind::TorrentRemoved { .. } => {
                        task_map.remove(&task_id);
                    }
                    AlertKind::TorrentPaused { .. } => {
                        emit_alert_event(&event_tx, "aria2.onDownloadPause", &task_id);
                    }
                    AlertKind::TorrentResumed { .. } => {
                        emit_alert_event(&event_tx, "aria2.onDownloadStart", &task_id);
                    }
                    AlertKind::TorrentFinished { .. } => {
                        emit_alert_event(&event_tx, "aria2.onDownloadComplete", &task_id);
                        emit_alert_event(
                            &event_tx,
                            "aria2.onBtDownloadComplete",
                            &task_id,
                        );

                        // Fetch stats OUTSIDE the app_handle lock so the guard drops before .await
                        let stats = session.torrent_stats(*info_hash).await.ok();
                        if let Some(ref app) = *lock(&app_handle) {
                            let _ = app.emit("download-completed", serde_json::json!({"id": task_id}));
                            if let Some(ref s) = stats {
                                let progress = serde_json::json!({
                                    "id": task_id,
                                    "state": "completed",
                                    "downloadedBytes": s.total_done,
                                    "totalBytes": s.total,
                                    "speedBytesPerSecond": 0,
                                    "connectionCount": s.peers_connected,
                                    "uploadedBytes": s.uploaded,
                                    "uploadSpeedBytesPerSecond": 0,
                                    "peerCount": s.peers_connected,
                                    "uploadStatus": "idle",
                                });
                                let _ = app.emit("download-progress", &progress);
                            }
                            // Emit download-updated so the frontend gets the full summary update
                            let updated = serde_json::json!({
                                "id": task_id,
                                "state": "completed",
                                "downloadedBytes": stats.as_ref().map(|s| s.total_done),
                                "totalBytes": stats.as_ref().map(|s| s.total),
                                "uploadedBytes": stats.as_ref().and_then(|s| if s.uploaded > 0 { Some(s.uploaded) } else { None }),
                                "uploadStatus": "idle",
                                "connectionCount": stats.as_ref().map(|s| s.peers_connected).unwrap_or(0),
                                "peerCount": stats.as_ref().map(|s| s.peers_connected),
                            });
                            let _ = app.emit("download-updated", &updated);
                        }
                    }
                    AlertKind::MetadataReceived { name, .. } => {
                        tracing::debug!("irontide: metadata received for {info_hash} ({name})");
                    }
                    AlertKind::TorrentError { message, .. } => {
                        emit_alert_event(&event_tx, "aria2.onDownloadError", &task_id);
                        if let Some(ref app) = *lock(&app_handle) {
                            let _ = app.emit(
                                "download-error",
                                serde_json::json!({"id": task_id, "error": message}),
                            );
                        }
                    }
                    AlertKind::StateChanged { prev_state, new_state, .. } => {
                        tracing::trace!(
                            "irontide: state change for {info_hash}: {prev_state:?} -> {new_state:?}"
                        );
                    }
                    AlertKind::TorrentChecked { pieces_have, pieces_total, .. } => {
                        tracing::debug!("irontide: check complete for {info_hash} ({pieces_have}/{pieces_total})");
                    }
                    AlertKind::FileCompleted { file_index, .. } => {
                        tracing::debug!("irontide: file #{file_index} complete for {info_hash}");
                    }
                    AlertKind::TrackerReply { num_peers, url, .. } => {
                        if *num_peers > 0 {
                            if let Some(ref app) = *lock(&app_handle) {
                                let _ = app.emit(
                                    "tracker-info",
                                    serde_json::json!({"id": task_id, "tracker": url, "peers": num_peers}),
                                );
                            }
                        }
                    }
                    AlertKind::TrackerError { message, url, .. } => {
                        tracing::warn!("irontide: tracker error for {url}: {message}");
                    }
                    AlertKind::TrackerWarning { message, url, .. } => {
                        tracing::warn!("irontide: tracker warning for {url}: {message}");
                    }
                    AlertKind::HashFailed { piece, .. } => {
                        tracing::warn!("irontide: hash check failed for {info_hash} piece {piece}");
                    }
                    AlertKind::PeerConnected { addr, .. } => {
                        tracing::trace!("irontide: peer connected {addr}");
                    }
                    AlertKind::PeerDisconnected { addr, .. } => {
                        tracing::trace!("irontide: peer disconnected {addr}");
                    }
                    AlertKind::StorageMoved { new_path, .. } => {
                        tracing::info!("irontide: storage moved to {}", new_path.display());
                    }
                    AlertKind::FileError { path, message, .. } => {
                        tracing::warn!("irontide: file error at {}: {message}", path.display());
                    }
                    // Session stats / non-torrent alerts — ignore.
                    _ => {}
                }
            }
            _ = progress_timer.tick() => {
                // Periodic progress emission for all active torrents.
                let hashes: Vec<Id20> = task_map.iter().map(|e| *e.value()).collect();
                for info_hash in hashes {
                    if let Ok(stats) = session.torrent_stats(info_hash).await {
                        let task_id = format!("{}{}", super::BT_PREFIX, info_hash.to_hex());

                        // Build a DownloadProgress-compatible JSON for the frontend.
                        let dl_state = map_state(&stats.state);
                        let total = stats.total_wanted;
                        // Use `downloaded` (payload bytes) for smooth progress.
                        let downloaded = stats.downloaded;
                        let terminal = dl_state.is_terminal();
                        let speed = if terminal {
                            0.0
                        } else {
                            stats.download_payload_rate as f64
                        };
                        let upload_speed = if terminal {
                            0.0
                        } else {
                            stats.upload_payload_rate as f64
                        };
                        let eta = if terminal || speed <= 0.0 {
                            None
                        } else {
                            estimate_eta(total, downloaded, Some(speed))
                        };
                        let upload_status: &str = match dl_state {
                            DownloadState::Paused => "paused",
                            _ if stats.upload_payload_rate > 0 => "uploading",
                            _ => "idle",
                        };
                        let progress = serde_json::json!({
                            "id": task_id,
                            "state": dl_state,
                            "downloadedBytes": downloaded,
                            "totalBytes": total,
                            "speedBytesPerSecond": speed,
                            "connectionCount": stats.peers_connected,
                            "uploadedBytes": stats.uploaded,
                            "uploadSpeedBytesPerSecond": upload_speed,
                            "peerCount": stats.peers_connected,
                            "uploadStatus": upload_status,
                            "etaSeconds": eta,
                        });

                        if let Some(ref app) = *lock(&app_handle) {
                            let _ = app.emit("download-progress", &progress);
                        }
                    }
                }
            }
        }
    }

    tracing::info!("irontide alert bridge stopped");
}

/// Helper: emit an event to the Aria2 RPC channel.
pub(crate) fn emit_alert_event(
    event_tx: &Arc<Mutex<Option<broadcast::Sender<String>>>>,
    method: &str,
    task_id: &str,
) {
    // Aria2 RPC broadcast
    if let Some(ref tx) = *lock(event_tx) {
        let gid = super::super::aria2_rpc::internal_id_to_gid(task_id);
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": [{"gid": gid}]
        });
        let _ = tx.send(serde_json::to_string(&payload).unwrap_or_default());
    }
}
