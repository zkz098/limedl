---
type: architecture
title: Protocol Routing and the Dispatcher Facade
description: The protocol abstraction layer of limedl — the DownloadBackend trait, BackendRegistry routing by TaskId, and the Dispatcher facade that unifies lifecycle, settings, disk, concurrency and protocol-specific operations while auto-emitting state events.
tags: [protocol, dispatcher, backend-registry, routing, facade]
sources:
  - id: openwiki-source-8ec1f0436491ce5daa75720b
    resource: repo://crates/limedl-core/src/aria2_rpc/context.rs
  - id: openwiki-source-2262be0eb4e0dcf867247c95
    resource: repo://crates/limedl-core/src/backend_registry/mod.rs
  - id: openwiki-source-4b83dfd7538606d5871938ef
    resource: repo://crates/limedl-core/src/dispatcher.rs
  - id: openwiki-source-91b6ee5086a47115791aca3c
    resource: repo://crates/limedl-core/src/protocol.rs
generated: { by: "pi", at: "2026-10-07T03:53:23.435Z" }
verified:
  - by: openwiki/0.7.1
    at: 2026-10-07T03:53:23.435Z
---

# Protocol Routing and the Dispatcher Facade

The engine supports more than one download protocol, so limedl separates three
concerns in `limedl-core`:

1. a uniform `DownloadBackend` interface every protocol implements;
2. a `BackendRegistry` that routes by task identity;
3. a `Dispatcher` facade that frontends call, so neither the desktop UI nor the
   Aria2 RPC layer re-implements cross-protocol logic.

## The DownloadBackend trait

`DownloadBackend` is an `async_trait` with `Send + Sync + 'static`. The required
surface is the download lifecycle (`start`, `pause`, `resume`, `cancel`, `remove`,
`purge`), file/folder opening (`open_in_explorer`), reads (`status`, `list`),
configuration (`update_settings`) and `shutdown`. Two methods have defaults:
`open_file`/`open_dir` fall back to `open_in_explorer`, and `set_priority` returns
an error unless the backend overrides it.

`start` returns a strongly-typed `TaskId` (not a string), so the registry can route
subsequent operations without parsing.

Evidence: `repo://crates/limedl-core/src/protocol.rs#L8-L45`.

## BackendRegistry

`BackendRegistry` keeps three structures:

- `by_kind: HashMap<TaskKind, Arc<dyn DownloadBackend>>` — the routing table;
- `by_type: HashMap<TypeId, Arc<dyn Any + Send + Sync>>` — concrete-type lookup
  for protocol-specific commands;
- `all: Vec<(TaskKind, Arc<dyn DownloadBackend>)>` — iteration order for list,
  settings broadcast and shutdown.

`dispatch(&TaskId)` is `by_kind.get(&task_id.kind())`. `register_arc` stores the
caller's `Arc` directly (unlike `register`, which wraps a value), which is the
mechanism that keeps `CoreSystems` and the registry pointing at the same atomics.

Cross-cutting helpers:

- `list_all()` calls every backend's `list()`, logs and skips a failing backend,
  and sorts the merged result by `created_at_ms` **descending**.
- `update_all_settings()` applies settings to every backend, records the first
  error but still applies to the rest, then returns that first error.
- `shutdown_all()` awaits each backend's `shutdown()` in registration order.

Evidence: `repo://crates/limedl-core/src/backend_registry/mod.rs#L8-L125`.

## Dispatcher composition

`Dispatcher` is `Clone` and holds the registry and event bus as required fields,
plus optional `SettingsService`, `DiskIoService`, `ConcurrencyManager`,
`CdnService` and a `reqwest::Client`. Three constructors exist for different
environments:

- `new(registry, event_bus)` — minimal, no services (tests);
- `with_settings_service(...)` — real settings read/write but no disk/CDN/concurrency
  (the UI test fixture);
- `full(...)` — the production wiring used by `bootstrap`.

Methods that need an absent optional service return a `DownloadError::Internal`
rather than panicking, which is what lets the minimal constructors be useful.

Evidence: `repo://crates/limedl-core/src/dispatcher.rs#L41-L136`.

## Lifecycle routing and automatic event emission

