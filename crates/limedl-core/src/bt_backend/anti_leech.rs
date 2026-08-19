use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use irontide::core::Id20;
use parking_lot::Mutex;

use super::IrontideBtBackend;
use crate::event_bus::{DownloadEvent, EventBus};
use crate::lock;
use crate::types::{BtAntiLeechAction, BtSettings};

/// Interval between anti-leech enforcement sweeps.
const ANTI_LEECH_INTERVAL: Duration = Duration::from_secs(10);

/// Fallback upload-slot count used when the current per-torrent slot value
/// cannot be read (e.g. engine quirk) — only used to restore LimitSlots caps.
const DEFAULT_UPLOAD_SLOTS: usize = 8;

/// Effective anti-leech configuration, derived from [`BtSettings`].
/// Kept as a small standalone struct so the detection logic is unit-testable
/// without constructing a full settings object.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AntiLeechConfig {
    pub grace_secs: u64,
    pub ratio: f64,
    pub ban_secs: u64,
    pub max_upload_slots: u32,
    pub action: BtAntiLeechAction,
}

impl AntiLeechConfig {
    fn from_settings(s: &BtSettings) -> Self {
        Self {
            grace_secs: s.anti_leech_grace_secs,
            ratio: s.anti_leech_ratio,
            ban_secs: s.anti_leech_ban_secs,
            max_upload_slots: s.anti_leech_max_upload_slots,
            action: s.anti_leech_action,
        }
    }
}

