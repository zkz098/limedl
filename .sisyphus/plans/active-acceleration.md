# Active Acceleration (Cloudflare CDN Speed Test + DNS Override)

## TL;DR

> **Quick Summary**: Cloudflare-focused "Active Acceleration" feature — fetch Cloudflare's public IPv4 ranges, speed-test each candidate IP via TCP connect + HTTPS download, then route Cloudflare-hosted downloads through the fastest IP using a separate accelerated reqwest Client. User-triggered from a new Settings panel.
>
> **Deliverables**:
> - Rust `cdn/` module (ip_ranges, speed_test, resolver, types) in `src-tauri/src/download/cdn/`
> - New `CdnAccelerationSettings` in `AppSettings` (Rust + TS)
> - `SettingsCdnAccelerationPanel.vue` following existing settings panel pattern
> - 6 Tauri IPC commands (cdn_fetch_ranges, cdn_test, cdn_apply, cdn_clear, cdn_status, cdn_cancel)
> - Test infrastructure: vitest config + cargo test in CI
> - i18n keys (en-US + zh-CN)
>
> **Estimated Effort**: Large
> **Parallel Execution**: YES — 4 waves
> **Critical Path**: Task 1 → Task 5 → Task 8 → Task 11 → Task 16 → F1-F4

---

## Context

### Original Request
实现主动加速功能：对中国网络环境下Cloudflare CDN不同IP连通性差异大的问题，通过预先测速确定高速节点，并将DNS解析结果修改为高速节点来加速下载。

### Interview Summary
**Key Discussions**:
- **Scope**: Cloudflare-focused only — use public IPv4 ranges from `https://www.cloudflare.com/ips-v4` (NOT general CDN)
- **IP Source**: Cloudflare's publicly published IP list, not DNS resolution — no hickory-resolver dependency needed
- **Trigger**: User manually clicks "Test and Accelerate" button in a global Settings panel (not per-download, not automatic)
- **Speed Test**: TCP connect latency screening → top-N candidates → HTTPS download throughput measurement (actual ~200MB download per candidate)
- **IP Override**: `reqwest::ClientBuilder::resolve_to_addrs()` — build a **separate** accelerated Client (NOT modify shared client)
- **Test Strategy**: TDD — set up vitest (frontend) + cargo test (backend). Current project has zero tests
- **Caching**: Session-only in-memory cache. Cloudflare BGP routing is ephemeral — don't persist IP
- **IPv4 only**: v1 scope-locked to IPv4. IPv6 adds complexity not warranted yet

**Research Findings**:
- **reqwest 0.13 CRITICAL**: `dns_resolver()` API was REMOVED in 0.13. Only `resolve()` / `resolve_to_addrs()` remain. We MUST build a separate accelerated Client (not modify the shared one)
- **SNI works correctly**: `resolve_to_addrs("host", "ip:0")` sends hostname as TLS SNI — certificates validate correctly
- **Empty `cdn/` directory exists**: `src-tauri/src/download/cdn/` has 0 files — perfectly positioned
- **`aria2_rpc` module** is the exact template: settings struct, IPC commands, background task, re-export pattern
- **Cloudflare IP ranges**: ~15 IPv4 CIDRs, change < once per year. Bundle as static fallback

### Metis Review
**Identified Gaps** (addressed in plan):
- reqwest 0.13 lacks custom Resolve trait → separate accelerated Client with `resolve_to_addrs()`
- Cloudflare IP page may be blocked → bundle static fallback list
- Speed test endpoint accessibility → document fallback behavior
- Concurrent downloads during acceleration → block acceleration while downloads active
- In-progress download disruption → apply only to NEW downloads after acceleration
- Data usage disclosure → show estimated 2GB usage before test with confirmation
- No test infrastructure → Task 1 sets up vitest + cargo test scaffolding

---

## Work Objectives

### Core Objective
Add a user-triggered Cloudflare CDN acceleration feature: fetch Cloudflare IPv4 ranges, speed-test candidate IPs, and build an accelerated reqwest Client that routes Cloudflare-hosted downloads through the fastest edge node.

### Concrete Deliverables
- `src-tauri/src/download/cdn/{mod,ip_ranges,speed_test,resolver,types}.rs` — Rust acceleration engine
- `src-tauri/src/download/types.rs` — `CdnAccelerationSettings` struct added to `AppSettings`
- `src/types/settings.ts` — matching TS interfaces
- `src/components/settings/SettingsCdnAccelerationPanel.vue` — new settings panel
- `src/i18n/{en-US,zh-CN}.ts` — translation keys
- `vitest.config.ts` + `src/__tests__/` — frontend test infrastructure
- `src-tauri/src/download/cdn/tests/` — Rust integration tests

### Definition of Done
- [ ] `cargo test --workspace` → all Rust tests pass (NEW: speed test, resolver, settings, integration)
- [ ] `bun run test` → all vitest tests pass (NEW: panel render, IPC mock, i18n)
- [ ] `cargo clippy -- -D warnings` → zero warnings
- [ ] `vue-tsc --noEmit` → zero type errors
- [ ] Manual trigger: Settings → CDN Acceleration → "Test and Accelerate" → speed test completes → Cloudflare downloads use accelerated IP

### Must Have
- User-triggered Cloudflare IP speed testing with progress display
- Separate accelerated reqwest Client with `resolve_to_addrs()` overrides
- Session-only in-memory IP caching (no persistence between sessions)
- Static bundled Cloudflare IPv4 fallback list
- Settings panel with enable/disable toggle + status display + trigger button
- Estimated data usage disclosure before testing
- Backward-compatible settings (old settings.json without `cdnAcceleration` field parses correctly)

### Must NOT Have (Guardrails)
- **MUST NOT add code to `manager.rs`** — 3414-line god object, AGENTS.md explicitly forbids
- **MUST NOT modify the shared `reqwest::Client`** in `DownloadManager` — acceleration uses a separate client
- **MUST NOT use `danger_accept_invalid_certs(true)`** or bypass TLS — SNI works correctly with `resolve_to_addrs()`
- **MUST NOT auto-apply acceleration** — manual trigger only
- **MUST NOT persist accelerated IP to settings.json** — session-only
- **MUST NOT test IPv6** — IPv4 only for v1
- **MUST NOT intercept non-Cloudflare domains** — only override DNS for Cloudflare-identified hostnames
- **MUST NOT test while downloads are active** — block or warn
- **MUST NOT add speed test history dashboard** — show only last result
- **MUST NOT support per-download IP selection** — single global accelerated IP

---

## Verification Strategy (MANDATORY)

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: NO → Task 1 sets up vitest + cargo test in CI
- **Automated tests**: TDD — RED (failing test) → GREEN (minimal impl) → REFACTOR
- **Framework**: vitest (frontend) + `#[tokio::test]` with `ntest::timeout()` (Rust backend)
- **CI**: Add `cargo test` and `bun run test` to `.github/workflows/ci.yml`

### QA Policy
Every task MUST include agent-executed QA scenarios (see TODO template below).
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Frontend/UI**: Use Playwright (playwright skill) — Navigate, interact, assert DOM, screenshot
- **Rust tests**: Use Bash (`cargo test`) — Run specific test, assert PASS/FAIL, capture output
- **Tauri IPC**: Use Bash (curl equivalent via Tauri MCP bridge) — Send IPC invoke, assert response
- **Settings**: Use Bash — Read settings.json, assert field presence/values

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately — foundation + scaffolding):
├── Task 1: Test infrastructure setup [quick]
├── Task 2: CdnAccelerationSettings types (Rust + TS) [quick]
├── Task 3: i18n translation keys [quick]
└── Task 4: Cloudflare IP range list (static bundle) [quick]

Wave 2 (After Wave 1 — core engine, MAX PARALLEL):
├── Task 5: IP range fetching + caching [unspecified-high]
├── Task 6: TCP connect latency screening [unspecified-high]
├── Task 7: HTTPS download throughput measurement [unspecified-high]
├── Task 8: Speed test orchestrator (candidate selection + parallel testing) [unspecified-high]
└── Task 9: DNS override resolver (accelerated Client builder) [unspecified-high]

Wave 3 (After Wave 2 — integration + IPC, MAX PARALLEL):
├── Task 10: CdnAccelerator struct (state management + lifecycle) [unspecified-high]
├── Task 11: Tauri IPC commands (6 commands) [unspecified-high]
├── Task 12: Module registration (mod.rs + lib.rs wiring) [quick]
└── Task 13: Settings integration (AppSettings field + backward compat) [quick]

Wave 4 (After Wave 3 — frontend, MAX PARALLEL):
├── Task 14: SettingsCdnAccelerationPanel.vue [visual-engineering]
├── Task 15: SettingsPage.vue integration (tab + form + watcher) [visual-engineering]
└── Task 16: Frontend IPC bridge (cdn-api.ts) [quick]

