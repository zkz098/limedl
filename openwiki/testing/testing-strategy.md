---
type: testing
title: Testing Strategy
description: How limedl is tested — the test layout conventions, nextest process isolation, the mock-server integration corpus, byte-level corruption oracles and adversarial servers, BT and Aria2 harnesses, the headless daemon end-to-end test, and the coverage gate.
tags: [testing, nextest, integration-tests, corruption, coverage]
sources:
  - id: openwiki-source-06de9eea8068258882d65c0b
    resource: repo://.github/workflows/aria2-oracle.yml
  - id: openwiki-source-164e2da859b5277df81c7d94
    resource: repo://.github/workflows/ci.yml
  - id: openwiki-source-c8ca1e3187dacc725ff333e9
    resource: repo://crates/limedl-core/src/aria2_rpc/e2e_tests.rs
  - id: openwiki-source-3659606b404344d4dd4d1487
    resource: repo://crates/limedl-core/src/aria2_rpc/interop_tests.rs
  - id: openwiki-source-cb3b278da9fc4917fdb881e9
    resource: repo://crates/limedl-core/src/aria2_rpc/oracle_tests.rs
  - id: openwiki-source-1c0eea0988a8be3129ce2126
    resource: repo://crates/limedl-core/src/bt_backend/tests/alerts.rs
  - id: openwiki-source-8a802dd79d30f7920922a1e7
    resource: repo://crates/limedl-core/src/bt_backend/tests/mod.rs
  - id: openwiki-source-167c6ec7cd42649ee514d116
    resource: repo://crates/limedl-core/src/database/tests/chunks.rs
  - id: openwiki-source-627a941d30d0b0f516cfa103
    resource: repo://crates/limedl-core/src/database/tests/connection.rs
  - id: openwiki-source-34256bebf0b881402462174e
    resource: repo://crates/limedl-core/src/database/tests/schema_migrations.rs
  - id: openwiki-source-05acffc41354e79e4b63b4e7
    resource: repo://crates/limedl-core/src/download/managed.rs
  - id: openwiki-source-b1f82d2f73c50a300ab96db2
    resource: repo://crates/limedl-core/src/http_executor/tests.rs
  - id: openwiki-source-9fa813eab6e27ac3f5fdbf1d
    resource: repo://crates/limedl-core/src/manifest.rs
  - id: openwiki-source-60ceefd6554ff8fc9f67e129
    resource: repo://crates/limedl-core/src/metalink/tests.rs
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
  - id: openwiki-source-b55d53f72185d12e19318ab3
    resource: repo://crates/limedl-core/src/tests/http_executor_tests/compression.rs
  - id: openwiki-source-b6ac5cc91bdd258dd969abbc
    resource: repo://crates/limedl-core/src/tests/mod.rs
  - id: openwiki-source-6b9d139f018792972b672af7
    resource: repo://crates/limedl-core/src/tests/persistence_tests.rs
  - id: openwiki-source-3742feb3f92f40e410145856
    resource: repo://crates/limedl-core/src/tests/resume_corruption_tests.rs
  - id: openwiki-source-1a1d4b50d244dfdbcf190f3a
    resource: repo://crates/limedl-core/tests/logging_reload_repro.rs
  - id: openwiki-source-12b12baa98b75281cf2fd260
    resource: repo://crates/limedl-server/tests/daemon.rs
  - id: openwiki-source-3fe9812b75a7522e89f74344
    resource: repo://docs/aria2-interop-testing.md
  - id: openwiki-source-feafbe9db788653e845840b8
    resource: repo://sonar-project.properties
generated: { by: "pi", at: "2026-10-07T03:53:23.435Z" }
verified:
  - by: openwiki/0.7.1
    at: 2026-10-07T03:53:23.435Z
---

# Testing Strategy

limedl's engine tests live beside the code (unit tests) or in
`crates/limedl-core/src/tests/` (cross-module integration), with two dedicated
integration binaries under `crates/limedl-core/tests/`. The UI layer has its own
in-process suite, documented on [Slint UI Testing](slint-ui-testing.md); the
headless daemon has an end-to-end test in its own crate (below).

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

## Crash consistency and durable progress

The tests that guard the durable-vs-received split (see
[SQLite Persistence, Durable Progress and Crash Recovery](../systems/persistence-and-recovery.md))
are deliberately layered, because the failure they prevent — resuming past data
that never reached the file — is invisible from the outside:

