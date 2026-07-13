# DOWNLOAD ENGINE (Rust)

**23 files, ~6000 lines.** Core download module handling HTTP, BitTorrent, and SFTP protocols.

## STRUCTURE

```
download/
├── mod.rs           # Module declarations + re-exports
├── commands.rs      # 13 #[tauri::command] functions (thin dispatch layer)
├── manager.rs       # DownloadManager — lifecycle, start/stop, settings, download listing (~1211 lines)
├── http_executor.rs # HTTP download execution — probe, run_download, download_single/chunked, finalize, download_chunk (~705 lines)
├── checksum.rs      # Checksum calculation (blake3/sha256)
├── retry.rs         # request_with_retry — retry logic with backoff
├── persistence.rs   # DB persist helpers (persist_manifest_snapshot)
├── settings.rs      # Settings normalization, user-agent resolution, HTTP client building (~345 lines)
├── scheduler.rs     # Background scheduler loop, AIMD rebalancing, network learning (~531 lines)
├── aimd.rs          # AimdState — additive-increase/multiplicative-decrease rate control
├── torrent.rs       # TorrentManager — librqbit wrapper (2 impl blocks, pending/resolved states)
├── sftp.rs          # SftpManager — ssh2 wrapper
├── types.rs         # All shared types/enums + AppSettings + defaults
├── error.rs         # DownloadError enum (thiserror)
├── http.rs          # Response classification, range support, content-disposition
├── manifest.rs      # Manifest model — chunk planning, checksum, serialization
├── metalink.rs      # Metalink XML parser
├── file_alloc.rs    # Sparse file preallocation + I/O (cfg(unix)/cfg(windows))
├── logging.rs       # tracing-subscriber setup + runtime reload
├── rate_limiter.rs  # Token-bucket rate limiter
├── database.rs      # SQLite database layer
├── migration.rs     # JSON manifest → SQLite migration
└── aria2_rpc.rs     # Aria2 RPC server emulation
```

## WHERE TO LOOK

| Task                 | Location                                                | Notes                                                  |
| -------------------- | ------------------------------------------------------- | ------------------------------------------------------ |
| HTTP download flow   | `http_executor.rs`                                      | Full HTTP download pipeline (probe → download → finalize) |
| AIMD rate control    | `aimd.rs` → `AimdState`                                 | 8-field struct, used during chunked downloads          |
| Settings management  | `settings.rs` → `normalize_settings`, `resolve_user_agent` | Settings normalization + HTTP client construction      |
| Background scheduler | `scheduler.rs`                                          | Rebalance loop, adaptive thread allocation, network learning |
| Task ID routing      | `commands.rs` → `dispatch_download_action!` macro       | Routes by `TaskId` enum (Http/Bt/Sftp)                 |
| Torrent lifecycle    | `torrent.rs` → `impl TorrentManager`                    | Two separate impl blocks (line 62, line 349)           |
| SFTP connection      | `sftp.rs`                                               | CONNECT_TIMEOUT 20s, IO_TIMEOUT 45s, BUFFER_SIZE 128KB |
| Error conversion     | `commands.rs` → `into_command_result()`                 | Wraps `anyhow::Result` → `Result<T, String>`           |

## CONVENTIONS

- **Error handling**: Commands return `Result<T, String>`. Use `into_command_result()` + `.context()` for enrichment. Domain errors via `thiserror` (`DownloadError`).
- **State management**: `AppState` holds `DownloadManager` + `TorrentManager` + `SftpManager`. Managed via `app.manage()` in `lib.rs`.
- **Module visibility**: `mod.rs` re-exports only the public API. No `pub use *` — explicit per-item.
- **IO**: Prefer `tokio::fs` for async file I/O. `fs4` for file locking. Sparse allocation via `file_alloc.rs`.
- **Hashing**: `blake3` for file integrity, `sha2` for piece hashing, `xxhash-rust` (xxh3) for dedup/fingerprinting.
- **Constants**: Module-level `const` blocks at file top. See `sftp.rs:26-28`, `aimd.rs`.
- **Serde**: `#[serde(rename_all = "camelCase")]` on all shared types for JSON interop with TypeScript.

## ANTI-PATTERNS

- ~~**manager.rs god object**~~ — RESOLVED. manager.rs is now ~1211 lines (down from ~2470). The HTTP execution chunk management functions, download flow, retry logic, persistence, settings normalization, and scheduling have all been extracted into dedicated modules.
- **Copy-paste dispatch** — all 13 commands in `commands.rs` repeat the same 3-branch `if is_bt → else if is_sftp → else` pattern. Extract to macro or trait-based router.
- **No bare `.unwrap()` calls** — all usages are safe `.unwrap_or()` variants with fallbacks. Use `lock_or_recover()` (in `mod.rs`) for poison-safe `std::sync::Mutex` access.
- **TaskId enum migration** — largely complete. `TaskId::Http`/`Bt`/`Sftp` enum replaces old `is_bt_task_id()`/`is_sftp_task_id()` prefix matching. Internal BT pending-task routing still uses string prefixes (encapsulated in `torrent.rs`).
- **Worker stop signaling** — `ManagedDownload.stop_notify: tokio::sync::Notify` signals worker completion. `wait_until_stopped()` uses a double-check + `notified()` pattern (no bounded polling). Workers MUST call `stop_notify.notify_one()` after setting `runtime = None`.
- **Inline tests** — `manager.rs` includes ~500 lines of test infrastructure (axum server, test functions) at the bottom. Move to `tests/` directory.
- **Two `impl TorrentManager` blocks** — separated by helper functions. First block (line 62) handles lifecycle/startup, second (line 349) handles CRUD actions. Defensible design, not an anti-pattern.