Wave FINAL (After ALL tasks — 4 parallel reviews):
├── Task F1: Plan compliance audit (oracle)
├── Task F2: Code quality review (unspecified-high)
├── Task F3: Real manual QA (unspecified-high)
└── Task F4: Scope fidelity check (deep)
```

Critical Path: Task 1 → Task 5 → Task 8 → Task 10 → Task 11 → Task 16 → F1-F4
Parallel Speedup: ~60% faster than sequential
Max Concurrent: 5 (Waves 2 & 3)

### Dependency Matrix (FULL)

| Task | Depends On | Blocks |
|------|-----------|--------|
| 1 (Test infra) | — | 2-16 |
| 2 (Types) | 1 | 7, 9, 10, 11, 13, 14, 15, 16 |
| 3 (i18n) | — | 14 |
| 4 (Static IPs) | — | 5, 6 |
| 5 (IP fetch) | 4 | 8, 10 |
| 6 (TCP screen) | 4 | 8 |
| 7 (Throughput) | 2, 6 | 8 |
| 8 (Orchestrator) | 5, 6, 7 | 10 |
| 9 (Resolver) | 2 | 10 |
| 10 (Accelerator) | 8, 9 | 11, 13 |
| 11 (IPC cmds) | 10 | 12, 16 |
| 12 (Module reg) | 11 | 16 |
| 13 (Settings int) | 2, 10 | 14, 15 |
| 14 (Panel) | 3, 13 | 15 |
| 15 (Page int) | 14 | — |
| 16 (IPC bridge) | 11, 12 | — |
| F1-F4 | ALL | — |

### Agent Dispatch Summary

- **Wave 1**: **4** — T1 → `quick`, T2 → `quick`, T3 → `quick`, T4 → `quick`
- **Wave 2**: **5** — T5 → `unspecified-high`, T6 → `unspecified-high`, T7 → `unspecified-high`, T8 → `unspecified-high` (sequential within wave), T9 → `unspecified-high`
- **Wave 3**: **4** — T10 → `unspecified-high` (sequential), T11 → `unspecified-high`, T12 → `quick`, T13 → `quick`
- **Wave 4**: **3** — T14 → `visual-engineering`, T15 → `visual-engineering`, T16 → `quick`
- **FINAL**: **4** — F1 → `oracle`, F2 → `unspecified-high`, F3 → `unspecified-high`, F4 → `deep`



---

## TODOs

- [x] 1. Test Infrastructure Setup

  **What to do**:
  - Add vitest to `package.json` devDependencies: `vitest`, `@vue/test-utils`, `jsdom`, `@vitejs/plugin-vue`
  - Create `vitest.config.ts` at project root with jsdom environment, Vue plugin, and path aliases matching Vite config
  - Add `"test": "vitest run"` and `"test:watch": "vitest"` scripts to `package.json`
  - Create `src/__tests__/` directory with one smoke test: `smoke.test.ts` — mounts a minimal Vue component, asserts text content
  - Create `src-tauri/src/download/cdn/tests/` directory with one smoke test: `smoke.rs` — `#[tokio::test]` that asserts `2 + 2 == 4`
  - Add `cargo test --workspace` step to `.github/workflows/ci.yml` (after `cargo clippy`)
  - Add `bun run test` step to CI (after `bun run lint`)
  - Update `src-tauri/src/download/cdn/mod.rs` — create empty module file with `mod tests;` only
  - Run `bun run test` and `cargo test --workspace` to verify infrastructure works

  **Must NOT do**:
  - Do NOT add any feature-specific tests yet — infrastructure scaffolding only
  - Do NOT modify existing test files or production code

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Configuration file creation, no complex logic
  - **Skills**: [`vite`, `vue-testing-best-practices`]
    - `vite`: For vitest.config.ts that mirrors existing Vite config
    - `vue-testing-best-practices`: For correct @vue/test-utils setup with composition API

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 2, 3, 4)
  - **Blocks**: Tasks 5-16 (all implementation tasks need test infra)
  - **Blocked By**: None (can start immediately)

  **References**:
  - `package.json` — existing scripts pattern to follow for adding `test` script
  - `vite.config.ts` (root) — resolve aliases and plugin config to mirror in vitest.config.ts
  - `.github/workflows/ci.yml` — existing CI steps to insert test steps after
  - `src-tauri/Cargo.toml:50-52` — existing dev-dependencies (tempfile, ntest) to match test patterns

  **Acceptance Criteria**:
  - [ ] `bun run test` exits 0 (at least 1 test, 0 failures)
  - [ ] `cargo test --workspace` exits 0 (existing + new smoke test pass)
  - [ ] `vitest.config.ts` exists at project root
  - [ ] `src/__tests__/smoke.test.ts` exists and passes
  - [ ] `src-tauri/src/download/cdn/mod.rs` exists with `mod tests;`
  - [ ] CI workflow includes `cargo test --workspace` and `bun run test` steps

  **QA Scenarios**:

  ```
  Scenario: Vitest smoke test passes
    Tool: Bash
    Preconditions: vitest installed, vitest.config.ts created
    Steps:
      1. Run: bun run test
      2. Assert exit code is 0
      3. Assert stdout contains "1 passed" or similar success indicator
    Expected Result: All tests pass, exit code 0
    Failure Indicators: Non-zero exit code, "0 tests" (no tests found), test failure message
    Evidence: .sisyphus/evidence/task-1-vitest-smoke.txt

  Scenario: Cargo test smoke passes
    Tool: Bash (with MSVC environment)
    Preconditions: Rust nightly toolchain, cargo test available
    Steps:
      1. Run: cargo test --workspace
      2. Assert exit code is 0
      3. Assert stdout contains test result summary with 0 failures
    Expected Result: All Rust tests pass (existing + new smoke)
    Failure Indicators: Non-zero exit code, "FAILED" in output, compilation errors
    Evidence: .sisyphus/evidence/task-1-cargo-smoke.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-1-vitest-smoke.txt` — vitest run output
  - [ ] `task-1-cargo-smoke.txt` — cargo test output

  **Commit**: YES
  - Message: `test(infra): set up vitest + cargo test infrastructure`
  - Files: `vitest.config.ts`, `package.json`, `.github/workflows/ci.yml`, `src/__tests__/smoke.test.ts`, `src-tauri/src/download/cdn/mod.rs`, `src-tauri/src/download/cdn/tests/smoke.rs`

---

- [x] 2. CdnAccelerationSettings Types (Rust + TypeScript)

  **What to do**:
  - In `src-tauri/src/download/types.rs`:
    - Add `CdnAccelerationSettings` struct with fields: `enabled: bool` (default false), `active_ip: Option<String>`, `active_speed_mbps: Option<f64>`, `last_test_at_ms: Option<u64>`, `last_error: Option<String>`
    - All fields `#[serde(default)]` — backward compatible
    - Derive `Debug, Clone, Serialize, Deserialize` with `#[serde(rename_all = "camelCase")]`
    - Implement `Default` — all fields to their zero/None defaults
    - Add `cdn_acceleration: CdnAccelerationSettings` field to `AppSettings` with `#[serde(default)]`
    - Add to the settings JSON detection list in `load_settings()` at `manager.rs:~2482-2488`: add `|| value.get("cdnAcceleration").is_some()` to the existing chain
  - In `src/types/settings.ts`:
    - Add `CdnAccelerationSettings` interface matching Rust struct exactly (camelCase fields)
    - Add `cdnAcceleration: CdnAccelerationSettings` to `AppSettings` interface
  - Write Rust test: serialization round-trip, missing field defaults, old settings.json without field deserializes
  - Write vitest test: TypeScript interface matches expected shape

  **Must NOT do**:
  - Do NOT change existing field order or names in `AppSettings`
  - Do NOT add the field without `#[serde(default)]` — would break existing user settings

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Straightforward type definitions, minimal logic
  - **Skills**: []
    - No specialized skills needed for type definitions

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 3, 4)
  - **Blocks**: Tasks 9, 10, 11, 13, 14, 15 (all need the settings types)
  - **Blocked By**: Task 1 (need test infrastructure to write tests)

  **References**:
  - `src-tauri/src/download/types.rs:548-565` — `AppSettings` struct with existing `#[serde(default)]` fields — exact pattern to follow
  - `src-tauri/src/download/types.rs:567-580` — `Aria2RpcSettings` struct — template for new settings struct
  - `src-tauri/src/download/manager.rs:2480-2491` — `load_settings()` field detection chain — add `cdnAcceleration` check
  - `src/types/settings.ts:98-107` — `AppSettings` interface with existing sections — pattern for adding new field

  **Acceptance Criteria**:
  - [ ] `cargo test types::test_cdn_settings_serialization` → PASS
  - [ ] `cargo test types::test_cdn_settings_defaults` → PASS (missing field → defaults)
  - [ ] `bun run test` — vitest test for TS interface shape → PASS
  - [ ] `cargo check` — no compilation errors
  - [ ] `vue-tsc --noEmit` — no type errors

  **QA Scenarios**:

  ```
  Scenario: Serialization round-trip
    Tool: Bash (cargo test)
    Preconditions: Test infrastructure from Task 1
    Steps:
      1. Run: cargo test cdn_settings -- --nocapture
      2. Assert: test output shows "ok" for round-trip test
      3. Assert: test output shows "ok" for missing-field-defaults test
    Expected Result: Both tests pass
    Failure Indicators: Test failure, serde error, missing field panic
    Evidence: .sisyphus/evidence/task-2-serde-test.txt

  Scenario: Old settings.json without cdnAcceleration field parses
    Tool: Bash (cargo test)
    Preconditions: Test written that loads JSON without cdnAcceleration
    Steps:
      1. Run: cargo test cdn_settings -- --nocapture
      2. Assert: deserialized AppSettings has cdnAcceleration.enabled == false
    Expected Result: Backward compatible — defaults applied
    Failure Indicators: serde error about missing field
    Evidence: .sisyphus/evidence/task-2-backward-compat.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-2-serde-test.txt` — serialization test output
  - [ ] `task-2-backward-compat.txt` — backward compat test output

  **Commit**: YES
  - Message: `feat(types): add CdnAccelerationSettings to AppSettings`
  - Files: `src-tauri/src/download/types.rs`, `src-tauri/src/download/manager.rs` (load_settings only), `src/types/settings.ts`

