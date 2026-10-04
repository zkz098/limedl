---
type: testing
title: Testing Strategy
description: How limedl's Rust engine is tested — the test layout conventions, nextest process isolation, the mock-server integration corpus, byte-level corruption oracles and adversarial servers, BT and Aria2 harnesses, and the coverage gate.
tags: [testing, nextest, integration-tests, corruption, coverage]
sources:
  - id: openwiki-source-164e2da859b5277df81c7d94
    resource: repo://.github/workflows/ci.yml
  - id: openwiki-source-c8ca1e3187dacc725ff333e9
    resource: repo://crates/limedl-core/src/aria2_rpc/e2e_tests.rs
  - id: openwiki-source-1c0eea0988a8be3129ce2126
    resource: repo://crates/limedl-core/src/bt_backend/tests/alerts.rs
  - id: openwiki-source-8a802dd79d30f7920922a1e7
    resource: repo://crates/limedl-core/src/bt_backend/tests/mod.rs
  - id: openwiki-source-e2995ec6bf7ff16128c760b6
    resource: repo://crates/limedl-core/src/test_harness/mod.rs
  - id: openwiki-source-37ca19c9c77c5d1241db1782
    resource: repo://crates/limedl-core/src/test_harness/tests.rs
  - id: openwiki-source-cb1832cd66a43fcb45eccffd
    resource: repo://crates/limedl-core/src/tests/adversarial_interception_tests.rs
  - id: openwiki-source-3b86741d4858b1e063b6d6f5
    resource: repo://crates/limedl-core/src/tests/buffer_integrity_tests.rs
  - id: openwiki-source-836224c8ad94a94584f3cb8b
    resource: repo://crates/limedl-core/src/tests/corruption_oracle_tests.rs
  - id: openwiki-source-c4e790430c7c56f8a34b5dc0
    resource: repo://crates/limedl-core/src/tests/dispatcher_tests.rs
  - id: openwiki-source-b6ac5cc91bdd258dd969abbc
    resource: repo://crates/limedl-core/src/tests/mod.rs
  - id: openwiki-source-3742feb3f92f40e410145856
    resource: repo://crates/limedl-core/src/tests/resume_corruption_tests.rs
  - id: openwiki-source-1a1d4b50d244dfdbcf190f3a
    resource: repo://crates/limedl-core/tests/logging_reload_repro.rs
