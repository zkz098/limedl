use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use dashmap::DashMap;
use parking_lot::Mutex;

use super::IrontideBtBackend;
use crate::error::Result;
use crate::event_bus::EventBus;
use crate::http_client_factory::build_http_client;
use crate::types::AppSettings;
use crate::types::{BtEncryptionMode, BtPreallocateMode};
use crate::lock;

impl IrontideBtBackend {
    /// Create a new irontide session and wrap it in an `IrontideBtBackend`.
    pub async fn new(
        settings: &AppSettings,
        state_dir: PathBuf,
        default_output_dir: PathBuf,
        event_bus: Arc<EventBus>,
        active_bt_count: Arc<AtomicUsize>,
        max_concurrent_bt: usize,
    ) -> Result<Self> {
        let bt = &settings.bt;

        let mut builder = irontide::ClientBuilder::new()
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
        let port = bt.listen_port.or_else(|| bt.listen_port_range.as_ref().map(|r| r.start));
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

        // Apply BT-specific global rate limits if set.
        if bt.global_download_rate_limit > 0 || bt.global_upload_rate_limit > 0 {
            let mut irontide_settings = irontide::prelude::Settings::default();
            if bt.global_download_rate_limit > 0 {
                irontide_settings.download_rate_limit = bt.global_download_rate_limit;
            }
            if bt.global_upload_rate_limit > 0 {
                irontide_settings.upload_rate_limit = bt.global_upload_rate_limit;
            }
            let _ = session.apply_settings(irontide_settings).await;
        }

        // Apply top-level global speed limit if set (per-torrent fallback).
        if settings.global_speed_limit_bps > 0 {
            tracing::info!(
                "irontide: fallback global speed limit {} B/s will be applied per-torrent",
                settings.global_speed_limit_bps
            );
        }

        let runtime_handle = tokio::runtime::Handle::try_current()
            .map_err(|e| crate::error::DownloadError::Torrent(format!("no tokio runtime: {e}")))?;

        Ok(Self {
            session,
            state_dir,
            default_output_dir,
            bt_settings: Arc::new(Mutex::new(bt.clone())),
            event_bus,
            task_map: Arc::new(DashMap::new()),
            alert_task: Arc::new(Mutex::new(None)),
            upload_policy_task: Arc::new(Mutex::new(None)),
            http_client,
            global_speed_limit_bps: settings.global_speed_limit_bps,
            paused_by_limit: Arc::new(DashMap::new()),
            runtime_handle,
            active_bt_count,
            max_concurrent_bt: AtomicUsize::new(max_concurrent_bt),
            bt_slot_guards: Arc::new(DashMap::new()),
        })
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
            let mut slot = lock(&self.alert_task);
            if let Some(h) = slot.take() {
                h.abort();
            }
        }

        // Phase 3: graceful shutdown
        let _ = self.session.shutdown().await;

        tracing::info!("irontide backend shut down.");
    }

    pub fn update_settings(&self, settings: &AppSettings) {
        let bt = settings.bt.clone();
        *lock(&self.bt_settings) = bt.clone();

        // Apply live rate limit changes immediately (no restart required).
        if bt.global_download_rate_limit > 0 || bt.global_upload_rate_limit > 0 {
            tokio::task::block_in_place(|| {
                self.runtime_handle.block_on(async {
                    let mut irontide_settings = irontide::prelude::Settings::default();
                    if bt.global_download_rate_limit > 0 {
                        irontide_settings.download_rate_limit = bt.global_download_rate_limit;
                    }
                    if bt.global_upload_rate_limit > 0 {
                        irontide_settings.upload_rate_limit = bt.global_upload_rate_limit;
                    }
                    let _ = self.session.apply_settings(irontide_settings).await;
                });
            });
        }

        tracing::debug!("irontide settings updated");
    }
}
