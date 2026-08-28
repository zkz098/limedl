slint::include_modules!();

mod bridge;

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Context;
use parking_lot::Mutex;
use slint::{ComponentHandle, SharedString, VecModel};
use tracing_subscriber::EnvFilter;

use limedl_core::bootstrap::bootstrap;
use limedl_core::event_bus::DownloadEvent;
use limedl_core::types::{
    DownloadProgress, DownloadState, DownloadSummary, StartDownloadRequest, TaskId,
};

use crate::bridge::{TaskStore, format_speed};

fn refresh_ui(ui: &MainWindow, store: &TaskStore) {
    let (all, downloading, paused, completed, failed) = store.counts();
    ui.set_count_all(SharedString::from(all.to_string()));
    ui.set_count_downloading(SharedString::from(downloading.to_string()));
    ui.set_count_paused(SharedString::from(paused.to_string()));
    ui.set_count_completed(SharedString::from(completed.to_string()));
    ui.set_count_failed(SharedString::from(failed.to_string()));

    let total_speed = store.total_speed();
    let speed_text = if total_speed > 0.0 {
        format_speed(Some(total_speed))
    } else {
        "0 B/s".to_string()
    };
    ui.set_global_speed_text(SharedString::from(speed_text));

    let items = store.filtered_items();
    ui.set_tasks(Rc::new(VecModel::from(items)).into());
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize console tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,limedl=debug")),
        )
        .init();

    tracing::info!("启动 limedl Native 桌面客户端 (Slint MVP)...");

    // Initialize core subsystems
    let state_dir = dirs_or_temp_dir().join("downloads");
    std::fs::create_dir_all(&state_dir)?;

    let core = bootstrap(state_dir.clone())
        .await
        .with_context(|| "初始化 limedl-core 失败")?;

    let default_download_dir = core
        .dispatcher
        .default_download_dir()
        .await
        .unwrap_or_else(|| state_dir.to_string_lossy().to_string());

    // Create Main Window
    let main_window = MainWindow::new()?;
    main_window.set_default_download_dir(SharedString::from(&default_download_dir));
    main_window.set_new_task_dir(SharedString::from(&default_download_dir));

    let store = Arc::new(Mutex::new(TaskStore::new()));

    // Load initial tasks from SQLite via Dispatcher
    if let Ok(initial_tasks) = core.dispatcher.list().await {
        let mut s = store.lock();
        s.replace_all(initial_tasks);
        refresh_ui(&main_window, &s);
    }

    // Subscribe to EventBus and stream updates into Slint event loop
    {
        let mut rx = core.event_bus.subscribe();
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();

        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                let store = store_clone.clone();
                let ui_weak = ui_weak.clone();

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let mut store = store.lock();
                        match event {
                            DownloadEvent::Updated { summary_json, .. } => {
                                if let Ok(summary) =
                                    serde_json::from_value::<DownloadSummary>(summary_json)
                                {
                                    store.insert_or_update(summary);
                                    refresh_ui(&ui, &store);
                                }
                            }
                            DownloadEvent::Progress { progress_json, .. } => {
                                if let Ok(progress) =
                                    serde_json::from_value::<DownloadProgress>(progress_json)
                                {
                                    store.update_progress(&progress);
                                    refresh_ui(&ui, &store);
                                }
                            }
                            DownloadEvent::FullState { downloads } => {
                                store.replace_all(downloads);
                                refresh_ui(&ui, &store);
                            }
                            _ => {}
                        }
                    }
                });
            }
        });
    }

    // UI Callbacks
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_select_category(move |cat| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.set_category(cat);
                ui.set_active_category(cat);
                refresh_ui(&ui, &store);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_open_new_task_dialog(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_new_task_dialog(true);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        main_window.on_close_new_task_dialog(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_new_task_dialog(false);
            }
        });
    }

    // Submit New Task
    {
        let dispatcher = core.dispatcher.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_submit_new_task(move |url, dir, filename| {
            let dispatcher = dispatcher.clone();
            let ui_weak = ui_weak.clone();
            let url_str = url.to_string();
            let dir_str = dir.to_string();
            let filename_opt = if filename.trim().is_empty() {
                None
            } else {
                Some(filename.trim().to_string())
            };

            tokio::spawn(async move {
                let req = StartDownloadRequest {
                    url: url_str,
                    destination_dir: dir_str,
                    file_name: filename_opt,
                    ..Default::default()
                };

                match dispatcher.start(req).await {
                    Ok(task_id) => {
                        tracing::info!("成功添加下载任务: {task_id}");
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_new_task_url(SharedString::default());
                                ui.set_new_task_filename(SharedString::default());
                                ui.set_show_new_task_dialog(false);
                            }
                        });
                    }
                    Err(err) => {
                        tracing::error!("添加下载任务失败: {err}");
                    }
                }
            });
        });
    }

    // Pause Task
    {
        let dispatcher = core.dispatcher.clone();
        main_window.on_pause_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str)
                    && let Err(err) = dispatcher.pause(&task_id).await
                {
                    tracing::error!("暂停任务失败: {err}");
                }
            });
        });
    }

    // Resume Task
    {
        let dispatcher = core.dispatcher.clone();
        main_window.on_resume_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str)
                    && let Err(err) = dispatcher.resume(&task_id).await
                {
                    tracing::error!("恢复任务失败: {err}");
                }
            });
        });
    }

    // Open in Explorer
    {
        let dispatcher = core.dispatcher.clone();
        main_window.on_open_task_explorer(move |id_str| {
            let dispatcher = dispatcher.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str)
                    && let Err(err) = dispatcher.open_in_explorer(&task_id).await
                {
                    tracing::error!("打开任务文件目录失败: {err}");
                }
            });
        });
    }

    // Remove Task
    {
        let dispatcher = core.dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_remove_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();
            let id_str = id_str.to_string();

            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str) {
                    if let Err(err) = dispatcher.remove(&task_id).await {
                        tracing::error!("删除任务失败: {err}");
                    } else {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut store = store_clone.lock();
                                store.remove(&id_str);
                                refresh_ui(&ui, &store);
                            }
                        });
                    }
                }
            });
        });
    }

    // Pause All
    {
        let dispatcher = core.dispatcher.clone();
        main_window.on_pause_all(move || {
            let dispatcher = dispatcher.clone();
            tokio::spawn(async move {
                if let Ok(list) = dispatcher.list().await {
                    for item in list {
                        if matches!(item.state, DownloadState::Downloading)
                            && let Ok(task_id) = TaskId::from_wire_string(&item.id)
                        {
                            let _ = dispatcher.pause(&task_id).await;
                        }
                    }
                }
            });
        });
    }

    // Resume All
    {
        let dispatcher = core.dispatcher.clone();
        main_window.on_resume_all(move || {
            let dispatcher = dispatcher.clone();
            tokio::spawn(async move {
                if let Ok(list) = dispatcher.list().await {
                    for item in list {
                        if matches!(item.state, DownloadState::Paused)
                            && let Ok(task_id) = TaskId::from_wire_string(&item.id)
                        {
                            let _ = dispatcher.resume(&task_id).await;
                        }
                    }
                }
            });
        });
    }

    tracing::info!("limedl Native UI 启动完毕，进入主事件循环");
    main_window.run()?;

    // Graceful shutdown
    tracing::info!("Native UI 正在退出，关闭核心引擎...");
    core.registry.shutdown_all().await;

    Ok(())
}

fn dirs_or_temp_dir() -> PathBuf {
    if let Some(dir) = dirs_local_data_dir() {
        dir.join("limedl")
    } else {
        std::env::temp_dir().join("limedl")
    }
}

fn dirs_local_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }
}
