//! Persistence for the limedl engine.
//!
//! `database` is the `redb`-backed task/chunk store (with active vs terminal
//! task separation and compact binary chunk progress) plus isolated ephemeral
//! `redb` caches for `bt_tasks` and `alt_svc` (`Alt-Svc` HTTP/3 endpoints).
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
