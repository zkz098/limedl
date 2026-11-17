//! `redb` table definitions and state classification helpers.
//!
//! Storage is split across three physically isolated `redb` databases:
//! - Primary store (`downloads.redb`):
//!   - `active_downloads`: non-terminal `Manifest` headers (`chunks` empty)
//!   - `terminal_downloads`: `completed`/`failed`/`canceled` `Manifest` headers
//!   - `chunk_progress`: compact binary `Vec<ChunkManifest>` keyed by `download_id`
//! - Ephemeral BT index cache (`bt_tasks.redb`):
//!   - `bt_tasks`: `id -> DownloadSummary` JSON
//! - Ephemeral HTTP/3 Alt-Svc cache (`alt_svc.redb`):
//!   - `alt_svc_cache`: `(origin_host, origin_port, protocol) -> AltSvcRecord` JSON

use redb::TableDefinition;

use crate::types::DownloadState;

/// Active (non-terminal) download manifests, keyed by `download_id`.
pub const ACTIVE_DOWNLOADS: TableDefinition<&str, &[u8]> = TableDefinition::new("active_downloads");

/// Terminal (`completed` / `failed` / `canceled`) download manifests, keyed by `download_id`.
pub const TERMINAL_DOWNLOADS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("terminal_downloads");

/// Compact binary chunk progress blobs, keyed by `download_id`.
pub const CHUNK_PROGRESS: TableDefinition<&str, &[u8]> = TableDefinition::new("chunk_progress");

/// Ephemeral BT task summary cache, keyed by `task_id`.
pub const BT_TASKS: TableDefinition<&str, &[u8]> = TableDefinition::new("bt_tasks");

/// Ephemeral HTTP/3 Alt-Svc cache, keyed by `(origin_host, origin_port, protocol)`.
pub const ALT_SVC_CACHE: TableDefinition<(&str, u16, &str), &[u8]> =
    TableDefinition::new("alt_svc_cache");

/// Return `true` when `state` is a terminal download state.
pub fn is_terminal_state(state: &DownloadState) -> bool {
    matches!(
        state,
        DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
    )
}

/// Return `true` when the serialized state name is terminal.
pub fn is_terminal_state_text(state: &str) -> bool {
    matches!(state, "completed" | "failed" | "canceled")
}
