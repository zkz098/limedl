---
type: guide
title: limedl Wiki Quickstart
description: Entry point and task-routing map for the limedl wiki — where to start for building, understanding the engine, adding a backend, debugging downloads, running the headless daemon, testing and shipping a release, plus the repository's load-bearing invariants.
tags: [quickstart, navigation, onboarding, build, workflow]
sources:
  - id: openwiki-source-06de9eea8068258882d65c0b
    resource: repo://.github/workflows/aria2-oracle.yml
  - id: openwiki-source-4d1d392666be6dfdd7a91a2e
    resource: repo://.github/workflows/release.yml
  - id: openwiki-source-8037e2358a2c4f9b2c722a11
    resource: repo://AGENTS.md
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-3659606b404344d4dd4d1487
    resource: repo://crates/limedl-core/src/aria2_rpc/interop_tests.rs
  - id: openwiki-source-9529b707cb48393fd5c5dcfb
    resource: repo://crates/limedl-core/src/aria2_rpc/options.rs
  - id: openwiki-source-cb3b278da9fc4917fdb881e9
    resource: repo://crates/limedl-core/src/aria2_rpc/oracle_tests.rs
  - id: openwiki-source-18347dd81d612fba86839f34
    resource: repo://crates/limedl-core/src/aria2_rpc/server.rs
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
  - id: openwiki-source-cc281d0872e580aa89866924
    resource: repo://crates/limedl-server/src/cli.rs
  - id: openwiki-source-2d1753b77bfe7d551752205e
    resource: repo://crates/limedl-server/src/lib.rs
  - id: openwiki-source-600bf3367c4599b56dc1a044
    resource: repo://docs/server-daemon.md
  - id: openwiki-source-7ae9f72a70d7b68c2986edb0
    resource: repo://packaging/server/docker-compose.yml
  - id: openwiki-source-7003b6883ff44b1304d949e8
    resource: repo://packaging/server/Dockerfile
  - id: openwiki-source-816a10881eb55b69eaf93236
    resource: repo://packaging/server/systemd/limedl-server.service
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
  - id: openwiki-source-ca864fd40fa4107ed35f840f
    resource: repo://xtask/src/fetch_font.rs
