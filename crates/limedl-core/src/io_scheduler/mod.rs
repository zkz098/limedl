//! Unified Global Device-Level I/O Scheduler (DiskDeviceManager).
//!
//! Provides hardware device topology mapping, media-aware queue dispatching (HDD SCAN vs SSD multi-channel),
//! and centralized cross-protocol I/O control for HTTP, BitTorrent, and future protocols.

pub mod queue;
pub mod topology;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fs::File;
use std::hash::BuildHasher;
use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;

pub use queue::{DeviceMetric, DeviceQueue};
pub use topology::{DeviceId, DeviceTopology};
use crate::buffer_pool::SyncMode;
use crate::error::DownloadError;
use crate::file_ops::MediaOverrides;
use crate::types::{DiskType, FastDashMap};

/// Unified global device-level I/O manager.
#[derive(Clone)]
pub struct DiskDeviceManager {
    topology: DeviceTopology,
    queues: Arc<FastDashMap<DeviceId, Arc<DeviceQueue>>>,
}

impl Default for DiskDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskDeviceManager {
    /// Create a new disk device manager instance.
    pub fn new() -> Self {
        Self {
            topology: DeviceTopology::new(),
            queues: Arc::new(FastDashMap::default()),
        }
    }

    /// Resolve the underlying device identifier and media type for a given path.
    pub fn resolve_device(&self, path: &Path) -> (DeviceId, DiskType) {
        self.topology.resolve_device(path)
    }

    /// Apply the per-directory media overrides from settings.
    ///
    /// A [`DeviceQueue`]'s writer-thread count is fixed when the queue is
    /// created, so a queue built under the previous policy cannot adopt a new
    /// one. Changing the overrides therefore drops the queue map: in-flight
    /// writers keep the `Arc` they already hold and their submissions still
    /// complete, and the next lookup creates a queue that matches the new
    /// policy. The alternative — leaving the old queues in place — is the reason
    /// a saved override used to need an app restart to change how many writer
    /// threads a device got.
    pub fn set_overrides<S: BuildHasher>(&self, overrides: &HashMap<String, DiskType, S>) {
        if !self.topology.set_overrides(MediaOverrides::new(overrides)) {
            return;
        }
        let rebuilt = self.queues.len();
        self.queues.clear();
        tracing::debug!("io_scheduler: media overrides changed, dropped {rebuilt} device queues");
    }

    /// Get or create the dedicated I/O queue for a given path.
    pub fn get_or_create_queue(&self, path: &Path) -> Arc<DeviceQueue> {
        let (device_id, disk_type) = self.resolve_device(path);
        self.queues
            .entry(device_id.clone())
            .or_insert_with(|| Arc::new(DeviceQueue::new(device_id, disk_type)))
            .value()
            .clone()
    }

    /// Submit a batch of writes routed directly to the appropriate physical device queue.
    pub async fn write_batch(
        &self,
        path: &Path,
        file: Arc<File>,
        entries: Vec<(u64, Bytes)>,
        sync: SyncMode,
    ) -> Result<(), DownloadError> {
        let queue = self.get_or_create_queue(path);
        queue.write_batch(file, entries, sync).await
    }

    /// Record write activity (bytes and operation count) for global/device-level metrics.
    pub fn record_write(&self, bytes: u64) {
        let queue = self.get_or_create_queue(Path::new("."));
        queue.record_direct_write(bytes);
    }

    /// Collect live runtime metrics for all active physical/logical device queues.
    pub fn get_device_metrics(&self) -> Vec<DeviceMetric> {
        self.queues.iter().map(|entry| entry.value().metrics()).collect()
    }
}
