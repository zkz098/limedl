---
id: architecture/bootstrap-and-services
title: Bootstrap, SystemContext and Shared Services
status: verified
generated_at: 2026-11-09T11:42:32Z
commit: e50d853ddda31dd2196341bd48fb7ca25d84e40c
evidence:
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 43
  - 46
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: 4c1789cdb0fd22da3366f19eb8b29253c2338b1b
  note: drift-projected L41-L44@e7734589 -> L43-L46 (similarity 1.00)
- path: crates/limedl-native/src/main.rs
  lines:
  - 113
  - 116
  blob: 9df6f3ec413bdfdd386fdcae9b5183feed2f7343
  snippet_sha1: 18dbb1deb1203241ff4672d56fc932d2d8a7b41e
  note: drift-projected L113-L116@de0b4727 -> L113-L116 (similarity 1.00)
- path: crates/limedl-server/src/lib.rs
  lines:
  - 64
  - 66
  blob: 1ad9a05546db8ebc2625bf165a1fbec487eb6375
  snippet_sha1: 7926ca2891a5beedb7bcc5fe841e16e814306f0f
  note: drift-projected L62-L64@db33f43e -> L64-L66 (similarity 1.00)
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 20
  - 41
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: 0f21351ef6746f579563d84d7e99ff3ceb288b17
  note: drift-projected L20-L39@e7734589 -> L20-L41 (similarity 1.00)
- path: crates/limedl-core/src/lib.rs
  lines:
  - 24
  - 51
  blob: c4e7c542d12b8a8da94a335fc916fb0ecd514c59
  snippet_sha1: 728c47e583f02814c14ca05f82ed820f59cf1530
  note: drift-projected L25-L52@8d1831bd -> L24-L51 (similarity 0.86)
- path: crates/limedl-core/src/lib.rs
  lines:
  - 12
  - 16
  blob: c4e7c542d12b8a8da94a335fc916fb0ecd514c59
  snippet_sha1: 276affb4c4f4b51d8e226109bc5a2eb48a125de2
  note: drift-projected L12-L16@8d1831bd -> L12-L16 (similarity 1.00)
- path: crates/limedl-engine/src/context.rs
  lines:
  - 29
  - 37
  blob: 26aa8d3d8e5c1669ccede7ef4f1c3f9670669554
  snippet_sha1: 78bdb895eda113ffac74690a4561aad24f382f87
  note: rename-detected crates/limedl-core/src/context.rs -> crates/limedl-engine/src/context.rs (identical blob)
- path: crates/limedl-engine/src/context.rs
  lines:
  - 42
  - 83
  blob: 26aa8d3d8e5c1669ccede7ef4f1c3f9670669554
  snippet_sha1: 66f6c6ed902162d38fa20a6d6903effd129d0594
  note: rename-detected crates/limedl-core/src/context.rs -> crates/limedl-engine/src/context.rs (identical blob)
- path: crates/limedl-engine/src/context.rs
  lines:
  - 86
  - 99
  blob: 26aa8d3d8e5c1669ccede7ef4f1c3f9670669554
  snippet_sha1: d7166eb2bff0fd195fb3e3d2b1dff3f3bb7ee948
  note: rename-detected crates/limedl-core/src/context.rs -> crates/limedl-engine/src/context.rs (identical blob)
- path: crates/limedl-server/src/config.rs
  lines:
  - 32
  - 43
  blob: 8ef47e880dcb943c9f06542eca334f8bcd2a1dcc
  snippet_sha1: 863c5a01f07d40ceb3064b4f9ab76d07c0d26cf4
