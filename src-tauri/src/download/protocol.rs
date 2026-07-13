use async_trait::async_trait;

use super::error::Result;
use super::manager::DownloadManager;
use super::torrent::TorrentManager;
use super::types::{DownloadSnapshot, DownloadSummary, TaskId};

/// Common protocol interface for all download managers (HTTP, BitTorrent).
///
/// Callers pass the **external** (wire-format) `download_id` (e.g. `"http:…"`,
/// `"bt:…"`).  Each implementation handles its own ID prefix
/// stripping and output-snapshot prefixing so that callers are uniform.
#[async_trait]
pub(crate) trait DownloadProtocol: Send + Sync {
    async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn open_in_explorer(&self, download_id: &str) -> Result<()>;
    async fn status(&self, download_id: &str) -> Result<DownloadSnapshot>;
    /// Available for use; currently `download_list` calls managers directly to
    /// merge all three lists.
    #[allow(dead_code)]
    async fn list(&self) -> Result<Vec<DownloadSummary>>;
}

// ---------------------------------------------------------------------------
// HTTP adapter – strips the "http:" prefix before calling the inner manager
// and re-adds it on each returned snapshot / summary so that callers always
// work with the external wire format.
// ---------------------------------------------------------------------------

fn prefix_http_id(id: &str) -> String {
    TaskId::make_http(id.to_string())
}

fn prefix_http_snapshot(mut snapshot: DownloadSnapshot) -> DownloadSnapshot {
    snapshot.id = prefix_http_id(&snapshot.id);
    snapshot
}

#[allow(dead_code)]
fn prefix_http_summaries(summaries: Vec<DownloadSummary>) -> Vec<DownloadSummary> {
    summaries
        .into_iter()
        .map(|mut s| {
            s.id = prefix_http_id(&s.id);
            s
        })
        .collect()
}

fn strip_http_prefix(download_id: &str) -> &str {
    download_id.strip_prefix("http:").unwrap_or(download_id)
}

#[async_trait]
impl DownloadProtocol for DownloadManager {
    async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        DownloadManager::pause(self, strip_http_prefix(download_id))
            .await
            .map(prefix_http_snapshot)
    }

    async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        DownloadManager::resume(self, strip_http_prefix(download_id))
            .await
            .map(prefix_http_snapshot)
    }

    async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        DownloadManager::cancel(self, strip_http_prefix(download_id))
            .await
            .map(prefix_http_snapshot)
    }

    async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        DownloadManager::remove(self, strip_http_prefix(download_id))
            .await
            .map(prefix_http_snapshot)
    }

    async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        DownloadManager::purge(self, strip_http_prefix(download_id))
            .await
            .map(prefix_http_snapshot)
    }

    async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        DownloadManager::open_in_explorer(self, strip_http_prefix(download_id)).await
    }

    async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        DownloadManager::status(self, strip_http_prefix(download_id))
            .await
            .map(prefix_http_snapshot)
    }

    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        DownloadManager::list(self).await.map(prefix_http_summaries)
    }
}

// ---------------------------------------------------------------------------
// BitTorrent – delegates directly (the manager already handles "bt:" prefix).
// ---------------------------------------------------------------------------

#[async_trait]
impl DownloadProtocol for TorrentManager {
    async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        TorrentManager::pause(self, download_id).await
    }

    async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        TorrentManager::resume(self, download_id).await
    }

    async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        TorrentManager::cancel(self, download_id).await
    }

    async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        TorrentManager::remove(self, download_id).await
    }

    async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        TorrentManager::purge(self, download_id).await
    }

    async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        TorrentManager::open_in_explorer(self, download_id).await
    }

    async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        TorrentManager::status(self, download_id).await
    }

    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        TorrentManager::list(self).await
    }
}


