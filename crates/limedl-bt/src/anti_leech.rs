use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use librqbit::api::TorrentIdOrHash;
use librqbit::dht::Id20;
use parking_lot::Mutex;

use crate::types::FastDashMap;

use super::RqbitBtBackend;
use crate::event_bus::{DownloadEvent, EventBus};
use crate::lock;
use crate::types::{BtAntiLeechAction, BtSettings};

/// Interval between anti-leech enforcement sweeps.
const ANTI_LEECH_INTERVAL: Duration = Duration::from_secs(10);

/// Effective anti-leech configuration, derived from [`BtSettings`].
#[derive(Debug, Clone, Copy)]
pub struct AntiLeechConfig {
    pub grace_secs: u64,
    pub ratio: f64,
    pub ban_secs: u64,
    pub max_upload_slots: u32,
    pub action: BtAntiLeechAction,
}

impl AntiLeechConfig {
    pub fn from_settings(s: &BtSettings) -> Self {
        Self {
            grace_secs: s.anti_leech_grace_secs,
            ratio: s.anti_leech_ratio,
            ban_secs: s.anti_leech_ban_secs,
            max_upload_slots: s.anti_leech_max_upload_slots,
            action: s.anti_leech_action,
        }
    }
}

/// Peer transfer summary for anti-leech evaluation.
#[derive(Debug, Clone)]
pub struct LeecherPeerInfo {
    pub addr: String,
    pub client_name: Option<String>,
    pub upload_only: bool,
    pub peer_choking: bool,
    pub am_choking: bool,
    pub download_rate: u64,
    pub upload_rate: u64,
    pub progress: f32,
    pub fetched_bytes: u64,
    pub uploaded_bytes: u64,
}

/// Test whether a single peer should be treated as a leecher.
pub fn peer_is_leecher(
    peer: &LeecherPeerInfo,
    unchoked_for: Option<Duration>,
    grace_period_secs: u64,
    ratio: f64,
) -> bool {
    // Seeders (progress >= 1.0 or upload_only) are never leechers.
    if peer.upload_only || peer.progress >= 1.0 {
        return false;
    }
    // We are choking the peer: we are not sending it data.
    if peer.am_choking {
        return false;
    }
    // Warm-up grace period.
    if let Some(dur) = unchoked_for
        && dur.as_secs() < grace_period_secs
    {
        return false;
    }
    // Leecher signal A: peer chokes us while taking our data
    if peer.peer_choking
        && (peer.uploaded_bytes > 0 || peer.upload_rate > 0)
        && (peer.fetched_bytes == 0 && peer.download_rate == 0)
    {
        return true;
    }
    // Leecher signal B: low give-back ratio
    if ratio > 0.0 {
        let up = if peer.upload_rate > 0 {
            peer.upload_rate as f64
        } else {
            peer.uploaded_bytes as f64
        };
        let down = if peer.download_rate > 0 {
            peer.download_rate as f64
        } else {
            peer.fetched_bytes as f64
        };
        if up > 0.0 {
            let share = down / up;
            if share < ratio {
                return true;
            }
        }
    }

    false
}

impl RqbitBtBackend {
    pub fn spawn_anti_leech_loop(self: Arc<Self>) {
        let handle = {
            let mut slot = lock(&self.anti_leech_task);
            slot.take()
        };
        if let Some(h) = handle {
            h.abort();
        }

        let api = self.api.clone();
        let bt_settings = self.bt_settings.clone();
        let task_map = self.task_map.clone();
        let event_bus = self.event_bus.clone();
        let banned_leechers = self.banned_leechers.clone();
        let slot_state = self.anti_leech_slot_state.clone();

        let join = tokio::spawn(async move {
            anti_leech_loop(
                api,
                bt_settings,
                task_map,
                event_bus,
                banned_leechers,
                slot_state,
            )
            .await;
        });

        *lock(&self.anti_leech_task) = Some(join);
    }
}

/// Background loop that periodically identifies leechers and records bans.
async fn anti_leech_loop(
    api: librqbit::api::Api,
    bt_settings: Arc<Mutex<BtSettings>>,
    task_map: Arc<FastDashMap<Id20, Id20>>,
    event_bus: Arc<EventBus>,
    banned_leechers: Arc<FastDashMap<IpAddr, u64>>,
    slot_state: Arc<FastDashMap<Id20, usize>>,
) {
    let mut interval = tokio::time::interval(ANTI_LEECH_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    tracing::info!("librqbit anti-leech loop started");

    loop {
        interval.tick().await;

        let settings = lock(&bt_settings).clone();
        if !settings.anti_leech_enabled {
            banned_leechers.clear();
            slot_state.clear();
            continue;
        }
        let cfg = AntiLeechConfig::from_settings(&settings);

        for entry in task_map.iter() {
            let info_hash = *entry.key();
            let Ok(stats) =
                api.api_peer_stats(TorrentIdOrHash::Hash(info_hash), Default::default())
            else {
                continue;
            };

            for (addr_str, peer) in stats.peers {
                let Ok(sock_addr) = addr_str.parse::<std::net::SocketAddr>() else {
                    continue;
                };
                let ip = sock_addr.ip();

                let info = LeecherPeerInfo {
                    addr: addr_str.clone(),
                    client_name: peer.client_name,
                    upload_only: false,
                    peer_choking: false,
                    am_choking: false,
                    download_rate: 0,
                    upload_rate: 0,
                    progress: 0.0,
                    fetched_bytes: peer.counters.fetched_bytes,
                    uploaded_bytes: peer.counters.uploaded_bytes,
                };

                if peer_is_leecher(&info, None, cfg.grace_secs, cfg.ratio)
                    && !banned_leechers.contains_key(&ip)
                {
                    let expiry = crate::now_ms() + cfg.ban_secs * 1000;
                    banned_leechers.insert(ip, expiry);
                    event_bus.publish(DownloadEvent::Warning {
                        id: info_hash.as_string(),
                        message: format!("Anti-leech: banned leecher {ip} for {}s", cfg.ban_secs),
                    });
                    tracing::warn!(
                        "anti-leech: banned leecher {ip} on torrent {}",
                        info_hash.as_string()
                    );
                }
            }
        }

        // Sweep expired bans
        let now_ms = crate::now_ms();
        let expired: Vec<IpAddr> = banned_leechers
            .iter()
            .filter(|e| now_ms >= *e.value())
            .map(|e| *e.key())
            .collect();
        for ip in expired {
            banned_leechers.remove(&ip);
            tracing::debug!("anti-leech: unban peer {ip} (ban expired)");
        }
    }
}
