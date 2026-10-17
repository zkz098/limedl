//! The write-side seam between the buffering policy and the device scheduler.
//!
//! `buffer_pool::IoWorker` has to submit batch writes somewhere, but the module
//! that actually schedules them per device (`io_scheduler`) is a different
//! subsystem.  Naming `DiskDeviceManager` directly made `buffer_pool` depend on
//! `io_scheduler`; this trait keeps the direction *policy -> capability* and
//! leaves the wiring in `context.rs` as the only place that knows both sides.
//!
//! Implemented by `io_scheduler::DiskDeviceManager`; tests can supply a fake.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;

use super::SyncMode;
use crate::error::DownloadError;

/// A sink that executes batch writes.
///
/// Implementors account for the bytes themselves (the device queue updates its
/// own metrics), so there is no separate `record_write`.
#[async_trait]
pub(crate) trait BatchWriteSink: Send + Sync {
    /// Submit a batch of offset-sorted `(offset, bytes)` entries for `path`.
    async fn write_batch(
        &self,
        path: &Path,
        file: Arc<File>,
        entries: Vec<(u64, Bytes)>,
        sync: SyncMode,
    ) -> Result<(), DownloadError>;
}
