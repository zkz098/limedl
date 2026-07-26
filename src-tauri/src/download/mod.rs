// Tauri-specific modules — everything else lives in limedl-core
pub(crate) mod commands;
pub(crate) mod commands_cdn;

// Re-export everything from limedl_core so existing import paths in lib.rs still work.
pub use limedl_core::*;

// Re-export Tauri-specific commands
pub use commands::{
    bt_get_peers, bt_get_pieces, bt_get_trackers, bt_preview_torrent, bt_runtime_status,
    bt_set_speed_limit, detect_disk_type, download_cancel, download_list,
    download_open_dir, download_open_file, download_open_in_explorer, factory_reset, download_pause,
    download_purge, download_remove, download_resume, download_set_priority, download_start, download_status,
    get_bt_files, get_io_status, get_overclock_mode, settings_fetch_tracker_list, settings_get,
    settings_save, toggle_game_mode, toggle_overclock_mode, update_bt_files,
};
pub use commands_cdn::{
    cdn_apply, cdn_cancel, cdn_candidates, cdn_clear, cdn_detail, cdn_fetch_ranges, cdn_status,
    cdn_test,
};
