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
// `bt_backend` is a crate now; re-exporting it under the old module name keeps
// `crate::bt_backend::{IrontideBtBackend, LazyBtBackend, info_hash_to_engine, …}`
// resolving for the dispatcher, the Aria2 layer and the test tree.
#[cfg(feature = "bt")]
pub use limedl_bt as bt_backend;
pub mod context;
pub mod dispatcher;
pub mod logging;
pub mod manager;

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

// The engine: chunk executor, scheduler, CDN, mirror racing, manifest state
// machine. Module re-exports again, so `crate::http_executor::…` and
// `crate::scheduler::…` keep resolving for the facade and the test tree.
pub use limedl_engine::{
    aimd, cdn, download, http_executor, manager_state, metalink, persistence, retry, scheduler,
    speed_tracker, task_lifecycle,
};

/// Fixtures that need the manager facade (see the module docs).
#[cfg(any(test, feature = "test-utils"))]
pub mod testing;

// The test harness is a crate now (`limedl-testkit`); re-exported under the old
// module name so `crate::test_harness::TestServer` keeps resolving in the ~12k
// lines of `src/tests/`. Optional in every sense: production builds do not get
// it, and it is the only thing that would pull `axum` into this graph.
#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
pub use limedl_testkit as test_harness;

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

// The Aria2 JSON-RPC layer is `limedl-rpc` now, a crate *above* this one: it
// speaks to the engine through `Dispatcher`/`SystemContext`, so the dependency
// runs one way and `axum`/`tower-http`/`argon2` are no longer in this graph.

#[cfg(test)]
mod tests;