impl IrontideBtBackend {
    pub fn spawn_anti_leech_loop(self: Arc<Self>) {
        // Cancel any existing loop first (mirrors spawn_upload_policy_loop).
        let handle = {
            let mut slot = lock(&self.anti_leech_task);
            slot.take()
        };
        if let Some(h) = handle {
            h.abort();
        }

        let session = self.session.clone();
        let bt_settings = self.bt_settings.clone();
        let task_map = self.task_map.clone();
        let event_bus = self.event_bus.clone();
        let banned_leechers = self.banned_leechers.clone();
        let slot_state = self.anti_leech_slot_state.clone();

        let join = tokio::spawn(async move {
            anti_leech_loop(
                session,
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

/// Test whether a single peer should be treated as a leecher.
///
/// `unchoked_for` is how long we have been unchoking this peer
/// (`peer_unchoke_durations`); `grace_secs` is the warm-up period before a peer
/// can be flagged. `ratio` is the minimum give-back share (0 disables the
/// rate-ratio check).
pub(crate) fn peer_is_leecher(
    peer: &irontide::session::PeerInfo,
    unchoked_for: Option<Duration>,
    grace_secs: u64,
    ratio: f64,
) -> bool {
    // A peer that declared upload-only (BEP 21) is a seeder — never a leecher.
    if peer.upload_only {
        return false;
    }
    // Only peers we are currently sending data to matter.
    if peer.am_choking {
        return false;
    }
    // Warm-up grace: don't penalise peers we have only just started unchoking.
    let unchoked = unchoked_for.unwrap_or_default().as_secs();
    if unchoked < grace_secs {
        return false;
    }
    // A peer that has finished downloading is a seeder sharing data back.
    if peer.progress >= 1.0 {
        return false;
    }

    // Leecher signal 1: the peer chokes us (refuses to send) while we keep
    // unchoking it and receive no meaningful data back in return.
    if peer.peer_choking && peer.download_rate <= 1 {
        return true;
    }

    // Leecher signal 2: even when not choking us, the peer gives back far less
    // than it takes (give-back share below the threshold) while still
    // downloading from us.
    if ratio > 0.0 && peer.download_rate > 0 && peer.upload_rate > 0 {
        let share = peer.download_rate as f64 / peer.upload_rate as f64;
        if share < ratio {
            return true;
        }
    }

    false
}

/// Background loop that periodically identifies leechers and enforces the
/// configured anti-leech action (ban or upload-slot limiting).
///
/// This is intentionally independent of [`crate::bt_backend::uploads::upload_policy_loop`]:
/// the upload policy loop enforces user-set seed/upload *limits* on the whole
/// torrent, while this loop acts on *individual under-contributing peers*.
async fn anti_leech_loop(
    session: irontide::session::SessionHandle,
    bt_settings: Arc<Mutex<BtSettings>>,
    task_map: Arc<DashMap<Id20, Id20>>,
    event_bus: Arc<EventBus>,
    banned_leechers: Arc<DashMap<IpAddr, u64>>,
    slot_state: Arc<DashMap<Id20, usize>>,
) {
    let mut interval = tokio::time::interval(ANTI_LEECH_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    tracing::info!("irontide anti-leech loop started");

    loop {
        interval.tick().await;

        let settings = lock(&bt_settings).clone();
        if !settings.anti_leech_enabled {
            // When the feature is disabled, clear any leftover slot caps / bans
            // we introduced so the session returns to normal behaviour.
            cleanup_disabled(&session, &banned_leechers, &slot_state).await;
            continue;
        }
        let cfg = AntiLeechConfig::from_settings(&settings);

        for entry in task_map.iter() {
            let info_hash = *entry.key();
            apply_anti_leech_to_torrent(
                &session,
                &event_bus,
                &banned_leechers,
                &slot_state,
                &cfg,
                info_hash,
            )
            .await;
        }

        // Sweep expired bans so previously-flagged peers can reconnect.
        if cfg.action == BtAntiLeechAction::Ban {
            let now_ms = crate::now_ms();
            let expired: Vec<IpAddr> = banned_leechers
                .iter()
                .filter(|e| now_ms >= *e.value())
                .map(|e| *e.key())
                .collect();
            for ip in expired {
                let _ = session.unban_peer(ip).await;
                banned_leechers.remove(&ip);
                tracing::debug!("anti-leech: unban peer {ip} (ban expired)");
            }
        }
    }
}

/// Apply the configured anti-leech action for one torrent based on its
/// currently-detected leechers.
async fn apply_anti_leech_to_torrent(
    session: &irontide::session::SessionHandle,
    event_bus: &Arc<EventBus>,
    banned_leechers: &Arc<DashMap<IpAddr, u64>>,
    slot_state: &Arc<DashMap<Id20, usize>>,
    cfg: &AntiLeechConfig,
    info_hash: Id20,
) {
    let Ok(peers) = session.get_peer_info(info_hash).await else {
        return;
    };
    if peers.is_empty() {
        return;
    }
    let unchoked = session
        .peer_unchoke_durations(info_hash)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    let leechers: Vec<IpAddr> = peers
        .iter()
        .filter(|p| peer_is_leecher(p, unchoked.get(&p.addr).copied(), cfg.grace_secs, cfg.ratio))
        .map(|p| p.addr.ip())
        .collect();

    if leechers.is_empty() && cfg.action == BtAntiLeechAction::LimitSlots {
        // Restore any slot cap we previously applied once the leechers are gone.
        if let Some(orig) = slot_state.remove(&info_hash).map(|(_, v)| v) {
            let _ = session.set_max_uploads(info_hash, orig).await;
            emit_anti_leech_event(event_bus, info_hash, &format!("anti-leech: restored upload slots to {orig}"));
        }
        return;
    }
    if leechers.is_empty() {
        return;
    }

    match cfg.action {
        BtAntiLeechAction::Ban => {
            let ban_until = crate::now_ms() + cfg.ban_secs.saturating_mul(1000);
            for ip in &leechers {
                // Ban each newly-flagged peer once; already-banned peers are
                // left alone until their ban expires (handled by the sweep).
                if banned_leechers.get(ip).is_none()
                    && session.ban_peer(*ip).await.is_ok()
                {
                    banned_leechers.insert(*ip, ban_until);
                    tracing::info!("anti-leech: banned leecher {ip} on {info_hash}");
                    emit_anti_leech_event(
                        event_bus,
                        info_hash,
                        &format!("anti-leech: banned leecher {ip}"),
                    );
                }
            }
        }
        BtAntiLeechAction::LimitSlots => {
            // Remember the original upload-slot count on first cap so we can
            // restore it later.
            if slot_state.get(&info_hash).is_none() {
                let orig = session
                    .max_uploads(info_hash)
                    .await
                    .unwrap_or(DEFAULT_UPLOAD_SLOTS);
                let cap = (cfg.max_upload_slots.max(1)) as usize;
                if session.set_max_uploads(info_hash, cap).await.is_ok() {
                    slot_state.insert(info_hash, orig);
                    emit_anti_leech_event(
                        event_bus,
                        info_hash,
                        &format!("anti-leech: capped upload slots at {cap} ({} leechers)", leechers.len()),
                    );
                }
            }
        }
    }
}

/// When the anti-leech feature is switched off, revoke every ban we issued and
/// restore any upload-slot caps we applied so the session behaves normally.
async fn cleanup_disabled(
    session: &irontide::session::SessionHandle,
    banned_leechers: &Arc<DashMap<IpAddr, u64>>,
    slot_state: &Arc<DashMap<Id20, usize>>,
) {
    let ips: Vec<IpAddr> = banned_leechers.iter().map(|e| *e.key()).collect();
    for ip in ips {
        let _ = session.unban_peer(ip).await;
        banned_leechers.remove(&ip);
    }
    let ihs: Vec<(Id20, usize)> = slot_state
        .iter()
        .map(|e| (*e.key(), *e.value()))
        .collect();
    for (ih, orig) in ihs {
        let _ = session.set_max_uploads(ih, orig).await;
        slot_state.remove(&ih);
    }
}

/// Emit a frontend-visible warning about an anti-leech action.
fn emit_anti_leech_event(event_bus: &Arc<EventBus>, info_hash: Id20, message: &str) {
    event_bus.publish(DownloadEvent::Warning {
        id: info_hash.to_hex(),
        message: message.to_string(),
    });
}