- path: crates/limedl-engine/src/context.rs
  lines:
  - 1
  - 11
  blob: 26aa8d3d8e5c1669ccede7ef4f1c3f9670669554
  snippet_sha1: f0adbc0e071ea17278b4465e031b87c0a573be9d
  note: rename-detected crates/limedl-core/src/context.rs -> crates/limedl-engine/src/context.rs (identical blob)
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 46
  - 145
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: 41fc6ae867d826a75101f49a95e3a7587267a8e0
  note: drift-projected L44-L141@e7734589 -> L46-L145 (similarity 1.00)
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 58
  - 89
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: a251be30504a7adc5ed70f7b8006f2a412817a80
  note: drift-projected L55-L86@e7734589 -> L58-L89 (similarity 1.00)
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 104
  - 117
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: 33b587fbe2abd6e6f29efdb1c9278c6d1f75a29f
  note: drift-projected L101-L114@e7734589 -> L104-L117 (similarity 1.00)
- path: crates/limedl-core/src/backend_registry/mod.rs
  lines:
  - 41
  - 53
  blob: 7efae12f82ab6a73adf8c0c5771570a774137be5
  snippet_sha1: e8a6027d268e3991de061bbf5e5185c421db6949
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 91
  - 97
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: b9a0876b28c5945ea0b3c31fff5e3d393f4be1c1
  note: drift-projected L88-L94@e7734589 -> L91-L97 (similarity 1.00)
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 70
  - 78
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: 6e424828b0db299ae82300163b370baeb24041b8
  note: drift-projected L67-L75@e7734589 -> L70-L78 (similarity 1.00)
- path: crates/limedl-rpc/Cargo.toml
  lines:
  - 14
  - 18
  blob: 42e51e93cbcb31a1aed11f971714e6d7eb6c1b2d
  snippet_sha1: d4f9efb33d7a8ba231fced8fc4fc8dbe2ae6d013
  note: drift-projected L14-L18@6bad5d87 -> L14-L18 (similarity 1.00)
- path: crates/limedl-core/src/lib.rs
  lines:
  - 84
  - 86
  blob: c4e7c542d12b8a8da94a335fc916fb0ecd514c59
  snippet_sha1: 369ddb17061744f6b6c8865db8ccc5717b0a0c2a
  note: drift-projected L85-L87@8d1831bd -> L84-L86 (similarity 1.00)
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 131
  - 136
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: 15df8c54907208b9d1f5b3ee946113b91e45e769
  note: drift-projected L131-L136@90bfb55a -> L131-L136 (similarity 1.00)
- path: crates/limedl-native/src/main.rs
  lines:
  - 127
  - 141
  blob: 9df6f3ec413bdfdd386fdcae9b5183feed2f7343
  snippet_sha1: 60c26bc5a1bbaad149b83c508e363d4099c67b99
  note: drift-projected L127-L141@de0b4727 -> L127-L141 (similarity 1.00)
- path: crates/limedl-types/src/types/settings.rs
  lines:
  - 568
  - 573
  blob: d36d3f34aad95b687828c40c0c27eaae53fccddb
  snippet_sha1: 4cec7623415e6ce50c9f9d3384853f214e28b21d
- path: crates/limedl-native/src/settings_sync.rs
  lines:
  - 105
  - 138
  blob: 8ca897db1aea6202ac0108e09bcee9d5f69d17e1
  snippet_sha1: 38695c3f028d8c83d3c838fb6980e61781281b83
- path: crates/limedl-server/src/lib.rs
  lines:
  - 87
  - 107
  blob: 1ad9a05546db8ebc2625bf165a1fbec487eb6375
  snippet_sha1: b70a45dce1923306edf7e35506c6c63e6f6ef528
  note: drift-projected L85-L105@db33f43e -> L87-L107 (similarity 1.00)
- path: crates/limedl-disk/src/services/settings_service.rs
  lines:
  - 12
  - 73
  blob: 1d991cce6273f612487db886742e353e68364f83
  snippet_sha1: b1d16abf89186cfbaf24f8a1247ea2cf3c37629f
- path: crates/limedl-disk/src/services/concurrency.rs
  lines:
  - 37
  - 57
  blob: bb145d2ddf019912e6213e47df15b4be5656dacc
  snippet_sha1: 4ae89b28f6356916b580d3f0b231067d99b4a3c9
  note: drift-projected L37-L57@9cdcba1d -> L37-L57 (similarity 1.00)
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 341
  - 342
  blob: 2f774d1c7b11e7af6fb302a7063501181b4f8a0b
  snippet_sha1: b6e4f54d3efdeda071904ba1e29a74fd0610e786
  note: drift-projected L331-L332@0cc22fba -> L341-L342 (similarity 1.00)
