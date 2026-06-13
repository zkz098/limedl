mod aimd;
mod aria2_rpc;
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
mod sftp;
mod torrent;
mod types;

pub use aria2_rpc::Aria2RpcServer;
pub use commands::{
    bt_runtime_status,
    download_cancel, download_list, download_open_in_explorer, download_pause, download_purge,
    download_remove, download_resume, download_start, download_status, settings_fetch_tracker_list,
    settings_get, settings_save,
};
pub use logging::init_logging;
pub use manager::{AppState, DownloadManager};
pub use sftp::SftpManager;
pub use torrent::TorrentManager;

pub(crate) fn lock_or_recover<'a, T>(mutex: &'a std::sync::Mutex<T>, name: &str) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("{name} lock poisoned, recovering with inner state");
            poisoned.into_inner()
        }
    }
}
