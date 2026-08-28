//! Shared subsystem initialization used by both Tauri desktop and NAS server.
//! Single canonical initialization sequence — add new subsystems here once.

use std::path::PathBuf;
use std::sync::Arc;

use crate::backend_registry::BackendRegistry;
#[cfg(feature = "bt")]
use crate::bt_backend::IrontideBtBackend;
use crate::cdn::CdnService;
use crate::context::SystemContext;
use crate::dispatcher::Dispatcher;
use crate::error::Result;
use crate::event_bus::EventBus;
use crate::manager::DownloadManager;
use crate::rate_limiter::RateLimiter;
use crate::services::{ConcurrencyManager, DiskIoService, SettingsService};
use crate::types::{AppSettings, TaskKind};

/// All initialized core subsystems. Each field is an Arc so it can be shared freely.
pub struct CoreSystems {
    pub context: Arc<SystemContext>,
    pub download_manager: Arc<DownloadManager>,
    #[cfg(feature = "bt")]
    pub bt_backend: Arc<IrontideBtBackend>,
    pub registry: Arc<BackendRegistry>,
    pub dispatcher: Arc<Dispatcher>,
    pub event_bus: Arc<EventBus>,
    pub rate_limiter: Arc<RateLimiter>,
    pub settings: AppSettings,
    pub cdn_service: Arc<CdnService>,
    pub settings_service: Arc<SettingsService>,
    pub disk_io_service: Arc<DiskIoService>,
    pub concurrency: Arc<ConcurrencyManager>,
}

/// Initialize all core subsystems in the correct order.
/// This is the SINGLE canonical initialization sequence used by both
/// Tauri desktop and NAS server targets.
pub async fn bootstrap(state_dir: PathBuf) -> Result<CoreSystems> {
    let context = Arc::new(SystemContext::new(state_dir.clone())?);
    let settings = context.settings_service.get_blocking();

    let download_manager = DownloadManager::new(&context)?;
    let download_manager = Arc::new(download_manager);
    download_manager
        .scheduler
        .clone()
        .start_scheduler_loop(download_manager.clone());

    // Initialize BT backend using ConcurrencyManager slots
    #[cfg(feature = "bt")]
    let bt_backend = {
        let bt_state_dir = state_dir.join("torrents");
        let bt_output_dir = state_dir.join("bt_files");
        std::fs::create_dir_all(&bt_state_dir)?;
        std::fs::create_dir_all(&bt_output_dir)?;
        let bt = Arc::new(
            IrontideBtBackend::new(
                &settings,
                bt_state_dir,
                bt_output_dir,
                context.event_bus.clone(),
                context.concurrency.active_bt_count.clone(),
                context.concurrency.max_concurrent_bt.clone(),
            )
            .await?,
        );
        bt.clone().spawn_upload_policy_loop();
        bt.clone().spawn_anti_leech_loop();
        bt.clone().setup_alert_bridge().await;
        bt
    };

    // Create registry — register_arc so the registry shares the SAME Arc
    // instances as CoreSystems (no Clone-with-snapshot divergence).
    let mut registry = BackendRegistry::new();
    registry.register_arc(TaskKind::Http, download_manager.clone());
    #[cfg(feature = "bt")]
    registry.register_arc(TaskKind::Bt, bt_backend.clone());
    let registry = Arc::new(registry);

    // Initialize CDN service
    let cdn_service = Arc::new(CdnService::new());

    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(concat!("limedl/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| crate::error::DownloadError::Internal(format!("Failed to build HTTP client: {e}")))?;

    let dispatcher = Arc::new(Dispatcher::full(
        registry.clone(),
        context.event_bus.clone(),
        context.settings_service.clone(),
        context.disk_io.clone(),
        context.concurrency.clone(),
        cdn_service.clone(),
        http_client,
    ));

    Ok(CoreSystems {
        context: context.clone(),
        download_manager,
        #[cfg(feature = "bt")]
        bt_backend,
        registry,
        dispatcher,
        event_bus: context.event_bus.clone(),
        rate_limiter: context.rate_limiter.clone(),
        settings,
        cdn_service,
        settings_service: context.settings_service.clone(),
        disk_io_service: context.disk_io.clone(),
        concurrency: context.concurrency.clone(),
    })
}
