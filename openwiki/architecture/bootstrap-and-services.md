---
type: architecture
title: Bootstrap, SystemContext and Shared Services
description: The single canonical initialization sequence that builds SystemContext, DownloadManager, the lazy BT backend, the BackendRegistry, CDN service and Dispatcher, plus the shared services that own global runtime state, which crate each one lives in after the engine split, and the two frontends that consume them.
tags: [bootstrap, systemcontext, services, initialization, dependency-injection, crate-boundaries]
verified:
  - by: openwiki/0.7.2
    at: 2026-10-25T12:09:48.740Z
---

# Bootstrap, SystemContext and Shared Services

Every frontend of limedl starts from one function: `bootstrap(state_dir)` in
`crates/limedl-core/src/bootstrap.rs`. There are two frontends: the Slint desktop
client (`crates/limedl-native/`) and the headless `limedl-server` daemon
(`crates/limedl-server/`). `bootstrap` is deliberately the *only* place that
constructs the engine subsystems, so the initialization order and the Arc-sharing
rules below apply uniformly and a new subsystem is added in exactly one place —
the daemon inherits the whole engine graph for free.

## CoreSystems: the returned handle bundle

`bootstrap` returns a `CoreSystems` struct whose fields are `Arc`s (plus a
settings snapshot):

| Field | Purpose | Type lives in |
| --- | --- | --- |
| `context: Arc<SystemContext>` | DB, EventBus, RateLimiter, BufferPool, device manager, concurrency, settings, disk I/O, shutdown token | `limedl-core` |
| `download_manager: Arc<DownloadManager>` | task creation, settings application, facades over the HTTP engine | `limedl-core` |
| `bt_backend: Arc<LazyBtBackend>` | BitTorrent engine wrapper (behind the `bt` feature, on by default) | `limedl-bt` |
| `registry: Arc<BackendRegistry>` | protocol routing table | `limedl-core` |
| `dispatcher: Arc<Dispatcher>` | unified facade used by every frontend | `limedl-core` |
| `event_bus`, `rate_limiter`, `cdn_service`, `settings_service`, `disk_io_service`, `concurrency` | shared services re-exported for callers | `limedl-types`, `limedl-disk`, `limedl-engine` |

Evidence: `repo://crates/limedl-core/src/bootstrap.rs#L21-L35`.

The right-hand column is the crate split made visible: `bootstrap` is the one
function that has to name a type from almost every engine crate, which is why it
stayed in `limedl-core` and why it is the top of the engine's dependency graph.

## SystemContext: where global state is created

`SystemContext` is a `Clone` bundle of shared infrastructure. Its `new()` creates a
default `RateLimiter` and an `EventBus` with capacity **8192**, then delegates to
`with_components`; tests and CLI paths can inject their own rate limiter and event
bus through that second constructor.

`with_components` performs the state-dependent construction:

- `state_dir` is created if missing, and the settings file is derived as its
  **parent** joined with `settings.json` — the state directory itself is
  `<data_dir>/downloads`, so settings live one level up.
- `SettingsService::new` loads and normalizes `settings.json`.
- `Database::open(state_dir/downloads.db)`.
- `BufferPool::new(...)` sized from `io_baseline` (buffer limit, game-mode
  buffer, HDD parallelism, game-mode parallelism).
- `DiskDeviceManager::new()` plus `set_overrides(&disk_type_overrides)` so the
  device queue is built from persisted media overrides *before* any settings
  save happens.
- `IoWorker::spawn_pool_with_device_manager(...)` with a thread count of
  `min(available_parallelism, 4)`.
- `ConcurrencyManager::new(5, 3)` (5 HTTP, 3 BT) and `DiskIoService`.
- A fresh `CancellationToken` for shutdown.

Evidence: `repo://crates/limedl-core/src/context.rs#L31-L100`.

