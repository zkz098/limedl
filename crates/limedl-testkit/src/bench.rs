//! Benchmark harness shared by the crates that own a `benches/` target.
//!
//! The harness used to live at `crates/limedl-core/benches/common/mod.rs`,
//! because `limedl-core` was the only crate that had benchmarks. After the
//! crate split the benchmarks describe three different layers — AIMD
//! (`limedl-engine`), the token bucket (`limedl-http`), the buffer pool
//! (`limedl-disk`) and one end-to-end download (`limedl-core`) — and a
//! `mod common;` copy per crate would have been the fourth variant of the same
//! twelve lines. The test harness is what this crate is for.
//!
//! The two benches that only need a runtime (the rate limiter and the buffer
//! pool) deliberately do not use this: they build their own runtime, so
//! `limedl-http` and `limedl-disk` keep `criterion` as their only benchmark
//! dependency and never pull the test server in.

use std::sync::Arc;

use crate::TestServer;

/// A multi-threaded runtime plus a local [`TestServer`].
///
/// Dropping the harness shuts down the server and the runtime.
pub struct BenchHarness {
    /// Multi-threaded tokio runtime (4 worker threads).
    pub rt: tokio::runtime::Runtime,
    /// Local HTTP server serving deterministic random content.
    pub server: Arc<TestServer>,
}

impl BenchHarness {
    /// Create a harness whose server serves `file_size` bytes.
    ///
    /// The server is started on the harness runtime and kept behind an `Arc` so
    /// a benchmark loop can share it across iterations.
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
