---
type: system
title: Settings and Configuration
description: limedl's configuration single source of truth — the AppSettings shape, SettingsService serialization and atomic persistence, normalize_settings validation, the save fan-out to every backend, and special cases such as disk overrides and the Aria2 RPC bind/auth/CORS settings.
tags: [settings, configuration, persistence, validation, hot-reload]
sources:
  - id: openwiki-source-fd061a9c15d2a04bc703746d
    resource: repo://crates/limedl-core/src/context.rs
  - id: openwiki-source-4b83dfd7538606d5871938ef
    resource: repo://crates/limedl-core/src/dispatcher.rs
  - id: openwiki-source-2468c28a2ebe626471835de0
    resource: repo://crates/limedl-core/src/services/settings_service.rs
  - id: openwiki-source-836a4b1e280dffbb2f0ca060
    resource: repo://crates/limedl-core/src/settings/mod.rs
  - id: openwiki-source-7ef10e5bb7f9bf65c86b6285
    resource: repo://crates/limedl-core/src/types/settings.rs
  - id: openwiki-source-ec243741e58f29f41733a43b
    resource: repo://crates/limedl-native/src/context.rs
  - id: openwiki-source-711bab97935422a9dd5fabf4
    resource: repo://crates/limedl-native/src/paths.rs
  - id: openwiki-source-9f796a37ea60bc20889db159
    resource: repo://crates/limedl-server/src/config.rs
generated: { by: "pi", at: "2026-10-04T12:36:56.946Z" }
verified:
  - by: openwiki/0.7.1
    at: 2026-10-07T03:53:23.435Z
---

# Settings and Configuration

Configuration is a JSON file, not SQLite. `SettingsService` owns it as the single
source of truth for every backend and frontend, and `Dispatcher::save_settings_with`
is the only supported write path.

## AppSettings

`AppSettings` is a flat struct of `#[serde(default)]` sub-settings:
`appearance`, `proxy`, `scheduler`, `download`, `bt`, `logging`, `aria2_rpc`,
`cdn_acceleration`, `url_rewrite`, `global_speed_limit_bps`,
`speed_limit_schedule`, `notifications`, `io_baseline`, `autostart`,
`setup_completed`, `last_setup_step`, `double_click` and
`max_in_memory_downloads`. Because every field has a serde default, a partial or
hand-edited file still loads and unknown keys are ignored.

Structs use `#[serde(rename_all = "camelCase")]` and enums use `snake_case`, so
the on-disk JSON is camelCase.

Evidence: `repo://crates/limedl-core/src/types/settings.rs#L419-L460`,
`repo://crates/limedl-core/src/settings/mod.rs#L380-L428`.

## Where settings.json lives

The desktop resolves the base data directory in `paths::dirs_or_temp_dir`:

| Platform | Base directory |
| --- | --- |
| Windows | `%LOCALAPPDATA%\limedl` |
| macOS | `~/Library/Application Support/limedl` |
| Linux | `$XDG_DATA_HOME/limedl` or `~/.local/share/limedl` |
| Override | `LIMEDL_DATA_DIR` wins over all of the above |

`settings.json` sits in that base directory and the SQLite DB/torrent state live
in `<base>/downloads`. `SystemContext::with_components` derives the settings path
as the **parent** of `state_dir` joined with `settings.json`. The headless
`limedl-server` daemon resolves the same base directory (with `--data-dir` as an
additional highest-precedence override) through its own `resolve_data_dir`, so a
daemon can read a desktop profile's settings and vice versa.

Evidence: `repo://crates/limedl-native/src/paths.rs#L1-L35`,
`repo://crates/limedl-core/src/context.rs#L47-L56`,
`repo://crates/limedl-server/src/config.rs#L89-L99`.

## SettingsService: serialized read-modify-write

