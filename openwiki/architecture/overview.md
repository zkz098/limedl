---
type: architecture
title: Workspace and System Architecture
description: Repository layout and runtime topology of limedl — the nine engine crates and what each owns, the frontends and tooling crate, which crate is allowed to name irontide/rusqlite/axum, protocol routing by TaskId, the typed EventBus fan-out, and the cross-cutting serialization and build conventions.
tags: [architecture, workspace, crates, layering, routing, event-bus, conventions]
verified:
  - by: openwiki/0.7.2
    at: 2026-10-25T12:09:48.740Z
---

# Workspace and System Architecture

limedl is a Cargo workspace with twelve members: nine engine crates, two
frontends and the tooling crate. All use Rust edition 2024 and inherit the
workspace version and license from the root manifest. Every crate is
`publish = false` — limedl ships binaries, a container image and signed
self-update manifests, never crates.io packages.

Evidence: `repo://Cargo.toml#L1-L16`.

## The engine crate graph

The engine used to be one 41.5k-line crate. It is now a stack of crates whose
dependency edges are the layering, so the compiler enforces what
`module_boundaries_tests` used to police by parsing source files:

| Crate | Owns | Dependency graph (crates) |
| --- | --- | --- |
| `limedl-types` | domain types, settings tree, `DownloadError`, `EventBus`, `Manifest`, `DownloadBackend`, checksum, url rewrite, `slot_guard` | 164 |
| `limedl-disk` | `file_ops` (preallocation, coalescing sink, fault hooks, media probe), `buffer_pool`, `io_scheduler`, disk services | 168 |
| `limedl-store` | `database` (SQLite task/chunk/manifest store + migrations), `alt_svc` cache | 170 |
| `limedl-http` | `http` helpers, `http_client_factory` (the one `reqwest` policy set), `rate_limiter` | 172 |
| `limedl-engine` | `http_executor`, `scheduler`, `cdn`, `metalink`, `download`, `task_lifecycle`, `persistence`, `aimd`, `manager_state` | 185 |
| `limedl-bt` | the BitTorrent backend and its irontide engine | 296 |
| `limedl-core` | facade: `manager`, `dispatcher`, `context`, `backend_registry`, `bootstrap`, `logging`, and the test fixtures | 308 |
| `limedl-rpc` | Aria2 JSON-RPC + WebSocket server (`axum`) | 332 |
| `limedl-testkit` | the deterministic HTTP test server, fault injection, compression endpoints | 192 |

The order is the direction: a crate may depend only on the ones above it.
`limedl-bt` and `limedl-engine` are siblings (neither depends on the other), and
`limedl-rpc` is the only crate that sits *above* the facade.

Evidence: `repo://crates/limedl-core/Cargo.toml#L14-L30`,
`repo://crates/limedl-engine/Cargo.toml#L14-L24`,
`repo://crates/limedl-bt/Cargo.toml#L14-L24`,
`repo://crates/limedl-rpc/Cargo.toml#L14-L24`,
`repo://crates/limedl-disk/Cargo.toml#L14-L22`,
`repo://crates/limedl-store/Cargo.toml#L14-L22`,
`repo://crates/limedl-http/Cargo.toml#L14-L22`,
`repo://crates/limedl-types/Cargo.toml#L14-L24`.

### One crate per heavy dependency

The split's most useful property is that each expensive third-party tree is
reachable from exactly one place:

- **`irontide`** (the pinned `=1.7.0` BitTorrent engine, 100+ crates of DHT,
  peer-wire and `aws-lc`) is named only by `limedl-bt`. `limedl-core` takes that
  crate as an *optional* dependency behind the `bt` feature, so
  `--no-default-features` builds an engine with no BT protocol and
  `cargo tree -i irontide` finds nothing.
- **`rusqlite`** (bundled SQLite, the most expensive C build in the graph) is
  named only by `limedl-store`.
- **`axum`/`tower-http`/`argon2`** are named only by `limedl-rpc` (plus `axum` in
  the test-only `limedl-testkit`), never by the engine.
- **`flate2`/`brotli`/`zstd`** encoders exist only in `limedl-testkit`; the
  decoder side arrives through `reqwest`'s features.

Evidence: `repo://crates/limedl-bt/Cargo.toml#L15-L22`,
`repo://crates/limedl-core/Cargo.toml#L60-L62`,
`repo://crates/limedl-store/Cargo.toml#L15`,
`repo://crates/limedl-rpc/Cargo.toml#L17-L22`,
`repo://crates/limedl-testkit/Cargo.toml#L17-L27`.

### What `limedl-core` still is

