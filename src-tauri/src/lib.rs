mod download;

use tauri::Manager;

use download::{
    AppState, DownloadManager, TorrentManager, download_cancel, download_list,
    download_open_in_explorer, download_pause, download_purge, download_remove, download_resume,
    download_start, download_status, settings_get, settings_save,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state_dir = app
                .path()
                .app_local_data_dir()
                .or_else(|_| app.path().app_data_dir())
                .unwrap_or_else(|_| std::env::temp_dir().join("downloader"))
                .join("downloads");

            std::fs::create_dir_all(&state_dir)
                .map_err(|error| -> Box<dyn std::error::Error> { Box::new(error) })?;

            let download_manager = DownloadManager::new(state_dir.clone())?;
            let settings = download_manager.initial_settings();
            let torrent_manager = tauri::async_runtime::block_on(TorrentManager::new(
                state_dir.join("torrents"),
                &settings,
            ))?;

            app.manage(AppState::new(download_manager, torrent_manager));

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            download_start,
            download_pause,
            download_resume,
            download_cancel,
            download_remove,
            download_purge,
            download_open_in_explorer,
            download_status,
            download_list,
            settings_get,
            settings_save
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
