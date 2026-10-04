---
type: system
title: SQLite Persistence and Crash Recovery
description: How limedl persists download state in SQLite — the dual read/write connection design and PRAGMAs, versioned migrations, the manifest and chunk repositories, the bt_tasks cache, and the startup reconstruction that clears stale chunk claims.
tags: [database, sqlite, persistence, migrations, crash-recovery]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T01:52:48.489Z
sources:
  - id: openwiki-source-f0a925ac2758f1bac742ca91
    resource: repo://crates/limedl-core/src/database/bt_task_repo.rs
  - id: openwiki-source-f33808f0ac3d227e495f1cb3
    resource: repo://crates/limedl-core/src/database/connection.rs
  - id: openwiki-source-be0a0543dfc1990a49281fb2
    resource: repo://crates/limedl-core/src/database/manifest_repo.rs
  - id: openwiki-source-d256d453001d5dddbed5926e
    resource: repo://crates/limedl-core/src/database/schema.rs
  - id: openwiki-source-41f2f85d035ed1edf0c09b2e
    resource: repo://crates/limedl-core/src/persistence.rs
generated: { by: "pi", at: "2026-10-04T01:52:48.489Z" }
---

# SQLite Persistence and Crash Recovery

Download tasks, chunk progress and BT task metadata live in a SQLite database at
`<state_dir>/downloads.db`. `crates/limedl-core/src/database/` owns the
connection and repositories, and `persistence.rs` owns loading tasks back into
memory.

## Dual connections and PRAGMAs

`Database` holds two `Arc<Mutex<Connection>>`: `write_conn` and `read_conn`.
`open(path)` configures the writer with:

| PRAGMA | Value | Why |
| --- | --- | --- |
| `journal_mode` | `WAL` | readers are not blocked by the writer |
| `wal_autocheckpoint` | 4096 | bound WAL growth |
| `foreign_keys` | `ON` | cascade chunk deletion |
| `busy_timeout` | 5000 | tolerate short lock contention |
| `synchronous` | `NORMAL` | safe with WAL; avoids an fsync per checkpoint |
| `cache_size` | `-32000` | ~32 MB page cache |

The reader opens the same file with `query_only = 1` and its own busy timeout, so
a read path cannot accidentally write. Both connections get a prepared-statement
cache of 64.

Evidence: `repo://crates/limedl-core/src/database/connection.rs#L12-L15`,
`repo://crates/limedl-core/src/database/connection.rs#L22-L114`.

All DB calls are synchronous and blocking; async callers wrap them in
`spawn_blocking` (or `block_in_place` during startup), and the mutexes are held
only for the duration of the statement.

## Migrations

Schema versioning uses `PRAGMA user_version` plus a static `MIGRATIONS` list of
`(version, name, fn)`. On open, migrations with `version > current` run in order
and update `user_version`; after any migration `ANALYZE` refreshes the planner
statistics.

There is a compatibility shim for databases written by the older code that added
columns without setting `user_version`: before running migrations, the code
detects the `chunk_size` / `mirror_urls` columns and bumps `user_version` to 2/3
accordingly. `table_has_column` queries `PRAGMA table_info` and is whitelisted to
the `downloads` table.

Evidence: `repo://crates/limedl-core/src/database/connection.rs#L52-L98`,
`repo://crates/limedl-core/src/database/schema.rs#L1-L20`,
`repo://crates/limedl-core/src/database/schema.rs#L71-L120`.

### In-memory test mode

`open_in_memory()` uses a single `:memory:` connection shared by both handles
(in-memory SQLite does not allow a second `:memory:` connection to see the same
data), runs all migrations, and deliberately does **not** enable WAL — doing so
causes "database is locked" for the shared connection.

Evidence: `repo://crates/limedl-core/src/database/connection.rs#L118-L150`.

## Schema

The initial schema has `downloads` (one row per task, with URL, paths, byte
counts, thread-mode fields, state, checksum fields and timestamps) and `chunks`
keyed by `(download_id, chunk_index)` with a `FOREIGN KEY ... ON DELETE CASCADE`,
so deleting a download removes its chunks automatically. Later migrations add
`chunk_size`, mirror columns, `priority`, CDN fields, `expected_checksum` and the
`bt_tasks` table.

Evidence: `repo://crates/limedl-core/src/database/schema.rs#L21-L69`,
`repo://crates/limedl-core/src/database/schema.rs#L121-L210`.

## Two serialization mappings

There is deliberately no single wire format shared between SQLite and JSON:

- **SQLite** stores enums as snake_case text and most fields as columns.
  `download_state_to_text` / `text_to_state`, `thread_mode_to_text`,
  `checksum_mode_to_text` and `adaptive_profile_to_text` are the column mappings.
  `mirror_urls` is the exception: a list, so it is stored as a JSON text column.