- **Pure range math** (`download/managed.rs` tests): `record_progress_on_managed`
moves only the received counters; `record_durable_bytes` advances the durable ones;
a coalesced write is credited to *every* chunk it overlaps; `durable_bytes()` never
exceeds the received count; and a manifest built like a loaded one starts
`durable == received`.
- **Persist rules** (`tests/persistence_tests.rs`):
  `persisted_progress_is_the_durable_progress` runs `DownloadManager::persist` with
  a chunk that is received-complete but only partly flushed and asserts the stored
  row says 40 bytes and `completed = false`;
  `incremental_persist_of_a_completed_but_unflushed_chunk_stays_incomplete` does the
  same through the 300 ms cycle. Both go through a real `Database`.
- **Finalize guard** (`http_executor::tests`): `ensure_core_fully_durable` accepts a
  fully durable download and a chunk-less single-stream one, and refuses a download
  with a shortfall with a message containing the missing byte count. It takes the
  already-held core guard on purpose — calling `lock_core()` there deadlocks.
- **Migrations and quarantine** (`database/tests`):
  `partially_applied_migration_is_recovered_on_open` opens a database whose schema
  is ahead of its `user_version`,
  `add_column_if_missing_is_idempotent` pins the no-op, and the connection tests
  cover the non-database quarantine and the lock-contention classifier.

### Fixture invariant: `end` is inclusive

`ChunkManifest::end` is the **last byte** of the chunk, so a chunk's size is
`byte_len() == end - start + 1`, and "complete" means
`durable_bytes() >= byte_len()`. Two database fixtures used `end` as an exclusive
bound (`start: 0, end: 500` with `downloaded: 500, completed: true`, i.e. a 501-byte
chunk); they passed only while `completed` was stored verbatim, and had to be made
self-consistent once the row derives completeness from the durable count. New
fixtures should state `end` as the last byte, never one past it.

Evidence: `repo://crates/limedl-core/src/download/managed.rs#L232-L403`,
`repo://crates/limedl-core/src/tests/persistence_tests.rs#L505-L583`,
`repo://crates/limedl-core/src/http_executor/tests.rs#L238-L295`,
`repo://crates/limedl-core/src/database/tests/schema_migrations.rs#L128-L178`,
`repo://crates/limedl-core/src/database/tests/connection.rs#L173-L230`.

### Adversarial servers

`adversarial_interception_tests.rs` uses two hostile hosts that return wrong
bytes for valid ranges: `range-shifted` (content shifted relative to the
advertised `Content-Range`) and `range-bitflip` (first byte of each range
flipped). Both keep the byte counts identical, so every chunk is accepted and the
download reaches finalize — only the checksum comparison can expose the problem.
With an explicit `expected_checksum`, the download must end in `Failed`.

Evidence: `repo://crates/limedl-core/src/tests/adversarial_interception_tests.rs#L1-L16`.

### Mid-stream body interruption

`http_executor_tests/http_errors.rs` serves a 64 KiB file whose **first** response
sends 16 KiB and then aborts the body stream with a `ConnectionReset` error; the
resumed request is answered from the `Range` header. The test
(`stream_interruption_resumes_and_completes`) requires the download to finish with
the correct checksum instead of failing with "error decoding response body" — the
single-stream loop must treat a broken body as a retryable transport error and
resume from its durable offset. The classification it depends on is pinned by
`task_lifecycle::tests::is_network_error_http_decode_is_true`, which asserts a
reqwest *decode* error counts as a network error (so a multi-URL task fails over
to the next mirror) while an HTTP builder error does not.

Evidence: `repo://crates/limedl-core/src/tests/http_executor_tests/http_errors.rs#L435-L592`,
`repo://crates/limedl-core/src/task_lifecycle/tests.rs#L262-L299`.

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
  and abuse, HTTP errors, rate limiting, response compression.
- `scheduler_tests/` — limits, lifecycle, scheduling.
- `persistence_tests.rs` / `persistence_e2e_tests.rs` — restart recovery,
  including the stale-claim test.
- `checksum_e2e_tests.rs`, `mirror_e2e_tests.rs`, `cdn_e2e_tests.rs`,
  `retry_tests.rs`, `settings_roundtrip_tests.rs`, `disk_detect_test.rs`.
- `metalink/tests.rs` — Metalink 4.0/3.0 and Metalink/HTTP header parsing
  (including the `best_checksum` and `sorted_mirror_urls` helpers), mirror
  scoring, and pool leasing/concurrency limits.

Evidence: `repo://crates/limedl-core/src/tests/mod.rs#L1-L27`.

### Response compression tests

