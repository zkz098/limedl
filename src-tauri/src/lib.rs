mod download;

use tauri::Manager;

use download::{
    download_cancel, download_list, download_pause, download_resume, download_start,
    download_status, AppState, DownloadManager,
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

            app.manage(AppState::new(DownloadManager::new(state_dir)?));

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            download_start,
            download_pause,
            download_resume,
            download_cancel,
            download_status,
            download_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
