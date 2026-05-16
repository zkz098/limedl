mod download;

use anyhow::Context;
use tauri::Manager;

use download::{
    AppState, DownloadManager, SftpManager, TorrentManager, bt_runtime_status, download_cancel,
    download_list, download_open_in_explorer, download_pause, download_purge, download_remove,
    download_resume, download_start, download_status, init_logging, settings_fetch_tracker_list,
    settings_get, settings_save,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let run_result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            (|| -> anyhow::Result<()> {
                let state_dir = app
                    .path()
                    .app_local_data_dir()
                    .or_else(|_| app.path().app_data_dir())
                    .unwrap_or_else(|_| std::env::temp_dir().join("downloader"))
                    .join("downloads");

                std::fs::create_dir_all(&state_dir)
                    .with_context(|| format!("创建下载状态目录失败: {}", state_dir.display()))?;

                let download_manager = DownloadManager::new(state_dir.clone())
                    .with_context(|| format!("初始化下载管理器失败: {}", state_dir.display()))?;
                let settings = download_manager.initial_settings();
                init_logging(&settings.logging, &state_dir).context("初始化日志系统失败")?;
                let torrent_manager = tauri::async_runtime::block_on(TorrentManager::new(
                    state_dir.join("torrents"),
                    &settings,
                ))
                .context("初始化 BT 管理器失败")?;
                let sftp_manager =
                    SftpManager::new(state_dir.join("sftp")).context("初始化 SFTP 管理器失败")?;

                app.manage(AppState::new(
                    download_manager,
                    torrent_manager,
                    sftp_manager,
                ));

                Ok(())
            })()
            .map_err(|error| -> Box<dyn std::error::Error> {
                Box::new(std::io::Error::other(error.to_string()))
            })
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
            bt_runtime_status,
            settings_fetch_tracker_list,
            settings_get,
            settings_save
        ])
        .run(tauri::generate_context!());

    if let Err(error) = run_result {
        eprintln!("[downloader] tauri runtime failed: {error}");
    }
}
