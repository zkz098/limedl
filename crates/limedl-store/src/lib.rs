//! Persistence for the limedl engine.
//!
//! `database` is the SQLite store (tasks, chunks, manifest snapshots and the
//! schema migrations that carry them forward) and `alt_svc` is the HTTP
//! `Alt-Svc` cache that keeps the CDN accelerator from re-discovering the same
//! endpoint on every start.
//!
//! `limedl-types` is re-exported at this crate root so the moved modules kept
//! their `crate::types::…` / `crate::error::…` spellings, exactly like
//! `limedl-disk` does.
//!
//! Note what is *not* here: `persistence` (the free functions the HTTP executor
//! and the scheduler call on their 300 ms cycle) stays in `limedl-core`. It
//! names `download::ManagedDownload`, so moving it down would invert the
//! dependency direction that `LAYERS` in
//! `crates/limedl-core/src/tests/module_boundaries_tests.rs` enforces.

pub use limedl_types::{error, manifest, now_ms, types};

pub mod alt_svc;
pub mod database;
