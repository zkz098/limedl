# DOWNLOAD ENGINE (Rust)

**12 files, ~6400 lines.** Core download module handling HTTP, BitTorrent, and SFTP protocols.

## STRUCTURE

```
download/
├── mod.rs           # Module declarations + re-exports
├── commands.rs      # 13 #[tauri::command] functions (thin dispatch layer)
├── manager.rs       # DownloadManager — 3414-line god object (HTTP engine + scheduling + AIMD + persistence)
├── torrent.rs       # TorrentManager — librqbit wrapper (2 impl blocks, pending/resolved states)
├── sftp.rs          # SftpManager — ssh2 wrapper
├── types.rs         # All shared types/enums + AppSettings + defaults
├── error.rs         # DownloadError enum (thiserror)
├── http.rs          # Response classification, range support, content-disposition
├── manifest.rs      # Manifest model — chunk planning, checksum, serialization
├── metalink.rs      # Metalink XML parser
├── file_alloc.rs    # Sparse file preallocation + I/O (cfg(unix)/cfg(windows))
└── logging.rs       # tracing-subscriber setup + runtime reload
```

## WHERE TO LOOK

| Task                 | Location                                                | Notes                                                  |
| -------------------- | ------------------------------------------------------- | ------------------------------------------------------ |
| HTTP download flow   | `manager.rs` → `impl DownloadManager`                   | ~1622-line impl block starting at ~line 123            |
| AIMD rate control    | `manager.rs` → `AimdState`                              | 8-field struct, embedded in manager                    |
| Task ID routing      | `commands.rs` → `is_bt_task_id()` / `is_sftp_task_id()` | Fragile string prefix matching                         |
| Torrent lifecycle    | `torrent.rs` → `impl TorrentManager`                    | Two separate impl blocks (line 62, line 349)           |
| SFTP connection      | `sftp.rs`                                               | CONNECT_TIMEOUT 20s, IO_TIMEOUT 45s, BUFFER_SIZE 128KB |
| Settings persistence | `types.rs` → `AppSettings` + `manager.rs` persistence   | Settings stored alongside download manifests           |
| Error conversion     | `commands.rs` → `into_command_result()`                 | Wraps `anyhow::Result` → `Result<T, String>`           |

## CONVENTIONS

- **Error handling**: Commands return `Result<T, String>`. Use `into_command_result()` + `.context()` for enrichment. Domain errors via `thiserror` (`DownloadError`).
- **State management**: `AppState` holds `DownloadManager` + `TorrentManager` + `SftpManager`. Managed via `app.manage()` in `lib.rs`.
- **Module visibility**: `mod.rs` re-exports only the public API. No `pub use *` — explicit per-item.
- **IO**: Prefer `tokio::fs` for async file I/O. `fs4` for file locking. Sparse allocation via `file_alloc.rs`.
- **Hashing**: `blake3` for file integrity, `sha2` for piece hashing, `xxhash-rust` (xxh3) for dedup/fingerprinting.
- **Constants**: Module-level `const` blocks at file top. See `manager.rs:43-47`, `sftp.rs:26-28`.
- **Serde**: `#[serde(rename_all = "camelCase")]` on all shared types for JSON interop with TypeScript.

## ANTI-PATTERNS

- **manager.rs god object** — 3414 lines, 108 functions. NEVER add more to this file. Split into `scheduler.rs`, `aimd.rs`, `persistence.rs`, `http_client.rs`.
- **Copy-paste dispatch** — all 13 commands in `commands.rs` repeat the same 3-branch `if is_bt → else if is_sftp → else` pattern. Extract to macro or trait-based router.
- **No bare `.unwrap()` calls** — all usages are safe `.unwrap_or()` variants with fallbacks.
- **String-based task routing** — `is_bt_task_id()` / `is_sftp_task_id()` use fragile prefix matching. Consider an enum `TaskId::Http(Uuid) | Bt(RqbitId) | Sftp(Uuid)`.
- **Inline tests** — `manager.rs` includes 579 lines of test infrastructure (axum server, test functions) at the bottom. Move to `tests/` directory.
- **Two `impl TorrentManager` blocks** — separated by helper functions. First block (line 62) handles lifecycle/startup, second (line 349) handles CRUD actions. Defensible design, not an anti-pattern.
