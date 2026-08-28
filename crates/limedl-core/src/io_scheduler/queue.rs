//! Media-aware device I/O queue implementation.
//!
//! Provides SCAN / elevator serialized scheduling for mechanical hard drives (HDD)
//! and multi-channel parallel non-blocking dispatch for solid-state drives (SSD).

use std::fs::File;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

use super::topology::DeviceId;
use crate::buffer_pool::worker::write_coalesced_entries;
use crate::buffer_pool::SyncMode;
use crate::error::DownloadError;
use crate::types::DiskType;

/// Metrics for a single storage device queue.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceMetric {
    pub device_id: DeviceId,
    pub disk_type: DiskType,
    pub bytes_written: u64,
    pub write_ops_count: u64,
    pub active_writes: usize,
    pub queued_writes: usize,
}

enum QueueCommand {
    WriteBatch {
        file: Arc<File>,
        entries: Vec<(u64, Bytes)>,
        sync: SyncMode,
        done: oneshot::Sender<Result<(), DownloadError>>,
    },
}

/// Device-specific I/O queue.
pub struct DeviceQueue {
    device_id: DeviceId,
    disk_type: DiskType,
    txs: Vec<mpsc::UnboundedSender<QueueCommand>>,
    bytes_written: Arc<AtomicU64>,
    write_ops_count: Arc<AtomicU64>,
    active_writes: Arc<AtomicUsize>,
    queued_writes: Arc<AtomicUsize>,
}

impl DeviceQueue {
    /// Create a new device queue tailored to the device's storage media type.
    pub fn new(device_id: DeviceId, disk_type: DiskType) -> Self {
        let channel_count = match disk_type {
            DiskType::Hdd => 1, // Single serialized writer thread for HDD to eliminate seek storm
            DiskType::Ssd => 4, // Multi-channel parallel writer threads for SSD/NVMe
        };

        let bytes_written = Arc::new(AtomicU64::new(0));
        let write_ops_count = Arc::new(AtomicU64::new(0));
        let active_writes = Arc::new(AtomicUsize::new(0));
        let queued_writes = Arc::new(AtomicUsize::new(0));

        let mut txs = Vec::with_capacity(channel_count);
        for i in 0..channel_count {
            let (tx, mut rx) = mpsc::unbounded_channel::<QueueCommand>();
            let bytes_written_c = Arc::clone(&bytes_written);
            let write_ops_c = Arc::clone(&write_ops_count);
            let active_writes_c = Arc::clone(&active_writes);
            let queued_writes_c = Arc::clone(&queued_writes);
            let dev_str = device_id.to_string();

            thread::Builder::new()
                .name(format!("limedl-dev-{dev_str}-{i}"))
                .spawn(move || {
                    let mut last_sync = Instant::now();
                    let mut bytes_since_last_sync = 0u64;

                    while let Some(cmd) = rx.blocking_recv() {
                        queued_writes_c.fetch_sub(1, Ordering::Relaxed);
                        active_writes_c.fetch_add(1, Ordering::Relaxed);

                        Self::process_command(
                            cmd,
                            &mut last_sync,
                            &mut bytes_since_last_sync,
                            &bytes_written_c,
                            &write_ops_c,
                        );

                        active_writes_c.fetch_sub(1, Ordering::Relaxed);
                    }

                    // Drain remaining commands on shutdown
                    while let Ok(cmd) = rx.try_recv() {
                        queued_writes_c.fetch_sub(1, Ordering::Relaxed);
                        active_writes_c.fetch_add(1, Ordering::Relaxed);
                        Self::process_command(
                            cmd,
                            &mut last_sync,
                            &mut bytes_since_last_sync,
                            &bytes_written_c,
                            &write_ops_c,
                        );
                        active_writes_c.fetch_sub(1, Ordering::Relaxed);
                    }
                })
                .expect("failed to spawn device I/O worker thread");

            txs.push(tx);
        }

        Self {
            device_id,
            disk_type,
            txs,
            bytes_written,
            write_ops_count,
            active_writes,
            queued_writes,
        }
    }

    fn process_command(
        cmd: QueueCommand,
        last_sync: &mut Instant,
        bytes_since_last_sync: &mut u64,
        bytes_written: &AtomicU64,
        write_ops_count: &AtomicU64,
    ) {
        match cmd {
            QueueCommand::WriteBatch {
                file,
                entries,
                sync,
                done,
            } => {
                let batch_bytes: u64 = entries.iter().map(|(_, d)| d.len() as u64).sum();
                let result = (|| -> Result<(), DownloadError> {
                    if !entries.is_empty() {
                        write_coalesced_entries(&file, &entries)?;
                        bytes_written.fetch_add(batch_bytes, Ordering::Relaxed);
                        write_ops_count.fetch_add(1, Ordering::Relaxed);
                    }

                    match sync {
                        SyncMode::None => {}
                        SyncMode::Force => {
                            file.sync_data().map_err(|e| {
                                DownloadError::Internal(format!("fsync failed: {e}"))
                            })?;
                            *last_sync = Instant::now();
                            *bytes_since_last_sync = 0;
                        }
                        SyncMode::Adaptive => {
                            *bytes_since_last_sync += batch_bytes;
                            if *bytes_since_last_sync >= 16 * 1024 * 1024
                                || last_sync.elapsed() >= Duration::from_secs(3)
                            {
                                file.sync_data().map_err(|e| {
                                    DownloadError::Internal(format!("fsync failed: {e}"))
                                })?;
                                *last_sync = Instant::now();
                                *bytes_since_last_sync = 0;
                            }
                        }
                    }
                    Ok(())
                })();

                let _ = done.send(result);
            }
        }
    }

    /// Submit a write batch to this device queue.
    pub async fn write_batch(
        &self,
        file: Arc<File>,
        entries: Vec<(u64, Bytes)>,
        sync: SyncMode,
    ) -> Result<(), DownloadError> {
        let idx = (Arc::as_ptr(&file) as usize) % self.txs.len();
        let tx = &self.txs[idx];

        self.queued_writes.fetch_add(1, Ordering::Relaxed);
        let (done_tx, done_rx) = oneshot::channel();

        tx.send(QueueCommand::WriteBatch {
            file,
            entries,
            sync,
            done: done_tx,
        })
        .map_err(|_| DownloadError::Internal("Device I/O queue thread closed".into()))?;

        done_rx
            .await
            .map_err(|_| DownloadError::Internal("Device I/O queue dropped response".into()))?
    }

    /// Record write activity directly without dispatching a queue command.
    pub fn record_direct_write(&self, bytes: u64) {
        self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
        self.write_ops_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current runtime metrics for this device queue.
    pub fn metrics(&self) -> DeviceMetric {
        DeviceMetric {
            device_id: self.device_id.clone(),
            disk_type: self.disk_type,
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
            write_ops_count: self.write_ops_count.load(Ordering::Relaxed),
            active_writes: self.active_writes.load(Ordering::Relaxed),
            queued_writes: self.queued_writes.load(Ordering::Relaxed),
        }
    }
}
