---
type: architecture
title: Workspace and System Architecture
description: Repository layout and runtime topology of limedl — the core engine, Slint desktop and headless server crates, protocol routing by TaskId, the typed EventBus fan-out, and the cross-cutting serialization and build conventions.
tags: [architecture, workspace, crates, routing, event-bus, conventions]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-05T01:38:26.934Z
sources:
  - id: openwiki-source-4905fab56ecf9fa5e1ebbf3f
    resource: repo://.cargo/config.toml
  - id: openwiki-source-4d1d392666be6dfdd7a91a2e
    resource: repo://.github/workflows/release.yml
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-e13f09c0428e593de7772c03
    resource: repo://crates/limedl-core/Cargo.toml
  - id: openwiki-source-1ad782a385f3efd488e8d368
    resource: repo://crates/limedl-core/src/aria2_rpc/transport.rs
  - id: openwiki-source-2262be0eb4e0dcf867247c95
    resource: repo://crates/limedl-core/src/backend_registry/mod.rs
  - id: openwiki-source-05acffc41354e79e4b63b4e7
    resource: repo://crates/limedl-core/src/download/managed.rs
  - id: openwiki-source-b90dbf5c7cbd9e3a7dc111aa
    resource: repo://crates/limedl-core/src/event_bus/mod.rs
  - id: openwiki-source-0b0075760500d97c47f037a4
    resource: repo://crates/limedl-core/src/lib.rs
  - id: openwiki-source-9fa813eab6e27ac3f5fdbf1d
    resource: repo://crates/limedl-core/src/manifest.rs
  - id: openwiki-source-91b6ee5086a47115791aca3c
    resource: repo://crates/limedl-core/src/protocol.rs
  - id: openwiki-source-5dec6002b80585dbbafe39ac
    resource: repo://crates/limedl-core/src/types/common.rs
  - id: openwiki-source-e2615e7c20989bf4c6750b4d
    resource: repo://crates/limedl-core/src/types/task.rs
  - id: openwiki-source-9c56a3da02ebd0bef2dc0798
    resource: repo://crates/limedl-native/Cargo.toml
  - id: openwiki-source-fb048d5fb7a13a8f9b8fad76
    resource: repo://crates/limedl-native/src/main.rs
  - id: openwiki-source-d94bdd15f85e5a65c6c7399a
    resource: repo://crates/limedl-native/src/renderer.rs
  - id: openwiki-source-d276c8311d32ca61eb9edb7d
    resource: repo://crates/limedl-server/Cargo.toml
  - id: openwiki-source-2d1753b77bfe7d551752205e
    resource: repo://crates/limedl-server/src/lib.rs
generated: { by: "pi", at: "2026-10-05T01:38:26.934Z" }
---

# Workspace and System Architecture

limedl is a Cargo workspace with four members: `crates/limedl-core` (the pure
download engine, lib name `limedl_core`), `crates/limedl-native` (the Slint desktop
client binary), `crates/limedl-server` (the headless daemon binary that serves the
Aria2 JSON-RPC API), and `xtask` (repository tooling for version bumps, font
fetching and release signing). All crates use Rust edition 2024 and the workspace
version (`0.4.7`) is inherited from the root manifest.

Evidence: `repo://Cargo.toml#L1-L11`.

## Two layers, one engine

The defining split is that the engine knows nothing about the UI. `limedl-core`
declares its public modules (manager, http_executor, scheduler, bt_backend, cdn,
aria2_rpc, database, settings, …) and depends on no GUI crate; the desktop client
and the headless server are both frontends over it. Both call the same
`bootstrap(state_dir)`, which is why the core maintains this boundary.

Evidence: `repo://crates/limedl-core/src/lib.rs#L3-L34`,
`repo://crates/limedl-core/Cargo.toml#L1-L10`.

`limedl-native` depends on `limedl-core` with the `aria2-rpc` feature enabled and
compiles the Slint renderer pair FemtoVG/OpenGL plus the software rasterizer. The
renderer is a compile-time choice; the software fallback exists because a machine
whose GL stack fails to initialize (RDP session, VM, broken driver) would
otherwise have no renderer able to open a window. The advertised `NAME` is the
*preferred* renderer, not a guarantee that the GPU path was taken. The crate also
enables tokio's `signal` feature for the `cfg(unix)` SIGTERM/SIGINT watcher in
`main.rs`.

