use async_trait::async_trait;

use super::error::Result;
use super::types::{AppSettings, DownloadSnapshot, DownloadSummary, Priority, StartDownloadRequest, TaskId};

/// Minimal common interface for all download protocol backends.
/// Each backend is responsible for its own ID prefix handling.
#[async_trait]
pub trait DownloadBackend: Send + Sync + 'static {
    /// Start a new download. Returns the strongly-typed task ID.
    async fn start(&self, request: StartDownloadRequest) -> Result<TaskId>;

    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()>;

    /// Open the downloaded file using the OS default handler.
    /// Default implementation falls back to open_in_explorer.
    async fn open_file(&self, task_id: &TaskId) -> Result<()> {
        self.open_in_explorer(task_id).await
    }

    /// Open the download directory in file explorer.
    /// Default implementation falls back to open_in_explorer.
    async fn open_dir(&self, task_id: &TaskId) -> Result<()> {
        self.open_in_explorer(task_id).await
    }

    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn list(&self) -> Result<Vec<DownloadSummary>>;

    /// Broadcast settings update to this backend.
    async fn update_settings(&self, settings: &AppSettings) -> Result<()>;

    /// Set the priority of a download.
    /// Default implementation returns an error — override for backends that support it.
    async fn set_priority(&self, _task_id: &TaskId, _priority: Priority) -> Result<()> {
        Err(super::error::DownloadError::Internal(
            "set_priority not supported for this backend".into(),
        ))
    }

    /// Gracefully shut down this backend.
    async fn shutdown(&self);
}
