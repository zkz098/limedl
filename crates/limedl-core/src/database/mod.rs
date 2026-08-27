//! SQLite-based persistence for download manifests and chunk progress.
//!
//! Maintains two tables:
//! - `downloads`: core metadata (URLs, paths, state, checksums, priorities)
//! - `chunks`: per-chunk byte ranges, progress, and worker claim tracking
//!
//! Uses a dual-connection architecture (dedicated write connection with WAL,
//! read-only connection for queries).

pub mod chunk_repo;
pub mod connection;
pub mod manifest_repo;
pub mod schema;

#[cfg(test)]
mod tests;

pub use chunk_repo::ProgressBatchEntry;
pub use connection::Database;
pub use manifest_repo::download_state_to_text;