---

- [x] 3. i18n Translation Keys

  **What to do**:
  - In `src/i18n/en-US.ts`:
    - Add `settings.cdnAcceleration` section under existing `settings` key:
      - `title`: "CDN Acceleration"
      - `description`: "Test Cloudflare edge nodes to find the fastest IP for downloads"
      - `enable`: "Enable Acceleration"
      - `status`: "Status"
      - `statusIdle`: "Idle"
      - `statusTesting`: "Testing..."
      - `statusReady`: "Ready"
      - `statusError`: "Error"
      - `triggerButton`: "Test and Accelerate"
      - `cancelButton`: "Cancel"
      - `clearButton`: "Clear"
      - `lastResult`: "Last Result"
      - `noResult`: "No speed test performed yet"
      - `bestIp`: "Best IP"
      - `speedMbps`: "Speed"
      - `testedAt`: "Tested at"
      - `dataWarning`: "This will download approximately 2GB of test data. Continue?"
      - `activeDownloadsWarning`: "Active downloads may be affected. Pause all downloads before testing?"
      - `noReachableNodes`: "No reachable Cloudflare edge nodes found"
    - Add `tokens.cdnAcceleration` section: (none needed — no enum selects for now)
  - In `src/i18n/zh-CN.ts`:
    - Add corresponding Chinese translations for ALL keys above
  - Run `vue-tsc --noEmit` to verify no i18n type errors

  **Must NOT do**:
  - Do NOT modify existing translation keys
  - Do NOT add keys that aren't used in the panel (Task 14)

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Translation key-value pairs, no logic
  - **Skills**: []
    - No specialized skills needed

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 2, 4)
  - **Blocks**: Task 14 (panel needs i18n keys)
  - **Blocked By**: None (can start immediately alongside others)

  **References**:
  - `src/i18n/en-US.ts` — existing settings section keys — follow exact nesting and naming convention
  - `src/i18n/zh-CN.ts` — existing Chinese translations — match structure exactly
  - `src/i18n/resources.ts` — i18next resource type definition

  **Acceptance Criteria**:
  - [ ] All en-US keys under `settings.cdnAcceleration.*` present and non-empty
  - [ ] All zh-CN keys under `settings.cdnAcceleration.*` present and non-empty
  - [ ] Key count matches between en-US and zh-CN
  - [ ] `vue-tsc --noEmit` passes (no "Property does not exist" errors)

  **QA Scenarios**:

  ```
  Scenario: All translation keys present in both locales
    Tool: Bash
    Preconditions: Task 1 test infra ready
    Steps:
      1. Run: bun run test -- -t "i18n"
      2. Assert: test verifies en-US and zh-CN have same key count for cdnAcceleration
    Expected Result: Key parity between locales, all values non-empty strings
    Failure Indicators: Missing key, empty translation string, key count mismatch
    Evidence: .sisyphus/evidence/task-3-i18n-parity.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-3-i18n-parity.txt` — i18n test output

  **Commit**: YES
  - Message: `feat(i18n): add CDN acceleration translation keys`
  - Files: `src/i18n/en-US.ts`, `src/i18n/zh-CN.ts`

---

- [x] 4. Cloudflare IPv4 Range Static Bundle

  **What to do**:
  - Create `src-tauri/src/download/cdn/ip_ranges.rs`
  - Define `CLOUDFLARE_IPV4_RANGES: &[&str]` — static slice of Cloudflare's ~15 IPv4 CIDRs (as of June 2026):
    - `173.245.48.0/20`, `103.21.244.0/22`, `103.22.200.0/22`, `103.31.4.0/22`, `141.101.64.0/18`, `108.162.192.0/18`, `190.93.240.0/20`, `188.114.96.0/20`, `197.234.240.0/22`, `198.41.128.0/17`, `162.158.0.0/15`, `104.16.0.0/13`, `104.24.0.0/14`, `172.64.0.0/13`, `131.0.72.0/22`
  - Define `expand_ipv4_cidrs(ranges: &[&str], samples_per_cidr: usize) -> Vec<Ipv4Addr>` — expand each CIDR to N sample IPs (default 3: network+1, network+2, network+3)
  - Write Rust test: `test_expand_cidrs` — verifies expansion produces valid IPv4 addresses, correct count (45 = 15 ranges × 3)
  - This is the **static fallback** — used when live fetch fails. Live fetch will be implemented in Task 5.

  **Must NOT do**:
  - Do NOT implement live HTTP fetching yet (Task 5 does that)
  - Do NOT include IPv6 ranges (scope-locked to IPv4 for v1)

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Simple data structure + parser, minimal logic
  - **Skills**: []
    - No specialized skills — std library only

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 2, 3)
  - **Blocks**: Task 5 (extends this), Task 8 (needs IP list for testing)
  - **Blocked By**: None (can start immediately)

  **References**:
  - `https://www.cloudflare.com/ips-v4` — live source to verify the static list
  - `src-tauri/src/download/sftp.rs:26-28` — `const` block pattern for module-level constants

  **Acceptance Criteria**:
  - [ ] `cargo test cdn::ip_ranges::test_expand_cidrs` → PASS
  - [ ] Expanded list produces exactly 45 IPv4 addresses (15 CIDRs × 3 samples each)
  - [ ] All expanded addresses are valid `Ipv4Addr`

  **QA Scenarios**:

  ```
  Scenario: CIDR expansion produces correct IPs
    Tool: Bash (cargo test)
    Preconditions: Task 1 test infra ready
    Steps:
      1. Run: cargo test ip_ranges -- --nocapture
      2. Assert: 45 IPs produced
      3. Assert: first three IPs: 173.245.48.1, 173.245.48.2, 173.245.48.3 (for 173.245.48.0/20)
    Expected Result: 45 valid IPv4 addresses, no panics
    Failure Indicators: Wrong count, invalid IP format, panic on bad CIDR
    Evidence: .sisyphus/evidence/task-4-cidr-test.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-4-cidr-test.txt` — test output

  **Commit**: YES
  - Message: `feat(cdn): add Cloudflare IPv4 static range bundle`
  - Files: `src-tauri/src/download/cdn/ip_ranges.rs`

---