`http_executor_tests/compression.rs` covers the gzip/brotli/zstd paths added by
reqwest's decompression features. The test server exposes `/file/encoded/{enc}`
(single-stream) and `/file/encoded-range/{enc}` (range-capable but returns the
whole compressed body when asked), compresses only when the request's
`Accept-Encoding` allows it, and counts how often it did
(`TestServer::encoded_responses`).

Each encoding is exercised twice:

- single-stream: the download must complete with the checksum of the **decoded**
  content and the server must have compressed at least once;
- range: a 4-thread download of a 9 MiB file must complete with the decoded
  checksum and the server must have compressed **zero** times — proving
  `identity_encoding` kept every segment request byte-exact.

Evidence: `repo://crates/limedl-core/src/tests/http_executor_tests/compression.rs#L1-L40`,
`repo://crates/limedl-core/src/test_harness/mod.rs#L180-L196`.

### Dispatcher facade tests

`dispatcher_tests.rs` covers the `Dispatcher` facade matrix and exposes
`make_manager`/`inject_download` as `pub(crate)` so the Aria2 tests reuse them for
`resolve_gid` and GID-cache eviction, and so the durability tests can persist
against a real `DownloadManager`.

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
every method, CORS fallback **and wildcard**, magnet-to-BT routing,
`addMetalink` (a base64 Metalink document creates one task per file and reports
status), `changeOption` matrices, `keys` filtering, `getUris`, and the
terminal-task eviction scenario. Because the CORS test now covers
`allow_any_origin`, it also asserts that a wildcard origin is never paired with
`Access-Control-Allow-Credentials`.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/e2e_tests.rs#L1-L20`,
`repo://crates/limedl-core/src/aria2_rpc/e2e_tests.rs#L1211-L1315`,
`repo://crates/limedl-core/src/aria2_rpc/e2e_tests.rs#L2404-L2460`.

### Aria2 interoperability fixtures (Tier 1)

`aria2_rpc/interop_tests.rs` is the client-contract layer: rather than the
hand-written request shapes in `e2e_tests.rs`, it encodes the shapes real clients
send. It exists because a shape mismatch can pass limedl's own tests while
breaking a real client — `addTorrent` used to read `params[1]` as the options
object, so AriaNg's `[torrent, [], options]` silently dropped
`dir`/`out`/`pause`/`select-file` while the `[torrent, options]` tests stayed
green.

The suite asserts:

- `system.listMethods` equals the routed handler set exactly (37 entries), and
  every advertised method is reachable (a routed method's error must not be
  method-not-found; since every domain error is code 1, that is recognized by
  its message).
- AriaNg's multicall shape (no outer token, per-call tokens) yields the
  single-element `[value]` wrapper.
- `addTorrent([torrent, [], options])` and `[torrent, options]` both keep the
  options, and `addUri([urls, options])` keeps `dir`/`out`.
- `tellStatus` carries aria2's always-present keys plus the HTTP piece map and
  the BT metadata object, and stays silent about `errorCode` until terminal.
- `getServers`, `changeUri` (`[deleted, added]`) and `changePosition` (the real
  resulting index) match their aria2 reply shapes.
- `getGlobalOption` and `getOption` answer AriaNg's complete global and task key
  sets (`interop_get_global_option_covers_ariang_keys` /
  `interop_get_option_covers_ariang_task_keys`).
- A top-level JSON-RPC batch returns one response per element; an
  all-notification batch returns nothing and an empty array is Invalid Request
  (`batch_request_returns_one_response_per_element` and friends in `tests.rs`).
- An abbreviated GID resolves only when its prefix is unique
  (`resolve_gid_accepts_a_unique_prefix_and_refuses_an_ambiguous_one`).

It is pure Rust with no external binary, so it runs inside the normal
`cargo nextest run --features "test-utils,aria2-rpc"` core gate. The Tier 2
(real `aria2c` oracle) and Tier 3 (AriaNg smoke test) layers, together with the
list of intentional deviations, live in `docs/aria2-interop-testing.md`.
`interop_get_version_is_truthful` additionally locks `aria2.getVersion` to the
real version and the truthful feature list (no XML-RPC/Firefox3 Cookie/SFTP;
Metalink, GZip, Brotli and Zstd present).

Evidence: `repo://crates/limedl-core/src/aria2_rpc/interop_tests.rs#L1-L60`,
`repo://crates/limedl-core/src/aria2_rpc/interop_tests.rs#L78-L380`,
`repo://docs/aria2-interop-testing.md#L1-L76`.

### Aria2 oracle (Tier 2, opt-in)

