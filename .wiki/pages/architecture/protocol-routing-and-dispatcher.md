---
id: architecture/protocol-routing-and-dispatcher
title: Protocol Routing and the Dispatcher Facade
status: verified
generated_at: 2026-10-31T14:51:29Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence:
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 13
  - 20
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: c7d7baca7ff3621bda1a0fe117e77b0a89f69dac
- path: crates/limedl-types/src/protocol.rs
  lines:
  - 8
  - 50
  blob: 7911ac44f1635ad31f54f84e8326e5af07c5c4a6
  snippet_sha1: 87fc749b71defbc9e53bdaaf8a2f2dd7d3977a6d
- path: crates/limedl-core/src/lib.rs
  lines:
  - 25
  - 28
  blob: 8d1831bda64242c4d6dbe2820d6f8cab738ac5e0
  snippet_sha1: 7ac352637baf232df044cb6fca9a68aef6ecaed9
- path: crates/limedl-core/src/backend_registry/mod.rs
  lines:
  - 9
  - 14
  blob: 7efae12f82ab6a73adf8c0c5771570a774137be5
  snippet_sha1: 184d6b5831e7045abf2099c571678b497b76cac8
- path: crates/limedl-core/src/backend_registry/mod.rs
  lines:
  - 41
  - 62
  blob: 7efae12f82ab6a73adf8c0c5771570a774137be5
  snippet_sha1: e93c6230701a05983d1ee43438fa296322f6fb65
- path: crates/limedl-core/src/backend_registry/mod.rs
  lines:
  - 82
  - 93
  blob: 7efae12f82ab6a73adf8c0c5771570a774137be5
  snippet_sha1: 0970652b8278fc98f61c4ed6daa7613bcb306c0b
- path: crates/limedl-core/src/backend_registry/mod.rs
  lines:
  - 95
  - 114
  blob: 7efae12f82ab6a73adf8c0c5771570a774137be5
  snippet_sha1: 4f6d99eea483ee64383a45fef73815141408345d
- path: crates/limedl-core/src/backend_registry/mod.rs
  lines:
  - 116
  - 121
  blob: 7efae12f82ab6a73adf8c0c5771570a774137be5
  snippet_sha1: 355fbf199a3f37546e6626abee6c61c5049e34af
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 43
  - 106
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 2840d852130fc1fd165e442ff35c6f5a63d8713b
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 261
  - 281
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 8a0d5ac5fd9ec2ae9775790f5854a9c602f47be1
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 148
  - 170
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 35e6a0a39292b4455eb4bbb0663b8d28833fbde5
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 172
  - 210
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: e75b849a772d1a7f6b3cad093bd35cd74b5b7055
- path: crates/limedl-rpc/src/context.rs
  lines:
  - 145
  - 149
  blob: 90629ec9cbc7eba44af2b1ff515327b00c9ded30
  snippet_sha1: 700c4ef497c921cbf4eea45401d4380278ec7e84
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 508
  - 520
  blob: e2b0f898b2e1fea578b56a582f8dd6d4165d682a
  snippet_sha1: 965e51d0a4b88303383ccd642d54c25d95cfe7fc
- path: crates/limedl-bt/src/alerts.rs
  lines:
  - 70
  - 124
  blob: a76fb8f533624b24bc4074a39ac715c662739fff
  snippet_sha1: 695be9600e97a93ccb2467eac2a48595ef0e1d84
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 212
  - 220
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: e848fb21f886a81e7ee8b2d6c4dd7c393f795b0c
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 222
  - 244
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 06801961b510f8c9b1c6b092212e0c67bc93fddb
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 246
  - 257
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 1449484a81b3d4c28845cd2ace70f8c016d2c3d6
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 259
  - 475
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 4ee5dc2ddc17b5584114508dec799c15e93a9ff6
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 379
  - 405
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 4405495c2529856b950697c5c53223cfd2cc03ff
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 573
  - 596
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 80eb18e5c0ffe47ca919212b4e2465471e676dc1
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 479
  - 571
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: b3cd92e8752bb1911bb206b5827a6a3f6706141d
- path: crates/limedl-rpc/src/context.rs
  lines:
  - 71
  - 86
  blob: 90629ec9cbc7eba44af2b1ff515327b00c9ded30
  snippet_sha1: 6c0349557dc6568f1877016b653c47fe3ead24ce
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 16
  - 20
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: ed5b7229d33ee78ba9e5ef7ef683a46b82e75cf6
- path: crates/limedl-core/src/backend_registry/mod.rs
  lines:
  - 41
  - 53
  blob: 7efae12f82ab6a73adf8c0c5771570a774137be5
  snippet_sha1: e8a6027d268e3991de061bbf5e5185c421db6949
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 13
  - 15
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 2d25d550d4727765d73450614701e1e3c6b79f9d
type: architecture
---
# Protocol Routing and the Dispatcher Facade