`SettingsService` holds `Arc<RwLock<AppSettings>>` plus a `tokio::sync::Mutex`
`update_lock`. `update_with(mutate)` takes the lock, clones the **current**
persisted settings as the base, applies the closure, then `persist()`:

1. `normalize_settings(candidate)`;
2. `persist_settings` to disk atomically;
3. replace the in-memory value and return it.

The lock is what makes concurrent saves safe: without it two saves could each
clone the same stale base and the later write would silently drop the other's
change. A closure that returns an error aborts before persistence, leaving the
stored settings untouched. `update` is the whole-struct variant, `factory_reset`
saves `AppSettings::default()`, and `get`/`get_blocking` are the reads.

Evidence: `repo://crates/limedl-core/src/services/settings_service.rs#L10-L96`.

### Atomic persistence

`persist_settings` creates the parent directory, writes `settings.json.tmp`,
**flushes and `sync_all`s it**, snapshots the outgoing file to
`settings.json.bak` (best effort — a failure only logs), and renames the temp
file over `settings.json`. The fsync before the rename matters: without it a power
loss can make the rename durable while the data is not, leaving an empty
`settings.json` behind — exactly the unreadable-file case the recovery path below
exists for.

Evidence: `repo://crates/limedl-core/src/settings/mod.rs#L452-L490`.

### Unreadable settings do not block startup

`load_settings` is infallible for a *content* problem. A missing file returns the
defaults; an unreadable one (permissions, I/O) still returns `Err`, because that
is an environment problem rather than a corrupt file. When the contents cannot be
parsed or normalized it does not propagate the error:

1. the file is moved aside to `settings.json.corrupt` (so the next save can create
   a fresh one and the user can still inspect what was there);
2. `settings.json.bak` is tried, and its value is used if it parses and
   normalizes;
3. otherwise `AppSettings::default()` applies.

Either way the outcome is an `error!` log entry. Before this, one bad byte in
`settings.json` made every start fail with an error the GUI had nowhere to show.

Evidence: `repo://crates/limedl-core/src/settings/mod.rs#L380-L450`.

## normalize_settings

All externally supplied settings pass through `normalize_settings`, which is the
key validation point:

- **Proxy**: `Disabled`/`System` clear `manual_url`; `Manual` requires a non-empty,
  parseable URL.
- **Scheduler**: traditional `max_parallel_tasks` clamped to 1..=32, automatic
  `max_parallel_threads` to 1..=64, per-task threads to 1..=32 and never above the
  global max; `min_threads_per_task` defaults to half the max when 0.
- **Download**: retries clamped to 0..=20, User-Agent normalized, and
  `default_download_dir` dropped to empty unless absolute.
- **IO baseline**: buffer limits, game-mode limits and HDD parallelism clamped to
  safe ranges.
- **Logging**: level kept, path trimmed, retention count/days clamped.
- **BT**: tracker list and tracker-list URL validated (http/https/udp schemes),
  sorted and de-duplicated.
- **Aria2 RPC**: `normalize_aria2_rpc_settings` drops clients whose `token_hash`
  is empty (they can never verify, so they would only be dead UI rows), trims
  names/hashes, falls `listen_address` back to `127.0.0.1` unless
  `is_plausible_bind_host` accepts it (an IP literal or hostname; a `host:port`,
  URL or whitespace is rejected so `format_bind_addr` cannot append a second
  port), and trims/drops blank CORS origins.
- **URL rewrite**: rules with an empty pattern are dropped, targets with an empty
  template are dropped, and `order` is reassigned to the filtered positions.
- `max_in_memory_downloads` is 0 (unlimited) or clamped to 10..=10000.

Evidence: `repo://crates/limedl-core/src/settings/mod.rs#L19-L125`,
`repo://crates/limedl-core/src/settings/mod.rs#L128-L170`,
`repo://crates/limedl-core/src/settings/mod.rs#L330-L378`.

## Aria2 RPC authentication and bind settings

