use std::path::Path;
use std::sync::Arc;

use foldhash::HashMap;
use parking_lot::Mutex;

use crate::buffer_pool::BufferPool;
use crate::file_ops::{detect_all_disk_types, detect_disk_type};
use crate::io_scheduler::DiskDeviceManager;
use crate::services::SettingsService;
use crate::types::DiskType;

/// Service for disk type detection, per-device caching, I/O baseline status, and game mode coordination.
#[derive(Clone)]
pub struct DiskIoService {
    #[allow(dead_code)]
    disk_type_cache: Arc<Mutex<HashMap<u64, DiskType>>>,
    buffer_pool: Arc<BufferPool>,
    settings_service: Arc<SettingsService>,
    device_manager: Arc<DiskDeviceManager>,
}

impl DiskIoService {
    pub fn new(buffer_pool: Arc<BufferPool>, settings_service: Arc<SettingsService>) -> Self {
        Self {
            disk_type_cache: Arc::new(Mutex::new(HashMap::default())),
            buffer_pool,
            settings_service,
            device_manager: Arc::new(DiskDeviceManager::new()),
        }
    }

    pub fn new_with_device_manager(
        buffer_pool: Arc<BufferPool>,
        settings_service: Arc<SettingsService>,
        device_manager: Arc<DiskDeviceManager>,
    ) -> Self {
        Self {
            disk_type_cache: Arc::new(Mutex::new(HashMap::default())),
            buffer_pool,
            settings_service,
            device_manager,
        }
    }

    /// Access the underlying DiskDeviceManager.
    pub fn device_manager(&self) -> &Arc<DiskDeviceManager> {
        &self.device_manager
    }

    /// Detect the disk type for a given path directly.
    pub fn detect_disk_type(&self, path: &Path) -> DiskType {
        detect_disk_type(path)
    }

    /// Detect disk types for all attached storage volumes.
    pub fn detect_all_disk_types(&self) -> std::collections::HashMap<String, DiskType> {
        detect_all_disk_types()
    }

    /// Resolve the disk type for a directory, checking settings overrides,
    /// per-device cache (on Unix), and finally performing OS detection.
    pub async fn resolve_disk_type(&self, dir: &Path) -> DiskType {
        let settings = self.settings_service.get().await;
        let dir_str = dir.to_string_lossy().to_string();
        if let Some(disk_type) = settings.io_baseline.disk_type_overrides.get(&dir_str) {
            return *disk_type;
        }
        drop(settings);

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = std::fs::metadata(dir) {
                let dev = meta.dev();
                let mut cache = self.disk_type_cache.lock();
                if let Some(cached) = cache.get(&dev) {
                    return *cached;
                }
                let detected = detect_disk_type(dir);
                cache.insert(dev, detected);
                return detected;
            }
        }

        detect_disk_type(dir)
    }

    /// Return full I/O baseline and buffer pool status payload.
    pub fn get_io_status(&self) -> serde_json::Value {
        let pool = &self.buffer_pool;
        let devices = self.device_manager.get_device_metrics();
        serde_json::json!({
            "gameMode": pool.game_mode(),
            "bufferUsageBytes": pool.current_usage(),
            "bufferLimitBytes": pool.effective_limit(),
            "activeSlots": pool.active_slots(),
            "maxSlots": pool.max_slots(),
            "queuedCount": pool.queued_count(),
            "degradationCount": pool.degradation_count(),
            "devices": devices,
        })
    }

    pub fn game_mode(&self) -> bool {
        self.buffer_pool.game_mode()
    }

    pub fn set_game_mode(&self, enabled: bool) {
        self.buffer_pool.set_game_mode(enabled);
    }

    pub fn toggle_game_mode(&self, enabled: Option<bool>) -> bool {
        let current = self.buffer_pool.game_mode();
        let new_state = enabled.unwrap_or(!current);
        self.buffer_pool.set_game_mode(new_state);
        new_state
    }

    pub fn buffer_pool(&self) -> &Arc<BufferPool> {
        &self.buffer_pool
    }
}