`limedl-core` is the facade: the router (`Dispatcher`), the manager, the
assembly point (`bootstrap`/`SystemContext`), logging, and the fixtures that need
a real manager. Its `lib.rs` is mostly `pub use` statements — one per extracted
layer, re-exported as *modules* rather than as item globs so that every
historical path (`crate::types::TaskId`, `crate::http_executor::…`,
`limedl_core::aria2_rpc::…` before the RPC split) keeps resolving inside the
engine and, for the types and modules it still re-exports, outside it. That shim
is why the ~12k lines of engine tests moved without a single edit.

Evidence: `repo://crates/limedl-core/src/lib.rs#L1-L95`.

The BT crate is re-exported under its old module name
(`#[cfg(feature = "bt")] pub use limedl_bt as bt_backend;`) precisely so callers
keep writing `limedl_core::bt_backend::LazyBtBackend`.

## Two layers, one engine

The defining split is that the engine knows nothing about the UI. `limedl-core`
and the crates under it depend on no GUI crate; the desktop client and the
headless server are both frontends over them, and both call the same
`bootstrap(state_dir)`. See
[Bootstrap, SystemContext and Shared Services](bootstrap-and-services.md).

`limedl-native` depends on `limedl-core` and `limedl-rpc` separately (the RPC
server is a frontend concern, not an engine one) and compiles the Slint renderer
pair FemtoVG/OpenGL plus the software rasterizer. The renderer is a compile-time
choice; the software fallback exists because a machine whose GL stack fails to
initialize (RDP session, VM, broken driver) would otherwise have no renderer able
to open a window. The advertised `NAME` is the *preferred* renderer, not a
guarantee that the GPU path was taken. The crate also enables tokio's `signal`
feature for the `cfg(unix)` SIGTERM/SIGINT watcher in `main.rs`.

`limedl-server` depends on the same engine crates plus `limedl-rpc`, `clap` and
`anyhow`/`tracing`. It has no Slint, tray or dialog dependency, which is what
lets the release pipeline ship it as a single static musl binary for NAS and
soft-router targets. See
[the headless server daemon page](../integrations/headless-server-daemon.md).

The client owns the concerns that are meaningless inside the engine — crash
reporting, the single-instance claim and its activation channel, the tray and the
window platform glue. See [the native UI page](../desktop/native-ui-architecture.md).

## Durable progress is an engine invariant

Download state has two progress counters per chunk: the *received* one
(`ChunkManifest::downloaded`/`completed`), which the scheduler and the UI follow,
and the *durable* one (`ChunkManifest::durable_downloaded`,
`DownloadCore::durable_bytes`), which only the write buffer advances after a flush
and which is the only thing the database stores. That split is what makes a crash
recoverable instead of silently resuming past data that never reached the file;
the mechanism is documented in
[SQLite Persistence, Durable Progress and Crash Recovery](../systems/persistence-and-recovery.md)
and [HTTP Download Lifecycle](../workflows/http-download-lifecycle.md).

It is also why the manifest type lives in `limedl-types` while the counters that
advance it live in `limedl-engine`: the schema is shared with the store, the
mutation is engine policy.

Evidence: `repo://crates/limedl-types/src/manifest.rs#L92-L117`,
`repo://crates/limedl-engine/src/download/managed.rs#L20-L45`.

## Task identity and protocol routing

Every download is identified by a strongly-typed `TaskId` whose variants are
`Http(Uuid)` and `Bt(InfoHash)`. `InfoHash` is `limedl-types`' own
20-byte newtype, not `irontide::core::Id20`: naming the engine's type here would
have dragged the whole BT engine under the foundation layer, so the BT crate
converts at its boundary instead. The wire string form is `http:<uuid-hyphenated>`
or `bt:<hex info-hash>`; `TaskId` itself deliberately does not implement
`Serialize`/`Deserialize`, because nothing persists it directly — summaries carry
a `String` id and the Aria2 GID map is in-memory. `TaskId::kind()` is the routing
key.

New requests are classified by `StartDownloadRequest::classify_kind`: an explicit
`kind` wins, otherwise `magnet:` and `.torrent` URLs (including local paths with a
`.torrent` extension) go to BT and `http://`/`https://` goes to HTTP. `http:` /
`bt:` prefixes on the wire are parsed back by `TaskId::from_wire_string`.

Evidence: `repo://crates/limedl-types/src/types/task.rs#L39-L103`,
`repo://crates/limedl-types/src/types/task.rs#L144-L178`,
`repo://crates/limedl-types/src/types/task.rs#L254-L300`.

The `DownloadBackend` trait is the uniform protocol interface: start, pause,
resume, cancel, remove, purge, open-in-explorer, status, list, update_settings,
optional set_priority, and shutdown. `BackendRegistry` stores each backend twice —
once as a `dyn DownloadBackend` keyed by `TaskKind` for routing, and once as
`Arc<dyn Any>` keyed by `TypeId` for protocol-specific downcasts (`get_typed::<T>`).
`BackendRegistry::dispatch(task_id)` is therefore a simple `task_id.kind()` lookup.
The trait lives in `limedl-types` because `limedl-bt` implements it while sitting
in the middle of the stack; the registry lives in `limedl-core` because only the
facade assembles backends.

