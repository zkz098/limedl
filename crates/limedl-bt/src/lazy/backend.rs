use async_trait::async_trait;

use super::{LazyBtBackend, info_hash_from_engine, info_hash_to_engine};
use crate::error::{DownloadError, Result};
use crate::protocol::DownloadBackend;
use crate::types::{AppSettings, DownloadSnapshot, DownloadSummary, StartDownloadRequest, TaskId};

#[async_trait]
impl DownloadBackend for LazyBtBackend {
    async fn start(&self, request: StartDownloadRequest) -> Result<TaskId> {
        let info_hash = self.engine().await?.start(request).await?;
        Ok(TaskId::Bt(info_hash_from_engine(info_hash)))
    }

    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine()
            .await?
            .pause(info_hash_to_engine(&info_hash))
            .await
    }

    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine()
            .await?
            .resume(info_hash_to_engine(&info_hash))
            .await
    }

    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        let snapshot = self
            .engine()
            .await?
            .cancel(info_hash_to_engine(&info_hash))
            .await?;
        self.db
            .delete_bt_task_async(info_hash.to_hex())
            .await
            .unwrap_or_else(|e| tracing::warn!("failed to drop the BT index row: {e}"));
        Ok(snapshot)
    }

    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        let snapshot = self
            .engine()
            .await?
            .remove(info_hash_to_engine(&info_hash))
            .await?;
        self.db
            .delete_bt_task_async(info_hash.to_hex())
            .await
            .unwrap_or_else(|e| tracing::warn!("failed to drop the BT index row: {e}"));
        Ok(snapshot)
    }

    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        let snapshot = self
            .engine()
            .await?
            .purge(info_hash_to_engine(&info_hash))
            .await?;
        self.db
            .delete_bt_task_async(info_hash.to_hex())
            .await
            .unwrap_or_else(|e| tracing::warn!("failed to drop the BT index row: {e}"));
        Ok(snapshot)
    }

    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine()
            .await?
            .open_in_explorer(info_hash_to_engine(&info_hash))
            .await
    }

    async fn open_file(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine()
            .await?
            .open_file(info_hash_to_engine(&info_hash))
            .await
    }

    async fn open_dir(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine()
            .await?
            .open_dir(info_hash_to_engine(&info_hash))
            .await
    }

    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine()
            .await?
            .status(info_hash_to_engine(&info_hash))
            .await
    }

    /// List running torrents from the engine, or the persisted index while the
    /// engine is unloaded instead of blocking startup on the session.
    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        match self.engine_if_ready() {
            Some(engine) => engine.list().await,
            None => self.index_summaries().await,
        }
    }

    async fn update_settings(&self, settings: &AppSettings) -> Result<()> {
        self.apply_settings(settings);
        Ok(())
    }

    async fn shutdown(&self) {
        LazyBtBackend::shutdown(self).await;
    }
}
