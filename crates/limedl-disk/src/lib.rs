//! Disk and I/O substrate for the limedl engine.
//!
//! Four modules, one layer: `file_ops` (preallocation, the coalescing write
//! sink, the deterministic fault hooks, media probing), `buffer_pool` (the
//! segmented buffer that feeds the chunk workers), `io_scheduler` (HDD vs SSD
//! queueing and topology) and `services` (the shared `DiskIoService`,
//! `ConcurrencyManager` and `SettingsService` handles).
//!
//! `limedl-types` is imported at this crate root so the moved modules kept
//! their existing `crate::types::…` / `crate::error::…` spellings. The import is
//! private: `limedl_disk::types` is not a thing, and adding it would have made
//! this crate a second path to the foundation layer.
//!
//! Nothing here may name `limedl-http`, `limedl-engine`, `limedl-bt` or
//! `limedl-core`: this is the bottom half of the engine and the direction is
//! enforced by `Cargo.toml`.

use limedl_types::{error, settings, slot_guard, types};

pub mod buffer_pool;
pub mod file_ops;
pub mod io_scheduler;
pub mod services;
