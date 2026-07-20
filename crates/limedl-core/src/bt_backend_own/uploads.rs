use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use irontide::core::Id20;
use parking_lot::Mutex;

use super::IrontideBtBackend;
use crate::event_bus::{DownloadEvent, EventBus};
use crate::lock;

impl IrontideBtBackend {
    pub fn spawn_upload_policy_loop(self: Arc<Self>) {
        // Cancel any existing loop
        let handle = {
            let mut slot = lock(&self.upload_policy_task);
            slot.take()
        };
        if let Some(h) = handle {
            h.abort();
        }

        let session = self.session.clone();
        let bt_settings = self.bt_settings.clone();
        let task_map = self.task_map.clone();
        let event_bus = self.event_bus.clone();
        let paused_by_limit = self.paused_by_limit.clone();

        let join = tokio::spawn(async move {
            upload_policy_loop(session, bt_settings, task_map, event_bus, paused_by_limit).await;
        });

        *lock(&self.upload_policy_task) = Some(join);
    }
}

// ---------------------------------------------------------------------------
//  Upload policy loop
// ---------------------------------------------------------------------------

/// Background loop that periodically enforces upload limits per-torrent.
async fn upload_policy_loop(
    session: irontide::session::SessionHandle,
    bt_settings: Arc<Mutex<crate::types::BtSettings>>,
    task_map: Arc<DashMap<Id20, Id20>>,
    event_bus: Arc<EventBus>,
    paused_by_limit: Arc<DashMap<Id20, ()>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    tracing::info!("irontide upload policy loop started");

    loop {
        interval.tick().await;

        let settings = lock(&bt_settings).clone();
        if settings.upload_limit_bytes == 0 && settings.upload_ratio_limit == 0.0 {
            // If limits were cleared, un-pause any previously paused torrents.
            if !paused_by_limit.is_empty() {
                let to_unpause: Vec<Id20> = paused_by_limit.iter().map(|e| *e.key()).collect();
                paused_by_limit.clear();
                for ih in &to_unpause {
                    let _ = session.set_upload_limit(*ih, 0).await;
                    // Emit a download-updated to reflect unpaused upload status
                    emit_upload_policy_event(&event_bus, *ih, "idle");
                }
            }
            continue;
        }

        for entry in task_map.iter() {
            let info_hash = *entry.key();
            match session.torrent_stats(info_hash).await {
                Ok(stats) => {
                    let limit_reached = settings.upload_limit_bytes > 0
                        && stats.uploaded >= settings.upload_limit_bytes;
                    let ratio_reached = settings.upload_ratio_limit > 0.0
                        && stats.total_done > 0
                        && (stats.uploaded as f64)
                            >= stats.total_done as f64 * settings.upload_ratio_limit;

                    if (limit_reached || ratio_reached)
                        && settings.pause_upload_when_limit_reached
                        && paused_by_limit.get(&info_hash).is_none()
                    {
                        paused_by_limit.insert(info_hash, ());
                        let _ = session.set_upload_limit(info_hash, 1).await;
                        // Emit a download-updated reflecting PausedByLimit
                        emit_upload_policy_event(&event_bus, info_hash, "paused_by_limit");
                    } else if paused_by_limit.get(&info_hash).is_some() {
                        // Was previously paused; un-pause by removing the rate cap.
                        // irontide treats 0 as unlimited.
                        paused_by_limit.remove(&info_hash);
                        let _ = session.set_upload_limit(info_hash, 0).await;
                        emit_upload_policy_event(&event_bus, info_hash, "idle");
                    }
                }
                Err(e) => {
                    tracing::trace!("upload policy: stats error for {info_hash}: {e}");
                }
            }
        }
    }
}

/// Emit a `download-updated` event from the upload policy loop with the given upload status.
fn emit_upload_policy_event(event_bus: &Arc<EventBus>, info_hash: Id20, upload_status: &str) {
    let task_id = info_hash.to_hex();
    event_bus.publish(DownloadEvent::Updated {
        id: task_id,
        summary_json: serde_json::json!({"uploadStatus": upload_status}),
    });
}
