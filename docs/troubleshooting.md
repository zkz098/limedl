# Troubleshooting

Known issues and accepted warnings. New issues with a non-zero exit code are
still real problems.

## Mirror anti-abuse `403` (e.g. Tsinghua TUNA)

**Symptom**: the browser can download the same URL but limedl returns
`403 Forbidden`, with
`http status 403 Forbidden (server anti-abuse check rejected this client; update the default User-Agent in Settings or try another mirror)`.

**Cause**: the TUNA edge (`mirrors.tuna.tsinghua.edu.cn`) classifies clients by
UA: a UA that claims to be a browser but whose version is outdated or not yet
released is treated as "uncommon software" (a disguised browser) and rejected.
Measured 2026-09, with Chrome 154 stable at the time:

| UA | Result |
| --- | --- |
| `Chrome/124` (old built-in default) | GET 403; HEAD 200 (HEAD is exempt, so probing looked fine) |
| `Chrome/137`–`Chrome/141` | 206 OK |
| `Chrome/143`, `Chrome/145` | 403 (unreleased versions are rejected too) |
| `curl/8.7.1`, `aria2/1.37.0`, `limedl/0.3.13` | 206 (tool UAs take another path) |
| IPv4 egress | all UAs 403 when the whole subnet is flagged; IPv6 works |

**Handling**:

1. Bump the built-in UA: change
   `crates/limedl-core/src/types/settings.rs::default_http_user_agent()` to the
   current stable Chrome major, and sync the UI copy and i18n (see
   [`openwiki/systems/networking-and-rate-control.md`](../openwiki/systems/networking-and-rate-control.md)).
2. Existing `settings.json` does **not** follow automatically (the user's custom
   value is preserved); clear the Default User-Agent in Settings or type a new
   browser UA.
3. If it still 403s after the UA update, compare `curl -4` / `curl -6`: when the
   IPv4 subnet is flagged by TUNA you need a different network or IPv6.
4. This is server policy, not a limedl Referer problem; the code already detects
   anti-abuse pages and skips pointless Referer probing (see
   [`openwiki/workflows/http-download-lifecycle.md`](../openwiki/workflows/http-download-lifecycle.md)).

## Durable vs received progress (crash consistency)

A download has two progress counters per chunk and they are deliberately not the
same number:

| Field | Meaning | Used by |
| --- | --- | --- |
| `ChunkManifest::downloaded`, `::completed` | bytes **received** into the write buffer | the scheduler, the claim/unclaim logic, the UI |
| `ChunkManifest::durable_downloaded` (+ `DownloadCore::durable_bytes`) | bytes that provably reached the file | persistence, and therefore resume |

`record_progress_on_managed` only advances the received counters;
`record_durable_bytes` is the **only** writer of the durable ones and is called
from the write buffer after a flush (or from the direct-write paths). The database
stores `downloaded = chunk.durable_bytes()` and
`completed = durable >= chunk.len()` (`chunk_to_params`), so a row can never claim
more than the file holds. Before this, the 300 ms persist cycle wrote the received
counters, which run ahead of the file by whatever the buffer still held — a hard
kill then left the database claiming a chunk was complete while its bytes were
only in memory, and the next resume skipped over the resulting hole.

**The durability boundary is `pwrite`, not `fsync`.** A successful flush counts,
which covers a process crash, `SIGKILL` and the OOM killer. It does **not** cover
power loss: those bytes are in the page cache, and so are the SQLite commits
(`synchronous = NORMAL` in WAL mode), so the two are lost together rather than one
running ahead of the other. Closing that gap would need an fsync per flush, which
the HDD path deliberately avoids (`SyncMode::Adaptive` syncs at ≥16 MB / ≥3 s).

`downloads.db` is written through three paths and all three substitute the durable
values: `persist_manifest_snapshot` (the 300 ms cycle), `persist_manifest_snapshots_batch`
(the scheduler's 2 s cycle) and `DownloadManager::persist` (the full upsert on state
transitions). `chunk.durable_bytes()` also clamps to the received count, so a reset
path that forgot to clear a counter can only under-report — which costs a
re-download, never a hole.

Finalization re-checks the invariant: `ensure_core_fully_durable` refuses to
publish a file whose chunks are not all durable (a failed flush, a dropped buffer)
instead of renaming a truncated file into place. It takes the already-held core
guard — calling `lock_core()` there would deadlock, see
[test-regression-notes.md](test-regression-notes.md).

