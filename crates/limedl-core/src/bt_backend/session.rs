use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use dashmap::DashMap;
use parking_lot::Mutex;

use irontide::core::Id20;

use super::IrontideBtBackend;
use crate::error::Result;
use crate::event_bus::EventBus;
use crate::http_client_factory::build_http_client;
use crate::lock;
use crate::types::AppSettings;
use crate::types::{BtEncryptionMode, BtPreallocateMode};

impl IrontideBtBackend {
    /// Create a new irontide session and wrap it in an `IrontideBtBackend`.
    pub async fn new(
        settings: &AppSettings,
        state_dir: PathBuf,
        default_output_dir: PathBuf,
        event_bus: Arc<EventBus>,
        active_bt_count: Arc<AtomicUsize>,
        max_concurrent_bt: Arc<AtomicUsize>,
    ) -> Result<Self> {
        let bt = &settings.bt;

        let resume_dir = state_dir.join("resume");
        std::fs::create_dir_all(&resume_dir).ok();
        let irontide_settings = irontide::session::Settings {
            resume_data_dir: Some(resume_dir),
            ..Default::default()
        };
        let mut builder = irontide::ClientBuilder::from_settings(irontide_settings)
            .download_dir(&default_output_dir)
            .enable_dht(bt.dht_enabled)
            .enable_upnp(bt.upnp_enabled)
            .enable_natpmp(bt.enable_natpmp)
            .enable_ipv6(bt.enable_ipv6)
            .enable_pex(bt.enable_pex)
            .enable_lsd(bt.enable_lsd)
            .enable_utp(bt.enable_utp)
            .enable_fast_extension(bt.enable_fast_extension)
            .enable_holepunch(bt.enable_holepunch)
            .enable_web_seed(bt.enable_web_seed)
            .super_seeding(bt.enable_super_seeding)
            .preallocate_mode(match bt.preallocate_mode {
                BtPreallocateMode::None => irontide::prelude::PreallocateMode::None,
                BtPreallocateMode::Full => irontide::prelude::PreallocateMode::Full,
            })
            .encryption_mode(match bt.encryption_mode {
                BtEncryptionMode::Enabled => irontide::prelude::EncryptionMode::Enabled,
                BtEncryptionMode::Disabled => irontide::prelude::EncryptionMode::Disabled,
                BtEncryptionMode::Forced => irontide::prelude::EncryptionMode::Forced,
            })
            .active_downloads(bt.max_downloads as i32)
            .active_seeds(bt.max_seeds as i32)
            .max_torrents(bt.max_torrents as usize)
            .active_limit(bt.active_limit as i32);

        // Set listen port if configured
        let port = bt
            .listen_port
            .or_else(|| bt.listen_port_range.as_ref().map(|r| r.start));
        if let Some(p) = port {
            builder = builder.listen_port(p);
        }

        let session = builder
            .start()
            .await
            .map_err(|e| crate::error::DownloadError::TorrentNetwork(e.to_string()))?;

        // Load resume data from any previous session so existing torrents
        // are restored.
        if let Err(e) = session.load_resume_state().await {
            tracing::warn!("irontide: failed to load resume state: {e}");
        }

        // Build an HTTP client with proxy support for .torrent URL fetching
        let http_client = build_http_client(settings).ok();

        // Apply BT-specific engine tuning (choker algorithms, upload slots, peer
        // counts, ban duration, data-contribution timeout) and global rate limits.
        let _ = session.apply_settings(build_engine_settings(bt)).await;

        // Apply top-level global speed limit if set (per-torrent fallback).
        if settings.global_speed_limit_bps > 0 {
            tracing::info!(
                "irontide: fallback global speed limit {} B/s will be applied per-torrent",
                settings.global_speed_limit_bps
            );
        }

        let runtime_handle = tokio::runtime::Handle::try_current()
            .map_err(|e| crate::error::DownloadError::Torrent(format!("no tokio runtime: {e}")))?;

        let backend = Self {
            session,
            state_dir,
            default_output_dir,
            bt_settings: Arc::new(Mutex::new(bt.clone())),
            event_bus,
            task_map: Arc::new(DashMap::new()),
            alert_task: Arc::new(Mutex::new(None)),
            upload_policy_task: Arc::new(Mutex::new(None)),
            anti_leech_task: Arc::new(Mutex::new(None)),
            banned_leechers: Arc::new(DashMap::new()),
            anti_leech_slot_state: Arc::new(DashMap::new()),
            applied_blocklist_key: Arc::new(Mutex::new(None)),
            http_client,
            global_speed_limit_bps: settings.global_speed_limit_bps,
            paused_by_limit: Arc::new(DashMap::new()),
            runtime_handle,
            active_bt_count,
            max_concurrent_bt,
            bt_slot_guards: Arc::new(DashMap::new()),
            torrent_created_at: Arc::new(DashMap::new()),
        };

        backend.apply_blocklist().await;

        Ok(backend)
    }

