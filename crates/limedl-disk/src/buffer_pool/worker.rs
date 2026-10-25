//! Handle to the device-level write sink, used by the download write buffers.
//!
//! `IoWorker` used to own a second, parallel I/O thread pool with its own copy
//! of the coalescing + fsync logic (the `write_batch_fallback` path).  It is now
//! a thin handle over a [`BatchWriteSink`]: every batch goes through the same
//! implementation in `io_scheduler` (per-device queues, HDD serialisation, the
//! `SyncMode` policy and the device metrics), so there is exactly one place
//! where a batch reaches the disk and exactly one place that can fail.
//!
//! The seam is declared in `file_ops` (see [`BatchWriteSink`]) so this module —
//! and therefore all of `buffer_pool` — does not have to name `io_scheduler`.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;

use crate::error::DownloadError;
use crate::file_ops::{BatchWriteSink, SyncMode};

/// Handle to the device-level batch-write sink.
///
/// Cheap to clone (an `Arc` bump); clones share the same device queues.
#[derive(Clone)]
pub struct IoWorker {
    sink: Arc<dyn BatchWriteSink>,
}

impl IoWorker {
    /// Wrap a batch-write sink.  Production wires `io_scheduler::DiskDeviceManager`
    /// in `context.rs`; tests may pass a fake.
    pub fn new(sink: Arc<dyn BatchWriteSink>) -> Self {
        Self { sink }
    }

    /// Submit a batch of offset-sorted `(offset, bytes)` entries.
    ///
    /// `path` selects the physical device queue the batch is routed to, so the
    /// flush lands on the same queue (and metrics bucket) as the rest of that
    /// download.  Writes to one file are serialised; writes to different files
    /// may proceed in parallel.
    pub async fn write_batch(
        &self,
        path: &Path,
        file: Arc<File>,
        entries: Vec<(u64, Bytes)>,
        sync: SyncMode,
    ) -> Result<(), DownloadError> {
        self.sink.write_batch(path, file, entries, sync).await
    }
}
