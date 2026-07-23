use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod backend_registry;
pub mod bootstrap;
pub mod bt_backend_own;
pub mod cdn;
pub mod checksum;
pub mod database;
pub mod dispatcher;
pub mod error;
pub mod event_bus;
pub mod file_ops;
pub mod http;
pub mod http_client_factory;
pub mod logging;
pub mod manager;
pub mod http_executor;
pub mod manifest;
pub mod migration;
pub mod mirror;
pub mod persistence;
pub mod protocol;
pub mod rate_limiter;
pub mod retry;
pub mod scheduler;
pub mod settings;
pub mod slot_guard;
pub mod task_lifecycle;
pub mod types;
pub mod ws_manifest;

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

pub use backend_registry::BackendRegistry;
pub use bt_backend_own::IrontideBtBackend;
pub use cdn::{CdnAccelerator, CdnService, CdnTestOutcome};
pub use checksum::calculate_checksum;
pub use dispatcher::Dispatcher;
pub use error::DownloadError;
pub use event_bus::{DownloadEvent, EventBus};
pub use logging::init_logging;
pub use manager::{AppState, DownloadManager};
pub use protocol::DownloadBackend;
pub use rate_limiter::RateLimiter;
pub use types::CloseBehavior;
pub use settings::{normalize_tracker_list_lossy, normalize_tracker_list_url};

// Aria2 JSON-RPC compatibility layer (enabled by default in the desktop app,
// available via `--features aria2-rpc` for NAS/server builds).
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