## Startup resilience

A GUI process has no console (`windows_subsystem = "windows"`) and, before this
was added, no way to report a fatal error — a failure during startup made the
window never appear with nothing written anywhere. Now:

- **Logging comes up before the engine** (`main` calls `init_logging` with the
  defaults, then re-applies the configured settings), so a bootstrap, settings or
  migration failure is logged. `logs/limedl.log` and `logs/crash.log` live in the
  core state directory (`<data-dir>/downloads`).
- **A crash log is written by a panic hook** (`crates/limedl-native/src/crash.rs`).
  The release profile builds with `panic = "abort"`, and the hook still runs on the
  way to `abort()`, so the report (timestamp, thread, `file:line`, payload,
  backtrace) is the only evidence a panic leaves. It rotates at 1 MiB to
  `crash.log.1`. Note that `catch_unwind` anywhere in the tree is a dev/test-only
  safety net for the same reason.
- **A fatal startup error opens a message box** with the error and the crash-log
  path, then exits non-zero.
- **A corrupt `settings.json` no longer bricks the app**: the parse failure moves
  the file to `settings.json.corrupt`, tries `settings.json.bak` (the previous
  save, written before every rename), and only then falls back to defaults.
  `persist_settings` flushes *and* `sync_all`s the temp file before the rename, so
  a power loss cannot leave an empty `settings.json` behind.
- **A `downloads.db` that fails `PRAGMA quick_check` — or cannot be opened at all
  — is moved to `downloads.db.corrupt-<unix_ms>`** (with its `-wal`/`-shm`
  sidecars) and a fresh database is created, so the app stays usable. Task history
  is lost, the downloaded files are not. Lock/busy errors are *not* treated as
  corruption, and databases above 128 MiB skip the probe so startup stays bounded.
- **A failing tray icon no longer prevents startup.** On Linux a missing
  StatusNotifier host used to abort the launch before the window was shown, even
  though `tray_init_failure_message` promises the app keeps running.

## Schema migrations are transactional and idempotent

Each migration body and its `user_version` bump commit in one transaction
(`Database::open`), and every `ALTER TABLE ... ADD COLUMN` goes through
`add_column_if_missing`. Without that, a failure midway (disk full, I/O error,
kill) left the schema half-applied while `user_version` still named the old
version; the next start replayed the same migration and failed with "duplicate
column name" on every launch thereafter. The legacy `user_version` probe now only
ever moves the detected version forward.

## Aria2 RPC is disabled by default

`Aria2RpcSettings::default()` has `enabled = false`. With an empty `secret` the
endpoint answers anonymously, and while it binds loopback that still lets any local
process (or a page the default CORS policy admits) drive downloads and read paths,
so opting in is an explicit action: enable it in Settings → Aria2 RPC and set a
secret or per-client tokens. An existing `settings.json` with `"enabled": true` is
honoured, and the server logs a warning whenever it serves without authentication.

## Accepted warnings (do not need fixing)

These used to be documented here and all came from the retired Tauri shell
(`src-tauri/`), which has been removed:

- **`LNK4078` "multiple `.rsrc` sections"** — came from `src-tauri/build.rs`
  embedding a ComCtl32 v6 manifest while `tauri_build::build()` embedded one
  through `tauri-winres`. `crates/limedl-native/build.rs` only calls `winres` for
  icon/version metadata, so the duplicate section cannot occur. There is no custom
  manifest code to preserve.
- **`quick-xml` RUSTSEC-2026-0194 / RUSTSEC-2026-0195** — accepted advisories for
  `quick-xml 0.39.x`, pinned by `wayland-scanner 0.31.10` in the Slint/winit Linux
  dependency tree. The tree now resolves `wayland-scanner 0.31.11` →
  `quick-xml 0.41.0`, which is patched (`patched = [">= 0.41.0"]`), so `deny.toml`
  no longer ignores them and CI runs a plain `cargo audit`.
- **`winreg` `multiple-versions` warning** — came from `auto-launch` (via
  `tauri-plugin-autostart`) and `embed-resource` (via `tauri-winres`). `winreg`
  now appears exactly once (`0.52.0`, used by
  `crates/limedl-native/src/autostart.rs`), so `cargo deny` reports no duplicate.
