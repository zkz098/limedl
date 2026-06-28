mod aimd;
mod aria2_rpc;
mod cdn;
mod commands;
pub(crate) mod database;
mod error;
mod file_alloc;
mod http;
mod logging;
mod manager;
mod manifest;
mod metalink;
pub(crate) mod migration;
mod rate_limiter;
mod sftp;
mod torrent;
mod types;

pub use aria2_rpc::Aria2RpcServer;
pub(crate) use cdn::CdnAccelerator;
pub use cdn::commands::{
    cdn_apply, cdn_cancel, cdn_candidates, cdn_clear, cdn_detail, cdn_fetch_ranges, cdn_status,
    cdn_test,
};
pub use commands::{
    bt_get_peers, bt_get_pieces, bt_get_trackers, bt_preview_torrent, bt_runtime_status,
    bt_set_speed_limit, download_cancel, download_list, download_open_in_explorer, download_pause,
    download_purge, download_remove, download_resume, download_start, download_status,
    settings_fetch_tracker_list, settings_get, settings_save,
};
pub use logging::init_logging;
pub use manager::{AppState, DownloadManager};
pub use rate_limiter::RateLimiter;
pub use sftp::SftpManager;
pub use torrent::TorrentManager;

pub(crate) fn lock_or_recover<'a, T>(
    mutex: &'a std::sync::Mutex<T>,
    name: &str,
) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("{name} lock poisoned, recovering with inner state");
            poisoned.into_inner()
        }
    }
}
