use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::buffer_pool::{BufferPool, IoWorker};
use crate::database::Database;
use crate::error::Result;
use crate::event_bus::EventBus;
use crate::io_scheduler::DiskDeviceManager;
use crate::rate_limiter::RateLimiter;
use crate::services::{ConcurrencyManager, DiskIoService, SettingsService};

/// Unified system infrastructure context holding shared runtime resources.
#[derive(Clone)]
pub struct SystemContext {
    pub state_dir: PathBuf,
    pub db: Arc<Database>,
    pub event_bus: Arc<EventBus>,
    pub rate_limiter: Arc<RateLimiter>,
    pub buffer_pool: Arc<BufferPool>,
    pub io_worker: IoWorker,
    pub device_manager: Arc<DiskDeviceManager>,
    pub concurrency: Arc<ConcurrencyManager>,
    pub settings_service: Arc<SettingsService>,
    pub disk_io: Arc<DiskIoService>,
    pub shutdown_token: CancellationToken,
}

impl SystemContext {
    /// Create a standard SystemContext using default RateLimiter and EventBus.
    pub fn new(state_dir: PathBuf) -> Result<Self> {
        let rate_limiter = Arc::new(RateLimiter::default());
        let event_bus = Arc::new(EventBus::new(8192));
        Self::with_components(state_dir, rate_limiter, event_bus)
    }

    /// Create a SystemContext with custom RateLimiter and EventBus instances (e.g. for CLI/tests).
    pub fn with_components(
        state_dir: PathBuf,
        rate_limiter: Arc<RateLimiter>,
        event_bus: Arc<EventBus>,
    ) -> Result<Self> {
        std::fs::create_dir_all(&state_dir)?;

        let settings_path = state_dir
            .parent()
            .ok_or_else(|| {
                crate::error::DownloadError::Internal(format!(
                    "state directory '{}' has no parent — cannot determine settings path",
                    state_dir.display()
                ))
            })?
            .join("settings.json");

        let settings_service = Arc::new(SettingsService::new(settings_path)?);
        let initial_settings = settings_service.get_blocking();

        let db_path = state_dir.join("downloads.db");
        let db = Arc::new(Database::open(&db_path)?);

        crate::migration::migrate_json_manifests(&db, &state_dir)?;

        let io = &initial_settings.io_baseline;
        let buffer_pool = Arc::new(BufferPool::new(
            io.buffer_limit_mb,
            io.game_mode_buffer_mb,
            io.max_parallel_hdd,
            io.game_mode_max_parallel,
        ));
        let device_manager = Arc::new(DiskDeviceManager::new());
        let io_worker = IoWorker::spawn_pool_with_device_manager(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .min(4),
            Some(device_manager.clone()),
        );

        let concurrency = Arc::new(ConcurrencyManager::new(5, 3));
        let disk_io = Arc::new(DiskIoService::new_with_device_manager(
            buffer_pool.clone(),
            settings_service.clone(),
            device_manager.clone(),
        ));
        let shutdown_token = CancellationToken::new();

        Ok(Self {
            state_dir,
            db,
            event_bus,
            rate_limiter,
            buffer_pool,
            io_worker,
            device_manager,
            concurrency,
            settings_service,
            disk_io,
            shutdown_token,
        })
    }
}
