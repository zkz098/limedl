use parking_lot::Mutex;

use limedl_core::dispatcher::Dispatcher;
use limedl_core::types::{
    AppSettings, DoubleClickOnCompleted, DoubleClickOnUncompleted, DownloadState, TaskId,
};

use crate::bridge::TaskStore;

pub fn open_path_in_explorer(path: &str) -> std::io::Result<()> {
    limedl_core::platform::open_in_file_manager(std::path::Path::new(path))
}

/// Open a task's downloaded file with the OS default handler (via backend).
pub async fn open_task_file(dispatcher: &Dispatcher, task_id: &TaskId) -> anyhow::Result<()> {
    let backend = dispatcher.registry().dispatch(task_id)?;
    Ok(backend.open_file(task_id).await?)
}

/// Open a task's download directory in the file explorer (via backend).
pub async fn open_task_dir(dispatcher: &Dispatcher, task_id: &TaskId) -> anyhow::Result<()> {
    let backend = dispatcher.registry().dispatch(task_id)?;
    Ok(backend.open_dir(task_id).await?)
}

/// Execute the configured double-click behavior for a task. Mirrors the web
/// client's semantics: completed tasks open the file / explorer / download
/// dir; uncompleted tasks toggle pause/resume (pause for queued/downloading/
/// retrying/verifying, resume for paused/failed).
pub async fn handle_task_double_click(
    dispatcher: &Dispatcher,
    store: &Mutex<TaskStore>,
    current_settings: &Mutex<AppSettings>,
    id_str: &str,
) -> anyhow::Result<()> {
    // Snapshot the task state + configured behavior.
    let (state, double_click) = {
        let store = store.lock();
        let Some(summary) = store.get_summary(id_str) else {
            return Ok(());
        };
        let settings = current_settings.lock();
        (summary.state, settings.double_click.clone())
    };
    let task_id = TaskId::from_wire_string(id_str)?;

    if matches!(state, DownloadState::Completed) {
        match double_click.on_completed {
            DoubleClickOnCompleted::None => {}
            DoubleClickOnCompleted::OpenFile => {
                open_task_file(dispatcher, &task_id).await?;
            }
            DoubleClickOnCompleted::OpenInExplorer => {
                dispatcher.open_in_explorer(&task_id).await?;
            }
            DoubleClickOnCompleted::OpenDownloadDir => {
                open_task_dir(dispatcher, &task_id).await?;
            }
        }
    } else {
        match double_click.on_uncompleted {
            DoubleClickOnUncompleted::None => {}
            DoubleClickOnUncompleted::TogglePauseResume => {
                if matches!(
                    state,
                    DownloadState::Queued
                        | DownloadState::Downloading
                        | DownloadState::Retrying
                        | DownloadState::Verifying
                ) {
                    dispatcher.pause(&task_id).await?;
                } else if matches!(state, DownloadState::Paused | DownloadState::Failed) {
                    dispatcher.resume(&task_id).await?;
                }
            }
        }
    }
    Ok(())
}

/// Opens a URL in the system default browser.
pub fn open_url_in_browser(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        // explorer.exe hands URLs to the default browser without cmd quoting quirks.
        std::process::Command::new("explorer").arg(url).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}
