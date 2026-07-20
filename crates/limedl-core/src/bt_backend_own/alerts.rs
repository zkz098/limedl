use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use irontide::core::Id20;
use tokio::sync::broadcast;

use super::IrontideBtBackend;
use super::snapshot::{StateHelpers, estimate_eta, map_state};
use crate::event_bus::{DownloadEvent, EventBus};
use crate::lock;
use crate::types::DownloadState;

impl IrontideBtBackend {
    /// Spawn the alert bridge that listens for irontide alerts and forwards
    /// relevant events to the frontend / Aria2 RPC channel.
    pub async fn setup_alert_bridge(self: &Arc<Self>) {
        let session = self.session.clone();
        let event_bus = self.event_bus.clone();
        let task_map = self.task_map.clone();

        let handle = tokio::spawn(async move {
            alert_bridge_loop(session, event_bus, task_map).await;
        });

        *lock(&self.alert_task) = Some(handle);
    }
}

// ---------------------------------------------------------------------------
//  Alert bridge — forwards irontide events to frontend / Aria2 RPC
// ---------------------------------------------------------------------------

/// Extract the `Id20` info hash from an `AlertKind`, if the variant carries one.
pub(crate) fn extract_info_hash(kind: &irontide::session::AlertKind) -> Option<&Id20> {
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
    event_bus: Arc<EventBus>,
    task_map: Arc<DashMap<Id20, Id20>>,
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

                let task_id = info_hash.to_hex();

                match &alert.kind {
                    AlertKind::TorrentAdded { .. } => {
                        if !task_map.contains_key(info_hash) {
                            task_map.insert(*info_hash, *info_hash);
                        }
                        event_bus.publish(DownloadEvent::Aria2Notification {
                            event_name: "aria2.onDownloadStart".into(),
                            gid: super::internal_id_to_gid(info_hash),
                        });
                    }
                    AlertKind::TorrentRemoved { .. } => {
                        task_map.remove(info_hash);
                    }
                    AlertKind::TorrentPaused { .. } => {
                        event_bus.publish(DownloadEvent::Aria2Notification {
                            event_name: "aria2.onDownloadPause".into(),
                            gid: super::internal_id_to_gid(info_hash),
                        });
                    }
                    AlertKind::TorrentResumed { .. } => {
                        event_bus.publish(DownloadEvent::Aria2Notification {
                            event_name: "aria2.onDownloadStart".into(),
                            gid: super::internal_id_to_gid(info_hash),
                        });
                    }
                    AlertKind::TorrentFinished { .. } => {
                        event_bus.publish(DownloadEvent::Aria2Notification {
                            event_name: "aria2.onDownloadComplete".into(),
                            gid: super::internal_id_to_gid(info_hash),
                        });
                        event_bus.publish(DownloadEvent::Aria2Notification {
                            event_name: "aria2.onBtDownloadComplete".into(),
                            gid: super::internal_id_to_gid(info_hash),
                        });

                        // Fetch stats
                        let stats = session.torrent_stats(*info_hash).await.ok();

                        // Emit progress with final stats
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
                            event_bus.publish(DownloadEvent::Progress {
                                id: task_id.clone(),
                                progress_json: progress,
                            });
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
                        event_bus.publish(DownloadEvent::Updated {
                            id: task_id,
                            summary_json: updated,
                        });
                    }
                    AlertKind::MetadataReceived { name, .. } => {
                        tracing::debug!("irontide: metadata received for {info_hash} ({name})");
                    }
                    AlertKind::TorrentError { message, .. } => {
                        event_bus.publish(DownloadEvent::Aria2Notification {
                            event_name: "aria2.onDownloadError".into(),
                            gid: super::internal_id_to_gid(info_hash),
                        });
                        event_bus.publish(DownloadEvent::Updated {
                            id: task_id.clone(),
                            summary_json: serde_json::json!({"id": task_id, "state": "error", "error": message}),
                        });
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
                            event_bus.publish(DownloadEvent::Updated {
                                id: task_id.clone(),
                                summary_json: serde_json::json!({"id": task_id, "tracker": url, "peers": num_peers}),
                            });
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
                let hashes: Vec<Id20> = task_map.iter().map(|e| *e.key()).collect();
                for info_hash in hashes {
                    if let Ok(stats) = session.torrent_stats(info_hash).await {
                        let task_id = info_hash.to_hex();

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

                        event_bus.publish(DownloadEvent::Progress {
                            id: task_id,
                            progress_json: progress,
                        });
                    }
                }
            }
        }
    }

    tracing::info!("irontide alert bridge stopped");
}