- [x] 5. IP Range Fetching + Caching

  **What to do**:
  - In `src-tauri/src/download/cdn/ip_ranges.rs`, add:
    - `fetch_cloudflare_ipv4_ranges() -> Result<Vec<Ipv4Addr>>` — HTTP GET `https://www.cloudflare.com/ips-v4`, parse each line as CIDR, expand to first usable IP
    - Use `reqwest::get()` (NOT the shared download client — simple ad-hoc request, like `commands.rs:368`)
    - Parse: split by newline, filter non-empty, parse each as CIDR, expand to IP
    - `IpRangesCache` struct with: `ips: Vec<Ipv4Addr>`, `fetched_at: Instant`, `from_fallback: bool`
    - `get_ip_ranges() -> IpRangesCache` — try live fetch (timeout 10s), fall back to static bundle on failure, cache result
    - Use `tokio::sync::Mutex<IpRangesCache>` for thread-safe caching
  - Write tests:
    - `test_fetch_parse` — mock or integration test that fetches real Cloudflare IP page (integration test, may be skipped in CI)
    - `test_fallback_on_fetch_failure` — mock a failed request → static fallback used
    - `test_caching` — second call returns cached result without re-fetching

  **Must NOT do**:
  - Do NOT use the shared `DownloadManager` client — use ad-hoc `reqwest::get()`
  - Do NOT block initialization on fetch failure — always return something (live or fallback)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Network I/O with error handling, parsing, caching — moderate complexity
  - **Skills**: [`domain-web`]
    - `domain-web`: For correct HTTP request patterns and error handling

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 6, 7, 8, 9)
  - **Blocks**: Task 8 (needs IP list), Task 10 (needs caching infra)
  - **Blocked By**: Task 4 (static bundle foundation)

  **References**:
  - `src-tauri/src/download/cdn/ip_ranges.rs` — Task 4 static bundle to extend
  - `src-tauri/src/download/commands.rs:360-400` — `settings_fetch_tracker_list` — ad-hoc reqwest Client pattern
  - `https://www.cloudflare.com/ips-v4` — live endpoint format (plain text, one CIDR per line)

  **Acceptance Criteria**:
  - [ ] `cargo test cdn::ip_ranges::test_fetch_parse` → PASS (or skipped with reason)
  - [ ] `cargo test cdn::ip_ranges::test_fallback_on_fetch_failure` → PASS
  - [ ] `cargo test cdn::ip_ranges::test_caching` → PASS

  **QA Scenarios**:

  ```
  Scenario: Live fetch returns IPs from Cloudflare
    Tool: Bash (cargo test)
    Preconditions: Internet connectivity
    Steps:
      1. Run: cargo test ip_ranges::test_fetch -- --nocapture --ignored
      2. Assert: returns Vec with > 10 IPs
    Expected Result: 45 IPs returned from live fetch (15 CIDRs × 3 samples)
    Failure Indicators: Timeout, parse error, empty result
    Evidence: .sisyphus/evidence/task-5-fetch-test.txt

  Scenario: Fetch failure falls back to static bundle
    Tool: Bash (cargo test)
    Preconditions: Mocked request failure
    Steps:
      1. Run: cargo test ip_ranges::test_fallback -- --nocapture
      2. Assert: returns exactly 45 IPs (from static bundle)
    Expected Result: Static bundle used when live fetch fails
    Failure Indicators: Error propagated, wrong IP count
    Evidence: .sisyphus/evidence/task-5-fallback-test.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-5-fetch-test.txt`
  - [ ] `task-5-fallback-test.txt`

  **Commit**: YES
  - Message: `feat(cdn): implement IP range fetching with static fallback`
  - Files: `src-tauri/src/download/cdn/ip_ranges.rs`

---

- [x] 6. TCP Connect Latency Screening

  **What to do**:
  - Create `src-tauri/src/download/cdn/speed_test.rs`
  - Implement `measure_tcp_latency(addr: SocketAddr, timeout: Duration) -> Option<Duration>`:
    - Wrap `TcpStream::connect(addr)` with `tokio::time::timeout(timeout)`
    - Measure elapsed time from `Instant::now()` to successful connect
    - Return `None` on timeout or connection refused
    - Drop the stream immediately (no data exchange)
  - Implement `screen_candidates(ips: &[Ipv4Addr], concurrency: usize, connect_timeout: Duration) -> Vec<(Ipv4Addr, Duration)>`:
    - Use `tokio::task::JoinSet` for concurrent testing
    - Spawn up to `concurrency` tasks (default 50)
    - Collect results, sort by latency ascending, return top-N (default 5)
  - Write tests:
    - `test_measure_latency_to_localhost` — connect to 127.0.0.1, assert latency < 100ms
    - `test_measure_latency_unreachable` — connect to 192.0.2.1 (TEST-NET), assert None
    - `test_screen_candidates_concurrent` — test with 10 fake IPs, assert sorted results

  **Must NOT do**:
  - Do NOT use `unsafe` or raw sockets — tokio TcpStream only

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Concurrent network I/O with JoinSet management
  - **Skills**: [`m07-concurrency`]
    - `m07-concurrency`: For correct JoinSet usage and timeout patterns

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 5, 7, 8, 9)
  - **Blocks**: Task 8 (screening is phase 1 of orchestrator)
  - **Blocked By**: Task 4 (needs IP list)

  **References**:
  - `src-tauri/src/download/manager.rs:17-19` — `JoinSet` import and usage pattern
  - `src-tauri/src/download/sftp.rs:322-326` — `TcpStream::connect_timeout` pattern

  **Acceptance Criteria**:
  - [ ] `cargo test cdn::speed_test::test_measure_latency_to_localhost` → PASS
  - [ ] `cargo test cdn::speed_test::test_measure_latency_unreachable` → PASS
  - [ ] `cargo test cdn::speed_test::test_screen_candidates_concurrent` → PASS

  **QA Scenarios**:

  ```
  Scenario: TCP screening filters and sorts candidates
    Tool: Bash (cargo test)
    Preconditions: Task 1 test infra ready
    Steps:
      1. Run: cargo test cdn::speed_test::test_screen -- --nocapture
      2. Assert: results sorted by latency (ascending)
      3. Assert: unreachable IPs excluded
    Expected Result: Sorted candidate list, dead IPs filtered
    Evidence: .sisyphus/evidence/task-6-tcp-screen.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-6-tcp-screen.txt`

  **Commit**: YES
  - Message: `feat(cdn): implement TCP connect latency screening`
  - Files: `src-tauri/src/download/cdn/speed_test.rs`

---

- [x] 7. HTTPS Download Throughput Measurement

  **What to do**:
  - In `src-tauri/src/download/cdn/speed_test.rs`, add:
    - `measure_throughput(ip: Ipv4Addr, test_url: &str, hostname: &str, duration: Duration) -> Result<f64>`:
      - Build a **throwaway** `reqwest::Client` with `resolve_to_addrs(hostname, SocketAddr::new(IpAddr::V4(ip), 0))`
      - GET `test_url` with `timeout(duration)` wrapping the entire request
      - Measure bytes downloaded / elapsed seconds → MB/s
      - Default: `test_url = "https://speed.cloudflare.com/__down?bytes=200000000"` (200MB), `duration = 10s`
    - `SPEED_TEST_URL: &str` — constant
    - `SPEED_TEST_DURATION: Duration` — constant (10 seconds)
  - Write tests:
    - `test_throughput_to_localhost` — local axum server, assert throughput within expected range
    - `test_throughput_timeout` — non-responding IP, assert timeout after duration
    - `test_throughput_unreachable` — TEST-NET IP, assert error returned

  **Must NOT do**:
  - Do NOT reuse the throughput test client — each measurement uses a fresh throwaway Client
  - Do NOT use `danger_accept_invalid_certs(true)`

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Network I/O, TLS verification, throughput math, axum test server
  - **Skills**: [`domain-web`]
    - `domain-web`: For correct reqwest Client building and HTTP patterns

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 5, 6, 8, 9)
  - **Blocks**: Task 8 (throughput is phase 2 of orchestrator)
  - **Blocked By**: Task 2 (needs settings types), Task 6 (TCP screening results feed into this)

  **References**:
  - `src-tauri/src/download/manager.rs:2449-2470` — `build_http_client` — Client builder pattern
  - `src-tauri/src/download/manager.rs` — bottom ~579 lines — axum test server pattern

  **Acceptance Criteria**:
  - [ ] `cargo test cdn::speed_test::test_throughput_to_localhost` → PASS
  - [ ] `cargo test cdn::speed_test::test_throughput_timeout` → PASS
  - [ ] `cargo test cdn::speed_test::test_throughput_unreachable` → PASS

  **QA Scenarios**:

  ```
  Scenario: Throughput measurement against local test server
    Tool: Bash (cargo test)
    Preconditions: axum test server running
    Steps:
      1. Run: cargo test cdn::speed_test::test_throughput -- --nocapture
      2. Assert: measured throughput > 0 MB/s
      3. Assert: measurement completes within timeout + 2s buffer
    Expected Result: Valid throughput measurement
    Evidence: .sisyphus/evidence/task-7-throughput.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-7-throughput.txt`

  **Commit**: YES
  - Message: `feat(cdn): implement HTTPS download throughput measurement`
  - Files: `src-tauri/src/download/cdn/speed_test.rs`

---

