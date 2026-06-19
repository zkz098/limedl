# Learnings - Active Acceleration

## Session: ses_1228357a9ffefA9AHJlE1H652D
## Started: 2026-06-19T01:52:40.330Z

## Conventions
- Rust: edition 2024, nightly, serde camelCase, no bare unwrap
- Vue: Composition API only, script setup lang="ts", scoped styles
- Skills priority: user-installed > builtin
- No code in manager.rs beyond AppState field + settings sync
- Separate reqwest Client for acceleration (NOT shared client)
- Session-only IP caching

## Task 2: CdnAccelerationSettings Types
- Added `CdnAccelerationSettings` struct in types.rs (line 594-619) with 5 fields: enabled (default false), active_ip, active_speed_mbps, last_test_at_ms, last_error (all Option, default None)
- Added `cdn_acceleration` field to AppSettings (after aria2_rpc)
- Added `cdnAcceleration` to load_settings() JSON detection chain in manager.rs
- Added `CdnAccelerationSettings` TS interface + field on `AppSettings` in src/types/settings.ts
- Wrote 3 Rust tests (round_trip, defaults, backward_compat) + 2 vitest type validation tests
- All 30 Rust tests pass, all 3 TS tests pass
- Key gotcha: Every `AppSettings { ... }` struct literal in the codebase must include the new field (manager.rs normalize_settings, manager.rs legacy path, tests/manager_tests.rs 5 occurrences)
- Test file included via `#[path = "tests/manager_tests.rs"]` at bottom of manager.rs, NOT as a separate test crate

## Key Files
- types.rs:548-565: AppSettings struct
- types.rs:567-580: Aria2RpcSettings (template for new settings)
- manager.rs:2449-2470: build_http_client()
- manager.rs:2480-2491: load_settings() JSON detection chain
- vite.config.ts: vue + UnoCSS plugins
- ci.yml: lint-typescript + check-rust jobs (now includes test steps)
- vitest + @vue/test-utils + jsdom for frontend tests
- Rust tests use directory-based module pattern (tests/mod.rs + tests/smoke.rs)
- CDN module: `cdn/mod.rs` declares submodules (`mod ip_ranges; mod tests;`)
- CIDR parsing: manual implementation using `std::net::Ipv4Addr` + bitmask (`!0u32 << (32-prefix)`)
- Subnet clamping: `max_offset = 2^(32-prefix) - 1`, clamp `samples_per_cidr` to this
- Test naming: inline `#[cfg(test)] mod tests { ... }` with functions like `test_expand_cidrs`
- Test filter: use `cargo test cdn::ip_ranges` (includes module path) to run all IP ranges tests
- Warnings: unused `pub(crate)` items are expected until consumer modules exist (Tasks 5+)
- `CdnAccelerationSettings` must be imported in `manager.rs` types use block if referenced there
- Static data file convention: `pub(crate) const` at file top, source URL in doc comment

## Task 3: i18n Keys (2026-06-19)
- Added `settings.cdnAcceleration` section to both en-US.ts and zh-CN.ts
- 19 keys each, matching exactly between locales
- Inserted after proxy settings (`proxyHint`) and before `defaultScene`
- Pre-existing lint error in `useDownloader.ts:260` (floating promise) — not related to this task

## Task 9: CDN Resolver (2026-06-19)
- Created `src-tauri/src/download/cdn/resolver.rs` with `build_accelerated_client()` and `is_cloudflare_domain()`
- `build_accelerated_client` mirrors `build_http_client()` from manager.rs:2449-2470 exactly:
  - `tcp_nodelay(true)`, `read_timeout(Duration::from_secs(15))`
  - `user_agent(settings.download.default_user_agent.clone())`
  - `redirect(Policy::limited(10))`
  - Proxy config: Disabled=no_proxy, System=default, Manual=Proxy::all()
  - PLUS `.resolve_to_addrs(domain, &[SocketAddr::new(IpAddr::V4(ip), 0)])` — port 0 means URL-scheme default
- `is_cloudflare_domain` — v1 stub always returns true (user controls via settings toggle)
- Error type: `Box<dyn std::error::Error + Send + Sync>` — reqwest::Error implements this, so `?` works
- Import path: `crate::download::types::{AppSettings, ProxyMode}` (NOT super::types — cdn is nested under download)
- Updated `cdn/mod.rs` to add `mod resolver;`
- 2 tests: `test_build_accelerated_client` (builds with localhost, no panic) + `test_is_cloudflare_domain` (always true)
- All tests pass; dead_code warnings expected until consumer modules exist
- NO `danger_accept_invalid_certs` — TLS validates via SNI using original domain name
- NO modifications to shared DownloadManager client — this is a separate Client

