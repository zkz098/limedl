use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::buffer_pool::{BufferPool, IoWorker};
use crate::database::Database;
use crate::error::Result;
use crate::event_bus::EventBus;
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
    pub concurrency: Arc<ConcurrencyManager>,
    pub settings_service: Arc<SettingsService>,
    pub disk_io: Arc<DiskIoService>,
    pub shutdown_token: CancellationToken,
}

impl SystemContext {
    pub fn new(state_dir: PathBuf) -> Result<Self> {
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
        let io_worker = IoWorker::spawn_pool(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .min(4),
        );

        let rate_limiter = Arc::new(RateLimiter::default());
        let event_bus = Arc::new(EventBus::new(8192));
        let concurrency = Arc::new(ConcurrencyManager::new(5, 3));
        let disk_io = Arc::new(DiskIoService::new(
            buffer_pool.clone(),
            settings_service.clone(),
        ));
        let shutdown_token = CancellationToken::new();

        Ok(Self {
            state_dir,
            db,
            event_bus,
            rate_limiter,
            buffer_pool,
            io_worker,
            concurrency,
            settings_service,
            disk_io,
            shutdown_token,
        })
    }
}