`Aria2RpcSettings` has `enabled` (**default `false`**), `port` (default 6800),
`listen_address` (default `127.0.0.1`), an optional `secret`, `auth_mode`,
`clients`, `cors_allowed_origins`, `allow_any_origin` (default `false`) and
`exit_on_shutdown` (default `false`). `Aria2AuthMode` is `single` (the default,
preserving the historical shared secret) or `per_client`.

The `enabled` default flipped from `true` to `false`: with an empty `secret` the
endpoint answers anonymously, so a fresh install must not expose it until the user
opts in. A `settings.json` that already says `"enabled": true` is honoured, and if
no secret is configured while the server is on, the server logs a warning and the
settings UI carries a note next to the toggle.

`listen_address` is what lets the headless daemon serve a LAN. It is not a pure
preference: a non-loopback value only starts together with authentication,
enforced by `public_bind_rejection` in the server. `allow_any_origin` emits a CORS
wildcard and `exit_on_shutdown` lets `aria2.shutdown` stop the process; both
default to the desktop's safe behavior (no wildcard, no exit) and the daemon
overrides them.

`Aria2Client` carries `id`, `name`, `token_hash` and `created_at_ms`. `token_hash`
is an **Argon2id PHC string** — the plaintext token is generated once in the UI,
shown once, and never persisted. The settings editor only writes `token_hash` on
save; `parse_aria2_clients` validates name uniqueness and non-empty hash without
running Argon2.

Evidence: `repo://crates/limedl-core/src/types/settings.rs#L330-L422`,
`repo://crates/limedl-core/src/settings/mod.rs#L128-L170`,
`repo://crates/limedl-core/src/aria2_rpc/server.rs#L51-L59`.

## Save fan-out

`Dispatcher::save_settings_with(mutate)` is the write path used by both
frontends:

1. `SettingsService::update_with(mutate)` normalizes, persists and updates memory.
2. `registry.update_all_settings(&saved)` broadcasts to every backend —
   `DownloadManager::apply_settings` rebuilds the HTTP client, rate limiter and
   buffer limits; `IrontideBtBackend::update_settings` hot-reloads engine tuning
   and rebuilds the `.torrent` client.
3. The CDN service is cleared when `cdn_acceleration.enabled` is false.
4. `ConcurrencyManager` max HTTP is recomputed from the active scheduler mode and
   max BT from `bt.max_downloads` (clamped 1..=1000), then the scheduler is
   notified.
5. Buffer-pool limits are pushed directly into `DiskIoService` as a fallback.

`save_settings` is a wrapper that replaces the whole struct; `factory_reset` saves
defaults. The daemon uses this path once at startup to persist a `--download-dir`
override.

Evidence: `repo://crates/limedl-core/src/dispatcher.rs#L281-L348`,
`repo://crates/limedl-server/src/lib.rs#L64-L76`.

The desktop adds UI-specific side effects in `settings_sync` (autostart sync,
Aria2 RPC restart, language/tray/appearance push) — see
[Native Desktop UI](../desktop/native-ui-architecture.md).

## The no-shadow-copy rule

The settings read is a convenience, never a cache. `AppContext::settings()` in the
desktop reads through `Dispatcher::get_settings_blocking()`, and writers must use
`save_settings_with`. The desktop once kept a shadow `AppSettings` copy with nine
write-back points; that was removed because it drifted from the persisted value.

Evidence: `repo://crates/limedl-native/src/context.rs#L37-L51`.

Related pages: [Bootstrap, SystemContext and Shared Services](../architecture/bootstrap-and-services.md),
[Networking, HTTP Clients and Rate Control](networking-and-rate-control.md),
[Aria2 JSON-RPC Compatibility Server](../integrations/aria2-rpc-server.md),
[Headless Server Daemon](../integrations/headless-server-daemon.md),
[Native Desktop UI (Slint)](../desktop/native-ui-architecture.md).
