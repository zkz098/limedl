use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use parking_lot::Mutex;

use crate::types::FastDashMap;

use librqbit::dht::Id20;

use super::IrontideBtBackend;
use crate::error::Result;
use crate::event_bus::EventBus;
use crate::http_client_factory::build_http_client;
use crate::lock;
use crate::types::AppSettings;

impl IrontideBtBackend {
    /// Create a new librqbit session and wrap it in an `IrontideBtBackend`.
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
        tokio::fs::create_dir_all(&resume_dir).await.ok();
        tokio::fs::create_dir_all(&default_output_dir).await.ok();

        let mut opts = librqbit::SessionOptions {
            disable_trackers: false,
            fastresume: true,
            persistence: Some(librqbit::SessionPersistenceConfig::Json {
                folder: Some(resume_dir.clone()),
            }),
            ..Default::default()
        };

        if bt.dht_enabled {
            opts.dht = Some(librqbit::DhtSessionConfig {
                bootstrap_addrs: None,
                port: bt.listen_port,
                persistence: None,
            });
        } else {
            opts.dht = None;
        }

        let mode = if bt.enable_utp {
            librqbit::ListenerMode::TcpAndUtp
        } else {
            librqbit::ListenerMode::TcpOnly
        };
        let mut listener_opts = librqbit::ListenerOptions {
            enable_upnp_port_forwarding: bt.upnp_enabled,
            ipv4_only: !bt.enable_ipv6,
            mode,
            ..Default::default()
        };

        let port = bt
            .listen_port
            .or_else(|| bt.listen_port_range.as_ref().map(|r| r.start));
        if let Some(p) = port {
            listener_opts.listen_addr = if bt.enable_ipv6 {
                (std::net::Ipv6Addr::UNSPECIFIED, p).into()
            } else {
                (std::net::Ipv4Addr::UNSPECIFIED, p).into()
            };
            listener_opts.announce_port = Some(p);
        }
        opts.listen = Some(listener_opts);

        if bt.max_peers_per_torrent > 0 {
            opts.peer_limit = Some(bt.max_peers_per_torrent as usize);
        }

        let mut ratelimits = librqbit::limits::LimitsConfig::default();
        if bt.global_download_rate_limit > 0 {
            ratelimits.download_bps =
                std::num::NonZeroU32::new(bt.global_download_rate_limit as u32);
        }
        if bt.global_upload_rate_limit > 0 {
            ratelimits.upload_bps = std::num::NonZeroU32::new(bt.global_upload_rate_limit as u32);
        }
        opts.ratelimits = ratelimits;

        if bt.blocklist_enabled && !bt.blocklist_path.trim().is_empty() {
            let p = bt.blocklist_path.trim();
            opts.blocklist_url = Some(format!("file://{p}"));
        }

        let session = librqbit::Session::new_with_opts(default_output_dir.clone(), opts)
            .await
            .map_err(|e| crate::error::DownloadError::TorrentNetwork(e.to_string()))?;

        let api = librqbit::api::Api::new(session.clone(), None);

        let http_client = build_http_client(settings)?;

        let runtime_handle = tokio::runtime::Handle::try_current()
            .map_err(|e| crate::error::DownloadError::Torrent(format!("no tokio runtime: {e}")))?;

        let task_map: Arc<FastDashMap<Id20, Id20>> = Arc::new(FastDashMap::default());
        let torrent_created_at: Arc<FastDashMap<Id20, u64>> = Arc::new(FastDashMap::default());

        let created_at = crate::now_ms();
        session.with_torrents(|torrents| {
            for (_, t) in torrents {
                let info_hash = t.info_hash();
                task_map.insert(info_hash, info_hash);
                torrent_created_at.insert(info_hash, created_at);
            }
        });

        if !task_map.is_empty() {
            tracing::info!(
                "librqbit: {} torrent(s) restored from resume state",
                task_map.len()
            );
        }

        let backend = Self {
            session,
            api,
            state_dir,
            default_output_dir,
            bt_settings: Arc::new(Mutex::new(bt.clone())),
            event_bus,
            task_map,
            pending_magnets: Arc::new(FastDashMap::default()),
            alert_task: Arc::new(Mutex::new(None)),
            upload_policy_task: Arc::new(Mutex::new(None)),
            anti_leech_task: Arc::new(Mutex::new(None)),
            banned_leechers: Arc::new(FastDashMap::default()),
            anti_leech_slot_state: Arc::new(FastDashMap::default()),
            applied_blocklist_key: Arc::new(Mutex::new(None)),
            http_client: Arc::new(parking_lot::RwLock::new(http_client)),
            global_speed_limit_bps: settings.global_speed_limit_bps,
            paused_by_limit: Arc::new(FastDashMap::default()),
            runtime_handle,
            active_bt_count,
            max_concurrent_bt,
            bt_slot_guards: Arc::new(FastDashMap::default()),
            torrent_created_at,
        };

        Ok(backend)
    }

    pub async fn shutdown(&self) {
        tracing::info!("librqbit backend shutting down...");

        // Phase 1: abort background tasks
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

        // Phase 2: abort pending magnet resolutions
        for entry in self.pending_magnets.iter() {
            entry.value().abort_handle.abort();
        }
        self.pending_magnets.clear();

        // Phase 3: stop session
        self.session.stop().await;

        tracing::info!("librqbit backend shut down.");
    }

    pub fn apply_settings(&self, settings: &AppSettings) {
        let bt = settings.bt.clone();
        *lock(&self.bt_settings) = bt.clone();

        match build_http_client(settings) {
            Ok(client) => *self.http_client.write() = client,
            Err(error) => tracing::warn!(
                "BT backend: keeping the previous HTTP client, rebuild failed: {error}"
            ),
        }

        let down_limit = std::num::NonZeroU32::new(bt.global_download_rate_limit as u32);
        let up_limit = std::num::NonZeroU32::new(bt.global_upload_rate_limit as u32);
        self.session.ratelimits.set_download_bps(down_limit);
        self.session.ratelimits.set_upload_bps(up_limit);

        tracing::debug!("librqbit settings update applied");
    }

    pub async fn apply_blocklist(&self) {
        // Blocklist is applied during session creation.
    }
}