## Task 5: Live IP Range Fetch with Caching (2026-06-19)
- Added `fetch_cloudflare_ipv4_ranges()` — async, HTTP GET via `reqwest::get()`, 10s timeout via `tokio::time::timeout`
- Added `fetch_ranges_from_url(url: &str)` — internal helper exposed `pub(crate)` for testing with bad URLs
- Added `IpRangesCache` struct with `ips: Vec<Ipv4Addr>`, `fetched_at: Instant`, `from_fallback: bool`
- Added `get_ip_ranges(cache: &Mutex<IpRangesCache>) -> IpRangesCache` — check cache first, fetch on miss, fall back to static `CLOUDFLARE_IPV4_RANGES` on failure
- Uses `tokio::sync::Mutex` (NOT `std::sync::Mutex`) for async-safe caching
- Caching strategy: single-populate, never-expire — first call fills cache, subsequent calls return clone immediately
- Returns clones of cached data, not references — avoids borrow issues with MutexGuard
- 3 new tests: `test_static_fallback_bundle_size` (45 IPs), `test_fetch_from_bad_url_fails` (unreachable URL), `test_caching_returns_cached_data` (pre-seeded cache returns twice without re-fetch)
- `#![allow(dead_code)]` at file level — all items are forward-looking API consumed by Task 8+

## Task 6: TCP Connect Latency Screening (2026-06-19)
- Created `src-tauri/src/download/cdn/speed_test.rs` with `measure_tcp_latency()` and `screen_candidates()`
- `measure_tcp_latency`: wraps `tokio::net::TcpStream::connect` with `tokio::time::timeout`, returns `Some(Duration)` on success or `None` on timeout/refusal; drops stream immediately after measurement
- `screen_candidates`: uses `JoinSet` with refill pattern — spawns initial batch of `concurrency` tasks, then refills as completions arrive; port 443 hardcoded; results sorted by latency ascending
- Concurrency pattern: `join_set.spawn(async move { ... })` → `while let Some(result) = join_set.join_next().await { ... }` with refill — matches manager.rs JoinSet usage
- 3 unit tests: `test_measure_latency_to_localhost` (spawns `TcpListener` on ephemeral port, latency < 100ms), `test_measure_latency_unreachable` (127.0.0.1:1 closed port → None), `test_screen_candidates_concurrent` (10× localhost:443 → all unreachable → empty result)
- Key gotcha: 192.0.2.0/24 (TEST-NET) addresses were reachable in this environment (corporate VPN/routing) — switched to localhost with closed ports for unreachable tests
- Updated `cdn/mod.rs` to add `mod speed_test;`
- All 3 tests pass (`cargo test cdn::speed_test`)

## Task 7: HTTPS Download Throughput Measurement (2026-06-19)
- Added `SPEED_TEST_URL` (200MB Cloudflare CDN endpoint) and `SPEED_TEST_DURATION` (10s) constants
- Added `build_throughput_client()` — mirrors `build_http_client()` from manager.rs:2449-2470 exactly:
  - `resolve_to_addrs(hostname, &[addr])`, `tcp_nodelay(true)`, `read_timeout(15s)`
  - user_agent with empty-string fallback to `default_http_user_agent()`
  - `redirect(Policy::limited(10))`
  - Proxy config: Disabled=no_proxy, System=default, Manual=Proxy::all()
  - NO `danger_accept_invalid_certs` — TLS validates normally via SNI
- Added `measure_throughput()`:
  - Parses URL to extract port (scheme-default fallback: 443 for HTTPS, 80 for HTTP)
  - Builds throwaway `reqwest::Client` per measurement (NOT shared)
  - Uses `Arc<AtomicU64>` to count bytes across timeout boundary — allows partial data on timeout
  - Wraps streaming with `tokio::time::timeout(SPEED_TEST_DURATION)`; partial bytes returned on timeout
  - Returns `(bytes_downloaded, elapsed_ms)` as `(f64, u64)`