Evidence: `repo://crates/limedl-native/Cargo.toml#L13-L31`,
`repo://crates/limedl-native/Cargo.toml#L33-L36`,
`repo://crates/limedl-native/src/renderer.rs#L1-L31`.

`limedl-server` depends on the same `limedl-core` with `aria2-rpc`, plus `clap`
for its CLI and `anyhow`/`tracing`. It has no Slint, tray or dialog dependency,
which is what lets the release pipeline ship it as a single static musl binary for
NAS and soft-router targets. See
[the headless server daemon page](../integrations/headless-server-daemon.md).

Evidence: `repo://crates/limedl-server/Cargo.toml#L1-L22`,
`repo://crates/limedl-server/src/lib.rs#L1-L10`.

The client owns the concerns that are meaningless inside the engine — crash
reporting (`crash.rs`), the single-instance claim and its activation channel
(`single_instance.rs`), the tray and the window platform glue. See
[the native UI page](../desktop/native-ui-architecture.md).

## Durable progress is a core invariant

Download state has two progress counters per chunk: the *received* one
(`ChunkManifest::downloaded`/`completed`), which the scheduler and the UI follow,
and the *durable* one (`ChunkManifest::durable_downloaded`,
`DownloadCore::durable_bytes`), which only the write buffer advances after a flush
and which is the only thing the database stores. That split is what makes a crash
recoverable instead of silently resuming past data that never reached the file;
the mechanism is documented in
[SQLite Persistence, Durable Progress and Crash Recovery](../systems/persistence-and-recovery.md)
and [HTTP Download Lifecycle](../workflows/http-download-lifecycle.md).

## Task identity and protocol routing

Every download is identified by a strongly-typed `TaskId` whose variants are
`Http(Uuid)` and `Bt(Id20)`. The wire string form is `http:<uuid-hyphenated>` or
`bt:<hex info-hash>`; `TaskId` itself deliberately does not implement
`Serialize`/`Deserialize`, because nothing persists it directly — summaries carry
a `String` id and the Aria2 GID map is in-memory. `TaskId::kind()` is the routing
key.

New requests are classified by `StartDownloadRequest::classify_kind`: an explicit
`kind` wins, otherwise `magnet:` and `.torrent` URLs (including local paths with a
`.torrent` extension) go to BT and `http://`/`https://` goes to HTTP. `http:` /
`bt:` prefixes on the wire are parsed back by `TaskId::from_wire_string`.

Evidence: `repo://crates/limedl-core/src/types/task.rs#L36-L103`,
`repo://crates/limedl-core/src/types/task.rs#L145-L178`.

The `DownloadBackend` trait is the uniform protocol interface: start, pause,
resume, cancel, remove, purge, open-in-explorer, status, list, update_settings,
optional set_priority, and shutdown. `BackendRegistry` stores each backend twice —
once as a `dyn DownloadBackend` keyed by `TaskKind` for routing, and once as
`Arc<dyn Any>` keyed by `TypeId` for protocol-specific downcasts (`get_typed::<T>`).
`BackendRegistry::dispatch(task_id)` is therefore a simple `task_id.kind()` lookup.

Evidence: `repo://crates/limedl-core/src/protocol.rs#L8-L45`,
`repo://crates/limedl-core/src/backend_registry/mod.rs#L8-L85`.

## The EventBus

`EventBus` is a thin wrapper over `tokio::sync::broadcast::Sender<DownloadEvent>`
and is cheap to clone. `DownloadEvent` carries strongly typed engine payloads, not
wire JSON: `Updated { summary }`, `Progress { progress }`,
`Aria2Notification { event_name, gid }`, `CdnProgress`, `CdnComplete` and
`Warning`. `publish` does exactly one thing — `tx.send(event)` — and logs a
warning if there are no subscribers. Serialization to JSON happens at the
boundaries that actually need it (Aria2 conversion, SQLite columns), never on the
bus.