The engine supports more than one download protocol, so limedl separates three
concerns:

1. a uniform `DownloadBackend` interface every protocol implements (in
   `limedl-types`);
2. a `BackendRegistry` that routes by task identity (in `limedl-core`);
3. a `Dispatcher` facade that frontends call, so neither the desktop UI nor the
   Aria2 RPC layer re-implements cross-protocol logic.

The facade's own module doc states the scope rule: cross-protocol operations must
go through this layer, and the aria2 surface (the `limedl-rpc` crate, which sits
*above* it) is deliberately not fully generic.
[[cite:crates/limedl-core/src/dispatcher.rs:L13-L20]]

## The DownloadBackend trait

`DownloadBackend` is an `async_trait` with `Send + Sync + 'static`. The required
surface is the download lifecycle (`start`, `pause`, `resume`, `cancel`, `remove`,
`purge`), file/folder opening (`open_in_explorer`), reads (`status`, `list`),
configuration (`update_settings`) and `shutdown`. Two methods have defaults:
`open_file`/`open_dir` fall back to `open_in_explorer`, and `set_priority` returns
an error unless the backend overrides it.

`start` returns a strongly-typed `TaskId` (not a string), so the registry can route
subsequent operations without parsing.

The trait lives in `limedl-types` — the bottom of the graph — precisely so
`limedl-bt` can implement it from the middle of the stack; `limedl-core` re-exports
it under the old path. [[cite:crates/limedl-types/src/protocol.rs:L8-L50]]
[[cite:crates/limedl-core/src/lib.rs:L25-L28]]

## BackendRegistry

`BackendRegistry` keeps three structures: [[cite:crates/limedl-core/src/backend_registry/mod.rs:L9-L14]]

- `by_kind: HashMap<TaskKind, Arc<dyn DownloadBackend>>` — the routing table;
- `by_type: HashMap<TypeId, Arc<dyn Any + Send + Sync>>` — concrete-type lookup
  for protocol-specific commands;
- `all: Vec<(TaskKind, Arc<dyn DownloadBackend>)>` — iteration order for list,
  settings broadcast and shutdown.

`dispatch(&TaskId)` is `by_kind.get(&task_id.kind())`. `register_arc` stores the
caller's `Arc` directly (unlike `register`, which wraps a value), which is the
mechanism that keeps `CoreSystems` and the registry pointing at the same atomics.
[[cite:crates/limedl-core/src/backend_registry/mod.rs:L41-L62]]

Cross-cutting helpers:

- `list_all()` calls every backend's `list()`, logs and skips a failing backend,
  and sorts the merged result by `created_at_ms` **descending**.
  [[cite:crates/limedl-core/src/backend_registry/mod.rs:L82-L93]]
- `update_all_settings()` applies settings to every backend, records the first
  error but still applies to the rest, then returns that first error.
  [[cite:crates/limedl-core/src/backend_registry/mod.rs:L95-L114]]
- `shutdown_all()` awaits each backend's `shutdown()` in registration order.
  [[cite:crates/limedl-core/src/backend_registry/mod.rs:L116-L121]]

## Dispatcher composition

`Dispatcher` is `Clone` and holds the registry and event bus as required fields,
plus optional `SettingsService`, `DiskIoService`, `ConcurrencyManager`,
`CdnService` and a `reqwest::Client`. Three constructors exist for different
environments: [[cite:crates/limedl-core/src/dispatcher.rs:L43-L106]]

- `new(registry, event_bus)` — minimal, no services (tests);
- `with_settings_service(...)` — real settings read/write but no disk/CDN/concurrency
  (the UI test fixture);
- `full(...)` — the production wiring used by `bootstrap`.

Methods that need an absent optional service return a `DownloadError::Internal`
rather than panicking — e.g. `get_settings` errors with "SettingsService not
configured" — which is what lets the minimal constructors be useful.
[[cite:crates/limedl-core/src/dispatcher.rs:L261-L281]]

## Lifecycle routing and automatic event emission

`Dispatcher::start` first fills in mirror URLs from the settings' URL-rewrite rules
when the caller supplied none, classifies the request, routes to the backend by
kind, and after a successful start fetches `backend.status` to emit an initial
`Updated` event for immediate UI synchronization.
[[cite:crates/limedl-core/src/dispatcher.rs:L148-L170]]

