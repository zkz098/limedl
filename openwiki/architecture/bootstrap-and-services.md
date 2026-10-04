---
type: architecture
title: Bootstrap, SystemContext and Shared Services
description: The single canonical initialization sequence that builds SystemContext, DownloadManager, the lazy BT backend, the BackendRegistry, CDN service and Dispatcher, plus the shared services that own global runtime state and the two frontends that consume them.
tags: [bootstrap, systemcontext, services, initialization, dependency-injection]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T12:36:56.946Z
sources:
  - id: openwiki-source-2262be0eb4e0dcf867247c95
    resource: repo://crates/limedl-core/src/backend_registry/mod.rs
  - id: openwiki-source-fd95ce7448707ad4ddaa3b43
    resource: repo://crates/limedl-core/src/bootstrap.rs
  - id: openwiki-source-fd061a9c15d2a04bc703746d
    resource: repo://crates/limedl-core/src/context.rs
  - id: openwiki-source-4b83dfd7538606d5871938ef
    resource: repo://crates/limedl-core/src/dispatcher.rs
  - id: openwiki-source-76cf0b396abab954f820977a
    resource: repo://crates/limedl-core/src/services/concurrency.rs
  - id: openwiki-source-9407da3da7a807b7713a5e9d
    resource: repo://crates/limedl-core/src/services/disk_io.rs
  - id: openwiki-source-2468c28a2ebe626471835de0
    resource: repo://crates/limedl-core/src/services/settings_service.rs
  - id: openwiki-source-a6c9307e4c12f6dae4a8f919
    resource: repo://crates/limedl-core/src/slot_guard.rs
  - id: openwiki-source-fb048d5fb7a13a8f9b8fad76
    resource: repo://crates/limedl-native/src/main.rs
  - id: openwiki-source-2d1753b77bfe7d551752205e
    resource: repo://crates/limedl-server/src/lib.rs
generated: { by: "pi", at: "2026-10-04T12:36:56.946Z" }
---

# Bootstrap, SystemContext and Shared Services

Every frontend of limedl starts from one function: `bootstrap(state_dir)` in
`crates/limedl-core/src/bootstrap.rs`. There are two frontends: the Slint desktop
client (`crates/limedl-native/`) and the headless `limedl-server` daemon
(`crates/limedl-server/`). `bootstrap` is deliberately the *only* place that
constructs the core subsystems, so the initialization order and the Arc-sharing
rules below apply uniformly and a new subsystem is added in exactly one place —
the daemon inherits the whole engine graph for free.

## CoreSystems: the returned handle bundle

`bootstrap` returns a `CoreSystems` struct whose fields are `Arc`s (plus a
settings snapshot):

| Field | Purpose |
| --- | --- |
| `context: Arc<SystemContext>` | DB, EventBus, RateLimiter, BufferPool, device manager, concurrency, settings, disk I/O, shutdown token |
| `download_manager: Arc<DownloadManager>` | HTTP engine |
| `bt_backend: Arc<LazyBtBackend>` | BitTorrent engine wrapper (behind the `bt` feature, on by default) |
| `registry: Arc<BackendRegistry>` | protocol routing table |
| `dispatcher: Arc<Dispatcher>` | unified facade used by every frontend |
| `event_bus`, `rate_limiter`, `cdn_service`, `settings_service`, `disk_io_service`, `concurrency` | shared services re-exported for callers |
| `settings: AppSettings` | the startup snapshot read from `SettingsService` |

Evidence: `repo://crates/limedl-core/src/bootstrap.rs#L21-L34`.

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

Evidence: `repo://crates/limedl-core/src/context.rs#L16-L100`.

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
   work.
4. `BackendRegistry::new()`, then `register_arc(TaskKind::Http, ...)` and
   `register_arc(TaskKind::Bt, ...)`.
5. `CdnService::new()`, injected into the manager via
   `download_manager.set_cdn_accelerator(...)` and restored from settings via
   `init_from_settings`.
6. A dedicated `reqwest::Client` built with `configure_client_builder` (so
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

Evidence: `repo://crates/limedl-core/src/backend_registry/mod.rs#L42-L60`,
`repo://crates/limedl-core/src/bootstrap.rs#L81-L87`.

## What bootstrap deliberately does not build

`Aria2RpcServer` is **not** part of `bootstrap`. The RPC server is optional and
feature-gated (`aria2-rpc`), and it needs a shutdown channel the owner controls.
Each frontend constructs it after `bootstrap` returns, from `core.registry`,
`core.event_bus` and its own `Aria2RpcSettings`:

- the **desktop** starts it only when `settings.aria2_rpc.enabled` is true, binds
  the configured address (loopback by default), and keeps the `watch` shutdown
  sender in the UI context so a settings save can hot-reload it;
- the **daemon** force-enables it (`enabled = true`) because the RPC endpoint *is*
  its interface, forces `exit_on_shutdown = true` so `aria2.shutdown` stops the
  process, and selects on the signal/shutdown handles before calling
  `core.registry.shutdown_all()`.

Evidence: `repo://crates/limedl-native/src/main.rs#L131-L145`,
`repo://crates/limedl-server/src/lib.rs#L83-L139`,
`repo://crates/limedl-core/src/lib.rs#L69-L74`.

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

Evidence: `repo://crates/limedl-core/src/services/settings_service.rs#L10-L73`.

### ConcurrencyManager — protocol-independent slot limits

`ConcurrencyManager` stores active HTTP/BT counters and their maxima as
`AtomicUsize`, an overclock `AtomicBool`, and a `Notify` used to wake the
scheduler on limit changes. `try_acquire_http` / `try_acquire_bt` are CAS loops:
on success they return a `DownloadSlotGuard` whose `Drop` decrements the counter.
`update_limits` and `set_overclock_mode` both notify waiters. The same manager is
shared by `DownloadManager` and `LazyBtBackend`, which is why `max_concurrent_bt`
can bound both protocols.

Evidence: `repo://crates/limedl-core/src/services/concurrency.rs#L8-L94`,
`repo://crates/limedl-core/src/slot_guard.rs#L8-L24`.

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

Evidence: `repo://crates/limedl-core/src/services/disk_io.rs#L11-L138`.

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

Evidence: `repo://crates/limedl-core/src/context.rs#L27-L30`,
`repo://crates/limedl-core/src/backend_registry/mod.rs#L120-L125`,
`repo://crates/limedl-native/src/main.rs#L287-L300`,
`repo://crates/limedl-server/src/lib.rs#L106-L139`.

## Extension seams

- Add a new core subsystem inside one of the three places, in dependency order:
  a field on `SystemContext` (created in `with_components`), then a field on
  `CoreSystems` and construction in `bootstrap`.
- A new protocol backend is registered with `register_arc` under its `TaskKind`
  and becomes reachable through `Dispatcher` for free.
- A new shared service should be exposed on `Dispatcher` for frontends rather
  than reaching into `SystemContext`, keeping the facade the single integration
  point.
- A new frontend should call `bootstrap`, construct its own transport over
  `registry`/`event_bus`, and own shutdown; it does not add construction logic to
  the core.

Related pages: [Workspace and System Architecture](overview.md),
[Protocol Routing and the Dispatcher Facade](protocol-routing-and-dispatcher.md),
[Settings and Configuration](../systems/settings-and-configuration.md),
[Headless Server Daemon](../integrations/headless-server-daemon.md),
[Disk I/O, Buffer Pool and Storage Detection](../systems/disk-io-and-storage.md).