- [x] 8. Speed Test Orchestrator

  **What to do**:
  - In `speed_test.rs`, add:
    - `SpeedTestConfig` struct: `concurrency: usize` (50), `tcp_timeout: Duration` (3s), `throughput_duration: Duration` (10s), `top_n_candidates: usize` (5)
    - `SpeedTestResult` struct: `ip: Ipv4Addr`, `tcp_latency_ms: f64`, `throughput_mbps: Option<f64>`, `error: Option<String>`
    - `run_speed_test(ips: &[Ipv4Addr], config: &SpeedTestConfig, on_progress: impl Fn(SpeedTestProgress)) -> Vec<SpeedTestResult>`:
      - Phase 1: `screen_candidates()` → top N by latency
      - Phase 2: For each top-N IP, concurrent `measure_throughput()`
      - Return results sorted by throughput descending
  - Write tests:
    - `test_orchestrator_with_mock_ips` — localhost IPs, assert phases execute, results sorted
    - `test_orchestrator_all_unreachable` — all dead IPs → empty results
    - `test_orchestrator_partial_failures` — mix reachable + unreachable

  **Must NOT do**:
  - Do NOT couple the orchestrator to Tauri IPC — accept a generic callback

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Two-phase concurrent pipeline with progress reporting
  - **Skills**: [`m07-concurrency`, `m06-error-handling`]
    - `m07-concurrency`: For JoinSet management across two phases
    - `m06-error-handling`: For partial failure handling

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on 6, 7)
  - **Parallel Group**: Wave 2 (last in wave — depends on 6 and 7)
  - **Blocks**: Task 10 (CdnAccelerator calls this)
  - **Blocked By**: Task 6 (TCP screening), Task 7 (throughput), Task 5 (IP list)

  **References**:
  - `src-tauri/src/download/cdn/speed_test.rs` — Tasks 6 and 7 functions

  **Acceptance Criteria**:
  - [ ] `cargo test cdn::speed_test::test_orchestrator_with_mock_ips` → PASS
  - [ ] `cargo test cdn::speed_test::test_orchestrator_all_unreachable` → PASS
  - [ ] `cargo test cdn::speed_test::test_orchestrator_partial_failures` → PASS

  **QA Scenarios**:

  ```
  Scenario: Full orchestrator pipeline
    Tool: Bash (cargo test)
    Preconditions: Tasks 6, 7 implemented
    Steps:
      1. Run: cargo test cdn::speed_test::test_orchestrator -- --nocapture
      2. Assert: Phase 1 completes before Phase 2
      3. Assert: results sorted by throughput descending
    Expected Result: Two-phase pipeline produces sorted results
    Evidence: .sisyphus/evidence/task-8-orchestrator.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-8-orchestrator.txt`

  **Commit**: YES
  - Message: `feat(cdn): implement speed test orchestrator`
  - Files: `src-tauri/src/download/cdn/speed_test.rs`

---

- [x] 9. DNS Override Resolver (Accelerated Client Builder)

  **What to do**:
  - Create `src-tauri/src/download/cdn/resolver.rs`
  - Implement `build_accelerated_client(domain: &str, ip: Ipv4Addr, settings: &AppSettings) -> Result<Client>`:
    - Create a NEW `reqwest::Client` (NOT modifying the shared one)
    - Copy settings from `build_http_client()`: `tcp_nodelay(true)`, `read_timeout(15s)`, `user_agent(settings.download.default_user_agent)`, `redirect(Policy::limited(10))`, proxy config
    - Call `.resolve_to_addrs(domain, &[SocketAddr::new(IpAddr::V4(ip), 0)])`
    - Build and return
  - Implement `is_cloudflare_domain(url: &str) -> bool` — for v1: return `true` (user controls via toggle)
  - Write tests:
    - `test_build_accelerated_client` — build, verify no panic
    - `test_accelerated_client_uses_correct_ip` — localhost axum test, verify routing
    - `test_is_cloudflare_domain` — various URLs

  **Must NOT do**:
  - Do NOT modify the shared `DownloadManager` client
  - Do NOT use `danger_accept_invalid_certs(true)`

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: reqwest Client building with DNS override, integration testing
  - **Skills**: [`domain-web`]
    - `domain-web`: For reqwest Client configuration

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 5, 6, 7, 8)
  - **Blocks**: Task 10 (CdnAccelerator uses this)
  - **Blocked By**: Task 2 (needs AppSettings)

  **References**:
  - `src-tauri/src/download/manager.rs:2449-2470` — `build_http_client` — settings mirroring

  **Acceptance Criteria**:
  - [ ] `cargo test cdn::resolver::test_build_accelerated_client` → PASS
  - [ ] `cargo test cdn::resolver::test_accelerated_client_uses_correct_ip` → PASS
  - [ ] Built client does NOT have invalid certs setting

  **QA Scenarios**:

  ```
  Scenario: Accelerated client routes to specified IP
    Tool: Bash (cargo test)
    Preconditions: axum test server on localhost
    Steps:
      1. Build accelerated client with resolve_to_addrs("test.local", 127.0.0.1)
      2. GET to test server, assert response received
    Expected Result: Request routed through overridden IP
    Evidence: .sisyphus/evidence/task-9-resolver-test.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-9-resolver-test.txt`

  **Commit**: YES
  - Message: `feat(cdn): implement accelerated reqwest Client builder`
  - Files: `src-tauri/src/download/cdn/resolver.rs`

---

- [x] 10. CdnAccelerator Struct (State Management + Lifecycle)

  **What to do**:
  - Create `src-tauri/src/download/cdn/accelerator.rs`
  - Implement `CdnAccelerator` struct:
    - Fields: `state: RwLock<AccelState>` (Idle/Testing/Ready/Error), `active_ip: RwLock<Option<Ipv4Addr>>`, `active_speed_mbps: RwLock<Option<f64>>`, `last_test_at: RwLock<Option<Instant>>`, `cancel_token: RwLock<Option<CancellationToken>>`, `accelerated_client: RwLock<Option<Client>>`
    - `AccelState` enum: `Idle`, `Testing { phase: TestPhase, progress: f64 }`, `Ready`, `Error(String)`
    - `TestPhase` enum: `Screening`, `Throughput`
    - `new() -> Self` — initialize all fields to default/empty
    - `start_test(&self) -> Result<()>` — spawns background `run_speed_test()`, updates state, stores cancel token
    - `cancel_test(&self)` — triggers CancellationToken, sets state to Idle
    - `status(&self) -> AccelStatus` — returns current state snapshot
    - `apply_ip(&self, ip: Ipv4Addr, speed_mbps: f64) -> Result<Client>` — calls `build_accelerated_client()`, stores client, updates state to Ready
    - `clear(&self)` — resets all state to Idle, drops accelerated client
    - `get_client(&self, url: &str) -> Option<Client>` — if Ready and domain is Cloudflare, return clone of accelerated client
    - Auto-clear on error: if speed test fails, set state to Error with message
  - Write tests:
    - `test_lifecycle` — new → start_test → Idle → Testing → Ready → clear → Idle
    - `test_cancel` — start_test → cancel → Idle (no Ready transition)
    - `test_apply_then_get_client` — apply IP → get_client returns Some(Client) for Cloudflare URL
    - `test_get_client_non_cloudflare` — non-Cloudflare URL → get_client returns None
    - `test_clear_drops_client` — apply IP → clear → get_client returns None

  **Must NOT do**:
  - Do NOT store the accelerator in `DownloadManager` — it's a standalone struct managed via `AppState`
  - Do NOT use `Mutex` from std where async is needed — use `tokio::sync::RwLock`

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: State machine with async cancellation, RwLock management
  - **Skills**: [`m07-concurrency`, `m12-lifecycle`]
    - `m07-concurrency`: For CancellationToken and RwLock patterns
    - `m12-lifecycle`: For RAII cleanup and state transitions

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on 8, 9)
  - **Parallel Group**: Wave 3 (first in wave — foundation for IPC)
  - **Blocks**: Task 11 (IPC commands wrap this)
  - **Blocked By**: Task 8 (speed test), Task 9 (resolver)

  **References**:
  - `src-tauri/src/download/manager.rs:691-698` — `CancellationToken` usage pattern in `spawn_download`
  - `src-tauri/src/download/manager.rs:17-19` — `tokio::sync::RwLock` import
  - `src-tauri/src/download/torrent.rs` — `impl TorrentManager` — Arc-based manager pattern

  **Acceptance Criteria**:
  - [ ] `cargo test cdn::accelerator::test_lifecycle` → PASS
  - [ ] `cargo test cdn::accelerator::test_cancel` → PASS
  - [ ] `cargo test cdn::accelerator::test_apply_then_get_client` → PASS
  - [ ] `cargo test cdn::accelerator::test_get_client_non_cloudflare` → PASS
  - [ ] `cargo test cdn::accelerator::test_clear_drops_client` → PASS

  **QA Scenarios**:

  ```
  Scenario: Full lifecycle: idle → test → ready → clear
    Tool: Bash (cargo test)
    Preconditions: Tasks 8, 9 implemented
    Steps:
      1. Run: cargo test cdn::accelerator::test_lifecycle -- --nocapture
      2. Assert: initial state is Idle
      3. Assert: after start_test, transitions to Testing
      4. Assert: on completion, transitions to Ready
      5. Assert: after clear, returns to Idle
    Expected Result: State machine transitions correctly
    Evidence: .sisyphus/evidence/task-10-lifecycle.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-10-lifecycle.txt`

  **Commit**: YES
  - Message: `feat(cdn): implement CdnAccelerator state machine`
  - Files: `src-tauri/src/download/cdn/accelerator.rs`