Evidence: `repo://crates/limedl-types/src/protocol.rs#L11-L45`,
`repo://crates/limedl-core/src/backend_registry/mod.rs#L10-L85`.

## The EventBus

`EventBus` is a thin wrapper over `tokio::sync::broadcast::Sender<DownloadEvent>`
and is cheap to clone. `DownloadEvent` carries strongly typed engine payloads, not
wire JSON: `Updated { summary }`, `Progress { progress }`,
`Aria2Notification { event_name, gid }`, `CdnProgress`, `CdnComplete` and
`Warning`. `publish` does exactly one thing — `tx.send(event)` — and logs a
warning if there are no subscribers. Serialization to JSON happens at the
boundaries that actually need it (Aria2 conversion, SQLite columns), never on the
bus.

Evidence: `repo://crates/limedl-types/src/event_bus/mod.rs#L21-L84`.

Frontend emission lives in each adapter, not in `publish`:

- The desktop subscribes once in `main()` and forwards into
  `event_stream::bus`, which updates the Slint model and repaints.
- The Aria2 WebSocket adapter (served from `limedl-rpc`, used by both the desktop
  and the daemon) subscribes independently and forwards only
  `Aria2Notification` to connected clients.

Both adapters must handle `RecvError::Lagged` explicitly: a `while let Ok(...)`
loop would exit permanently on lag. The desktop resynchronizes by calling
`Dispatcher::list()` and replacing the whole store; the Aria2 adapter simply
continues.

Evidence: `repo://crates/limedl-native/src/main.rs#L176-L179`,
`repo://crates/limedl-native/src/event_stream/bus.rs#L1-L18`,
`repo://crates/limedl-rpc/src/transport.rs#L25-L55`.

## Cross-cutting conventions

- **Serialization**: Rust structs use `#[serde(rename_all = "camelCase")]` and
  enums use `#[serde(rename_all = "snake_case")]` throughout `types/`. This is
  what the JSON settings file and the Aria2 wire responses rely on.
  Evidence: `repo://crates/limedl-types/src/types/common.rs#L7-L15`,
  `repo://crates/limedl-types/src/types/task.rs#L11-L20`.
- **Feature flags**: `limedl-core` defaults to `bt`, which pulls in the optional
  `limedl-bt` and forwards `bt` to `limedl-types`/`limedl-store` (a type-level
  switch there: `TaskKind::Bt`/`TaskId::Bt` must not exist when BT is absent, or
  every gated match arm in the engine becomes non-exhaustive). `test-utils`
  enables `limedl-testkit` and forwards the `test-utils` seam to
  `limedl-disk`/`limedl-store`/`limedl-http`/`limedl-engine`. There is no
  `aria2-rpc` feature any more: the RPC layer is a crate.
  Evidence: `repo://crates/limedl-core/Cargo.toml#L58-L72`.
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
  Evidence: `repo://Cargo.toml#L100-L125`.
- **Layering guard**: `module_boundaries_tests` in `limedl-core` walks the eight
  modules that are still in the facade, fails on a dependency edge that points at
  a higher layer, and fails on a module missing from its `LAYERS` table or on a
  `LAYERS` entry whose module no longer exists.

## Runtime topology

```
Slint desktop (limedl-native)
  UI callbacks -> Dispatcher -> BackendRegistry -> DownloadManager / LazyBtBackend
    -> EventBus::publish -> broadcast -> desktop subscriber -> Slint model -> repaint

Aria2 RPC (limedl-rpc, optional, local port)
  AriaNg / Motrix -> JSON-RPC / WebSocket -> Aria2RpcServer -> Dispatcher -> same backends
    -> EventBus -> desktop subscriber (UI stays in sync)

limedl-server (headless daemon)
  AriaNg / Motrix -> JSON-RPC / WebSocket -> Aria2RpcServer -> Dispatcher -> same backends
    (no UI subscriber; the RPC endpoint is the only frontend)
```

All three arrows terminate at the same `Dispatcher`, which is why the frontends
see identical state and none re-implements lifecycle or settings logic. The RPC
server is constructed by whichever frontend wants it — the engine's `bootstrap`
never starts one.

Related pages: [Bootstrap, SystemContext and Shared Services](bootstrap-and-services.md),
[Protocol Routing and the Dispatcher Facade](protocol-routing-and-dispatcher.md),
[Headless Server Daemon](../integrations/headless-server-daemon.md),
[Native Desktop UI (Slint)](../desktop/native-ui-architecture.md),
[Build, Release and CI](../operations/build-release-and-ci.md).
