use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
pub mod aimd;

#[cfg(not(any(test, feature = "test-utils")))]
mod aimd;
pub(crate) mod event_bus;
mod aria2_rpc;
mod bt_backend_own;
#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
pub mod buffer_pool;

#[cfg(not(any(test, feature = "test-utils")))]
mod buffer_pool;
mod cdn;
pub(crate) mod checksum;
mod commands;
pub(crate) mod database;
mod error;
pub(crate) mod file_ops;
mod http;
pub(crate) mod http_client_factory;
mod logging;
mod manager;
mod manifest;
mod mirror;
pub(crate) mod migration;
pub(crate) mod persistence;
pub(crate) mod protocol;
pub(crate) mod backend_registry;
mod rate_limiter;
pub(crate) mod retry;
pub(crate) mod scheduler;
pub(crate) mod settings;
pub(crate) mod types;

#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
pub mod test_harness;

pub(crate) use checksum::calculate_checksum;

pub use aria2_rpc::Aria2RpcServer;
pub(crate) use aria2_rpc::cleanup_old_aria2_temp_files;
pub(crate) use bt_backend_own::IrontideBtBackend;
pub(crate) use cdn::CdnAccelerator;
pub use cdn::commands::{
    cdn_apply, cdn_cancel, cdn_candidates, cdn_clear, cdn_detail, cdn_fetch_ranges, cdn_status,
    cdn_test,
};
pub use commands::{
    bt_get_peers, bt_get_pieces, bt_get_trackers, bt_preview_torrent, bt_runtime_status,
    bt_set_speed_limit, download_cancel, download_list, download_open_in_explorer, download_pause,
    download_purge, download_remove, download_resume, download_start, download_status,
    get_bt_files, get_io_status, get_overclock_mode, settings_fetch_tracker_list, settings_get,
    settings_save, toggle_game_mode, toggle_overclock_mode, update_bt_files,
};
pub use logging::init_logging;
pub use manager::{AppState, DownloadManager};
pub use rate_limiter::RateLimiter;

pub(crate) fn lock<T>(
    mutex: &parking_lot::Mutex<T>,
) -> parking_lot::MutexGuard<'_, T> {
    mutex.lock()
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}
