//! Background and oversized direct flush task helpers for [`super::download_buffer::DownloadBuffer`].

use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use bytes::Bytes;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use super::download_buffer::{FlushObserver, FlushSink, PingPongCfg};
use super::pingpong_sync::set_bg_error;
use crate::error::DownloadError;
use crate::file_ops::write_all_at;

/// A single chunk larger than a half bypasses the buffer entirely.
pub(super) async fn write_oversized_direct(
    file: &Arc<File>,
    data: Bytes,
    offset: u64,
    observer: Option<&FlushObserver>,
) -> Result<(), DownloadError> {
    let len = data.len() as u64;
    let f = file.clone();
    tokio::task::spawn_blocking(move || write_all_at(&f, &data, offset))
        .await
        .map_err(|e| DownloadError::Internal(format!("background write failed: {e}")))??;
    notify_flushed(observer, &[(offset, len)]);
    Ok(())
}

/// The `(offset, len)` pairs of a flush batch, for the observer.
pub(super) fn entry_ranges(entries: &[(u64, Bytes)]) -> Vec<(u64, u64)> {
    entries
        .iter()
        .map(|(offset, data)| (*offset, data.len() as u64))
        .collect()
}

/// Report ranges as durable. Never called after a failed write.
pub(super) fn notify_flushed(observer: Option<&FlushObserver>, ranges: &[(u64, u64)]) {
    if let Some(observer) = observer {
        observer(ranges);
    }
}

/// Spawn the background flush for a half that was just rotated out.
///
/// With a sink, the batch goes to the device queue; without one (the
/// test/bench-only constructors) it is written inline on `spawn_blocking` with a
/// panic guard (unwind builds only — see the note in the body). Both paths set
/// `error_flag` on failure and notify waiters when done.
pub(super) fn spawn_background_flush(
    sink: Option<&FlushSink>,
    cfg: &PingPongCfg<'_>,
    file: &Arc<File>,
    error_flag: &Arc<AtomicBool>,
    notify: &Arc<Notify>,
    entries: Vec<(u64, Bytes)>,
) -> JoinHandle<()> {
    let bg_file = file.clone();
    let bg_error = Arc::clone(error_flag);
    let bg_notify = notify.clone();
    let bg_sync = cfg.bg_sync;
    let bg_fsync = cfg.bg_fsync;
    let label = cfg.label;
    let bg_observer = cfg.observer.cloned();
    let ranges = entry_ranges(&entries);

    if let Some(sink) = sink {
        let worker = sink.worker.clone();
        let bg_path = sink.path.clone();
        tokio::spawn(async move {
            match worker
                .write_batch(&bg_path, bg_file, entries, bg_sync)
                .await
            {
                Ok(()) => notify_flushed(bg_observer.as_ref(), &ranges),
                Err(e) => {
                    set_bg_error(&bg_error);
                    tracing::error!("background {label} buffer flush failed (device queue): {e}");
                }
            }
            bg_notify.notify_waiters();
        })
    } else {
        tokio::task::spawn_blocking(move || {
            // The panic guard is a development-build (and `test-utils`) safety
            // net only. The shipping profile builds with `panic = "abort"`
            // (workspace `Cargo.toml`), where `catch_unwind` cannot catch
            // anything: a panic here takes the process down, and the only
            // evidence is the native client's panic hook (`crash.rs`). Do not
            // treat this arm as production error recovery.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                write_all_entries(&bg_file, &entries)
            }));
            match result {
                Ok(Ok(())) => {
                    if bg_fsync && let Err(e) = bg_file.sync_data() {
                        set_bg_error(&bg_error);
                        tracing::error!("background {label} buffer flush fsync failed: {e}");
                    }
                    // `sync_data` failing still means the bytes are in the file
                    // (only the power-loss guarantee is missing), so the ranges
                    // count as durable either way.
                    notify_flushed(bg_observer.as_ref(), &ranges);
                }
                Ok(Err(e)) => {
                    set_bg_error(&bg_error);
                    tracing::error!("background {label} buffer flush failed: {e}");
                }
                Err(payload) => {
                    set_bg_error(&bg_error);
                    let msg = panic_payload_message(payload.as_ref());
                    tracing::error!("background {label} flush task panicked: {msg}");
                }
            }
            bg_notify.notify_waiters();
        })
    }
}

/// Write a batch of entries sequentially.
pub(super) fn write_all_entries(
    file: &File,
    entries: &[(u64, Bytes)],
) -> Result<(), DownloadError> {
    for (off, chunk) in entries {
        write_all_at(file, chunk, *off)?;
    }
    Ok(())
}

/// Message from a `catch_unwind` payload, for the flush-failure log.
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&'static str>().copied())
        .unwrap_or("<non-string panic payload>")
}