`Dispatcher::start` first fills in mirror URLs from the settings' URL-rewrite rules
when the caller supplied none, classifies the request, routes to the backend by
kind, and after a successful start fetches `backend.status` to emit an initial
`Updated` event for immediate UI synchronization.

Every mutating lifecycle operation (`pause`, `resume`, `cancel`, `remove`,
`purge`) routes through the registry, calls the backend, and on success emits
`DownloadEvent::Updated` from the returned snapshot. This is why the frontends
never emit lifecycle events themselves: HTTP start/pause/resume events from the
Aria2 handlers are broadcast explicitly, but completion/error events come from the
executor/lifecycle layer, and BT events come only from the alert bridge.

`set_priority` follows the same pattern but re-reads `status` after the mutation
before emitting — the event carries a snapshot, so updating only the manifest
would leave the UI priority stale until the next refresh.

`open_in_explorer`, `open_file`, `open_dir` and `status` do **not** emit: they are
either side effects or read-only.

Evidence: `repo://crates/limedl-core/src/dispatcher.rs#L138-L245`.

## Aggregation

- `list()` delegates to `registry.list_all()`.
- `has_active_downloads()` lists every backend and returns true if any summary is
  in `DownloadState::Downloading`. This is the cross-protocol aggregation the tray
  uses, so a running BT torrent is not missed by an HTTP-only check.

Evidence: `repo://crates/limedl-core/src/dispatcher.rs#L247-L259`.

## Settings, disk and concurrency through the facade

`Dispatcher` exposes the settings read/write path (`get_settings`,
`get_settings_blocking`, `save_settings`, `save_settings_with`, `factory_reset`,
`default_download_dir`) and delegates disk/concurrency operations
(`detect_disk_type`, `detect_all_disk_types`, `get_io_status`, `toggle_game_mode`,
`game_mode`, `get_overclock_mode`, `toggle_overclock_mode`) to the optional
services. The detailed settings fan-out is documented on the
[Bootstrap and services page](bootstrap-and-services.md).

`fetch_tracker_list` uses the dispatcher's own HTTP client when present and falls
back to a plain client in minimal environments; it normalizes the fetched text
through `normalize_tracker_list_lossy`. `probe_checksum` derives the target file
name from the URL path when the caller does not supply one and probes candidate
SHA-256 checksum files with the configured client.

Evidence: `repo://crates/limedl-core/src/dispatcher.rs#L262-L584`.

## The typed downcast seam

Protocol-specific data cannot be expressed by the trait — for example Aria2's
`getOption` reads the HTTP manager's in-memory manifest while `tellStatus.files`
asks the BT engine. `Dispatcher` therefore has a private helper:

```rust
#[cfg(feature = "bt")]
fn bt_backend(&self) -> Result<&LazyBtBackend, DownloadError> {
    self.registry.get_typed::<LazyBtBackend>()
        .ok_or_else(|| DownloadError::Internal("BT backend not registered".into()))
}
```

and exposes BT-only operations (`bt_runtime_status`, `bt_set_speed_limit`,
`bt_preview_torrent`, `bt_get_peers`/`bt_get_trackers`/`bt_get_pieces`/`bt_get_files`,
`bt_update_files`) on top of it. The Aria2 RPC layer has exactly two sanctioned
`get_typed` accessors — `RpcContext::http()` and `RpcContext::bt()` — and the
rule is that new Aria2 handlers must use those, not a raw `get_typed`.

Evidence: `repo://crates/limedl-core/src/dispatcher.rs#L466-L505`,
`repo://crates/limedl-core/src/aria2_rpc/context.rs#L54-L68`.

## Extension seams

- A new protocol is a new `DownloadBackend` implementation plus one
  `register_arc` under its `TaskKind`. Routing, list merging, settings broadcast
  and shutdown come for free.
- A new cross-protocol operation belongs on `Dispatcher`, not in a frontend, so
  the desktop UI and Aria2 RPC cannot drift.
- A new protocol-specific accessor for Aria2 must go through
  `RpcContext::http()`/`bt()`.

Related pages: [Bootstrap, SystemContext and Shared Services](bootstrap-and-services.md),
[HTTP Download Lifecycle](../workflows/http-download-lifecycle.md),
[Aria2 JSON-RPC Compatibility Server](../integrations/aria2-rpc-server.md).
