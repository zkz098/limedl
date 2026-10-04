---
type: workflow
title: BitTorrent Backend
description: The irontide-backed BitTorrent subsystem — lazy session startup and the restartable engine slot, task identity and snapshots, the alert bridge as the sole Aria2 event source, upload-policy and anti-leech loops, lightweight mode with the bt_tasks index, blocklist handling and shutdown ordering.
tags: [bittorrent, irontide, lazy-startup, alerts, anti-leech, uploads]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T01:52:48.489Z
sources:
  - id: openwiki-source-093388d09b520118fa26ce32
    resource: repo://crates/limedl-core/src/bt_backend/alerts.rs
  - id: openwiki-source-8b15f323c622a7a895e67544
    resource: repo://crates/limedl-core/src/bt_backend/anti_leech.rs
  - id: openwiki-source-67acfabd9189a7b8f7baf9b1
    resource: repo://crates/limedl-core/src/bt_backend/lazy.rs
  - id: openwiki-source-9c98a0f751be514a0dbfa1c0
    resource: repo://crates/limedl-core/src/bt_backend/queries.rs
  - id: openwiki-source-7627118219ac1ac878844790
    resource: repo://crates/limedl-core/src/bt_backend/session.rs
  - id: openwiki-source-184dce8cdc2200a7026a9540
    resource: repo://crates/limedl-core/src/bt_backend/snapshot.rs
  - id: openwiki-source-1345c12b6a8173e9a1e84731
    resource: repo://crates/limedl-core/src/bt_backend/uploads.rs
  - id: openwiki-source-f0a925ac2758f1bac742ca91
    resource: repo://crates/limedl-core/src/database/bt_task_repo.rs
generated: { by: "pi", at: "2026-10-04T01:52:48.489Z" }
---

# BitTorrent Backend

The BT backend is `LazyBtBackend` (registered as the `TaskKind::Bt` backend) plus
`IrontideBtBackend` (the running engine). It is built on the `irontide` crate,
pinned to `=1.7.0` because the engine is frozen and moving off it is a migration,
not a version bump.

Evidence: `repo://Cargo.toml#L70-L74`.

## Lazy startup and the restartable engine slot

Creating an irontide session binds TCP/uTP sockets, starts DHT/LSD, loads resume
data, applies tuning and parses the blocklist. `LazyBtBackend` moves that off the
startup path:

- `new()` only records the launch configuration — no network, no disk.
- `spawn_startup()` kicks off a background warm-up; `bootstrap()` returns without
  waiting.
- Operations that need the engine await `engine()`; `list` and read-only
  peer/tracker/piece/file queries do **not** block (the task list is loaded before
  the desktop window is shown).

The engine slot is `RwLock<Option<Arc<IrontideBtBackend>>>`, not a `OnceCell`, so
the session can be unloaded and rebuilt. Creation and teardown are serialized by
a `lifecycle: tokio::sync::Mutex<()>`, so an idle shutdown in flight and a fresh
`engine()` cannot both bind the listen port.

A failed start is cached in `startup_error`: later calls fail fast with the
original message instead of silently re-creating a session that holds exclusive
resources. An explicit `spawn_startup()` clears that cache so the user can retry.

Evidence: `repo://crates/limedl-core/src/bt_backend/lazy.rs#L1-L50`,
`repo://crates/limedl-core/src/bt_backend/lazy.rs#L111-L186`.

`engine()`:

1. returns the cached startup error or the shutting-down error;
2. records `last_activity_ms` (so the idle supervisor does not unload between
   returning and the torrent being inserted);
3. returns an already-ready engine;
4. takes the lifecycle guard, re-checks, creates the engine, records a failure;
5. if `shutting_down` was set while the session came up, shuts the late engine
   down and errors;
