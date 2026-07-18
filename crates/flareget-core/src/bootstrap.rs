//! Shared subsystem initialization used by both Tauri desktop and NAS server.
//! Single canonical initialization sequence — add new subsystems here once.

use std::path::PathBuf;
use std::sync::Arc;

use crate::error::Result;
use crate::backend_registry::BackendRegistry;
use crate::bt_backend_own::IrontideBtBackend;
use crate::event_bus::EventBus;
use crate::manager::DownloadManager;
use crate::rate_limiter::RateLimiter;
use crate::types::{AppSettings, TaskKind};

/// All initialized core subsystems. Each field is an Arc so it can be shared freely.
pub struct CoreSystems {
    pub download_manager: Arc<DownloadManager>,
    pub bt_backend: Arc<IrontideBtBackend>,
    pub registry: Arc<BackendRegistry>,
    pub event_bus: Arc<EventBus>,
    pub rate_limiter: Arc<RateLimiter>,
    pub settings: AppSettings,
}

/// Initialize all core subsystems in the correct order.
/// This is the SINGLE canonical initialization sequence used by both
/// Tauri desktop and NAS server targets.
pub async fn bootstrap(state_dir: PathBuf) -> Result<CoreSystems> {
    std::fs::create_dir_all(&state_dir)?;

    let rate_limiter = Arc::new(RateLimiter::default());
    let event_bus = Arc::new(EventBus::new(1024));

    let download_manager = DownloadManager::new(
        state_dir.clone(),
        rate_limiter.clone(),
        event_bus.clone(),
    )?;
    let download_manager = Arc::new(download_manager);
    download_manager.clone().start_scheduler_loop();

    let settings = download_manager.initial_settings();

    // Initialize BT backend
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
                event_bus.clone(),
            )
            .await?,
        );
        bt.clone().spawn_upload_policy_loop();
        bt.clone().setup_alert_bridge().await;
        bt
    };

    // Create registry
    let mut registry = BackendRegistry::new();
    registry.register(TaskKind::Http, (*download_manager).clone());
    registry.register(TaskKind::Bt, (*bt_backend).clone());
    let registry = Arc::new(registry);

    Ok(CoreSystems {
        download_manager,
        bt_backend,
        registry,
        event_bus,
        rate_limiter,
        settings,
    })
}
