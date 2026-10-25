//! # Crate layout after the split
//!
//! The foundation layer — domain types, settings, errors, events, the download
//! manifest and the `DownloadBackend` contract — moved to the `limedl-types`
//! crate. The module re-exports below keep every historical path
//! (`limedl_core::types::TaskId`, `crate::error::DownloadError`, …) resolving,
//! so the ~25 000 lines of tests in this crate and the two consumers did not
//! have to be touched.

pub mod backend_registry;
pub mod bootstrap;
#[cfg(feature = "bt")]
pub mod bt_backend;
pub mod cdn;
pub mod context;
pub mod dispatcher;
pub mod download;
pub mod http_executor;
pub mod logging;
pub mod manager;
pub mod manager_state;
pub mod metalink;
pub mod persistence;
pub mod retry;
pub mod scheduler;
pub mod speed_tracker;
pub mod task_lifecycle;

// Foundation modules, re-exported as modules (not as a glob of items): the
// re-export has to preserve the *module* path, because the engine and the test
// tree reach for `crate::<module>::Item` and not `crate::Item`.
pub use limedl_types::{
    checksum, error, event_bus, manifest, platform, protocol, settings, slot_guard, types,
    url_rewrite,
};
pub use limedl_types::{lock, now_ms};

// Disk/I/O substrate. Re-exported as modules for the same reason as the
// foundation layer: `crate::file_ops::…` / `crate::buffer_pool::…` keep
// resolving for the engine and its ~12k lines of tests. `buffer_pool` is now
// an ordinary public module instead of a `cfg(any(test, feature = "test-utils"))`
// gate — a crate cannot hide a module from its own dependents.
pub use limedl_disk::{buffer_pool, file_ops, io_scheduler, services};

// Persistence. Same module-level re-export as the two layers below it.
pub use limedl_store::{alt_svc, database};

// HTTP transport policy: the client factory, the shared header/url helpers and
// the rate limiter. Module re-exports, so `crate::http::identity_encoding` and
// `crate::http_client_factory::…` keep resolving.
pub use limedl_http::{http, http_client_factory, rate_limiter};

#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
pub mod aimd;

#[cfg(not(any(test, feature = "test-utils")))]
mod aimd;

#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
pub mod test_harness;

pub use backend_registry::BackendRegistry;
#[cfg(feature = "bt")]
pub use bt_backend::{IrontideBtBackend, LazyBtBackend};
pub use cdn::{CdnAccelerator, CdnService, CdnTestOutcome};
pub use checksum::{calculate_checksum, calculate_checksum_with_progress};
pub use context::SystemContext;
pub use dispatcher::Dispatcher;
pub use error::DownloadError;
pub use event_bus::{DownloadEvent, EventBus};
pub use io_scheduler::DiskDeviceManager;
pub use logging::init_logging;
pub use manager::DownloadManager;
pub use protocol::DownloadBackend;
pub use rate_limiter::RateLimiter;
pub use services::{ConcurrencyManager, DiskIoService, SettingsService};
pub use settings::{normalize_tracker_list_lossy, normalize_tracker_list_url};
pub use speed_tracker::SpeedTracker;
pub use types::CloseBehavior;

// Aria2 JSON-RPC compatibility layer (enabled by default in the desktop app,
// available via `--features aria2-rpc` for NAS/server builds).
#[cfg(feature = "aria2-rpc")]
pub mod aria2_rpc;
#[cfg(feature = "aria2-rpc")]
pub use aria2_rpc::{Aria2RpcServer, cleanup_old_aria2_temp_files};

#[cfg(test)]
mod tests;