---

- [x] 11. Tauri IPC Commands

  **What to do**:
  - In `src-tauri/src/download/cdn/commands.rs` (NEW file), implement 6 `#[tauri::command]` functions:
    - `cdn_fetch_ranges(state: State<AppState>) -> Result<Vec<String>>` — returns Cloudflare IPv4 ranges (CIDR strings)
    - `cdn_test(state: State<AppState>, on_event: Channel<AccelerationEvent>) -> Result<()>` — runs speed test, streams progress via Channel
    - `cdn_apply(state: State<AppState>, ip: String, speed_mbps: f64) -> Result<()>` — applies selected IP, builds accelerated client
    - `cdn_clear(state: State<AppState>) -> Result<()>` — clears acceleration state
    - `cdn_status(state: State<AppState>) -> Result<CdnAccelStatus>` — returns current state snapshot
    - `cdn_cancel(state: State<AppState>) -> Result<()>` — cancels running speed test
  - Define `AccelerationEvent` enum (already planned) with `#[serde(tag = "event", content = "data")]`:
    - `Started { ip_count: usize }`, `ScreeningProgress { current: usize, total: usize }`, `ThroughputProgress { current: usize, total: usize, best_ip: Option<String>, best_speed: Option<f64> }`, `Completed { best_ip: String, speed_mbps: f64 }`, `Error { message: String }`
  - Define `CdnAccelStatus` struct: `state: String`, `active_ip: Option<String>`, `active_speed_mbps: Option<f64>`, `last_test_at_ms: Option<u64>`, `last_error: Option<String>`
  - Add `cdn_accelerator: Arc<CdnAccelerator>` to `AppState` in `manager.rs`
  - Write tests:
    - `test_cdn_status_idle` — initial state is Idle
    - `test_cdn_fetch_ranges` — returns non-empty list
    - `test_cdn_test_and_apply` — integration test: test → apply → status shows Ready
    - `test_cdn_cancel` — start test → cancel → status returns to Idle

  **Must NOT do**:
  - Do NOT add these commands to `commands.rs` — they go in `cdn/commands.rs`
  - Do NOT register commands in `lib.rs` without re-exporting from `mod.rs`

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: 6 IPC commands with Channel streaming, state management, integration testing
  - **Skills**: [`domain-web`]
    - `domain-web`: For Tauri command patterns and IPC design

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on 10)
  - **Parallel Group**: Wave 3 (with Tasks 12, 13)
  - **Blocks**: Task 16 (frontend IPC bridge wraps these)
  - **Blocked By**: Task 10 (CdnAccelerator)

  **References**:
  - `src-tauri/src/download/commands.rs:83-100` — `#[tauri::command]` and `State<AppState>` pattern
  - `src-tauri/src/download/manager.rs:58-65` — `AppState` struct (to add `cdn_accelerator` field)
  - `src-tauri/src/download/aria2_rpc.rs` — IPC command in separate module pattern

  **Acceptance Criteria**:
  - [ ] `cargo test cdn::commands::test_cdn_status_idle` → PASS
  - [ ] `cargo test cdn::commands::test_cdn_fetch_ranges` → PASS
  - [ ] `cargo test cdn::commands::test_cdn_test_and_apply` → PASS
  - [ ] `cargo test cdn::commands::test_cdn_cancel` → PASS
  - [ ] All 6 commands return `Result<T, String>` (Tauri command convention)

  **QA Scenarios**:

  ```
  Scenario: Full IPC flow: fetch → test → apply → status
    Tool: Bash (cargo test)
    Preconditions: CdnAccelerator wired into AppState
    Steps:
      1. Run: cargo test cdn::commands::test_full_flow -- --nocapture
      2. Assert: cdn_fetch_ranges returns 15 CIDRs
      3. Assert: cdn_test returns Ok, cdn_status shows Testing
      4. Assert: after test completes, cdn_apply succeeds
      5. Assert: cdn_status shows Ready with active IP
    Expected Result: All 6 commands work end-to-end
    Evidence: .sisyphus/evidence/task-11-ipc-flow.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-11-ipc-flow.txt`

  **Commit**: YES
  - Message: `feat(cdn): implement Tauri IPC commands for CDN acceleration`
  - Files: `src-tauri/src/download/cdn/commands.rs`, `src-tauri/src/download/manager.rs` (AppState field only), `src-tauri/src/download/cdn/mod.rs`

---

- [x] 12. Module Registration (mod.rs + lib.rs Wiring)

  **What to do**:
  - In `src-tauri/src/download/cdn/mod.rs`:
    - Add `mod accelerator; mod commands; mod ip_ranges; mod resolver; mod speed_test; mod types;` (types.rs for CDN-specific types if any)
    - Re-export public API: `pub use accelerator::CdnAccelerator;`, `pub use commands::*;`
  - In `src-tauri/src/download/mod.rs`:
    - Add `mod cdn;` to existing module declarations
    - Add `pub use cdn::CdnAccelerator;` to re-exports
  - In `src-tauri/src/lib.rs`:
    - Register 6 new commands: `cdn_fetch_ranges`, `cdn_test`, `cdn_apply`, `cdn_clear`, `cdn_status`, `cdn_cancel`
    - Add `use downloader_lib::download::cdn::*;` or use qualified paths
    - Initialize `CdnAccelerator` in `run()` and add to `AppState`
  - In `src-tauri/src/download/manager.rs`:
    - Add `pub cdn_accelerator: Arc<CdnAccelerator>` field to `AppState`
    - Initialize in the `run()` function (or wherever `AppState` is constructed)
    - **ONLY this one field addition** — no other changes to manager.rs

  **Must NOT do**:
  - Do NOT add any other code to `manager.rs` beyond the `AppState` field
  - Do NOT forget to register ANY of the 6 commands

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Module declarations and imports — straightforward wiring
  - **Skills**: []
    - No specialized skills needed

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Tasks 11, 13)
  - **Blocks**: Task 16 (frontend needs registered commands)
  - **Blocked By**: Task 11 (commands defined), Task 10 (CdnAccelerator defined)

  **References**:
  - `src-tauri/src/download/mod.rs:1-37` — module declarations pattern
  - `src-tauri/src/lib.rs` — command registration pattern (13 existing commands)
  - `src-tauri/src/download/aria2_rpc.rs` — example of separate module with own commands

  **Acceptance Criteria**:
  - [ ] `cargo check` — no compilation errors
  - [ ] `cargo clippy -- -D warnings` — zero warnings
  - [ ] All 6 commands appear in `tauri::Builder::default().invoke_handler(tauri::generate_handler![...])`

  **QA Scenarios**:

  ```
  Scenario: All 6 commands are registered and invocable
    Tool: Bash (cargo check)
    Preconditions: Task 11 complete
    Steps:
      1. Run: cargo check
      2. Assert: exit code 0, no errors
      3. Run: cargo clippy -- -D warnings
      4. Assert: exit code 0, no warnings
    Expected Result: Clean compilation with all commands registered
    Failure Indicators: Unresolved import, missing command registration, clippy errors
    Evidence: .sisyphus/evidence/task-12-cargo-check.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-12-cargo-check.txt`

  **Commit**: YES
  - Message: `feat(cdn): register CDN module and IPC commands`
  - Files: `src-tauri/src/download/cdn/mod.rs`, `src-tauri/src/download/mod.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/download/manager.rs` (AppState field only)

---

- [x] 13. Settings Integration (AppSettings Field + Backward Compat)

  **What to do**:
  - Verify the `CdnAccelerationSettings` field added in Task 2 integrates correctly:
    - Confirm `AppSettings` in `types.rs` has `cdn_acceleration: CdnAccelerationSettings` with `#[serde(default)]`
    - Confirm `load_settings()` in `manager.rs` has the `cdnAcceleration` check in the JSON detection chain
    - Confirm `normalize_settings()` handles the new field (should pass through since it has Default)
  - Add `sync_settings_to_accelerator()` function in `manager.rs` or `cdn/` module:
    - When `settings.cdn_acceleration.enabled` is toggled ON, load `active_ip`/`active_speed_mbps` from settings (if any)
    - When toggled OFF, clear the accelerator
    - Called from `update_settings()` in `DownloadManager`
  - Write test: `test_settings_round_trip_with_cdn` — serialize AppSettings with CdnAccelerationSettings, deserialize, assert field preserved

  **Must NOT do**:
  - Do NOT add more than 5 lines to `manager.rs` — just the sync call in `update_settings()`
  - Do NOT break existing settings.json files

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Settings plumbing, backward compat verification
  - **Skills**: []
    - No specialized skills needed

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Tasks 11, 12)
  - **Blocks**: Task 14 (panel needs settings structure), Task 15 (SettingsPage needs settings field)
  - **Blocked By**: Task 2 (types defined), Task 10 (accelerator exists)

  **References**:
  - `src-tauri/src/download/types.rs:548-565` — `AppSettings` struct
  - `src-tauri/src/download/manager.rs:210-228` — `update_settings()` — where sync call goes
  - `src-tauri/src/download/manager.rs:2472-2500` — `load_settings()` — JSON detection chain

  **Acceptance Criteria**:
  - [ ] `cargo test settings_round_trip_with_cdn` → PASS
  - [ ] Old `settings.json` (no cdnAcceleration field) parses without error
  - [ ] `update_settings()` propagates CDN enable/disable to accelerator
  - [ ] `cargo clippy` clean

  **QA Scenarios**:

  ```
  Scenario: Settings backward compatibility
    Tool: Bash (cargo test)
    Preconditions: Task 2 types defined
    Steps:
      1. Run: cargo test settings_round_trip -- --nocapture
      2. Assert: old JSON parses, new field defaults to disabled
      3. Assert: new JSON with cdnAcceleration survives round-trip
    Expected Result: Backward compatible, no data loss
    Evidence: .sisyphus/evidence/task-13-settings-compat.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-13-settings-compat.txt`

  **Commit**: YES
  - Message: `feat(cdn): integrate acceleration settings with DownloadManager`
  - Files: `src-tauri/src/download/manager.rs` (update_settings only), `src-tauri/src/download/cdn/accelerator.rs`

