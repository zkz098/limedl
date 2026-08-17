//! Shared benchmark harness for limedl benchmarks.
//!
//! Provides a [`BenchHarness`] struct that owns a multi-threaded tokio runtime
//! and a local HTTP [`TestServer`] serving deterministic random content.
//!
//! # Usage
//!
//! ```ignore
//! use limedl_core::test_harness::TestServer;
//! use common::BenchHarness;
//!
//! let harness = BenchHarness::new(1024 * 1024);
//! let url = harness.server.file_url();
//! ```

use limedl_core::test_harness::TestServer;
use std::sync::Arc;

/// Benchmark harness owning a tokio runtime and a local test server.
///
/// Dropping the harness shuts down the server and runtime.
#[allow(dead_code)]
pub struct BenchHarness {
    /// Multi-threaded tokio runtime (4 worker threads, all features enabled).
    pub rt: tokio::runtime::Runtime,
    /// Local HTTP test server serving deterministic random content.
    #[allow(dead_code)]
    pub server: Arc<TestServer>,
}

impl BenchHarness {
    /// Create a new harness with a test server serving `file_size` bytes.
    ///
    /// The server is started asynchronously on the runtime and wrapped in
    /// an `Arc` so it can be shared across benchmark iterations.
    #[allow(dead_code)]
    pub fn new(file_size: u64) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime for benchmark");
        let server = Arc::new(rt.block_on(TestServer::new(file_size)));
        Self { rt, server }
    }
}