- path: crates/limedl-disk/src/services/concurrency.rs
  lines:
  - 10
  - 23
  blob: bb145d2ddf019912e6213e47df15b4be5656dacc
  snippet_sha1: 299101b2f817d0d6bfe4fd24bb7accb74e5e6f6f
  note: drift-projected L10-L23@9cdcba1d -> L10-L23 (similarity 1.00)
- path: crates/limedl-types/src/slot_guard.rs
  lines:
  - 1
  - 46
  blob: f5e14f83880cd91e937ea5b85b44af7af538e362
  snippet_sha1: 2d38d6171ba25ad71f402727a4d3a70f40743bd7
  note: drift-projected L1-L24@788daafd -> L1-L46 (similarity 1.00)
- path: crates/limedl-disk/src/services/disk_io.rs
  lines:
  - 15
  - 135
  blob: ef3b86ce99edd9017d1c9b71ecdd56268d8e902a
  snippet_sha1: 1ea63e274182a4e85f162c26b9c46f51d67a52b6
  note: drift-projected L15-L135@20bead2f -> L15-L135 (similarity 0.91)
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 369
  - 373
  blob: 2361c059cc2d5b4fa1ca688ea8acdfce132258e7
  snippet_sha1: 84e99ce806e3c98621ac4cad57421f42bfffe658
  note: renamed crates/limedl-core/src/manager.rs -> crates/limedl-engine/src/manager.rs, drift-projected L368-L372 -> L369-L373 (similarity 1.00)
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 297
  - 358
  blob: 2f774d1c7b11e7af6fb302a7063501181b4f8a0b
  snippet_sha1: baf27842f9063b848d4030f5f0356663f1c4ae62
  note: drift-projected L297-L348@0cc22fba -> L297-L358 (similarity 1.00)
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 351
  - 379
  blob: 2361c059cc2d5b4fa1ca688ea8acdfce132258e7
  snippet_sha1: 727a99958d7cda31f22e493826cf0510509c11ff
  note: renamed crates/limedl-core/src/manager.rs -> crates/limedl-engine/src/manager.rs, drift-projected L350-L378 -> L351-L379 (similarity 1.00)
- path: crates/limedl-bt/src/lazy.rs
  lines:
  - 879
  - 882
  blob: 7b6185c3e58c5da27f87e499637f4a37fd9acf57
  snippet_sha1: a629625ca48d2e5936fe572cd7bcf4d120fa21a4
  note: drift-projected L879-L882@e73993a3 -> L879-L882 (similarity 1.00)
- path: crates/limedl-server/src/lib.rs
  lines:
  - 68
  - 78
  blob: 1ad9a05546db8ebc2625bf165a1fbec487eb6375
  snippet_sha1: 72ab92b0e09c86b3b5d9379ae20d6c95774d0f8e
  note: drift-projected L66-L76@db33f43e -> L68-L78 (similarity 1.00)
- path: crates/limedl-core/src/backend_registry/mod.rs
  lines:
  - 116
  - 121
  blob: 7efae12f82ab6a73adf8c0c5771570a774137be5
  snippet_sha1: 355fbf199a3f37546e6626abee6c61c5049e34af
- path: crates/limedl-native/src/main.rs
  lines:
  - 318
  - 328
  blob: 9df6f3ec413bdfdd386fdcae9b5183feed2f7343
  snippet_sha1: bdec97fc40c3f60c1d0e8e349813258b8a3ee84a
  note: drift-projected L305-L315@de0b4727 -> L318-L328 (similarity 1.00)
- path: crates/limedl-server/src/lib.rs
  lines:
  - 112
  - 156
  blob: 1ad9a05546db8ebc2625bf165a1fbec487eb6375
  snippet_sha1: 815b1949e49dcb50786dc51de1f9644867795378
  note: drift-projected L110-L141@db33f43e -> L112-L156 (similarity 0.88)