---

- [ ] 14. SettingsCdnAccelerationPanel.vue

  **What to do**:
  - Create `src/components/settings/SettingsCdnAccelerationPanel.vue`
  - Follow the `SettingsAria2RpcPanel.vue` pattern exactly:
    - `<script setup lang="ts">` with `defineProps<{ draft: AppSettings; t: Function; ... }>()`
    - Enable/disable toggle using `settings-toggle` / `settings-toggle--active` CSS classes (matching existing pattern)
    - Status display area: shows `Idle` / `Testing...` / `Ready (IP: x.x.x.x, Speed: N MB/s)` / `Error: message`
    - "Test and Accelerate" button — disabled when already testing or downloads active
    - "Cancel" button — visible only during testing
    - "Clear" button — visible when Ready, resets to Idle
    - Last result display: best IP, speed, tested timestamp
    - Data usage warning modal/confirmation before starting test
  - Use i18n keys from Task 3: `t("settings.cdnAcceleration.*")`
  - Emitting: panel directly mutates `draft.cdnAcceleration.*` (matching existing pattern — no emit needed)
  - Progress display: real-time progress from Channel during testing

  **Must NOT do**:
  - Do NOT use Pinia or Options API — Composition API with `<script setup>` only
  - Do NOT add the panel to SettingsPage.vue yet (Task 15 does that)
  - Do NOT hardcode Chinese text — use `t()` for all strings

  **Recommended Agent Profile**:
  - **Category**: `visual-engineering`
    - Reason: Vue SFC with CSS styling, toggle buttons, progress display
  - **Skills**: [`vue`, `vue-best-practices`]
    - `vue`: For Composition API, defineProps, template syntax
    - `vue-best-practices`: For script setup patterns

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 4 (with Tasks 15, 16)
  - **Blocks**: Task 15 (SettingsPage imports this component)
  - **Blocked By**: Task 3 (i18n keys), Task 13 (settings structure)

  **References**:
  - `src/components/settings/SettingsAria2RpcPanel.vue` — exact panel template to follow
  - `src/components/settings/SettingsBtPanel.vue` — toggle button pattern with `settings-toggle` classes
  - `src/components/settings/SettingsPage.vue:54-78` — option arrays pattern
  - `src/components/AGENTS.md:18-30` — component conventions

  **Acceptance Criteria**:
  - [ ] Panel renders without errors (vitest component test)
  - [ ] Toggle toggles `draft.cdnAcceleration.enabled`
  - [ ] "Test and Accelerate" button disabled during testing
  - [ ] Cancel button appears during testing, clears on completion
  - [ ] All text uses i18n `t()` calls
  - [ ] `vue-tsc --noEmit` passes

  **QA Scenarios**:

  ```
  Scenario: Panel renders in Idle state
    Tool: Playwright
    Preconditions: Settings page open, CDN tab selected
    Steps:
      1. Navigate to Settings → CDN Acceleration tab
      2. Assert: toggle shows acceleration is disabled
      3. Assert: status shows "Idle"
      4. Assert: "Test and Accelerate" button is enabled
      5. Assert: "Cancel" button not visible
    Expected Result: Panel renders correctly in initial state
    Evidence: .sisyphus/evidence/task-14-panel-idle.png

  Scenario: Toggle enables acceleration
    Tool: Playwright
    Preconditions: CDN Acceleration panel visible
    Steps:
      1. Click the enable toggle
      2. Assert: toggle state changes to active (CSS class settings-toggle--active)
      3. Assert: settings dirty indicator appears
    Expected Result: Toggle works, dirty state tracked
    Evidence: .sisyphus/evidence/task-14-toggle-enabled.png
  ```

  **Evidence to Capture**:
  - [ ] `task-14-panel-idle.png` — screenshot of idle panel
  - [ ] `task-14-toggle-enabled.png` — screenshot after toggle

  **Commit**: YES
  - Message: `feat(ui): create CDN acceleration settings panel`
  - Files: `src/components/settings/SettingsCdnAccelerationPanel.vue`

---

- [ ] 15. SettingsPage.vue Integration

  **What to do**:
  - In `src/components/settings/SettingsPage.vue`:
    - Import `SettingsCdnAccelerationPanel` component
    - Add `cdnAcceleration: { enabled: false, activeIp: null, activeSpeedMbps: null, lastTestAtMs: null, lastError: null }` to the `reactive<AppSettings>({...})` form defaults
    - Add field sync in `watch(props.settings, ...)` handler: `form.cdnAcceleration = { ...props.settings.cdnAcceleration }` (or deep copy)
    - Add `cdnAcceleration: { ...form.cdnAcceleration }` to `buildSettingsPayload()`
    - Add tab entry: `{ id: "cdnAcceleration", label: t("settings.cdnAcceleration.title"), icon: "i-ri-speed-up-line" }`
    - Add template: `<SettingsCdnAccelerationPanel v-show="activeTab === 'cdnAcceleration'" :draft="form" :t="t" />`
  - Update `useSettingsSummaries.ts`:
    - Add `cdnAccelerationSummary` computed property: shows "Ready: X MB/s" or "Idle" or "Error"
    - Pass summary as prop to the panel
  - Write vitest test: `test_settings_page_has_cdn_tab` — SettingsPage renders with CDN tab

  **Must NOT do**:
  - Do NOT change existing tab layout or order (add CDN tab after existing tabs)
  - Do NOT break existing dirty tracking logic

  **Recommended Agent Profile**:
  - **Category**: `visual-engineering`
    - Reason: Vue component integration, reactive form wiring, tab management
  - **Skills**: [`vue`, `vue-best-practices`]
    - `vue`: For reactive form and watch patterns
    - `vue-best-practices`: For Composition API conventions

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 4 (with Tasks 14, 16)
  - **Blocks**: None (final frontend task)
  - **Blocked By**: Task 14 (panel component), Task 13 (settings structure)

  **References**:
  - `src/components/settings/SettingsPage.vue:23-30` — panel imports
  - `src/components/settings/SettingsPage.vue:1-80` — reactive form and watch pattern
  - `src/components/settings/SettingsPage.vue:200-250` — tab definitions and template
  - `src/components/settings/useSettingsSummaries.ts` — summary computed properties

  **Acceptance Criteria**:
  - [ ] `bun run test -- -t "settings page has CDN tab"` → PASS
  - [ ] CDN tab appears in settings navigation
  - [ ] Panel toggles correctly integrate with dirty tracking
  - [ ] Save persists `cdnAcceleration` settings
  - [ ] `vue-tsc --noEmit` passes

  **QA Scenarios**:

  ```
  Scenario: CDN tab appears and is navigable
    Tool: Playwright
    Preconditions: Settings page open
    Steps:
      1. Navigate to Settings page
      2. Assert: CDN Acceleration tab visible in tab bar
      3. Click CDN Acceleration tab
      4. Assert: CDN Acceleration panel content renders
      5. Assert: tab is highlighted as active
    Expected Result: New tab navigable, panel renders
    Evidence: .sisyphus/evidence/task-15-cdn-tab.png

  Scenario: Settings save includes CDN acceleration
    Tool: Playwright
    Preconditions: CDN panel visible
    Steps:
      1. Enable CDN acceleration toggle
      2. Click Save button
      3. Assert: success notification appears
      4. Reload settings (invoke settings_get)
      5. Assert: cdnAcceleration.enabled is true
    Expected Result: CDN settings persist across save/load
    Evidence: .sisyphus/evidence/task-15-save-persist.png
  ```

  **Evidence to Capture**:
  - [ ] `task-15-cdn-tab.png`
  - [ ] `task-15-save-persist.png`

  **Commit**: YES
  - Message: `feat(ui): integrate CDN acceleration into SettingsPage`
  - Files: `src/components/settings/SettingsPage.vue`, `src/components/settings/useSettingsSummaries.ts`