Every mutating lifecycle operation (`pause`, `resume`, `cancel`, `remove`,
`purge`) routes through the registry, calls the backend, and on success emits
`DownloadEvent::Updated` from the returned snapshot.
[[cite:crates/limedl-core/src/dispatcher.rs:L172-L210]]
This is why the frontends never emit lifecycle events themselves: the Aria2 layer
pushes its compatibility notifications through a single `broadcast_event` helper,
completion and progress events come from the engine's lifecycle/executor layer,
and BT lifecycle events are published by `limedl-bt`'s alert/state-monitor bridge —
each of those publishers owns its own `EventBus` handle rather than going through
the Dispatcher. [[cite:crates/limedl-rpc/src/context.rs:L145-L149]]
[[cite:crates/limedl-engine/src/task_lifecycle/mod.rs:L508-L520]]
[[cite:crates/limedl-bt/src/alerts.rs:L70-L124]]

`set_priority` follows the same pattern but re-reads `status` after the mutation
before emitting — the event carries a snapshot, so updating only the manifest
would leave the UI priority stale until the next refresh.
[[cite:crates/limedl-core/src/dispatcher.rs:L212-L220]]

`open_in_explorer`, `open_file`, `open_dir` and `status` do **not** emit: they are
either side effects or read-only. [[cite:crates/limedl-core/src/dispatcher.rs:L222-L244]]

## Aggregation

- `list()` delegates to `registry.list_all()`.
- `has_active_downloads()` lists every backend and returns true if any summary is
  in `DownloadState::Downloading`. This is the cross-protocol aggregation the tray
  uses, so a running BT torrent is not missed by an HTTP-only check.
  [[cite:crates/limedl-core/src/dispatcher.rs:L246-L257]]

## Settings, disk and concurrency through the facade

`Dispatcher` exposes the settings read/write path (`get_settings`,
`get_settings_blocking`, `save_settings`, `save_settings_with`, `factory_reset`,
`default_download_dir`) and delegates disk/concurrency operations
(`detect_disk_type`, `detect_all_disk_types`, `get_io_status`, `toggle_game_mode`,
`game_mode`, `get_overclock_mode`, `toggle_overclock_mode`) to the optional
services; where a disk-only dispatcher lacks the service, detection falls back to
the free function in `limedl-disk` rather than erroring. The detailed settings
fan-out is documented on the [[wiki:architecture/bootstrap-and-services]].
[[cite:crates/limedl-core/src/dispatcher.rs:L259-L475]]

`fetch_tracker_list` uses the dispatcher's own HTTP client when present and falls
back to a plain client in minimal environments; it normalizes the fetched text
through `normalize_tracker_list_lossy`. `probe_checksum` derives the target file
name from the URL path when the caller does not supply one and probes candidate
SHA-256 checksum files with the configured client.
[[cite:crates/limedl-core/src/dispatcher.rs:L379-L405]]
[[cite:crates/limedl-core/src/dispatcher.rs:L573-L596]]

## The typed downcast seam

Protocol-specific data cannot be expressed by the trait — for example Aria2's
`getOption` reads the HTTP manager's in-memory manifest while `tellStatus.files`
asks the BT engine. `Dispatcher` therefore has a private helper:

```rust
#[cfg(feature = "bt")]
fn bt_backend(&self) -> std::result::Result<&LazyBtBackend, DownloadError> {
    self.registry
        .get_typed::<LazyBtBackend>()
        .ok_or_else(|| DownloadError::Internal("BT backend not registered".into()))
}
```

and exposes BT-only operations (`bt_runtime_status`, `bt_set_speed_limit`,
`bt_preview_torrent`, `bt_get_peers`/`bt_get_trackers`/`bt_get_pieces`/`bt_get_files`,
`bt_update_files`) on top of it. [[cite:crates/limedl-core/src/dispatcher.rs:L479-L571]]
The Aria2 RPC layer has exactly two sanctioned `get_typed` accessors —
`RpcContext::http()` and `RpcContext::bt()` — and the rule, stated on both the
accessors and the facade's module doc, is that new Aria2 handlers must use those,
not a raw `get_typed`. [[cite:crates/limedl-rpc/src/context.rs:L71-L86]]
[[cite:crates/limedl-core/src/dispatcher.rs:L16-L20]]

## Extension seams

- A new protocol is a new `DownloadBackend` implementation plus one
  `register_arc` under its `TaskKind`. Routing, list merging, settings broadcast
  and shutdown come for free. [[cite:crates/limedl-core/src/backend_registry/mod.rs:L41-L53]]
- A new cross-protocol operation belongs on `Dispatcher`, not in a frontend, so
  the desktop UI and Aria2 RPC cannot drift. [[cite:crates/limedl-core/src/dispatcher.rs:L13-L15]]
- A new protocol-specific accessor for Aria2 must go through
  `RpcContext::http()`/`bt()`. [[cite:crates/limedl-rpc/src/context.rs:L71-L86]]

Related pages: [[wiki:architecture/bootstrap-and-services]],
[[wiki:workflows/http-download-lifecycle]],
[[wiki:integrations/aria2-rpc-server]].
