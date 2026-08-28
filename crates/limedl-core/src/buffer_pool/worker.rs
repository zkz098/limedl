use std::fs::File;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

use crate::error::DownloadError;
use crate::file_ops::{write_all_at, write_all_vectored_at};

/// Write sync policy for IoWorker batch processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// No sync (SSD mode).
    None,
    /// Adaptive sync (HDD periodic/volume threshold: >=16MB or >=3s).
    Adaptive,
    /// Force immediate sync (on pause, cancel, finish, or flush_all).
    Force,
}

/// Command sent to the dedicated I/O worker threads.
enum IoCommand {
    /// Write a batch of (offset, chunk) pairs to a file. The entries are
    /// already in ascending-offset order (drained from a BTreeMap).
    WriteBatch {
        file: Arc<File>,
        entries: Vec<(u64, Bytes)>,
        sync: SyncMode,
        done: oneshot::Sender<Result<(), DownloadError>>,
    },
}

/// Write a list of offset-sorted byte chunks to disk, coalescing adjacent entries
/// into a single system call / vectored I/O (pwritev on Unix) to drastically reduce
/// context switches and eliminate memory allocations.
pub fn write_coalesced_entries(file: &File, entries: &[(u64, Bytes)]) -> Result<(), DownloadError> {
    if entries.is_empty() {
        return Ok(());
    }
    if entries.len() == 1 {
        return write_all_at(file, &entries[0].1, entries[0].0);
    }

    let mut idx = 0;
    while idx < entries.len() {
        let start_offset = entries[idx].0;
        let mut end_offset = start_offset + entries[idx].1.len() as u64;
        let mut next_idx = idx + 1;

        // Group all contiguous slices
        while next_idx < entries.len() && entries[next_idx].0 == end_offset {
            end_offset += entries[next_idx].1.len() as u64;
            next_idx += 1;
        }

        if next_idx == idx + 1 {
            // Single chunk, direct zero-copy write
            write_all_at(file, &entries[idx].1, start_offset)?;
        } else {
            // Multiple adjacent chunks: zero-copy vectored write (Unix) or coalesced single syscall (Windows)
            let slices: Vec<&[u8]> = entries[idx..next_idx]
                .iter()
                .map(|(_, d)| d.as_ref())
                .collect();
            write_all_vectored_at(file, &slices, start_offset)?;
        }

        idx = next_idx;
    }
    Ok(())
}

/// Handle to a pool of dedicated I/O worker threads that serialise flush calls.
///
/// Writes are hash-routed to a specific worker based on file identity,
/// ensuring same-file writes are always serialised (correct for data
/// integrity) while writes to different files can proceed in parallel.
///
/// Cloning produces another set of senders to the same worker threads —
/// all clones share the same underlying OS threads.
#[derive(Clone)]
pub struct IoWorker {
    txs: Vec<mpsc::UnboundedSender<IoCommand>>,
}

impl IoWorker {
    /// Spawn a pool of dedicated I/O worker threads and return a handle to them.
    ///
    /// `n` must be at least 1. Each thread has its own channel for independent
    /// command processing.
    pub fn spawn_pool(n: usize) -> Self {
        let n = n.max(1);
        let mut txs = Vec::with_capacity(n);
        for i in 0..n {
            let (tx, mut rx) = mpsc::unbounded_channel::<IoCommand>();
            thread::Builder::new()
                .name(format!("limedl-io-worker-{i}"))
                .spawn(move || {
                    let mut last_sync = Instant::now();
                    let mut bytes_since_last_sync = 0u64;

                    // Normal processing loop
                    while let Some(cmd) = rx.blocking_recv() {
                        Self::process_command(cmd, &mut last_sync, &mut bytes_since_last_sync);
                    }
                    // All senders dropped — drain any commands that were queued
                    // before the final sender dropped.
                    while let Ok(cmd) = rx.try_recv() {
                        Self::process_command(cmd, &mut last_sync, &mut bytes_since_last_sync);
                    }
                })
                .expect("failed to spawn I/O worker thread");
            txs.push(tx);
        }
        Self { txs }
    }

    /// Convenience: spawn a single-threaded I/O worker (backward-compatible).
    pub fn spawn() -> Self {
        Self::spawn_pool(1)
    }

    /// Process a single I/O command (extracted for reuse in normal loop and drain phase).
    fn process_command(
        cmd: IoCommand,
        last_sync: &mut Instant,
        bytes_since_last_sync: &mut u64,
    ) {
        match cmd {
            IoCommand::WriteBatch {
                file,
                entries,
                done,
                sync,
            } => {
                // Test-only fault injection: force this background batch write to
                // fail, reproducing the production "background buffer flush failed"
                // path that surfaces via `error_flag` + `flush_all`.
                #[cfg(any(test, feature = "test-utils"))]
                {
                    if crate::buffer_pool::fault::consume_batch(Arc::as_ptr(&file) as usize) {
                        let e = DownloadError::Internal(
                            "injected write-batch failure (test fault)".into(),
                        );
                        let _ = done.send(Err(e));
                        return;
                    }
                }

                let batch_bytes: u64 = entries.iter().map(|(_, d)| d.len() as u64).sum();
                let result = (|| -> Result<(), DownloadError> {
                    if !entries.is_empty() {
                        // High-performance coalesced write: merges contiguous slices into single syscalls
                        write_coalesced_entries(&file, &entries)?;
                    }

                    // Adaptive vs Forced hardware sync
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
                // Ignore send error — caller dropped the receiver.
                let _ = done.send(result);
            }
        }
    }

    /// Submit a batch write to a worker thread and await completion.
    ///
    /// Writes are hash-routed to a specific worker based on file identity,
    /// so writes to the same file are always processed by the same thread.
    pub async fn write_batch(
        &self,
        file: Arc<File>,
        entries: Vec<(u64, Bytes)>,
        sync: SyncMode,
    ) -> Result<(), DownloadError> {
        let idx = (Arc::as_ptr(&file) as usize) % self.txs.len();
        let tx = &self.txs[idx];

        let (done_tx, done_rx) = oneshot::channel();
        tx.send(IoCommand::WriteBatch {
            file,
            entries,
            sync,
            done: done_tx,
        })
        .map_err(|_| DownloadError::Internal("I/O worker thread exited unexpectedly".into()))?;
        done_rx
            .await
            .map_err(|_| DownloadError::Internal("I/O worker dropped response".into()))?
    }
}