---

- [ ] 16. Frontend IPC Bridge (cdn-api.ts)

  **What to do**:
  - Create `src/lib/tauri/cdn-api.ts`
  - Implement 6 IPC bridge functions matching the 6 Rust commands:
    - `fetchCloudflareRanges(): Promise<string[]>` — invokes `cdn_fetch_ranges`
    - `testAcceleration(onEvent: Channel<AccelerationEvent>): Promise<void>` — invokes `cdn_test` with Channel
    - `applyAcceleration(ip: string, speedMbps: number): Promise<void>` — invokes `cdn_apply`
    - `clearAcceleration(): Promise<void>` — invokes `cdn_clear`
    - `getAccelerationStatus(): Promise<CdnAccelStatus>` — invokes `cdn_status`
    - `cancelAcceleration(): Promise<void>` — invokes `cdn_cancel`
  - Define TypeScript types matching Rust types:
    - `AccelerationEvent` tagged union: `{ event: "started"; data: { ipCount: number } } | { event: "screeningProgress"; data: ... } | ...`
    - `CdnAccelStatus` interface: `state: string; activeIp: string | null; ...`
  - Export all functions and types
  - Write vitest test: mock `invoke()` from `@tauri-apps/api/core`, verify each function calls correct command with correct args

  **Must NOT do**:
  - Do NOT import from `@tauri-apps/api/core` in test — mock it
  - Do NOT add functions to `settings-api.ts` — separate file for CDN-specific IPC

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Thin IPC wrapper functions, no complex logic
  - **Skills**: []
    - No specialized skills needed

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 4 (with Tasks 14, 15)
  - **Blocks**: None (final task before verification)
  - **Blocked By**: Task 11 (IPC commands defined), Task 2 (types)

  **References**:
  - `src/lib/tauri/settings-api.ts` — exact pattern to follow (invoke, types, exports)
  - `src/lib/tauri/download-api.ts` — Channel usage pattern if any
  - `src/types/settings.ts` — TS type conventions

  **Acceptance Criteria**:
  - [ ] `bun run test -- -t "cdn-api"` → PASS (mocked invoke calls verified)
  - [ ] All 6 functions exported
  - [ ] TypeScript types match Rust `AccelerationEvent` and `CdnAccelStatus` exactly
  - [ ] `vue-tsc --noEmit` passes

  **QA Scenarios**:

  ```
  Scenario: IPC bridge calls correct commands
    Tool: Bash (bun test)
    Preconditions: Mocked @tauri-apps/api/core
    Steps:
      1. Run: bun run test -- -t "cdn-api"
      2. Assert: fetchCloudflareRanges calls invoke("cdn_fetch_ranges")
      3. Assert: testAcceleration calls invoke("cdn_test") with Channel
      4. Assert: all 6 functions map to correct command names
    Expected Result: All IPC wrappers correctly typed and routed
    Evidence: .sisyphus/evidence/task-16-ipc-bridge.txt
  ```

  **Evidence to Capture**:
  - [ ] `task-16-ipc-bridge.txt`

  **Commit**: YES
  - Message: `feat(ui): add CDN acceleration IPC bridge`
  - Files: `src/lib/tauri/cdn-api.ts`

---

## Final Verification Wave (MANDATORY — after ALL implementation tasks)

> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.

- [ ] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists (read file, cargo test, bun test). For each "Must NOT Have": search codebase for forbidden patterns — reject with file:line if found. Check evidence files exist in `.sisyphus/evidence/`. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [16/16] | VERDICT: APPROVE/REJECT`

- [ ] F2. **Code Quality Review** — `unspecified-high`
  Run `cargo clippy -- -D warnings` + `cargo check` + `vue-tsc --noEmit` + `bun run lint` + `bun run test` + `cargo test --workspace`. Review all changed files for: `as any`/`@ts-ignore`, empty catches, `console.log` in prod, commented-out code, unused imports. Check AI slop: excessive comments, over-abstraction, generic names (data/result/item/temp). Check `manager.rs` modification is ONLY the `AppState` field addition.
  Output: `Build [PASS/FAIL] | Lint [PASS/FAIL] | Tests [N pass/N fail] | Files [N clean/N issues] | VERDICT`

- [ ] F3. **Real Manual QA** — `unspecified-high` (+ `playwright` skill if UI)
  Start from clean state. Execute EVERY QA scenario from EVERY task — follow exact steps, capture evidence. Test cross-task integration (speed test → apply → download uses accelerated IP). Test edge cases: cancel during test, all IPs dead, settings backward compat. Save to `.sisyphus/evidence/final-qa/`.
  Output: `Scenarios [N/N pass] | Integration [N/N] | Edge Cases [N tested] | VERDICT`

- [ ] F4. **Scope Fidelity Check** — `deep`
  For each task: read "What to do", read actual diff (git log/diff). Verify 1:1 — everything in spec was built (no missing), nothing beyond spec was built (no creep). Check "Must NOT do" compliance. Detect cross-task contamination: Task N touching Task M's files. Flag unaccounted changes. Especially check `manager.rs` — must NOT have more than the `AppState` field added.
  Output: `Tasks [16/16 compliant] | Contamination [CLEAN/N issues] | Unaccounted [CLEAN/N files] | VERDICT`

---

## Commit Strategy

| # | Message | Files |
|---|---------|-------|
| 1 | `test(infra): set up vitest + cargo test infrastructure` | vitest.config.ts, package.json, ci.yml, smoke tests |
| 2 | `feat(types): add CdnAccelerationSettings to AppSettings` | types.rs (Rust), settings.ts (TS), manager.rs (load_settings) |
| 3 | `feat(i18n): add CDN acceleration translation keys` | en-US.ts, zh-CN.ts |
| 4 | `feat(cdn): add Cloudflare IPv4 static range bundle` | cdn/ip_ranges.rs |
| 5 | `feat(cdn): implement IP range fetching with static fallback` | cdn/ip_ranges.rs |
| 6 | `feat(cdn): implement TCP connect latency screening` | cdn/speed_test.rs |
| 7 | `feat(cdn): implement HTTPS download throughput measurement` | cdn/speed_test.rs |
| 8 | `feat(cdn): implement speed test orchestrator` | cdn/speed_test.rs |
| 9 | `feat(cdn): implement accelerated reqwest Client builder` | cdn/resolver.rs |
| 10 | `feat(cdn): implement CdnAccelerator state machine` | cdn/accelerator.rs |
| 11 | `feat(cdn): implement Tauri IPC commands for CDN acceleration` | cdn/commands.rs, cdn/mod.rs, manager.rs (AppState) |
| 12 | `feat(cdn): register CDN module and IPC commands` | cdn/mod.rs, download/mod.rs, lib.rs, manager.rs (AppState) |
| 13 | `feat(cdn): integrate acceleration settings with DownloadManager` | manager.rs (update_settings), cdn/accelerator.rs |
| 14 | `feat(ui): create CDN acceleration settings panel` | SettingsCdnAccelerationPanel.vue |
| 15 | `feat(ui): integrate CDN acceleration into SettingsPage` | SettingsPage.vue, useSettingsSummaries.ts |
| 16 | `feat(ui): add CDN acceleration IPC bridge` | cdn-api.ts |

## Success Criteria

### Verification Commands
```bash
# Rust — all tests pass
cargo test --workspace
# Expected: test result: ok. N passed; 0 failed

# Rust — lint clean
cargo clippy -- -D warnings
# Expected: exit code 0

# Frontend — tests pass
bun run test
# Expected: Tests N passed (N)

# Frontend — type check
bunx vue-tsc --noEmit
# Expected: exit code 0

# Frontend — lint
bun run lint
# Expected: exit code 0
```

### Final Checklist
- [ ] All 10 "Must Have" requirements present and verifiable
- [ ] All 10 "Must NOT Have" guardrails respected (zero violations)
- [ ] 16 implementation tasks complete with evidence
- [ ] `cargo test --workspace` → all tests pass
- [ ] `bun run test` → all vitest tests pass
- [ ] `cargo clippy -- -D warnings` → zero warnings
- [ ] `vue-tsc --noEmit` → zero type errors
- [ ] `manager.rs` modification limited to `AppState` field + `update_settings()` sync call only
- [ ] Empty `cdn/` directory now contains 6+ Rust files with tests
- [ ] Settings page has 9th tab "CDN Acceleration" with full functionality
- [ ] Old `settings.json` files (no `cdnAcceleration` field) parse without error
- [ ] No `unsafe` code added
- [ ] No `danger_accept_invalid_certs` anywhere
- [ ] F1-F4 all return APPROVE









