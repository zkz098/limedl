//! `redb`-based persistence for download manifests, compact chunk progress,
//! and isolated ephemeral caches (`bt_tasks`, `alt_svc_cache`).

pub mod bt_task_repo;
pub mod chunk_repo;
pub mod connection;
pub mod manifest_repo;
pub mod schema;

#[cfg(test)]
mod tests;

pub use chunk_repo::ProgressBatchEntry;
pub use connection::Database;
pub use manifest_repo::download_state_to_text;