generated: { by: "pi", at: "2026-10-04T12:58:25.182Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-05T02:20:21.738Z
---

# limedl Wiki Quickstart

limedl is a multi-protocol download manager (HTTP, BitTorrent, CDN acceleration)
with a Rust engine and two frontends: a Slint desktop client and a headless
Aria2-RPC server. The workspace has four members: `crates/limedl-core` (engine,
lib `limedl_core`), `crates/limedl-native` (Slint desktop binary),
`crates/limedl-server` (headless daemon binary) and `xtask` (repository tooling).

Evidence: `repo://Cargo.toml#L1-L11`, `repo://crates/limedl-core/src/lib.rs#L1-L34`.

## Route by goal

| Goal | Read first | Start from source |
| --- | --- | --- |
| Understand the whole system | [Workspace and System Architecture](architecture/overview.md) | `Cargo.toml`, `crates/limedl-core/src/lib.rs` |
| See how the engine boots | [Bootstrap, SystemContext and Shared Services](architecture/bootstrap-and-services.md) | `crates/limedl-core/src/bootstrap.rs` |
| Follow a request to a backend | [Protocol Routing and the Dispatcher Facade](architecture/protocol-routing-and-dispatcher.md) | `crates/limedl-core/src/dispatcher.rs`, `protocol.rs` |
| Trace an HTTP download | [HTTP Download Lifecycle](workflows/http-download-lifecycle.md) | `crates/limedl-core/src/http_executor/`, `manager.rs` |
| Understand thread counts / AIMD | [Scheduler, AIMD and Concurrency Control](workflows/scheduler-and-concurrency.md) | `crates/limedl-core/src/scheduler/mod.rs`, `aimd/mod.rs` |
| Work on BitTorrent | [BitTorrent Backend](workflows/bit-torrent-backend.md) | `crates/limedl-core/src/bt_backend/` |
| Work on CDN acceleration | [CDN Acceleration](workflows/cdn-acceleration.md) | `crates/limedl-core/src/cdn/` |
| Change persistence | [SQLite Persistence and Crash Recovery](systems/persistence-and-recovery.md) | `crates/limedl-core/src/database/`, `persistence.rs` |
| Change settings | [Settings and Configuration](systems/settings-and-configuration.md) | `crates/limedl-core/src/settings/`, `services/settings_service.rs` |
| Work on disk I/O / buffers | [Disk I/O, Buffer Pool and Storage Detection](systems/disk-io-and-storage.md) | `crates/limedl-core/src/buffer_pool/`, `file_ops/` |
| Change networking / proxy / UA | [Networking, HTTP Clients and Rate Control](systems/networking-and-rate-control.md) | `crates/limedl-core/src/http_client_factory/mod.rs` |
| Work on the Aria2 RPC API | [Aria2 JSON-RPC Compatibility Server](integrations/aria2-rpc-server.md) | `crates/limedl-core/src/aria2_rpc/`; interop fixtures in `aria2_rpc/interop_tests.rs`, Tier 2 `aria2c` oracle in `aria2_rpc/oracle_tests.rs`, runbook `docs/aria2-interop-testing.md` |
| Run the headless daemon (NAS / 软路由) | [Headless Server Daemon](integrations/headless-server-daemon.md) | `crates/limedl-server/`; deployment runbook `docs/server-daemon.md` |
| Deploy the daemon (systemd / Docker) | [Server Deployment and Packaging](operations/server-deployment-and-packaging.md) | `packaging/server/`; image `ghcr.io/zkz098/limedl-server` |
| Work on the desktop UI | [Native Desktop UI (Slint)](desktop/native-ui-architecture.md) | `crates/limedl-native/src/main.rs`, `ui_boot.rs`, `handlers/` |
| Change the updater / packaging | [Self-Update and Distribution Channels](desktop/self-update-and-distribution.md) | `crates/limedl-native/src/update/mod.rs` |
| Build / CI / release | [Build, Tooling, CI and Release Operations](operations/build-release-and-ci.md) | `.github/workflows/`, `xtask/` |
| Write engine tests | [Testing Strategy](testing/testing-strategy.md) | `crates/limedl-core/src/tests/` |
| Write UI tests | [Slint UI Testing](testing/slint-ui-testing.md) | `crates/limedl-native/src/ui_tests/` |

## Build and run

```bash
# One-time: the MiSans VF font is embedded at compile time and is not in git
cargo xtask fetch-font

# Desktop client (Windows needs MSVC initialized first)
cargo run -p limedl-native

# Headless daemon, LAN reachable (a non-loopback bind requires a secret)
cargo run -p limedl-server -- --rpc-listen 0.0.0.0 --rpc-secret "$LIMEDL_RPC_SECRET"

# Or run the published container image
docker run -d --name limedl-server -p 6800:6800 \
  -e LIMEDL_RPC_SECRET="$LIMEDL_RPC_SECRET" \
  -v ./data:/var/lib/limedl -v ./downloads:/downloads \
  ghcr.io/zkz098/limedl-server:latest
```

Linux also needs `libfontconfig1-dev` for the desktop client (the daemon does
not); both keep their data under the OS local data directory and honour
`LIMEDL_DATA_DIR` as an override.

Evidence: `repo://README.md#L70-L96`, `repo://AGENTS.md#L3-L21`,
`repo://crates/limedl-server/src/cli.rs#L25-L61`.

## The mandatory pre-commit gate

CI fails on any test failure, warning or error regardless of who caused it, so the
same gate must be green locally before committing (Windows: initialize MSVC first):

```powershell
$env:CARGO_BUILD_WARNINGS="deny"
cargo clippy --workspace --all-targets -- -D warnings

cargo nextest run --manifest-path crates/limedl-core/Cargo.toml --features "test-utils,aria2-rpc"
cargo nextest run --manifest-path xtask/Cargo.toml
cargo nextest run --manifest-path crates/limedl-native/Cargo.toml
cargo nextest run --manifest-path crates/limedl-server/Cargo.toml
```

Coverage is a hard CI gate at 85 % lines for `limedl-core`. The full contract and
its Windows blind spot are on
[Build, Tooling, CI and Release Operations](operations/build-release-and-ci.md).

Evidence: `repo://AGENTS.md#L99-L131`.

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
- **Aria2 RPC arguments are parsed by JSON type, not position** (AriaNg sends
  `addTorrent([torrent, [], options])`); a new method must add a Tier 1 fixture in
  the real client's request shape.
  `repo://crates/limedl-core/src/aria2_rpc/options.rs#L89-L142`,
  `repo://crates/limedl-core/src/aria2_rpc/interop_tests.rs#L1-L60`
- **A non-loopback Aria2 RPC bind must carry authentication**; the server refuses
  to start otherwise, because the RPC `dir` option can write outside a download
  root.
  `repo://crates/limedl-core/src/aria2_rpc/server.rs#L51-L59`
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
  generated by git-cliff. `repo://.github/workflows/release.yml#L77-L128`
- After dependency changes, commit the updated `Cargo.lock`.
  `repo://AGENTS.md#L132-L139`

## Suggested reading order

1. [Workspace and System Architecture](architecture/overview.md)
2. [Bootstrap, SystemContext and Shared Services](architecture/bootstrap-and-services.md)
3. [Protocol Routing and the Dispatcher Facade](architecture/protocol-routing-and-dispatcher.md)
4. [HTTP Download Lifecycle](workflows/http-download-lifecycle.md)
5. [Scheduler, AIMD and Concurrency Control](workflows/scheduler-and-concurrency.md)
6. [Aria2 JSON-RPC Compatibility Server](integrations/aria2-rpc-server.md)
7. [Headless Server Daemon](integrations/headless-server-daemon.md)
8. [Server Deployment and Packaging](operations/server-deployment-and-packaging.md)
9. [Testing Strategy](testing/testing-strategy.md)