    pub async fn shutdown(&self) {
        tracing::info!("irontide backend shutting down...");

        // Phase 1: persist session state (before aborting background tasks so any
        // in-flight state updates like TorrentFinished or MetadataReceived are saved)
        if let Err(e) = self.session.save_session_state().await {
            tracing::error!("irontide: failed to save session state: {e}");
        }

        // Phase 2: abort background tasks
        {
            let mut slot = lock(&self.upload_policy_task);
            if let Some(h) = slot.take() {
                h.abort();
            }
        }
        {
            let mut slot = lock(&self.anti_leech_task);
            if let Some(h) = slot.take() {
                h.abort();
            }
        }
        {
            let mut slot = lock(&self.alert_task);
            if let Some(h) = slot.take() {
                h.abort();
            }
        }

        // Phase 3: flush resume data for all active torrents so in-flight disk
        // I/O has a chance to complete before session shutdown
        let active_count = self.task_map.len();
        if active_count > 0 {
            tracing::info!("BT backend: flushing {active_count} active torrents...");
            let info_hashes: Vec<Id20> = self.task_map.iter().map(|entry| *entry.key()).collect();
            for info_hash in &info_hashes {
                if let Err(e) = self.session.save_torrent_resume_data(*info_hash).await {
                    tracing::warn!("irontide: failed to save resume data for {info_hash}: {e}");
                }
            }
        }

        // Phase 4: proportional grace period — allow time for pending disk
        // writes to complete before tearing down the session. Duration scales
        // with active torrent count (500ms each, clamped 1-5s).
        let grace_ms = (active_count as u64 * 500).clamp(1_000, 5_000);
        tracing::info!("BT backend: waiting {grace_ms}ms for pending disk writes...");
        tokio::time::sleep(std::time::Duration::from_millis(grace_ms)).await;
        let _ = self.session.shutdown().await;

        tracing::info!("irontide backend shut down.");
    }

    pub fn apply_settings(&self, settings: &AppSettings) {
        let bt = settings.bt.clone();
        *lock(&self.bt_settings) = bt.clone();

        // Apply engine tuning + rate limits and reload the blocklist. Scheduled
        // onto the captured runtime without blocking so this works whether we
        // are called from a sync Tauri handler or from inside a current-thread
        // runtime (tests).
        let session = self.session.clone();
        let bt_settings = self.bt_settings.clone();
        let applied_blocklist_key = self.applied_blocklist_key.clone();
        self.runtime_handle.spawn(async move {
            let _ = session.apply_settings(build_engine_settings(&bt)).await;
            apply_blocklist_impl(&session, &bt_settings, &applied_blocklist_key).await;
        });

        tracing::debug!("irontide settings update scheduled");
    }

    /// Load the configured peer IP blocklist (if any) and set it on the session,
    /// replacing the previous filter. Re-applies only when the config changes.
    pub(crate) async fn apply_blocklist(&self) {
        apply_blocklist_impl(&self.session, &self.bt_settings, &self.applied_blocklist_key).await;
    }
}

/// Core blocklist application logic (used at startup and on settings reload).
async fn apply_blocklist_impl(
    session: &irontide::session::SessionHandle,
    bt_settings: &Arc<Mutex<crate::types::BtSettings>>,
    applied_key: &Arc<Mutex<Option<String>>>,
) {
    let settings = lock(bt_settings).clone();
    let enabled = settings.blocklist_enabled;
    let path = settings.blocklist_path.trim().to_string();

    // Skip redundant re-applies (config unchanged since last successful load).
    let key = format!("{enabled}:{path}");
    {
        let last = lock(applied_key);
        if last.as_ref() == Some(&key) {
            return;
        }
    }

    if !enabled || path.is_empty() {
        match session.set_ip_filter(irontide::session::IpFilter::new()).await {
            Ok(()) => *lock(applied_key) = Some(key),
            Err(e) => tracing::warn!("blocklist: failed to clear IP filter: {e}"),
        }
        return;
    }

    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("blocklist: failed to read {path}: {e}");
            return;
        }
    };
    let is_dat = path.to_ascii_lowercase().ends_with(".dat");
    let parsed = if is_dat {
        irontide::session::parse_dat(&content)
    } else {
        irontide::session::parse_p2p(&content)
    };

    match parsed {
        Ok(filter) => {
            let ranges = filter.num_ranges();
            match session.set_ip_filter(filter).await {
                Ok(()) => {
                    *lock(applied_key) = Some(key);
                    tracing::info!("blocklist: applied {ranges} blocked ranges from {path}");
                }
                Err(e) => tracing::warn!("blocklist: failed to set IP filter: {e}"),
            }
        }
        Err(e) => {
            // Don't cache the failure so the user can fix the file and re-save.
            *lock(applied_key) = None;
            tracing::warn!("blocklist: failed to parse {path} into an IP filter: {e}");
        }
    }
}