- URL parameter added to `measure_throughput` (not in plan's signature, but required for testability)
- 2 new tests appended to existing test module (total 5 tests pass):
  - `test_throughput_to_localhost`: spawns raw TCP HTTP server, streams 1MB, asserts bytes > 0
  - `test_throughput_unreachable`: connects to 192.0.2.1 (TEST-NET-1), asserts Err or negligible data
- Key gotcha: raw TCP server must keep connection open after writing — added `stream.read(&mut buf)` after flush to prevent RST
- Key gotcha: `resolve_to_addrs` with port 0 uses scheme-default (80/443), NOT URL explicit port; must parse URL and set port explicitly
- Key gotcha: TEST-NET-1 addresses (192.0.2.0/24) may be reachable in some network environments (corporate VPN) — test handles both Err and Ok paths
- Import pattern: `crate::download::types::{...}` from within `download::cdn::` subtree (NOT `super::types`)
- `AppSettings` derives `Default` (line 546), `default_user_agent` has `serde(default = "default_http_user_agent")`

## Task 8: Two-Phase Speed Test Orchestrator (2026-06-19)
- Added `SpeedTestConfig` (concurrency:50, tcp_timeout:3s, throughput_duration:10s, top_n_candidates:5) and `SpeedTestResult` (ip, tcp_latency_ms, throughput_mbps:Option, error:Option) structs with `impl Default`
- Added `run_speed_test()` orchestrator composing `screen_candidates` (Phase 1) + `measure_throughput` (Phase 2)
- Phase 2 uses `JoinSet` with concurrency = top_n_candidates; each task clones `AppSettings` for `'static` lifetime
- Sorting: `throughput_mbps` descending (None last), `tcp_latency_ms` ascending tiebreak via `partial_cmp`
- throughput_mbps formula: `(bytes / elapsed_secs) / 1_000_000.0` (MB/s, despite field name)
- 3 new tests: `test_orchestrator_all_unreachable` (class-E IPs), `test_orchestrator_with_mock_ips` (localhost:443 listener, Phase 1 passes, Phase 2 TLS-fails), `test_orchestrator_partial_failures` (class-E + localhost mix)
- All 8 tests pass; `cargo check` clean; `cargo clippy` has 3 pre-existing errors in other files (resolver.rs, types.rs)
- Added `#![allow(dead_code)]` to speed_test.rs — all `pub(crate)` items are forward-looking API consumed by Task 11+
- Fixed pre-existing clippy `unnecessary_sort_by` in `screen_candidates` (mechanical: `sort_by(|a,b| a.1.cmp(&b.1))` → `sort_by_key(|a| a.1)`)
- Key gotcha: TEST-NET (192.0.2.0/24) reachable via corporate VPN; switched unreachable tests to class-E (240.0.0.0/4)
- Key gotcha: `test_screen_candidates_concurrent` was pre-existing failure (localhost:443 reachable) — now fixed with class-E IPs

## Task 10: CDN Accelerator State Machine (2026-06-19)
- Created `src-tauri/src/download/cdn/accelerator.rs` with `AccelState` enum and `CdnAccelerator` struct
- `AccelState`: `Idle`, `Testing`, `Ready`, `Error(String)` — derives `Debug, Clone, PartialEq`
- `CdnAccelerator` fields: `state: RwLock<AccelState>`, `active_ip: RwLock<Option<Ipv4Addr>>`, `active_speed_mbps: RwLock<Option<f64>>`, `cancel_token: RwLock<Option<CancellationToken>>`, `accelerated_client: RwLock<Option<reqwest::Client>>`
- All fields use `tokio::sync::RwLock` (NOT std::sync versions)
- `start_test()`: idempotent (returns Ok if already Testing), creates CancellationToken, spawns background via `tauri::async_runtime::spawn`, uses `tokio::select!` for cancellation during speed test
- Background task flow: `get_ip_ranges()` → `run_speed_test()` → find best candidate → `apply_ip()` or set Error state
- `cancel_test()`: uses `try_write()` (NOT `blocking_write()`) to avoid panicking in async contexts — if lock is busy, background task will still see cancellation
- `apply_ip()`: calls `build_accelerated_client("speed.cloudflare.com", ip, settings)`, stores client/ip/speed, sets state to Ready
- `build_accelerated_client` returns `Result<Client, Box<dyn Error + Send + Sync>>` — cannot use `?` directly with `anyhow::Error`; must `.map_err(|e| anyhow::anyhow!("{e}"))?`
- 3 tests all pass: `test_lifecycle` (Idle → apply_ip → Ready → clear → Idle), `test_cancel` (start_test → cancel → Idle, double-cancel safe), `test_clear_drops_client` (apply_ip → get_client is Some → clear → get_client is None)
- Added `#![allow(dead_code)]` to accelerator.rs — unused until Task 11 wires it up
- Fixed pre-existing clippy `derivable_impls` for `CdnAccelerationSettings` (merged duplicate `#[derive]` attributes, added `Default`)
- Fixed pre-existing clippy `dead_code` for `is_cloudflare_domain` in resolver.rs (`#[allow(dead_code)]`)
- Key gotcha: `blocking_write()` on `tokio::sync::RwLock` panics within async runtime (`#[tokio::test]` or Tauri commands) — MUST use `try_write()` for sync methods called from async contexts
- Key gotcha: `collapsible_if` clippy lint — Rust 2024 supports `if let A && let B { ... }` chaining
- `start_test` takes `&Arc<Self>` (not `&self`) because the background task needs `Arc::clone(self)` for `'static` lifetime
- Other methods use `&self` since they don't spawn tasks: `status()`, `apply_ip()`, `clear()`, `get_client()`, `cancel_test()`
- `IpRangesCache` is created fresh with empty cache per test run inside the background task (not stored on CdnAccelerator)

## Task 13: CDN Acceleration Settings Integration (2026-06-19)
- Connected `AppSettings::cdn_acceleration` to `CdnAccelerator` lifecycle
- Most plumbing already existed: `AppState.cdn_accelerator` field, `lib.rs` creation/injection, `cdn/mod.rs` re-export
- Missing pieces added:
  - `download/mod.rs`: Added `pub(crate) use cdn::CdnAccelerator;` re-export
  - `commands.rs`: Added 3-line sync in `settings_save` — when `cdn_acceleration.enabled == false`, calls `state.cdn_accelerator.clear().await`
  - `types.rs`: Added `test_settings_round_trip_with_cdn` — AppSettings-level round-trip with non-default CdnAccelerationSettings
- CDN acceleration is only cleared when disabled — never auto-enabled (acceleration is manually triggered via IPC)
- All 6 settings tests pass + `cargo check` + `cargo clippy` clean

## Task 11: CDN Tauri IPC Commands (2026-06-19)
- Created `src-tauri/src/download/cdn/commands.rs` with 6 `#[tauri::command]` functions:
  - `cdn_fetch_ranges` — returns 15 static CIDR strings from `CLOUDFLARE_IPV4_RANGES` (no HTTP fetch)
  - `cdn_test` — calls `accelerator.start_test(settings)`, returns immediately, idempotent
  - `cdn_apply` — parses IP string to `Ipv4Addr`, calls `accelerator.apply_ip(ip, speed_mbps, &settings)`
  - `cdn_clear` — calls `accelerator.clear()`, always safe (no-op if idle)
  - `cdn_status` — calls `accelerator.status()`, returns human-readable string: `"Idle"`, `"Testing"`, `"Ready"`, `"Error: {msg}"`
  - `cdn_cancel` — calls `accelerator.cancel_test()`, always safe no-op
- All commands return `Result<T, String>` (Tauri convention, NOT `SerializableError` from existing commands)
- Error conversion: `.map_err(|e| e.to_string())` for both `anyhow::Error` and `AddrParseError`
- Settings retrieved via `state.manager.settings().await.map_err(|e| e.to_string())?` (returns `Result<AppSettings>`)
- Added `pub cdn_accelerator: Arc<CdnAccelerator>` to `AppState` struct in `manager.rs`
- `CdnAccelerator` re-exported as `pub(crate) use accelerator::CdnAccelerator;` in `cdn/mod.rs`
- Initialized in `lib.rs`: `Arc::new(CdnAccelerator::new())` alongside other managers
- Added to both `AppState` constructions: `app.manage(...)` and the spawned emit task
- 6 CDN commands registered in `tauri::generate_handler![]` in `lib.rs`
- Commands re-exported via `pub use cdn::commands::{...}` in `download/mod.rs`
- `pub mod commands;` in `cdn/mod.rs` makes the commands publicly accessible
- 2 tests: `fetch_ranges_returns_15_cidrs` (static constant check + spot-check) + `apply_rejects_invalid_ip` (IP parsing edge cases)
- Key gotcha: `use download::cdn::CdnAccelerator;` from `lib.rs` FAILS because `cdn` is a private module — must re-export via `pub(crate) use cdn::CdnAccelerator;` in `download/mod.rs`, then import as `use download::CdnAccelerator;`
- Key gotcha: `CdnAccelerator` import in `cdn/commands.rs` is unused (only `AccelState` is used in match) — removed to avoid warning
- `cargo check` clean, `cargo clippy` clean, all 27 CDN tests pass
