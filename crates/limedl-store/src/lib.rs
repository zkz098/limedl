//! Persistence for the limedl engine.
//!
//! `database` is the SQLite store (tasks, chunks, manifest snapshots and the
//! schema migrations that carry them forward) and `alt_svc` is the HTTP
//! `Alt-Svc` cache that keeps the CDN accelerator from re-discovering the same
//! endpoint on every start.
//!
//! `limedl-types` is imported at this crate root so the moved modules kept
//! their `crate::types::…` / `crate::error::…` spellings, exactly like
//! `limedl-disk` does. The import is private — `limedl_store::types` is not a
//! path.
//!
//! Note what is *not* here: `persistence` (the free functions the HTTP executor
//! and the scheduler call on their 300 ms cycle) lives in `limedl-engine`. It
//! names `download::ManagedDownload`, which sits above this crate, so moving it
//! down would invert the dependency direction that `Cargo.toml` enforces.

use limedl_types::{error, manifest, now_ms, types};

pub mod alt_svc;
pub mod database;
