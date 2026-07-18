use async_trait::async_trait;

use super::error::Result;
use super::types::{DownloadSnapshot, DownloadSummary, StartDownloadRequest, TaskId};

/// Minimal common interface for all download protocol backends.
/// Each backend is responsible for its own ID prefix handling.
#[async_trait]
pub trait DownloadBackend: Send + Sync + 'static {
    /// Start a new download. Returns the prefixed task ID (e.g. "http:uuid", "bt:hexhash").
    async fn start(&self, request: StartDownloadRequest) -> Result<String>;

    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()>;
    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn list(&self) -> Result<Vec<DownloadSummary>>;
}
