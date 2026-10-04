---
type: guide
title: limedl Wiki Quickstart
description: Entry point and task-routing map for the limedl wiki — where to start for building, understanding the engine, adding a backend, debugging downloads, testing and shipping a release, plus the repository's load-bearing invariants.
tags: [quickstart, navigation, onboarding, build, workflow]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T01:52:48.489Z
sources:
  - id: openwiki-source-4d1d392666be6dfdd7a91a2e
    resource: repo://.github/workflows/release.yml
  - id: openwiki-source-8037e2358a2c4f9b2c722a11
    resource: repo://AGENTS.md
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-2262be0eb4e0dcf867247c95
    resource: repo://crates/limedl-core/src/backend_registry/mod.rs
  - id: openwiki-source-093388d09b520118fa26ce32
    resource: repo://crates/limedl-core/src/bt_backend/alerts.rs
  - id: openwiki-source-4b83dfd7538606d5871938ef
    resource: repo://crates/limedl-core/src/dispatcher.rs
  - id: openwiki-source-b90dbf5c7cbd9e3a7dc111aa
    resource: repo://crates/limedl-core/src/event_bus/mod.rs
  - id: openwiki-source-3c484547210ce754a4755d21
    resource: repo://crates/limedl-core/src/file_ops/mod.rs
  - id: openwiki-source-f4ce02d0eec9d2510e1b00e5
    resource: repo://crates/limedl-core/src/http_client_factory/mod.rs
  - id: openwiki-source-0b0075760500d97c47f037a4
    resource: repo://crates/limedl-core/src/lib.rs
  - id: openwiki-source-351431881f59a7b65e1583b0
    resource: repo://crates/limedl-core/src/scheduler/mod.rs
  - id: openwiki-source-5dec6002b80585dbbafe39ac
    resource: repo://crates/limedl-core/src/types/common.rs
  - id: openwiki-source-e3df85aea81b1db0ad56647b
    resource: repo://crates/limedl-native/src/i18n/mod.rs
  - id: openwiki-source-d4153d1b0168cae501e8c53a
    resource: repo://crates/limedl-native/src/ui_tests/mod.rs
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
  - id: openwiki-source-ca864fd40fa4107ed35f840f
    resource: repo://xtask/src/fetch_font.rs
generated: { by: "pi", at: "2026-10-04T01:52:48.489Z" }
---

# limedl Wiki Quickstart

limedl is a multi-protocol download manager (HTTP, BitTorrent, CDN acceleration)
with a Rust engine and a Slint desktop client. The workspace has three members:
`crates/limedl-core` (engine, lib `limedl_core`), `crates/limedl-native` (Slint
desktop binary) and `xtask` (repository tooling).

Evidence: `repo://Cargo.toml#L1-L11`, `repo://crates/limedl-core/src/lib.rs#L1-L34`.

## Route by goal