`aria2_rpc/oracle_tests.rs` is the layer above the fixtures: it starts a real
`aria2c --enable-rpc` beside limedl's server and reports how the same requests
differ, which is the only way to catch "aria2 itself returns something else".

The suite is **opt-in** via `ARIA2_ORACLE_BIN`. Unset, every test returns early,
so the normal gate never needs a third-party binary; set but unusable, it panics,
so a job cannot pass by accident. A separate nightly/manual workflow
(`.github/workflows/aria2-oracle.yml`) sets it on Linux.

Differences are **allowlisted**: `GapReport` fails the run on an aria2 key limedl
is required to answer (AriaNg's option keys, the always-present `tellStatus`
keys), a `listMethods`/`listNotifications` regression, an error object that is not
code 1, or a batch that is not an array; the remaining aria2-only differences are
printed as allowlisted notes (nextest needs `--success-output=final` to show
captured output from passing tests). The job also fails when the oracle cannot
start or a server stops answering. The
`Aria2Oracle` guard handles the operational contract — free port, `--no-conf`,
`--enable-dht=false`, a `TempDir`, a readiness poll and a `Drop` that kills the
child. The readiness poll tolerates the connection-refused state that is expected
until `aria2c` binds its port: it calls a non-panicking `rpc_try` (returning
`None` on transport or parse errors) in a bounded retry loop, and checks
`child.try_wait()` each round so an early child exit fails with the real status
instead of a readiness timeout.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/oracle_tests.rs#L1-L68`,
`repo://crates/limedl-core/src/aria2_rpc/oracle_tests.rs#L94-L170`,
`repo://.github/workflows/aria2-oracle.yml#L1-L45`,
`repo://docs/aria2-interop-testing.md#L128-L190`.

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

## Headless server daemon

The `limedl-server` crate adds two layers:

- **Unit tests** for the parts that need no engine: CLI parsing, the
  `LogLevelArg` → `LogLevel` mapping, config precedence/trimming, absolute-path
  validation for `--download-dir`, the explicit data-dir override, and the
  instance lock's exclusivity (a second lock on the same directory fails, and the
  lock is reusable after release).
- **One end-to-end test** (`tests/daemon.rs`) that starts a real daemon against a
  temp data directory on a reserved port and proves the wiring the core's
  `aria2_rpc` tests cannot: `bootstrap` → force-enabled RPC → token enforcement
  (a request without `token:` is `Unauthorized`) → `aria2.shutdown` →
  `registry.shutdown_all()` → the daemon task exits cleanly within a timeout. The
  shutdown future is injected (`std::future::pending()`), so the only way the
  process may stop is the JSON-RPC call the test sends. Readiness is polled with a
  fallible `rpc_try` that treats a refused connection as "not ready yet", because
  the engine bootstraps and binds its port asynchronously; only the request after
  readiness may panic.

It runs in the Linux `check-rust` job only. The Windows/macOS legs compile the
crate (including the `cfg(not(unix))` Ctrl+C path) through `cargo clippy
--workspace --all-targets` but do not execute the port-binding test, because the
daemon targets musl Linux and a bootstrapped engine on those runners would add
flake for no signal.

Evidence: `repo://crates/limedl-server/tests/daemon.rs#L24-L123`,
`repo://crates/limedl-server/src/lib.rs#L194-L213`,
`repo://.github/workflows/ci.yml#L507-L514`.

## Coverage

`check-rust` runs `cargo llvm-cov nextest` for `limedl-core` with
`--fail-under-lines 85` (pinned `cargo-llvm-cov@0.9.0`), then a second lcov for
`limedl-native`. cargo-llvm-cov ignores `tests/` directories and
`tests.rs`/`*_tests.rs` files by default, so the metric counts **product code
only**. A `cargo test --doc` step would be needed if a doctest is ever added.
`limedl-server` has no lcov report and is listed in Sonar's
`sonar.coverage.exclusions`, so its lines are not counted as 0% — its value is in
the end-to-end test, not per-line coverage.

Evidence: `repo://.github/workflows/ci.yml#L451-L472`,
`repo://sonar-project.properties#L76`.

Related pages: [Slint UI Testing](slint-ui-testing.md),
[Headless Server Daemon](../integrations/headless-server-daemon.md),
[Aria2 JSON-RPC Compatibility Server](../integrations/aria2-rpc-server.md),
[Metalink Parsing and Mirror Selection](../workflows/metalink-and-mirror-selection.md),
[Build, Tooling, CI and Release Operations](../operations/build-release-and-ci.md),
[HTTP Download Lifecycle](../workflows/http-download-lifecycle.md).
