use std::fs::File;
use std::sync::Arc;
use std::thread;

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

use crate::error::DownloadError;
use crate::file_ops::write_all_at;

/// Command sent to the dedicated I/O worker threads.
enum IoCommand {
    /// Write a batch of (offset, chunk) pairs to a file.  The entries are
    /// already in ascending-offset order (drained from a BTreeMap).
    WriteBatch {
        file: Arc<File>,
        entries: Vec<(u64, Bytes)>,
        /// Whether to fsync after this batch. HDD double-buffer writes should
        /// sync for crash safety; SSD write-combining batches are large enough
        /// that per-batch fsync provides diminishing returns.
        sync: bool,
        done: oneshot::Sender<Result<(), DownloadError>>,
    },
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
                    // Normal processing loop
                    while let Some(cmd) = rx.blocking_recv() {
                        Self::process_command(cmd);
                    }
                    // All senders dropped — drain any commands that were queued
                    // before the final sender dropped.
                    while let Ok(cmd) = rx.try_recv() {
                        Self::process_command(cmd);
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
    fn process_command(cmd: IoCommand) {
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
                // entries are already sorted by offset (BTreeMap drain order).
                let result = (|| -> Result<(), DownloadError> {
                    if entries.is_empty() {
                        if sync {
                            file.sync_data().map_err(|e| {
                                DownloadError::Internal(format!("fsync failed: {e}"))
                            })?;
                        }
                        return Ok(());
                    }
                    // Merge adjacent contiguous entries to reduce syscall count.
                    let mut i = 0;
                    while i < entries.len() {
                        let (start_off, ref first_data) = entries[i];
                        let mut end_off = start_off + first_data.len() as u64;
                        let mut j = i + 1;
                        while j < entries.len() && entries[j].0 == end_off {
                            end_off += entries[j].1.len() as u64;
                            j += 1;
                        }
                        if j == i + 1 {
                            // Single entry, write directly (no allocation).
                            write_all_at(&file, &entries[i].1, entries[i].0)?;
                        } else {
                            // Merge entries i..j into one contiguous buffer.
                            let total_len = (end_off - start_off) as usize;
                            let mut merged = Vec::with_capacity(total_len);
                            for (_, chunk) in &entries[i..j] {
                                merged.extend_from_slice(chunk);
                            }
                            write_all_at(&file, &merged, start_off)?;
                        }
                        i = j;
                    }
                    if sync {
                        file.sync_data().map_err(|e| {
                            DownloadError::Internal(format!("fsync failed: {e}"))
                        })?;
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
        sync: bool,
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