| Goal | Read first | Start from source |
| --- | --- | --- |
<!-- openwiki: broken internal link [/openwiki/architecture/overview.md] link "/openwiki/architecture/overview.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Understand the whole system | [Workspace and System Architecture](/openwiki/architecture/overview.md) | `Cargo.toml`, `crates/limedl-core/src/lib.rs` |
<!-- openwiki: broken internal link [/openwiki/architecture/bootstrap-and-services.md] link "/openwiki/architecture/bootstrap-and-services.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| See how the engine boots | [Bootstrap, SystemContext and Shared Services](/openwiki/architecture/bootstrap-and-services.md) | `crates/limedl-core/src/bootstrap.rs` |
<!-- openwiki: broken internal link [/openwiki/architecture/protocol-routing-and-dispatcher.md] link "/openwiki/architecture/protocol-routing-and-dispatcher.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Follow a request to a backend | [Protocol Routing and the Dispatcher Facade](/openwiki/architecture/protocol-routing-and-dispatcher.md) | `crates/limedl-core/src/dispatcher.rs`, `protocol.rs` |
<!-- openwiki: broken internal link [/openwiki/workflows/http-download-lifecycle.md] link "/openwiki/workflows/http-download-lifecycle.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Trace an HTTP download | [HTTP Download Lifecycle](/openwiki/workflows/http-download-lifecycle.md) | `crates/limedl-core/src/http_executor/`, `manager.rs` |
<!-- openwiki: broken internal link [/openwiki/workflows/scheduler-and-concurrency.md] link "/openwiki/workflows/scheduler-and-concurrency.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Understand thread counts / AIMD | [Scheduler, AIMD and Concurrency Control](/openwiki/workflows/scheduler-and-concurrency.md) | `crates/limedl-core/src/scheduler/mod.rs`, `aimd/mod.rs` |
<!-- openwiki: broken internal link [/openwiki/workflows/bit-torrent-backend.md] link "/openwiki/workflows/bit-torrent-backend.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Work on BitTorrent | [BitTorrent Backend](/openwiki/workflows/bit-torrent-backend.md) | `crates/limedl-core/src/bt_backend/` |
<!-- openwiki: broken internal link [/openwiki/workflows/cdn-acceleration.md] link "/openwiki/workflows/cdn-acceleration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Work on CDN acceleration | [CDN Acceleration](/openwiki/workflows/cdn-acceleration.md) | `crates/limedl-core/src/cdn/` |
<!-- openwiki: broken internal link [/openwiki/systems/persistence-and-recovery.md] link "/openwiki/systems/persistence-and-recovery.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Change persistence | [SQLite Persistence and Crash Recovery](/openwiki/systems/persistence-and-recovery.md) | `crates/limedl-core/src/database/`, `persistence.rs` |
<!-- openwiki: broken internal link [/openwiki/systems/settings-and-configuration.md] link "/openwiki/systems/settings-and-configuration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Change settings | [Settings and Configuration](/openwiki/systems/settings-and-configuration.md) | `crates/limedl-core/src/settings/`, `services/settings_service.rs` |
<!-- openwiki: broken internal link [/openwiki/systems/disk-io-and-storage.md] link "/openwiki/systems/disk-io-and-storage.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Work on disk I/O / buffers | [Disk I/O, Buffer Pool and Storage Detection](/openwiki/systems/disk-io-and-storage.md) | `crates/limedl-core/src/buffer_pool/`, `file_ops/` |
<!-- openwiki: broken internal link [/openwiki/systems/networking-and-rate-control.md] link "/openwiki/systems/networking-and-rate-control.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Change networking / proxy / UA | [Networking, HTTP Clients and Rate Control](/openwiki/systems/networking-and-rate-control.md) | `crates/limedl-core/src/http_client_factory/mod.rs` |
<!-- openwiki: broken internal link [/openwiki/integrations/aria2-rpc-server.md] link "/openwiki/integrations/aria2-rpc-server.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Work on the Aria2 RPC API | [Aria2 JSON-RPC Compatibility Server](/openwiki/integrations/aria2-rpc-server.md) | `crates/limedl-core/src/aria2_rpc/` |
<!-- openwiki: broken internal link [/openwiki/desktop/native-ui-architecture.md] link "/openwiki/desktop/native-ui-architecture.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Work on the desktop UI | [Native Desktop UI (Slint)](/openwiki/desktop/native-ui-architecture.md) | `crates/limedl-native/src/main.rs`, `ui_boot.rs`, `handlers/` |
<!-- openwiki: broken internal link [/openwiki/desktop/self-update-and-distribution.md] link "/openwiki/desktop/self-update-and-distribution.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Change the updater / packaging | [Self-Update and Distribution Channels](/openwiki/desktop/self-update-and-distribution.md) | `crates/limedl-native/src/update/mod.rs` |
<!-- openwiki: broken internal link [/openwiki/operations/build-release-and-ci.md] link "/openwiki/operations/build-release-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Build / CI / release | [Build, Tooling, CI and Release Operations](/openwiki/operations/build-release-and-ci.md) | `.github/workflows/`, `xtask/` |
<!-- openwiki: broken internal link [/openwiki/testing/testing-strategy.md] link "/openwiki/testing/testing-strategy.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Write engine tests | [Testing Strategy](/openwiki/testing/testing-strategy.md) | `crates/limedl-core/src/tests/` |
<!-- openwiki: broken internal link [/openwiki/testing/slint-ui-testing.md] link "/openwiki/testing/slint-ui-testing.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
| Write UI tests | [Slint UI Testing](/openwiki/testing/slint-ui-testing.md) | `crates/limedl-native/src/ui_tests/` |

## Build and run

```bash
# One-time: the MiSans VF font is embedded at compile time and is not in git
cargo xtask fetch-font

# Desktop client (Windows needs MSVC initialized first)
cargo run -p limedl-native
```

Linux also needs `libfontconfig1-dev`; the app keeps its data under the OS local
data directory and honours `LIMEDL_DATA_DIR` as an override.

Evidence: `repo://README.md#L70-L96`, `repo://AGENTS.md#L5-L40`.

## The mandatory pre-commit gate

CI fails on any test failure, warning or error regardless of who caused it, so the
same gate must be green locally before committing (Windows: initialize MSVC first):

```powershell
$env:CARGO_BUILD_WARNINGS="deny"
cargo clippy --workspace --all-targets -- -D warnings

cargo nextest run --manifest-path crates/limedl-core/Cargo.toml --features "test-utils,aria2-rpc"
cargo nextest run --manifest-path xtask/Cargo.toml
cargo nextest run --manifest-path crates/limedl-native/Cargo.toml
```