6. stores the engine, clears `startup_error`, and refreshes+publishes the index
   (restored torrents' `TorrentAdded` alerts fired before the bridge subscribed).

Evidence: `repo://crates/limedl-core/src/bt_backend/lazy.rs#L303-L350`.

## Background loops

- `spawn_index_sync_loop()` runs every 5 s, mirrors the engine's `list()` into
  `bt_tasks`, and self-heals eager mode: if lightweight mode was turned off at
  runtime and no engine is ready with no cached error, it starts one.
- `spawn_idle_supervisor()` runs every 10 s, is a no-op unless lightweight mode
  is on, and unloads the engine when `has_active_torrents()` is false after an
  idle settle period. "Seeding counts as active", so a seeding session stays up.
  An error from the active check keeps the engine up rather than tearing it down.

`has_unfinished_tasks()` reads the persisted index and falls back to the on-disk
resume files when the index is empty (first run after enabling lightweight mode,
or an upgrade from a build with no index).

Evidence: `repo://crates/limedl-core/src/bt_backend/lazy.rs#L186-L300`.

## Lightweight mode and the bt_tasks cache

With `bt.lightweight_mode` on, `bootstrap` warms the engine only when
`has_unfinished_tasks()` is true. While the engine is down, `list()` serves rows
from the `bt_tasks` SQLite cache, clearing volatile metrics (speed/eta/peers) so
the UI does not show a dead session's old numbers.

`bt_tasks` is a **cache, not the source of truth**: the irontide `.resume` files
are, and a running engine's `list()` overwrites the index. Writes happen when the
engine becomes ready, on the 5 s sync tick, and before unload/shutdown;
`cancel`/`remove`/`purge` delete rows. The whole table is replaced in one
transaction so a crash cannot leave a half-updated index.

Evidence: `repo://crates/limedl-core/src/bt_backend/lazy.rs#L29-L50`,
`repo://crates/limedl-core/src/database/bt_task_repo.rs#L20-L45`.

## Task identity and snapshots

A BT task id is the raw info-hash hex string (no `bt:` prefix internally), and
`task_map: DashMap<Id20, Id20>` maps download id to info hash.
`torrent_created_at` records creation time (set on start, cleaned on
cancel/remove/purge). `stats_to_snapshot` uses `downloaded` (all payload bytes)
rather than `total_done` (verified pieces only) for smooth progress, and estimates
ETA from total/downloaded/speed.

`get_torrent_files()` derives `included` from `session.file_priorities()` where
`Skip` means unselected — **not** from file open/closed status, because pausing a
torrent closes every file and would mislabel the whole set as unselected. A
single-file torrent synthesizes one 0-based entry.

Evidence: `repo://crates/limedl-core/src/bt_backend/snapshot.rs#L11-L70`,
`repo://crates/limedl-core/src/bt_backend/queries.rs#L131-L190`.

## The alert bridge is the sole Aria2 emitter

`setup_alert_bridge()` subscribes to the session's alert broadcast **before**
spawning the loop, so a `TorrentAdded` that arrives between setup returning and
the task's first poll is not lost. The loop selects between alerts and a 2 s
progress tick that emits a `Progress` event for every torrent in `task_map`.

`handle_alert` is the mapping table (extracted so tests can feed synthetic
alerts):

| Alert | Events |
| --- | --- |
| `TorrentAdded` | `Aria2Notification(onDownloadStart)` + task_map insert |
| `TorrentPaused` | `Aria2Notification(onDownloadPause)` |
| `TorrentResumed` | `Aria2Notification(onDownloadStart)` |
| `TorrentFinished` | `onDownloadComplete` + `onBtDownloadComplete` + `Progress` + `Updated` |
| `TorrentError` | `Aria2Notification(onDownloadError)` + `Updated` |

`MetadataReceived`, `TrackerReply` and the rest only log. Lifecycle
`Updated` events from start/cancel/remove/purge are emitted by the Dispatcher,
not here. This is why the Aria2 RPC handlers broadcast notifications only for
HTTP tasks: BT notifications have exactly one source.

Evidence: `repo://crates/limedl-core/src/bt_backend/alerts.rs#L100-L200`,
`repo://crates/limedl-core/src/bt_backend/alerts.rs#L270-L322`.

## Upload policy

`spawn_upload_policy_loop()` runs every 5 s and enforces the user's whole-torrent
limits. It pauses a torrent's upload by capping `set_upload_limit(ih, 1)` when
either `upload_limit_bytes` or `upload_ratio_limit` is reached and
`pause_upload_when_limit_reached` is set, tracking state in `paused_by_limit`.
Upload is restored (limit 0 = unlimited) only when the condition no longer holds,
which avoids per-tick pause/unpause oscillation.

Evidence: `repo://crates/limedl-core/src/bt_backend/uploads.rs#L38-L98`.

## Anti-leech

`spawn_anti_leech_loop()` runs every 10 s and acts on **individual
under-contributing peers**, independent of the whole-torrent upload policy.
`peer_is_leecher` is a pure function:

1. skip upload-only peers (BEP 21 seeders), peers we are choking, peers within
   the grace period, and peers with `progress >= 1.0`;
2. signal A: the peer chokes us while we keep unchoking it and we receive
   essentially nothing;
3. signal B: even unchoked, the give-back ratio is below the threshold
   (`ratio == 0` disables this check).

Actions are Ban (with an expiry timestamp so shared NAT/VPN peers are eventually
unbanned) or LimitSlots (cap upload slots, recording the original value to
restore). Warnings are published as `DownloadEvent::Warning`. Disabling the
feature calls `cleanup_disabled`, which removes only the bans and caps this loop
introduced.

Evidence: `repo://crates/limedl-core/src/bt_backend/anti_leech.rs#L68-L165`.

## Engine tuning and blocklist hot-reload

`apply_settings(settings)` stores the new snapshot and, when the engine is
running, schedules `session.apply_settings(build_engine_settings(&bt))` plus
`apply_blocklist_impl` on the captured runtime handle (so it works from a
synchronous UI handler and from a current-thread test runtime). It also rebuilds
the `.torrent` HTTP client through `build_http_client`, keeping the previous
client on failure rather than silently bypassing the configured proxy.

`build_engine_settings` maps BtSettings onto irontide: global download/upload rate
limits, seed/choking algorithms, max upload slots and peers per torrent,
smart-ban failures/parole, eviction ban duration and data-contribution timeout.

Blocklist application reads the file and parses `.dat` with `parse_dat`, anything
else with `parse_p2p`, then replaces the session IP filter. The last applied
`enabled:path` is cached so an unchanged config is not re-parsed; a parse failure
is **not** cached, so fixing the file and re-saving takes effect.

Evidence: `repo://crates/limedl-core/src/bt_backend/session.rs#L201-L300`,
`repo://crates/limedl-core/src/bt_backend/session.rs#L326-L340`.

## Shutdown ordering

`LazyBtBackend::shutdown()`:

1. sets `shutting_down` (new work is rejected and a late warm-up tears itself
   down);
2. aborts the index mirror (but leaves the idle supervisor alone so an in-flight
   teardown is not interrupted);
3. waits for the warm-up task, with a grace timeout;
4. takes the lifecycle guard so any in-flight creation finishes;
5. writes the final index and calls `engine.shutdown()`.

`IrontideBtBackend::shutdown()` then runs four phases: save session state; abort
the upload-policy, anti-leech and alert loops; save resume data for every active
torrent; wait a proportional grace period (500 ms per torrent, clamped 1–5 s) for
pending disk writes; and finally shut the session down.

Evidence: `repo://crates/limedl-core/src/bt_backend/lazy.rs#L442-L472`,
`repo://crates/limedl-core/src/bt_backend/session.rs#L154-L200`.

<!-- openwiki: broken internal link [/openwiki/architecture/bootstrap-and-services.md] link "/openwiki/architecture/bootstrap-and-services.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Related pages: [Bootstrap, SystemContext and Shared Services](/openwiki/architecture/bootstrap-and-services.md),
<!-- openwiki: broken internal link [/openwiki/integrations/aria2-rpc-server.md] link "/openwiki/integrations/aria2-rpc-server.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Aria2 JSON-RPC Compatibility Server](/openwiki/integrations/aria2-rpc-server.md),
<!-- openwiki: broken internal link [/openwiki/systems/persistence-and-recovery.md] link "/openwiki/systems/persistence-and-recovery.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[SQLite Persistence and Crash Recovery](/openwiki/systems/persistence-and-recovery.md),
<!-- openwiki: broken internal link [/openwiki/workflows/scheduler-and-concurrency.md] link "/openwiki/workflows/scheduler-and-concurrency.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Scheduler, AIMD and Concurrency Control](/openwiki/workflows/scheduler-and-concurrency.md).