`SystemContext` names types from four crates at once (`limedl-store`'s `Database`,
`limedl-disk`'s buffer pool / device manager / services, `limedl-http`'s
`RateLimiter`, `limedl-types`' `EventBus`), and it is the reason `limedl-disk`
must not depend on `limedl-core`: the dependency points the other way.

## The canonical initialization order

`bootstrap` builds components in a strong dependency chain:

1. `SystemContext::new(state_dir)`, then `settings_service.get_blocking()` for the
   startup snapshot.
2. `DownloadManager::new(&context)`, wrapped in `Arc`, then
   `download_manager.scheduler.clone().start_scheduler_loop(download_manager.clone())`
   starts the background scheduler.
3. The BT backend: `LazyBtBackend::new(...)` is cheap (it only records config);
   `spawn_index_sync_loop()` keeps the persisted index fresh and
   `spawn_idle_supervisor()` unloads an idle engine in lightweight mode. The
   expensive irontide session is started by `spawn_startup()`, and only when
   either lightweight mode is off **or** the index already holds unfinished
   work. Everything in this step is `limedl-bt`, reachable only when the `bt`
   feature pulls it in.
4. `BackendRegistry::new()`, then `register_arc(TaskKind::Http, ...)` and
   `register_arc(TaskKind::Bt, ...)`.
5. `CdnService::new()`, injected into the manager via
   `download_manager.set_cdn_accelerator(...)` and restored from settings via
   `init_from_settings`.
6. A dedicated `reqwest::Client` built with
   `limedl_http::http_client_factory::configure_client_builder` (so
   Dispatcher-side requests honour proxy and User-Agent) plus a 5-redirect
   policy, 15 s timeout and `limedl/<version>` UA.
7. `Dispatcher::full(...)` with the registry, event bus, settings service, disk
   I/O, concurrency, CDN service and that HTTP client.

Evidence: `repo://crates/limedl-core/src/bootstrap.rs#L40-L145`.

## Arc identity is load-bearing

The registry is populated with `register_arc`, not `register`. The distinction is
documented on the method: `register` wraps a backend by value in a fresh `Arc`,
while `register_arc` stores the caller's existing `Arc`. Using `register` with a
cloned `DownloadManager`/`LazyBtBackend` would snapshot mutable state (fresh
atomics) and silently diverge from the copy in `CoreSystems`. The correlation
counters, overclock flag and BT slot state must be the *same* objects the
frontends talk to.

Evidence: `repo://crates/limedl-core/src/backend_registry/mod.rs#L47-L60`,
`repo://crates/limedl-core/src/bootstrap.rs#L81-L88`.

## What bootstrap deliberately does not build

`Aria2RpcServer` is **not** part of `bootstrap`, and after the crate split that
is no longer a feature flag inside the engine — the RPC layer is a separate
crate, `limedl-rpc`, that sits *above* `limedl-core`. `limedl-core` has no
dependency on it at all, which is what keeps `axum`, `tower-http` and `argon2`
out of the engine's dependency graph.

Each frontend constructs it after `bootstrap` returns, from `core.registry`,
`core.event_bus` and its own `Aria2RpcSettings`:

- the **desktop** starts it only when `settings.aria2_rpc.enabled` is true, binds
  the configured address (loopback by default), and keeps the `watch` shutdown
  sender in the UI context so a settings save can hot-reload it;
- the **daemon** force-enables it (`enabled = true`) because the RPC endpoint *is*
  its interface, forces `exit_on_shutdown = true` so `aria2.shutdown` stops the
  process, and selects on the signal/shutdown handles before calling
  `core.registry.shutdown_all()`.

Evidence: `repo://crates/limedl-native/src/main.rs#L128-L136`,
`repo://crates/limedl-native/src/settings_sync.rs#L125-L131`,
`repo://crates/limedl-server/src/lib.rs#L104-L107`,
`repo://crates/limedl-rpc/src/server.rs` (the `Aria2RpcServer::new`
signature this call shape has to match).

## Shared services

### SettingsService — configuration single source of truth

`SettingsService` holds `Arc<RwLock<AppSettings>>` plus a `tokio::sync::Mutex`
`update_lock`. `update_with(mutate)` takes the lock, clones the **current**
persisted settings as the mutation base, applies the closure, then
`persist()` → `normalize_settings` → atomic disk write → replace the in-memory
value. The lock is what makes concurrent read-modify-write saves safe; without it
two saves could each start from the same stale base and the later one would drop
the other's change. A closure that returns an error leaves the persisted value
untouched.

Lives in `limedl-disk`, not `limedl-core`: settings normalization is shared with
`limedl-http` (the client factory reads the same struct) and nothing in it needs
the engine.

Evidence: `repo://crates/limedl-disk/src/services/settings_service.rs#L12-L73`.

### ConcurrencyManager — protocol-independent slot limits

`ConcurrencyManager` stores active HTTP/BT counters and their maxima as
`AtomicUsize`, an overclock `AtomicBool`, and a `Notify` used to wake the
scheduler on limit changes. `try_acquire_http` / `try_acquire_bt` are CAS loops:
on success they return a `DownloadSlotGuard` whose `Drop` decrements the counter.
`update_limits` and `set_overclock_mode` both notify waiters. The same manager is
shared by `DownloadManager` and `LazyBtBackend`, which is why `max_concurrent_bt`
can bound both protocols.

The manager lives in `limedl-disk` and the guard it hands out lives in
`limedl-types` (`slot_guard.rs`, 24 lines) — the guard is the one piece of the
pair `limedl-bt` also needs, so it had to sit below both.

Evidence: `repo://crates/limedl-disk/src/services/concurrency.rs#L10-L94`,
`repo://crates/limedl-types/src/slot_guard.rs#L1-L24`.

### DiskIoService — media resolution and buffer status

`DiskIoService` wraps the `BufferPool`, the `SettingsService` and the
`DiskDeviceManager`. `resolve_disk_type(dir)` first consults the
`disk_type_overrides` path-prefix match, then (on Unix) a per-device cache keyed
by `st_dev`, and only then performs OS detection. `apply_overrides` forwards new
overrides to the device manager so per-device write queues are rebuilt; it is
called from the settings save path because queue channel counts are fixed at
construction. `get_io_status()` reports buffer usage/limits, active slots,
queued count and per-device metrics, and game-mode toggling is delegated to the
buffer pool.

Evidence: `repo://crates/limedl-disk/src/services/disk_io.rs#L17-L138`.

## How a settings change fans out

`Dispatcher::save_settings_with(mutate)` is the write path used by both
frontends. It calls `SettingsService::update_with`, then:

- `registry.update_all_settings(&saved)` broadcasts to every backend (HTTP and
  BT), which is where `DownloadManager::apply_settings` rebuilds the HTTP client,
  rate limiter and buffer limits and where the BT engine hot-reloads its settings;
- clears the CDN service when `cdn_acceleration.enabled` is false;
- recomputes `ConcurrencyManager` max HTTP from the active scheduler mode and
  max BT from `bt.max_downloads` (clamped to 1..=1000), then notifies the
  scheduler;
- pushes buffer-pool limits directly into `DiskIoService` as a robustness
  fallback for minimal registries.

`save_settings` is just `save_settings_with` with a closure that replaces the
whole struct. `factory_reset` saves `AppSettings::default()` through the same
path. The daemon uses this path once at startup to persist `--download-dir` when
it is given.

Note that this path crosses four crates in one call: the facade is in
`limedl-core`, the service it drives is in `limedl-disk`, the settings tree is in
`limedl-types` and the backends it broadcasts to live in `limedl-core` and
`limedl-bt`. Keeping the *sequence* in one function is what makes that tractable.

Evidence: `repo://crates/limedl-core/src/dispatcher.rs#L281-L348`,
`repo://crates/limedl-server/src/lib.rs#L64-L76`.

## Lifecycle and shutdown

`SystemContext` carries a `CancellationToken` for cooperative cancellation of
background work. Frontends shut the engine down through
`registry.shutdown_all()`, which awaits each backend's `shutdown()`. The desktop
additionally flips the RPC `watch` channel and saves window geometry before
calling it; the daemon reaches the same call through a `tokio::select!` on the
SIGTERM/SIGINT future, the RPC server task, and the shutdown `Notify` that
`aria2.shutdown` fires.

Evidence: `repo://crates/limedl-core/src/context.rs#L26-L27`,
`repo://crates/limedl-core/src/backend_registry/mod.rs#L120-L126`,
`repo://crates/limedl-native/src/main.rs#L287-L300`,
`repo://crates/limedl-server/src/lib.rs#L106-L139`.

## Extension seams

- Add a new core subsystem inside one of the three places, in dependency order:
  a field on `SystemContext` (created in `with_components`), then a field on
  `CoreSystems` and construction in `bootstrap`. If the subsystem needs no engine
  type, put it in the crate that matches its concern (`limedl-disk`, `limedl-http`,
  `limedl-store`) rather than in `limedl-core`.
- A new protocol backend is registered with `register_arc` under its `TaskKind`
  and becomes reachable through `Dispatcher` for free; a backend that owns a
  heavy dependency should live in its own crate the way `limedl-bt` does.
- A new shared service should be exposed on `Dispatcher` for frontends rather
  than reaching into `SystemContext`, keeping the facade the single integration
  point.
- A new frontend should call `bootstrap`, construct its own transport over
  `registry`/`event_bus`, and own shutdown; it does not add construction logic to
  the core. `limedl-rpc` is the worked example: it is a consumer of the engine,
  not part of it.

Related pages: [Workspace and System Architecture](overview.md),
[Protocol Routing and the Dispatcher Facade](protocol-routing-and-dispatcher.md),
[Settings and Configuration](../systems/settings-and-configuration.md),
[Headless Server Daemon](../integrations/headless-server-daemon.md),
[Disk I/O, Buffer Pool and Storage Detection](../systems/disk-io-and-storage.md).

