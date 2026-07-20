use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod event_bus;
pub mod error;
pub mod types;
pub mod protocol;
pub mod backend_registry;
pub mod database;
pub mod file_ops;
pub mod http_client_factory;
pub mod http;
pub mod manifest;
pub mod manager;
pub mod migration;
pub mod mirror;
pub mod persistence;
pub mod retry;
pub mod scheduler;
pub mod settings;
pub mod logging;
pub mod cdn;
pub mod bt_backend_own;
pub mod checksum;
pub mod rate_limiter;
pub mod bootstrap;
pub mod slot_guard;

#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
pub mod aimd;

#[cfg(not(any(test, feature = "test-utils")))]
mod aimd;

#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
pub mod buffer_pool;

#[cfg(not(any(test, feature = "test-utils")))]
mod buffer_pool;

#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
pub mod test_harness;

pub use bt_backend_own::IrontideBtBackend;
pub use cdn::CdnAccelerator;
pub use manager::{AppState, DownloadManager};
pub use rate_limiter::RateLimiter;
pub use event_bus::{DownloadEvent, EventBus};
pub use protocol::DownloadBackend;
pub use backend_registry::BackendRegistry;
pub use error::DownloadError;
pub use checksum::calculate_checksum;
pub use settings::{
    normalize_tracker_list_lossy,
    normalize_tracker_list_url,
};
pub use logging::init_logging;

// The Aria2 JSON-RPC server is an experimental compatibility layer.
// It is not yet considered stable for production use.
// Enable with `--features aria2-rpc` when building.
#[cfg(feature = "aria2-rpc")]
pub mod aria2_rpc;
#[cfg(feature = "aria2-rpc")]
pub use aria2_rpc::{Aria2RpcServer, cleanup_old_aria2_temp_files};

pub fn lock<T>(mutex: &parking_lot::Mutex<T>) -> parking_lot::MutexGuard<'_, T> {
    mutex.lock()
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}

#[cfg(test)]
mod tests;