type: architecture
---
# Bootstrap, SystemContext and Shared Services

Every frontend of limedl starts from one function: `bootstrap(state_dir)` in
`crates/limedl-core/src/bootstrap.rs`, documented there as the "SINGLE canonical
initialization sequence used by both the desktop client and NAS server targets".
[[cite:crates/limedl-core/src/bootstrap.rs:L41-L44]] There are two frontends: the
Slint desktop client (`crates/limedl-native/`) and the headless `limedl-server`
daemon (`crates/limedl-server/`), and both call it exactly once at startup.
[[cite:crates/limedl-native/src/main.rs:L113-L116]]
[[cite:crates/limedl-server/src/lib.rs:L62-L64]] `bootstrap` is deliberately the
*only* place that constructs the engine subsystems, so the initialization order
and the Arc-sharing rules below apply uniformly and a new subsystem is added in
exactly one place — the daemon inherits the whole engine graph for free.

## CoreSystems: the returned handle bundle

`bootstrap` returns a `CoreSystems` struct whose fields are `Arc`s (plus a
settings snapshot): [[cite:crates/limedl-core/src/bootstrap.rs:L20-L39]]

| Field | Purpose | Type lives in |
| --- | --- | --- |
| `context: Arc<SystemContext>` | DB, EventBus, RateLimiter, BufferPool, device manager, concurrency, settings, disk I/O, shutdown token | `limedl-core` |
| `download_manager: Arc<DownloadManager>` | task creation, settings application, facades over the HTTP engine | `limedl-core` |
| `bt_backend: Arc<LazyBtBackend>` | BitTorrent engine wrapper (behind the `bt` feature, on by default) | `limedl-bt` |
| `registry: Arc<BackendRegistry>` | protocol routing table | `limedl-core` |
| `dispatcher: Arc<Dispatcher>` | unified facade used by every frontend | `limedl-core` |
| `event_bus`, `rate_limiter`, `cdn_service`, `settings_service`, `disk_io_service`, `concurrency` | shared services re-exported for callers | `limedl-types` (EventBus), `limedl-http` (RateLimiter), `limedl-disk` (settings/disk/concurrency services), `limedl-engine` (CdnService) |
| `legacy_bt_resume_files: usize` | count of resume files removed from the pre-librqbit BT engine during bootstrap; the desktop turns it into a one-time notice | `limedl-core` |

The right-hand column is the crate split made visible: the facade re-exports the
extracted layers as modules (`pub use limedl_types::{… event_bus …}`,
`pub use limedl_disk::{… services …}`, `pub use limedl_engine::{… cdn …}`), so
`bootstrap` is the one function that has to name a type from almost every engine
crate, which is why it stayed in `limedl-core` and why it is the top of the
engine's dependency graph. [[cite:crates/limedl-core/src/lib.rs:L25-L52]]
[[cite:crates/limedl-core/src/lib.rs:L12-L16]]

## SystemContext: where global state is created

`SystemContext` is a `Clone` bundle of shared infrastructure. Its `new()` creates
a default `RateLimiter` and an `EventBus` with capacity **8192**, then delegates
to `with_components`; tests and CLI paths can inject their own rate limiter and
event bus through that second constructor. [[cite:crates/limedl-core/src/context.rs:L29-L37]]

`with_components` performs the state-dependent construction: [[cite:crates/limedl-core/src/context.rs:L42-L83]]
[[cite:crates/limedl-core/src/context.rs:L86-L99]]

- `state_dir` is created if missing, and the settings file is derived as its
  **parent** joined with `settings.json` — the state directory itself is
  `<data_dir>/downloads`, so settings live one level up (the daemon's
  `resolve_data_dir` follows the same rule).
  [[cite:crates/limedl-server/src/config.rs:L32-L43]]