Coverage is a hard CI gate at 85 % lines for `limedl-core`. The full contract and
its Windows blind spot are on
<!-- openwiki: broken internal link [/openwiki/operations/build-release-and-ci.md] link "/openwiki/operations/build-release-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Build, Tooling, CI and Release Operations](/openwiki/operations/build-release-and-ci.md).

Evidence: `repo://AGENTS.md#L214-L309`.

## Load-bearing invariants (read before refactoring)

- **Register backends with `register_arc`**, never `register`, so `CoreSystems`
  and the registry share the same atomics. `register` clones state.
  `repo://crates/limedl-core/src/backend_registry/mod.rs#L42-L60`
- **`EventBus::publish` only sends**; each adapter (desktop, Aria2) subscribes
  independently and must handle `RecvError::Lagged`.
  `repo://crates/limedl-core/src/event_bus/mod.rs#L48-L84`
- **Settings writes go through `Dispatcher::save_settings_with`**; there is no
  shadow settings copy.
  `repo://crates/limedl-core/src/dispatcher.rs#L293-L348`
- **The scheduler takes the `core` lock before the `aimd` lock**, and every
  `desired_thread_count` change syncs the snapshot.
  `repo://crates/limedl-core/src/scheduler/mod.rs#L317-L360`
- **The BT alert bridge is the sole source of BT Aria2 notifications**; the RPC
  handler broadcasts only for HTTP.
  `repo://crates/limedl-core/src/bt_backend/alerts.rs#L100-L200`
- **Never guess a filesystem from a size or an unrelated error**; single-file
  limits are only decided in `reservation_error`.
  `repo://crates/limedl-core/src/file_ops/mod.rs#L586-L618`
- **Every outbound HTTP client goes through `http_client_factory`**, or proxy and
  User-Agent settings are silently bypassed.
  `repo://crates/limedl-core/src/http_client_factory/mod.rs#L28-L66`
- **`.slint` element ids are test contracts**; adding a clickable control usually
  means adding a snake_case `id:`.
  `repo://crates/limedl-native/src/ui_tests/mod.rs#L1-L30`
- **The MiSans font is not in git** (license); fetch it with
  `cargo xtask fetch-font`.
  `repo://xtask/src/fetch_font.rs#L44-L57`
- **`irontide` is pinned exactly** (`=1.7.0`); moving it is a migration.
  `repo://Cargo.toml#L70-L74`

## Conventions

- Rust structs use `#[serde(rename_all = "camelCase")]`; enums use
  `snake_case`. `repo://crates/limedl-core/src/types/common.rs#L7-L15`
- `.slint` user-visible strings use `@tr(...)`; Rust-side dynamic text uses
  `i18n::format_*`, never hardcoded CJK.
  `repo://crates/limedl-native/src/i18n/mod.rs#L1-L16`
- Commit subjects stay Conventional (`feat:`, `fix:`, …) because release notes are
  generated by git-cliff. `repo://.github/workflows/release.yml#L77-L121`
- After dependency changes, commit the updated `Cargo.lock`.
  `repo://AGENTS.md#L310-L336`

## Suggested reading order

<!-- openwiki: broken internal link [/openwiki/architecture/overview.md] link "/openwiki/architecture/overview.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
1. [Workspace and System Architecture](/openwiki/architecture/overview.md)
<!-- openwiki: broken internal link [/openwiki/architecture/bootstrap-and-services.md] link "/openwiki/architecture/bootstrap-and-services.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
2. [Bootstrap, SystemContext and Shared Services](/openwiki/architecture/bootstrap-and-services.md)
<!-- openwiki: broken internal link [/openwiki/architecture/protocol-routing-and-dispatcher.md] link "/openwiki/architecture/protocol-routing-and-dispatcher.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
3. [Protocol Routing and the Dispatcher Facade](/openwiki/architecture/protocol-routing-and-dispatcher.md)
<!-- openwiki: broken internal link [/openwiki/workflows/http-download-lifecycle.md] link "/openwiki/workflows/http-download-lifecycle.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
4. [HTTP Download Lifecycle](/openwiki/workflows/http-download-lifecycle.md)
<!-- openwiki: broken internal link [/openwiki/workflows/scheduler-and-concurrency.md] link "/openwiki/workflows/scheduler-and-concurrency.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
5. [Scheduler, AIMD and Concurrency Control](/openwiki/workflows/scheduler-and-concurrency.md)
<!-- openwiki: broken internal link [/openwiki/testing/testing-strategy.md] link "/openwiki/testing/testing-strategy.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
6. [Testing Strategy](/openwiki/testing/testing-strategy.md)
