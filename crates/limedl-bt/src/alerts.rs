use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use librqbit::dht::Id20;

use super::RqbitBtBackend;
use crate::event_bus::DownloadEvent;
use crate::lock;
use crate::types::{DownloadProgress, DownloadSummary};

/// High-level BT lifecycle events detected by the state monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtEvent {
    Added(Id20),
    Paused(Id20),
    Resumed(Id20),
    Finished(Id20),
    Error(Id20, String),
}

impl BtEvent {
    pub fn info_hash(&self) -> &Id20 {
        match self {
            BtEvent::Added(ih)
            | BtEvent::Paused(ih)
            | BtEvent::Resumed(ih)
            | BtEvent::Finished(ih)
            | BtEvent::Error(ih, _) => ih,
        }
    }
}

/// Extract info-hash from a BT event.
pub fn extract_info_hash(event: &BtEvent) -> Option<&Id20> {
    Some(event.info_hash())
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct TorrentLastState {
    paused: bool,
    finished: bool,
    error: bool,
}

impl RqbitBtBackend {
    /// Spawn the state monitor bridge that tracks task status changes and forwards
    /// relevant events to the frontend / Aria2 RPC channel.
    pub async fn setup_alert_bridge(self: &Arc<Self>) {
        // Cancel any existing alert bridge
        {
            let mut slot = lock(&self.alert_task);
            if let Some(h) = slot.take() {
                h.abort();
            }
        }

        let weak = Arc::downgrade(self);
        let handle = tokio::spawn(async move {
            state_monitor_loop(weak).await;
        });

        *lock(&self.alert_task) = Some(handle);
    }

    /// Handle one detected BT event and emit the corresponding Aria2/frontend events.
    pub async fn handle_event(&self, event: &BtEvent) {
        let info_hash = *event.info_hash();
        let gid = super::internal_id_to_gid(&info_hash);

        match event {
            BtEvent::Added(_) => {
                if !self.task_map.contains_key(&info_hash) {
                    self.task_map.insert(info_hash, info_hash);
                }
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onDownloadStart".into(),
                    gid,
                });
            }
            BtEvent::Paused(_) => {
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onDownloadPause".into(),
                    gid,
                });
                if let Ok(snapshot) = self.status(info_hash).await {
                    self.event_bus.publish(DownloadEvent::Updated {
                        summary: Box::new(DownloadSummary::from(&snapshot)),
                    });
                }
            }
            BtEvent::Resumed(_) => {
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onDownloadStart".into(),
                    gid,
                });
                if let Ok(snapshot) = self.status(info_hash).await {
                    self.event_bus.publish(DownloadEvent::Updated {
                        summary: Box::new(DownloadSummary::from(&snapshot)),
                    });
                }
            }
            BtEvent::Finished(_) => {
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onDownloadComplete".into(),
                    gid: gid.clone(),
                });
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onBtDownloadComplete".into(),
                    gid,
                });
                if let Ok(snapshot) = self.status(info_hash).await {
                    self.event_bus.publish(DownloadEvent::Updated {
                        summary: Box::new(DownloadSummary::from(&snapshot)),
                    });
                    self.event_bus.publish(DownloadEvent::Progress {
                        progress: DownloadProgress::from(&snapshot),
                    });
                }
            }
            BtEvent::Error(_, _err) => {
                self.event_bus.publish(DownloadEvent::Aria2Notification {
                    event_name: "aria2.onDownloadError".into(),
                    gid,
                });
                if let Ok(snapshot) = self.status(info_hash).await {
                    self.event_bus.publish(DownloadEvent::Updated {
                        summary: Box::new(DownloadSummary::from(&snapshot)),
                    });
                }
            }
        }
    }

    /// Emit a `Progress` event for every torrent in the task map.
    pub async fn emit_progress_for_all_torrents(&self) {
        let hashes: Vec<Id20> = self.task_map.iter().map(|e| *e.key()).collect();
        for info_hash in hashes {
            if let Ok(snapshot) = self.status(info_hash).await {
                self.event_bus.publish(DownloadEvent::Progress {
                    progress: DownloadProgress::from(&snapshot),
                });
            }
        }
    }
}

/// Background loop that periodically monitors librqbit torrent states and emits progress.
async fn state_monitor_loop(weak: Weak<RqbitBtBackend>) {
    let mut check_interval = tokio::time::interval(Duration::from_millis(500));
    check_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut last_progress_tick = tokio::time::Instant::now();
    let mut known_states: HashMap<Id20, TorrentLastState> = HashMap::new();

    tracing::info!("librqbit state monitor bridge started");

    loop {
        check_interval.tick().await;

        let Some(backend) = weak.upgrade() else {
            break;
        };

        let hashes: Vec<Id20> = backend.task_map.iter().map(|e| *e.key()).collect();
        for info_hash in &hashes {
            let Ok(snapshot) = backend.status(*info_hash).await else {
                continue;
            };

            let is_paused = snapshot.state == crate::types::DownloadState::Paused;
            let is_finished = snapshot.state == crate::types::DownloadState::Completed;
            let is_error = snapshot.state == crate::types::DownloadState::Failed;

            let current = TorrentLastState {
                paused: is_paused,
                finished: is_finished,
                error: is_error,
            };

            match known_states.get(info_hash).copied() {
                None => {
                    backend.handle_event(&BtEvent::Added(*info_hash)).await;
                }
                Some(prev) => {
                    if !prev.finished && current.finished {
                        backend.handle_event(&BtEvent::Finished(*info_hash)).await;
                    } else if !prev.paused && current.paused {
                        backend.handle_event(&BtEvent::Paused(*info_hash)).await;
                    } else if prev.paused && !current.paused {
                        backend.handle_event(&BtEvent::Resumed(*info_hash)).await;
                    } else if !prev.error && current.error {
                        let err = snapshot.error.clone().unwrap_or_default();
                        backend.handle_event(&BtEvent::Error(*info_hash, err)).await;
                    }
                }
            }

            known_states.insert(*info_hash, current);
        }

        known_states.retain(|ih, _| backend.task_map.contains_key(ih));

        if last_progress_tick.elapsed() >= Duration::from_secs(2) {
            backend.emit_progress_for_all_torrents().await;
            last_progress_tick = tokio::time::Instant::now();
        }
    }

    tracing::info!("librqbit state monitor bridge stopped");
}