- **JSON** is produced only at the boundaries that need it — Aria2 responses and
  the `bt_tasks.summary_json` cache column — by serializing the `Manifest` /
  `DownloadSummary` structs.

The checksum mapping carries a migration rule: `sha1` and `xxh3_128` rows map to
`ChecksumMode::None` with a warning, because those algorithms were removed and
their stored digest cannot be compared against a supported one; unknown values
still error. This lets old rows finish instead of failing verification forever.

Evidence: `repo://crates/limedl-core/src/database/manifest_repo.rs#L31-L95`,
`repo://crates/limedl-core/src/database/manifest_repo.rs#L126-L135`.

`insert_manifest_row` / `update_manifest_row` use `prepare_cached` with
`named_params!` so values are passed by reference with zero `String` cloning;
`insert_download` writes the download row and its chunk rows together.

Evidence: `repo://crates/limedl-core/src/database/manifest_repo.rs#L126-L200`,
`repo://crates/limedl-core/src/database/manifest_repo.rs#L348-L420`.

## Crash recovery on startup

`DownloadManager::load_downloads_from_db` reconstructs in-memory
`ManagedDownload` entries from stored headers:

1. A task left in `Verifying` with the destination present and the temp file
   gone is promoted to `Completed` (the rename finished, the crash happened
   before the state write).
2. A task left in `Downloading` **stays** `Downloading` but its
   `connection_count` is zeroed and `allocated_thread_count` set to 0, so the
   scheduler re-allocates threads on the next rebalance.
3. `Retrying`, `Verifying` and `Queued` become `Paused`.
4. Completed/Failed/Canceled/Paused are unchanged.
5. Chunks are loaded lazily only for non-terminal tasks, and **every
   `claimed_by` is cleared**. A fresh manager owns no chunk workers, so any
   persisted claim belongs to a process that no longer exists; leaving it would
   make `claim_next_chunk` skip that chunk while the chunk map still reports work
   to do, stalling the resumed download. A clean shutdown releases claims before
   persisting, so this is the crash-recovery equivalent.

Evidence: `repo://crates/limedl-core/src/persistence.rs#L30-L95`.

## The bt_tasks cache

`bt_tasks(id, summary_json, created_at_ms)` lets the UI show BT tasks while the
irontide engine is unloaded. `replace_bt_tasks` runs `BEGIN IMMEDIATE`, deletes
all rows and re-inserts the engine's current list in one transaction, so a crash
cannot leave a half-updated index; `DownloadSummary` is stored as JSON, which
means new fields need no migration. `list_bt_tasks` skips a row whose JSON fails
to parse with a warning rather than failing the whole list. `delete_bt_task`
removes one row on cancel/remove/purge.

This table is a **cache**, not the source of truth — the irontide `.resume` files
are; while the engine runs its `list()` overwrites the index.

Evidence: `repo://crates/limedl-core/src/database/bt_task_repo.rs#L20-L108`.

## Terminal-row visibility

`max_in_memory_downloads` evicts tasks from memory only. The database remains the
authority for terminal results, which the Aria2 RPC layer queries through
`get_download_header(id)`, `list_download_headers()` and
`count_terminal_downloads()`, until `purgeDownloadResult`/`removeDownloadResult`
deletes the row. `delete_download` removes the download row and cascades to its
chunks.

Evidence: `repo://crates/limedl-core/src/database/manifest_repo.rs#L422-L465`.

## Maintenance

On clean shutdown `Database::shutdown()` runs
`PRAGMA wal_checkpoint(TRUNCATE)` to shrink the WAL file. `vacuum_if_needed`
runs an incremental vacuum only when the freelist exceeds a threshold, so normal
operation does not pay for a full vacuum.

Evidence: `repo://crates/limedl-core/src/database/connection.rs#L153-L180`.

<!-- openwiki: broken internal link [/openwiki/workflows/http-download-lifecycle.md] link "/openwiki/workflows/http-download-lifecycle.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Related pages: [HTTP Download Lifecycle](/openwiki/workflows/http-download-lifecycle.md),
<!-- openwiki: broken internal link [/openwiki/workflows/bit-torrent-backend.md] link "/openwiki/workflows/bit-torrent-backend.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[BitTorrent Backend](/openwiki/workflows/bit-torrent-backend.md),
<!-- openwiki: broken internal link [/openwiki/integrations/aria2-rpc-server.md] link "/openwiki/integrations/aria2-rpc-server.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Aria2 JSON-RPC Compatibility Server](/openwiki/integrations/aria2-rpc-server.md).
