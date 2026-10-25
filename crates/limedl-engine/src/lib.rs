//! The HTTP download engine.
//!
//! Everything between "the user asked for a URL" and "the bytes are on disk":
//! the chunk executor and its workers, the scheduler that decides how many
//! downloads and threads run, the CDN accelerator and mirror racing, the
//! manifest state machine that makes a download resumable, and the persistence
//! cycle that keeps the database in step with it.
//!
//! It sits above `limedl-http`, `limedl-disk` and `limedl-store`, and below
//! `limedl-core`, which adds the router (`Dispatcher`), the manager facade and
//! the boot sequence.
//!
//! The layers below are imported at this crate root so the moved modules kept
//! their existing `crate::types::…` / `crate::database::…` spellings. Those are
//! private bindings, not re-exports: `limedl_engine::types` does not exist.
//! Re-exporting them made every module reachable under two names (`engine`,
//! `types`) and quietly burdened `limedl-engine` with the duty of re-exporting
//! whatever a downstream crate wanted.

use limedl_disk::{buffer_pool, file_ops, services};
use limedl_http::{http, http_client_factory, rate_limiter};
use limedl_store::{alt_svc, database};
use limedl_types::{
    calculate_checksum_with_progress, checksum, error, event_bus, lock, manifest, now_ms, types,
};

pub mod aimd;
pub mod cdn;
pub mod download;
pub mod http_executor;
pub mod manager_state;
pub mod metalink;
pub mod persistence;
pub mod retry;
pub mod scheduler;
pub mod speed_tracker;
pub mod task_lifecycle;
