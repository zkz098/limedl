use std::sync::{Arc, Weak};
use std::time::Duration;

use irontide::core::Id20;
use tokio::sync::broadcast;

use super::IrontideBtBackend;
use crate::event_bus::DownloadEvent;
use crate::lock;
use crate::types::{DownloadProgress, DownloadState, DownloadSummary};

impl IrontideBtBackend {
    /// Spawn the alert bridge that listens for irontide alerts and forwards
    /// relevant events to the frontend / Aria2 RPC channel.
    pub async fn setup_alert_bridge(self: &Arc<Self>) {
        // Cancel any existing alert bridge
        {
            let mut slot = lock(&self.alert_task);
            if let Some(h) = slot.take() {
                h.abort();
            }
        }

        // Subscribe here rather than inside the spawned task: alerts emitted
        // between this function returning and the task's first poll would be
        // lost (a broadcast channel has no history for receivers that do not
        // exist yet), so a caller that starts a torrent right after setup would
        // miss its `TorrentAdded`.
        let rx = self.session.subscribe();

        // The loop holds a `Weak`, not an `Arc`: it is stored in `self.alert_task`,
        // so a strong reference would keep the backend (and its session)
        // immortal. It ends when the session's alert sender is dropped.
        let weak = Arc::downgrade(self);
        let handle = tokio::spawn(async move {
            alert_bridge_loop(rx, weak).await;
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

/// # Event Emission Policy
///
/// This bridge is the **single source of truth** for Aria2-compatible events
/// emitted from the BT backend. Lifecycle methods (`pause`/`resume`/`start`)
/// do NOT emit Aria2 events directly — they rely on irontide alerts flowing
/// through this bridge. This avoids duplicate emissions.
///
/// ## Alert → Event mapping:
///
/// | Irontide Alert       | Frontend Events                                      |
/// |----------------------|------------------------------------------------------|
/// | `TorrentAdded`       | `Aria2Notification(aria2.onDownloadStart)`           |
/// | `TorrentPaused`      | `Aria2Notification(aria2.onDownloadPause)`           |
/// | `TorrentResumed`     | `Aria2Notification(aria2.onDownloadStart)`           |
/// | `TorrentFinished`    | `Aria2Notification(onDownloadComplete)` +            |
/// |                      | `Aria2Notification(onBtDownloadComplete)` +          |
/// |                      | `Progress` + `Updated`                               |
/// | `TorrentError`       | `Aria2Notification(onDownloadError)` + `Updated`     |
///
/// The `MetadataReceived` and `TrackerReply` alerts only log; tracker and peer
/// counts reach the UI through the periodic `Progress` tick and the inspector's
/// query path, not through a partial summary event.
///
/// The `Updated` events from lifecycle operations (start, cancel, remove,
/// purge) are emitted by the Dispatcher layer, not this bridge.
///
impl IrontideBtBackend {
    /// Handle one alert. Extracted from [`alert_bridge_loop`] so the mapping above
    /// is testable without waiting for a live engine to emit each variant; alerts
    /// that carry no info hash are ignored.
    pub(super) async fn handle_alert(&self, kind: &irontide::session::AlertKind) {
        use irontide::session::AlertKind;

        let Some(info_hash) = extract_info_hash(kind).copied() else {
            return;
        };

        let task_id = info_hash.to_hex();

        match kind {
            AlertKind::TorrentAdded { .. } => {
                if !self.task_map.contains_key(&info_hash) {
                    self.task_map.insert(info_hash, info_hash);
                }
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onDownloadStart".into(),
                    gid: super::internal_id_to_gid(&info_hash),
                });
            }
            AlertKind::TorrentRemoved { .. } => {
                self.task_map.remove(&info_hash);
            }
            AlertKind::TorrentPaused { .. } => {
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onDownloadPause".into(),
                    gid: super::internal_id_to_gid(&info_hash),
                });
            }
            AlertKind::TorrentResumed { .. } => {
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onDownloadStart".into(),
                    gid: super::internal_id_to_gid(&info_hash),
                });
            }
            AlertKind::TorrentFinished { .. } => {
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onDownloadComplete".into(),
                    gid: super::internal_id_to_gid(&info_hash),
                });
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onBtDownloadComplete".into(),
                    gid: super::internal_id_to_gid(&info_hash),
                });

                match self.session.torrent_stats(info_hash).await {
                    Ok(stats) => {
                        let mut snapshot = self.stats_to_snapshot(&info_hash, &stats);
                        // `TorrentFinished` is authoritative even if the engine's
                        // state lags a tick behind.
                        snapshot.state = DownloadState::Completed;
                        snapshot.speed_bytes_per_second = None;
                        snapshot.upload_speed_bytes_per_second = None;
                        snapshot.eta_seconds = None;
                        self.event_bus.publish(DownloadEvent::Progress {
                            progress: DownloadProgress::from(&snapshot),
                        });
                        self.event_bus.publish(DownloadEvent::Updated {
                            summary: Box::new(DownloadSummary::from(&snapshot)),
                        });
                    }
                    Err(_) => {
                        self.event_bus.publish(DownloadEvent::Updated {
                            summary: Box::new(super::queries::fallback_summary(
                                &task_id,
                                &self.default_output_dir,
                                DownloadState::Completed,
                                &task_id,
                                "irontide reported completion before stats were available",
                                None,
                            )),
                        });
                    }
                }
            }
            AlertKind::MetadataReceived { name, .. } => {
                tracing::debug!("irontide: metadata received for {info_hash} ({name})");
            }
            AlertKind::TorrentError { message, .. } => {
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onDownloadError".into(),
                    gid: super::internal_id_to_gid(&info_hash),
                });
                let summary = match self.session.torrent_stats(info_hash).await {
                    Ok(stats) => {
                        let mut snapshot = self.stats_to_snapshot(&info_hash, &stats);
                        snapshot.state = DownloadState::Failed;
                        snapshot.error = Some(message.clone());
                        DownloadSummary::from(&snapshot)
                    }
                    Err(_) => super::queries::fallback_summary(
                        &task_id,
                        &self.default_output_dir,
                        DownloadState::Failed,
                        &task_id,
                        "irontide reported an error before stats were available",
                        Some(message.clone()),
                    ),
                };
                self.event_bus
                    .publish(DownloadEvent::Updated {
                        summary: Box::new(summary),
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
                // Tracker replies carry no summary fields the UI model reads;
                // the inspector fetches trackers/peers on its own poll.
                tracing::trace!("irontide: tracker {url} replied {num_peers} peers for {info_hash}");
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

    /// Emit a `Progress` event for every torrent in the task map.
    ///
    /// Runs on the alert bridge's 2-second tick (and is called directly by tests).
    pub(super) async fn emit_progress_for_all_torrents(&self) {
        let hashes: Vec<Id20> = self.task_map.iter().map(|e| *e.key()).collect();
        for info_hash in hashes {
            if let Ok(stats) = self.session.torrent_stats(info_hash).await {
                let snapshot = self.stats_to_snapshot(&info_hash, &stats);
                self.event_bus.publish(DownloadEvent::Progress {
                    progress: DownloadProgress::from(&snapshot),
                });
            }
        }
    }
}

/// Background loop that forwards irontide alerts (see [`IrontideBtBackend::handle_alert`])
/// and emits a periodic `Progress` tick for all active torrents every 2 seconds.
async fn alert_bridge_loop(
    mut rx: broadcast::Receiver<irontide::session::Alert>,
    weak: Weak<IrontideBtBackend>,
) {
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

                if let Some(backend) = weak.upgrade() {
                    backend.handle_alert(&alert.kind).await;
                }
            }
            _ = progress_timer.tick() => {
                if let Some(backend) = weak.upgrade() {
                    backend.emit_progress_for_all_torrents().await;
                }
            }
        }
    }

    tracing::info!("irontide alert bridge stopped");
}