- `SettingsService::new` loads and normalizes `settings.json`.
- `Database::open(state_dir/downloads.db)`.
- `BufferPool::new(...)` sized from `io_baseline` (buffer limit, game-mode
  buffer, HDD parallelism, game-mode parallelism).
- `DiskDeviceManager::new()` plus `set_overrides(&disk_type_overrides)` so the
  device queue is built from persisted media overrides *before* any settings
  save happens.
- `IoWorker::new(device_manager)` — the thread count now comes from the device
  queue itself (one serialised writer for HDD, 4 channels for SSD/network), so
  there is no pool-size knob here any more.
- `ConcurrencyManager::new(5, 3)` (5 HTTP, 3 BT) and `DiskIoService`.
- A fresh `CancellationToken` for shutdown.

`SystemContext` names types from four crates at once (`limedl-store`'s `Database`,
`limedl-disk`'s buffer pool / device manager / services, `limedl-http`'s
`RateLimiter`, `limedl-types`' `EventBus`), and it is the reason `limedl-disk`
must not depend on `limedl-core`: the dependency points the other way.
[[cite:crates/limedl-core/src/context.rs:L1-L11]]

## The canonical initialization order

`bootstrap` builds components in a strong dependency chain:
[[cite:crates/limedl-core/src/bootstrap.rs:L44-L141]]

1. `SystemContext::new(state_dir)`, then `settings_service.get_blocking()` for the
   startup snapshot.
2. `DownloadManager::new(&context)`, wrapped in `Arc`, then
   `download_manager.scheduler.clone().start_scheduler_loop(download_manager.clone())`
   starts the background scheduler.
3. The BT backend: `take_legacy_resume_files()` first strips resume files left by
   the pre-librqbit engine (the count is reported to the user), then
   `LazyBtBackend::new(...)` is cheap — the session (socket binds, DHT/LSD,
   resume-state load, blocklist parse) deliberately starts in the background so
   app startup does not wait for it. `spawn_index_sync_loop()` keeps the
   persisted index fresh and `spawn_idle_supervisor()` unloads an idle engine in
   lightweight mode. The warm-up itself is started by `spawn_startup()`, and only
   when either lightweight mode is off **or** the index already holds unfinished
   work. Everything in this step is `limedl-bt`, reachable only when the `bt`
   feature pulls it in. [[cite:crates/limedl-core/src/bootstrap.rs:L55-L86]]
4. `BackendRegistry::new()`, then `register_arc(TaskKind::Http, ...)` and
   `register_arc(TaskKind::Bt, ...)`.
5. `CdnService::new()`, injected into the manager via
   `download_manager.set_cdn_accelerator(...)` and restored from settings via
   `init_from_settings`.
6. A dedicated `reqwest::Client` built with
   `http_client_factory::configure_client_builder` (so Dispatcher-side requests
   honour proxy and User-Agent — a bare `Client::builder()` here once silently
   bypassed `settings.proxy`) plus a 5-redirect policy, 15 s timeout and
   `limedl/<version>` UA. [[cite:crates/limedl-core/src/bootstrap.rs:L101-L114]]
7. `Dispatcher::full(...)` with the registry, event bus, settings service, disk
   I/O, concurrency, CDN service and that HTTP client.

## Arc identity is load-bearing

The registry is populated with `register_arc`, not `register`. The distinction is
documented on the method: `register` wraps a backend by value in a fresh `Arc`,
while `register_arc` stores the caller's existing `Arc` — using `register` with a
cloned backend "would snapshot mutable state (e.g. fresh atomics) and silently
diverge from the caller's copy".
[[cite:crates/limedl-core/src/backend_registry/mod.rs:L41-L53]]
Bootstrap's own comment repeats the rule ("Create registry — register_arc so the
registry shares the SAME Arc instances as CoreSystems").
[[cite:crates/limedl-core/src/bootstrap.rs:L88-L94]] The correlation counters,
overclock flag and BT slot state must be the *same* objects the frontends talk
to; the BT counters handed to `LazyBtBackend::new` are exactly the context's
`active_bt_count` / `max_concurrent_bt` clones. [[cite:crates/limedl-core/src/bootstrap.rs:L67-L75]]

## What bootstrap deliberately does not build

`Aria2RpcServer` is **not** part of `bootstrap`, and after the crate split that
is no longer a feature flag inside the engine — the RPC layer is a separate
crate, `limedl-rpc`, that sits *above* `limedl-core`. `limedl-core` has no
dependency on it at all ("One direction only: the RPC layer speaks to the engine
through `limedl-core`'s facade"), which is what keeps `axum`, `tower-http` and
`argon2` out of the engine's dependency graph.
[[cite:crates/limedl-rpc/Cargo.toml:L14-L18]]
[[cite:crates/limedl-core/src/lib.rs:L85-L87]]

Each frontend constructs it after `bootstrap` returns, from `core.registry`,
`core.event_bus` and its own `Aria2RpcSettings` — the three-argument shape of
`Aria2RpcServer::new(registry, settings, event_bus)` is what both call sites have
to match. [[cite:crates/limedl-rpc/src/server.rs:L131-L136]]

- the **desktop** starts it only when `settings.aria2_rpc.enabled` is true
  [[cite:crates/limedl-native/src/main.rs:L127-L141]], binds the configured
  address (loopback by default: `default_aria2_listen_address()` returns
  `127.0.0.1`; a non-loopback bind must be passed explicitly and then requires
  authentication) [[cite:crates/limedl-types/src/types/settings.rs:L568-L573]],
  and keeps the `watch` shutdown sender in the UI context so a settings save can
  hot-reload it — `sync_aria2_rpc` tears the old server down through that channel
  and respawns on change. [[cite:crates/limedl-native/src/settings_sync.rs:L105-L138]]
- the **daemon** force-enables it (`enabled = true`) because the RPC endpoint *is*
  its interface, forces `exit_on_shutdown = true` so `aria2.shutdown` stops the
  process, and selects on the signal/shutdown handles before calling
  `core.registry.shutdown_all()`. [[cite:crates/limedl-server/src/lib.rs:L85-L105]]

## Shared services

### SettingsService — configuration single source of truth

`SettingsService` holds `Arc<RwLock<AppSettings>>` plus a `tokio::sync::Mutex`
`update_lock`. `update_with(mutate)` takes the lock, clones the **current**
persisted settings as the mutation base, applies the closure, then
`persist()` → `normalize_settings` → atomic disk write → replace the in-memory
value. The lock is what makes concurrent read-modify-write saves safe; without it
two saves could each start from the same stale base and the later one would drop
the other's change. A closure that returns an error leaves the persisted value
untouched (`mutate(&mut candidate)?` aborts before `persist`).

Lives in `limedl-disk`, not `limedl-core`: settings normalization is shared with
`limedl-http` (the client factory reads the same struct) and nothing in it needs
the engine. [[cite:crates/limedl-disk/src/services/settings_service.rs:L12-L73]]

### ConcurrencyManager — protocol-independent slot limits

`ConcurrencyManager` stores active HTTP/BT counters and their maxima as
`AtomicUsize`, an overclock `AtomicBool`, and a `Notify` used to wake the
scheduler on limit changes. `try_acquire_http` / `try_acquire_bt` are CAS loops:
on success they return a `DownloadSlotGuard` whose `Drop` decrements the counter.
`set_overclock_mode` and `toggle_overclock_mode` notify waiters themselves;
`update_limits` only stores the two maxima — the scheduler wake-up on a limit
change is issued by the caller instead, in `Dispatcher::save_settings_with`
(`concurrency.rebalance_notify.notify_waiters()` right after `update_limits`).
[[cite:crates/limedl-disk/src/services/concurrency.rs:L37-L57]]
[[cite:crates/limedl-core/src/dispatcher.rs:L331-L332]] The same manager is
shared by `DownloadManager` and `LazyBtBackend`, which is why `max_concurrent_bt`
can bound both protocols. [[cite:crates/limedl-disk/src/services/concurrency.rs:L10-L23]]

The manager lives in `limedl-disk` and the guard it hands out lives in
`limedl-types` (`slot_guard.rs`, 24 lines) — the guard is the one piece of the
pair `limedl-bt` also needs, so it had to sit below both.
[[cite:crates/limedl-types/src/slot_guard.rs:L1-L24]]

### DiskIoService — media resolution and buffer status

`DiskIoService` wraps the `BufferPool`, the `SettingsService` and the
`DiskDeviceManager`. `resolve_disk_type(dir)` first consults the
`disk_type_overrides` path-prefix match, then (on Unix) a per-device cache keyed
by `st_dev`, and only then performs OS detection. `apply_overrides` forwards new
overrides to the device manager so per-device write queues are rebuilt; it is
called from the settings-save path (`DownloadManager::apply_settings`) because
queue channel counts are fixed at construction. `get_io_status()` reports buffer
usage/limits, active slots, queued count and per-device metrics, and game-mode
toggling is delegated to the buffer pool.
[[cite:crates/limedl-disk/src/services/disk_io.rs:L15-L135]]
[[cite:crates/limedl-core/src/manager.rs:L368-L372]]

## How a settings change fans out

`Dispatcher::save_settings_with(mutate)` is the write path used by both
frontends. It calls `SettingsService::update_with`, then: [[cite:crates/limedl-core/src/dispatcher.rs:L297-L348]]

- `registry.update_all_settings(&saved)` broadcasts to every backend (HTTP and
  BT), which is where `DownloadManager::apply_settings` hot-reloads the rate
  limiter, buffer-pool limits, media overrides and — only when proxy or
  user-agent actually changed — rebuilds the HTTP client,
  [[cite:crates/limedl-core/src/manager.rs:L350-L378]]
  and where the BT backend applies its settings through
  `LazyBtBackend::update_settings`. [[cite:crates/limedl-bt/src/lazy.rs:L879-L882]]
- clears the CDN service when `cdn_acceleration.enabled` is false;
- recomputes `ConcurrencyManager` max HTTP from the active scheduler mode and
  max BT from `bt.max_downloads` (clamped to 1..=1000), then notifies the
  scheduler;
- pushes buffer-pool limits directly into `DiskIoService` as a robustness
  fallback for minimal registries.

`save_settings` is just `save_settings_with` with a closure that replaces the
whole struct. `factory_reset` saves `AppSettings::default()` through the same
path. The daemon uses this path once at startup to persist `--download-dir` when
it is given. [[cite:crates/limedl-server/src/lib.rs:L66-L76]]

Note that this path crosses four crates in one call: the facade is in
`limedl-core`, the service it drives is in `limedl-disk`, the settings tree is in
`limedl-types` and the backends it broadcasts to live in `limedl-core` and
`limedl-bt`. Keeping the *sequence* in one function is what makes that tractable.

## Lifecycle and shutdown

`SystemContext` carries a `CancellationToken` for cooperative cancellation of
background work. [[cite:crates/limedl-core/src/context.rs:L86-L99]] Frontends shut
the engine down through `registry.shutdown_all()`, which awaits each backend's
`shutdown()` in iteration order. [[cite:crates/limedl-core/src/backend_registry/mod.rs:L116-L121]]
The desktop additionally saves window geometry and flips the RPC `watch` channel
before calling it; [[cite:crates/limedl-native/src/main.rs:L305-L315]] the daemon
reaches the same call through a `tokio::select!` on the SIGTERM/SIGINT future,
the RPC server task, and the shutdown `Notify` that `aria2.shutdown` fires.
[[cite:crates/limedl-server/src/lib.rs:L110-L141]]

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
  not part of it. [[cite:crates/limedl-rpc/Cargo.toml:L14-L18]]

Related pages: [[wiki:architecture/overview]],
[[wiki:architecture/protocol-routing-and-dispatcher]],
[[wiki:systems/settings-and-configuration]],
[[wiki:integrations/headless-server-daemon]],
[[wiki:systems/disk-io-and-storage]].