Evidence: `repo://crates/limedl-core/src/event_bus/mod.rs#L8-L84`.

Frontend emission lives in each adapter, not in `publish`:

- The desktop subscribes once in `main()` and forwards into
  `event_stream::bus`, which updates the Slint model and repaints.
- The Aria2 WebSocket adapter (served by both the desktop and the daemon)
  subscribes independently and forwards only `Aria2Notification` to connected
  clients.

Both adapters must handle `RecvError::Lagged` explicitly: a `while let Ok(...)`
loop would exit permanently on lag. The desktop resynchronizes by calling
`Dispatcher::list()` and replacing the whole store; the Aria2 adapter simply
continues.

Evidence: `repo://crates/limedl-native/src/main.rs#L176-L179`,
`repo://crates/limedl-native/src/event_stream/bus.rs#L1-L18`,
`repo://crates/limedl-core/src/aria2_rpc/transport.rs#L25-L55`.

## Cross-cutting conventions

- **Serialization**: Rust structs use `#[serde(rename_all = "camelCase")]` and
  enums use `#[serde(rename_all = "snake_case")]` throughout `types/`. This is
  what the JSON settings file and the Aria2 wire responses rely on.
  Evidence: `repo://crates/limedl-core/src/types/common.rs#L7-L15`,
  `repo://crates/limedl-core/src/types/task.rs#L13-L43`.
- **Feature flags**: `limedl-core` defaults to `bt`, and the `aria2-rpc` feature
  pulls in axum/tower-http/argon2/subtle and implies `bt`. `test-utils` exposes
  AIMD/buffer-pool/test-harness internals that are private in release builds; it
  also enables the gzip/brotli/zstd encoders the compression test endpoints use
  (the decoder side is always on through reqwest).
  Evidence: `repo://crates/limedl-core/Cargo.toml#L53-L64`,
  `repo://crates/limedl-core/src/lib.rs#L36-L48`.
- **Target flags**: `.cargo/config.toml` adds `target-cpu=x86-64-v3` (desktop)
  and `--cfg reqwest_unstable` for every target; the HTTP/3 feature in reqwest
  hard-fails to compile without that cfg. New targets must carry both. The two
  musl entries (`x86_64`, `aarch64`) are the headless server's, with the x86_64
  one lowered to `x86-64-v2` for older NAS CPUs. The desktop Linux release reuses
  the `x86_64-unknown-linux-gnu` entry through `cargo zigbuild --target
  x86_64-unknown-linux-gnu.2.17`, which needs no separate `[target.*]` entry.
  Evidence: `repo://.cargo/config.toml#L1-L96`.
- **Release profile**: the workspace release profile optimizes for size, but
  `limedl-native` is overridden to `opt-level = 3` because rendering is CPU/GPU
  heavy rather than I/O bound.
  Evidence: `repo://Cargo.toml#L88-L111`.

## Runtime topology

```
Slint desktop (limedl-native)
  UI callbacks -> Dispatcher -> BackendRegistry -> DownloadManager / LazyBtBackend
    -> EventBus::publish -> broadcast -> desktop subscriber -> Slint model -> repaint

Aria2 RPC (optional, local port)
  AriaNg / Motrix -> JSON-RPC / WebSocket -> Aria2RpcServer -> Dispatcher -> same backends
    -> EventBus -> desktop subscriber (UI stays in sync)

limedl-server (headless daemon)
  AriaNg / Motrix -> JSON-RPC / WebSocket -> Aria2RpcServer -> Dispatcher -> same backends
    (no UI subscriber; the RPC endpoint is the only frontend)
```

All three arrows terminate at the same `Dispatcher`, which is why the frontends see
identical state and none re-implements lifecycle or settings logic. The next pages
cover the initialization sequence and the facade in depth.

Related pages: [Bootstrap, SystemContext and Shared Services](bootstrap-and-services.md),
[Protocol Routing and the Dispatcher Facade](protocol-routing-and-dispatcher.md),
[Headless Server Daemon](../integrations/headless-server-daemon.md),
[Native Desktop UI (Slint)](../desktop/native-ui-architecture.md).
