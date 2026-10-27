use std::sync::Arc;
use std::time::Duration;

use librqbit::api::TorrentIdOrHash;
use librqbit::dht::Id20;
use parking_lot::Mutex;

use crate::types::FastDashMap;

use super::IrontideBtBackend;
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
        let paused_by_limit = self.paused_by_limit.clone();

        let join = tokio::spawn(async move {
            upload_policy_loop(session, bt_settings, task_map, paused_by_limit).await;
        });

        *lock(&self.upload_policy_task) = Some(join);
    }
}

// ---------------------------------------------------------------------------
//  Upload policy loop
// ---------------------------------------------------------------------------

/// Background loop that periodically enforces upload limits per-torrent.
async fn upload_policy_loop(
    session: Arc<librqbit::Session>,
    bt_settings: Arc<Mutex<crate::types::BtSettings>>,
    task_map: Arc<FastDashMap<Id20, Id20>>,
    paused_by_limit: Arc<FastDashMap<Id20, ()>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    tracing::info!("librqbit upload policy loop started");

    loop {
        interval.tick().await;

        let settings = lock(&bt_settings).clone();
        if settings.upload_limit_bytes == 0 && settings.upload_ratio_limit == 0.0 {
            if !paused_by_limit.is_empty() {
                paused_by_limit.clear();
            }
            continue;
        }

        for entry in task_map.iter() {
            let info_hash = *entry.key();
            let Some(handle) = session.get(TorrentIdOrHash::Hash(info_hash)) else {
                continue;
            };

            let stats = handle.stats();
            let limit_reached = settings.upload_limit_bytes > 0
                && stats.uploaded_bytes >= settings.upload_limit_bytes;
            let ratio_reached = settings.upload_ratio_limit > 0.0
                && stats.progress_bytes > 0
                && (stats.uploaded_bytes as f64)
                    >= stats.progress_bytes as f64 * settings.upload_ratio_limit;

            if (limit_reached || ratio_reached)
                && settings.pause_upload_when_limit_reached
                && paused_by_limit.get(&info_hash).is_none()
            {
                paused_by_limit.insert(info_hash, ());
                if stats.finished {
                    let _ = session.pause(&handle).await;
                }
            } else if !(limit_reached || ratio_reached) && paused_by_limit.get(&info_hash).is_some()
            {
                paused_by_limit.remove(&info_hash);
            }
        }
    }
}