generated: { by: "pi", at: "2026-10-04T03:21:09.297Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T03:21:09.297Z
---

# Testing Strategy

limedl's engine tests live beside the code (unit tests) or in
`crates/limedl-core/src/tests/` (cross-module integration), with two dedicated
integration binaries under `crates/limedl-core/tests/`. The UI layer has its own
<!-- openwiki: broken internal link [/openwiki/testing/slint-ui-testing.md] link "/openwiki/testing/slint-ui-testing.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
in-process suite, documented on [Slint UI Testing](/openwiki/testing/slint-ui-testing.md).

## Test layout conventions

1. Small `#[cfg(test)] mod tests` blocks stay inline in the production file
   (under ~150 lines).
2. A larger block moves to a sibling `tests.rs` (e.g. `settings/tests.rs`,
   `http_executor/tests.rs`, `scheduler/tests.rs`), leaving only
   `#[cfg(test)] mod tests;` in the production file; tests still reach private
   items via `use super::*;`.
3. Cross-module E2E tests that need a mock server or several subsystems go in
   `crates/limedl-core/src/tests/`, declared by `lib.rs`'s `mod tests`.
4. A test file over ~800 lines or ~30 tests is split into a same-named directory
   (`mod.rs` holds imports and shared fixtures, `<scenario>.rs` holds tests), as
   already done for `manager_tests/`, `http_executor_tests/`, `scheduler_tests/`,
   `bt_backend/tests/`, `buffer_pool/tests/` and `database/tests/`.
5. Tests that take a process-global slot (notably `tracing_subscriber::fmt().init()`)
   get their **own integration binary**, because libtest runs sibling tests in
   one process and a second `init()` panics. That is why
   `tests/logging_reload_repro.rs` (clean process) and
   `tests/logging_preinstalled_subscriber.rs` (pre-installed subscriber) are
   separate files.

Evidence: `repo://crates/limedl-core/src/tests/mod.rs#L1-L27`,
`repo://crates/limedl-core/tests/logging_reload_repro.rs#L1-L12`.

## nextest process isolation

Tests run under `cargo nextest`, which executes each test in its own process.
That parallelizes the suite and removes the cross-test global-state interference
a single libtest process per binary exposes — shared temp dirs, env vars,
`LazyLock`. `cargo nextest run` does **not** run doctests; the workspace has none,
and adding one requires a `cargo test --doc` step in CI and the gate.

Evidence: `repo://.github/workflows/ci.yml#L84-L93`.

## The corruption oracle

The strongest tests in the repo target the user report "download finishes but the
SHA does not match". `corruption_oracle_tests.rs` exercises the **full** engine
path (probe → chunked range download → buffered writes → finalize), then re-reads
the assembled file from disk and hashes it **independently** with SHA-256,
comparing against the server's known hash. This is deliberately stronger than
asserting `state == Completed`, which cannot see a corrupt file. The server
content is deterministic (seeded PRNG, seed 42), so a failure in any iteration is
a real engine nondeterminism — a bug report, not a flaky test — and the corrupt
temp file should be preserved for diagnosis.

Evidence: `repo://crates/limedl-core/src/tests/corruption_oracle_tests.rs#L1-L30`.

### Adversarial servers

`adversarial_interception_tests.rs` uses two hostile hosts that return wrong
bytes for valid ranges: `range-shifted` (content shifted relative to the
advertised `Content-Range`) and `range-bitflip` (first byte of each range
flipped). Both keep the byte counts identical, so every chunk is accepted and the
download reaches finalize — only the checksum comparison can expose the problem.
With an explicit `expected_checksum`, the download must end in `Failed`.

Evidence: `repo://crates/limedl-core/src/tests/adversarial_interception_tests.rs#L1-L16`.

### Resume and buffer integrity

`resume_corruption_tests.rs` pauses at staggered points, resumes, finishes, and
re-reads the file to require SHA-256 equality. Resume is a prime suspect because
the incremental hasher resets on pause and incomplete-chunk offsets are
re-derived from the manifest; the tests avoid racing a fast localhost download by
throttling the origin instead of the pausing loop. The single-stream cases use
the bandwidth endpoint with `pause_at_fraction`. The multi-threaded case pauses
on the first progress (`pause_on_any_progress`) through
`TestServer::file_url_range_bandwidth`, a range-capable endpoint that throttles
each chunk connection so the transfer is still in flight when the pause lands —
without it a 17 MiB localhost download could finish and enter `Verifying`
between the progress poll and the pause call. The harness endpoints themselves
are covered by unit tests in `test_harness/tests.rs`
(`range_bandwidth_endpoint_serves_throttled_ranges`,
`range_bandwidth_endpoint_serves_full_file_without_range`).

`buffer_integrity_tests.rs` injects deterministic I/O failures into the
write-combining buffers through `buffer_pool::fault` (compiled only under
`test-utils`). It proves two things: a background flush failure sets the buffer's
error flag (`has_degraded()`, `flush_all` errors) without corrupting
already-written bytes, and a real download using the buffer ends in `Failed`
rather than a silent `Completed`.

Evidence: `repo://crates/limedl-core/src/tests/resume_corruption_tests.rs#L99-L118`,
`repo://crates/limedl-core/src/tests/resume_corruption_tests.rs#L172-L205`,
`repo://crates/limedl-core/src/test_harness/mod.rs#L547-L617`,
`repo://crates/limedl-core/src/test_harness/tests.rs#L63-L106`,
`repo://crates/limedl-core/src/tests/buffer_integrity_tests.rs#L1-L18`.

## Mock-server integration corpus

`crates/limedl-core/src/tests/` also holds the scenario suites:

- `manager_tests/` — start/validation, thread resolution, progress, checksum,
  eviction, lifecycle, queries, scheduling.
- `http_executor_tests/` — single stream, multi stream, chunk workers, mirrors
  and abuse, HTTP errors, rate limiting.
- `scheduler_tests/` — limits, lifecycle, scheduling.
- `persistence_tests.rs` / `persistence_e2e_tests.rs` — restart recovery,
  including the stale-claim test.
- `checksum_e2e_tests.rs`, `mirror_e2e_tests.rs`, `cdn_e2e_tests.rs`,
  `retry_tests.rs`, `settings_roundtrip_tests.rs`, `disk_detect_test.rs`.

Evidence: `repo://crates/limedl-core/src/tests/mod.rs#L1-L27`.

### Dispatcher facade tests

`dispatcher_tests.rs` is the facade matrix: lifecycle `Updated` emission,
`status`/`list`/`has_active_downloads` aggregation, degraded branches when a
service is absent, `save_settings` synchronization to ConcurrencyManager /
BufferPool, CDN clear when disabled, mirror resolution and tracker-list
normalization. Its `make_manager` / `inject_download` helpers are `pub(crate)` so
the Aria2 tests reuse them to cover `resolve_gid` and GID-cache eviction instead
of duplicating a `ManagedDownload` fixture.

Evidence: `repo://crates/limedl-core/src/tests/dispatcher_tests.rs#L1-L12`,
`repo://crates/limedl-core/src/tests/mod.rs#L9-L12`.

### Aria2 RPC E2E

`aria2_rpc/e2e_tests.rs` boots a real `Aria2RpcServer` over a live
`bootstrap()` `CoreSystems` on a random port and drives it with real HTTP POST
requests: the handler matrix, `system.multicall` response shape, secret gating on
every method, CORS fallback, magnet-to-BT routing, `changeOption` matrices, `keys`
filtering, `getUris`, and the terminal-task eviction scenario.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/e2e_tests.rs#L1-L20`.

### BT backend tests

`bt_backend/tests/` builds a real irontide session via `make_backend()` with DHT
disabled and `listen_port = Some(0)`, so tests bind no fixed port and need no
network. Torrent fixtures are hand-written bencode in `tests/mod.rs`
(`multi_file_torrent_bytes`, `single_file_torrent_bytes` with all-zero piece
hashes, since parsing only needs structure). Any test that goes through
`block_in_place` must use `#[tokio::test(flavor = "multi_thread")]`, because
`block_in_place` panics on a current-thread runtime.

Evidence: `repo://crates/limedl-core/src/bt_backend/tests/mod.rs#L158-L180`,
`repo://crates/limedl-core/src/bt_backend/tests/mod.rs#L243-L290`.

## Coverage

`check-rust` runs `cargo llvm-cov nextest` for `limedl-core` with
`--fail-under-lines 85` (pinned `cargo-llvm-cov@0.9.0`), then a second lcov for
`limedl-native`. cargo-llvm-cov ignores `tests/` directories and
`tests.rs`/`*_tests.rs` files by default, so the metric counts **product code
only**. A `cargo test --doc` step would be needed if a doctest is ever added.

Evidence: `repo://.github/workflows/ci.yml#L451-L472`.

Related pages: [Slint UI Testing](slint-ui-testing.md),
[Build, Tooling, CI and Release Operations](../operations/build-release-and-ci.md),
[HTTP Download Lifecycle](../workflows/http-download-lifecycle.md).
