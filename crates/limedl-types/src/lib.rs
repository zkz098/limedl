//! Foundation layer for the limedl engine.
//!
//! Everything the other crates agree on lives here: the domain types, the
//! settings tree, the error type, the event bus, the download manifest, the
//! `DownloadBackend` contract and the small helpers (`now_ms`, `lock`) that the
//! rest of the engine shares.
//!
//! This crate must never depend on another `limedl-*` crate — it is the bottom
//! of the dependency graph, and every other layer is free to depend on *it*.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod checksum;
pub mod error;
pub mod event_bus;
pub mod manifest;
pub mod platform;
pub mod protocol;
pub mod settings;
pub mod slot_guard;
pub mod types;
pub mod url_rewrite;

pub use checksum::{calculate_checksum, calculate_checksum_with_progress};
pub use error::DownloadError;
pub use event_bus::{DownloadEvent, EventBus};
pub use protocol::DownloadBackend;
pub use settings::{normalize_tracker_list_lossy, normalize_tracker_list_url};
pub use types::CloseBehavior;

pub fn lock<T>(mutex: &parking_lot::Mutex<T>) -> parking_lot::MutexGuard<'_, T> {
    mutex.lock()
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}