/// Map a limedl seed-choking enum to the irontide engine enum.
fn map_seed_choking(a: crate::types::BtSeedChokingAlgorithm) -> irontide::session::SeedChokingAlgorithm {
    use crate::types::BtSeedChokingAlgorithm::*;
    match a {
        FastestUpload => irontide::session::SeedChokingAlgorithm::FastestUpload,
        RoundRobin => irontide::session::SeedChokingAlgorithm::RoundRobin,
        AntiLeech => irontide::session::SeedChokingAlgorithm::AntiLeech,
    }
}

/// Map a limedl choking enum to the irontide engine enum.
fn map_choking(a: crate::types::BtChokingAlgorithm) -> irontide::session::ChokingAlgorithm {
    use crate::types::BtChokingAlgorithm::*;
    match a {
        FixedSlots => irontide::session::ChokingAlgorithm::FixedSlots,
        RateBased => irontide::session::ChokingAlgorithm::RateBased,
    }
}

/// Build an irontide [`Settings`] from the limedl BT settings: engine tuning
/// fields plus global rate limits. Used at startup and on hot-reload.
fn build_engine_settings(bt: &crate::types::BtSettings) -> irontide::session::Settings {
    irontide::session::Settings {
        download_rate_limit: bt.global_download_rate_limit,
        upload_rate_limit: bt.global_upload_rate_limit,
        seed_choking_algorithm: map_seed_choking(bt.seed_choking_algorithm),
        choking_algorithm: map_choking(bt.choking_algorithm),
        max_upload_slots_per_torrent: bt.max_upload_slots_per_torrent as i32,
        max_peers_per_torrent: bt.max_peers_per_torrent as usize,
        smart_ban_max_failures: bt.smart_ban_max_failures,
        smart_ban_parole: bt.smart_ban_parole,
        eviction_ban_duration_secs: bt.eviction_ban_duration_secs,
        data_contribution_timeout_secs: bt.data_contribution_timeout_secs,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BtChokingAlgorithm, BtSeedChokingAlgorithm, BtSettings};

    #[test]
    fn test_map_seed_choking_algorithms() {
        assert_eq!(
            map_seed_choking(BtSeedChokingAlgorithm::FastestUpload),
            irontide::session::SeedChokingAlgorithm::FastestUpload
        );
        assert_eq!(
            map_seed_choking(BtSeedChokingAlgorithm::RoundRobin),
            irontide::session::SeedChokingAlgorithm::RoundRobin
        );
        assert_eq!(
            map_seed_choking(BtSeedChokingAlgorithm::AntiLeech),
            irontide::session::SeedChokingAlgorithm::AntiLeech
        );
    }

    #[test]
    fn test_map_choking_algorithms() {
        assert_eq!(
            map_choking(BtChokingAlgorithm::FixedSlots),
            irontide::session::ChokingAlgorithm::FixedSlots
        );
        assert_eq!(
            map_choking(BtChokingAlgorithm::RateBased),
            irontide::session::ChokingAlgorithm::RateBased
        );
    }

    #[test]
    fn test_build_engine_settings_maps_tuning_and_limits() {
        let bt = BtSettings {
            global_download_rate_limit: 1024,
            global_upload_rate_limit: 2048,
            seed_choking_algorithm: BtSeedChokingAlgorithm::AntiLeech,
            choking_algorithm: BtChokingAlgorithm::RateBased,
            max_upload_slots_per_torrent: 6,
            max_peers_per_torrent: 200,
            smart_ban_max_failures: 2,
            smart_ban_parole: false,
            eviction_ban_duration_secs: 900,
            data_contribution_timeout_secs: 45,
            ..BtSettings::default()
        };
        let s = build_engine_settings(&bt);
        assert_eq!(s.download_rate_limit, 1024);
        assert_eq!(s.upload_rate_limit, 2048);
        assert_eq!(s.seed_choking_algorithm, irontide::session::SeedChokingAlgorithm::AntiLeech);
        assert_eq!(s.choking_algorithm, irontide::session::ChokingAlgorithm::RateBased);
        assert_eq!(s.max_upload_slots_per_torrent, 6);
        assert_eq!(s.max_peers_per_torrent, 200);
        assert_eq!(s.smart_ban_max_failures, 2);
        assert!(!s.smart_ban_parole);
        assert_eq!(s.eviction_ban_duration_secs, 900);
        assert_eq!(s.data_contribution_timeout_secs, 45);
    }

    #[test]
    fn test_build_engine_settings_defaults() {
        let bt = BtSettings::default();
        let s = build_engine_settings(&bt);
        assert_eq!(s.max_upload_slots_per_torrent, 4);
        assert_eq!(s.max_peers_per_torrent, 128);
        assert_eq!(s.smart_ban_max_failures, 3);
        assert!(s.smart_ban_parole);
        assert_eq!(s.eviction_ban_duration_secs, 600);
        assert_eq!(s.data_contribution_timeout_secs, 60);
    }
}
