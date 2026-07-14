# PROJECT KNOWLEDGE BASE

**Generated:** 2025-06-12
**Commit:** 42328c3
**Branch:** main

## OVERVIEW

Tauri v2 desktop download manager — Rust backend (single crate) + Vue 3/TypeScript frontend. Supports HTTP, BitTorrent (librqbit), Metalink, and SFTP protocols.

## STRUCTURE

```
./
├── src/                     # Vue 3 + TypeScript frontend (Vite 8, UnoCSS, Bun)
│   ├── components/          # → AGENTS.md (54 lines, design system + feature components)
│   ├── composables/         # Reactive download state management (7 files)
│   ├── lib/tauri/           # Tauri IPC bridge (download-api, settings-api, dialog-api)
│   ├── types/               # TS interfaces mirroring Rust serde types
│   └── i18n/                # en-US + zh-CN translations
├── src-tauri/               # Rust backend (single crate, edition 2024, nightly)
│   ├── src/main.rs          # Binary entry (mimalloc allocator → downloader_lib::run())
│   ├── src/lib.rs           # Tauri builder, 3 managers, 13 IPC commands
│   └── src/download/        # → AGENTS.md (core download engine, ~6000 lines, manager.rs split into http_executor.rs + checksum.rs + retry.rs + persistence.rs + settings.rs + scheduler.rs)
├── .github/workflows/ci.yml # CI: Bun lint + vue-tsc + cargo check + clippy (no tests)
└── package.json             # Bun scripts: dev, build, lint (oxlint), format (oxfmt), tauri
```

## WHERE TO LOOK

| Task                 | Location                             | Notes                                                                                                                                                                                                  |
| -------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Rust entry point     | `src-tauri/src/main.rs`              | 11-line shim, mimalloc allocator                                                                                                                                                                       |
| Tauri setup + IPC    | `src-tauri/src/lib.rs`               | 13 commands registered, 3 managers initialized                                                                                                                                                         |
| HTTP download engine | `src-tauri/src/download/`            | Split across: `manager.rs` (~1211 lines, lifecycle), `http_executor.rs` (~705 lines, HTTP execution), `checksum.rs`, `retry.rs`, `persistence.rs`, `settings.rs`, `scheduler.rs` — see child AGENTS.md |
| BitTorrent           | `src-tauri/src/download/torrent.rs`  | librqbit wrapper                                                                                                                                                                                       |
| SFTP                 | `src-tauri/src/download/sftp.rs`     | ssh2 wrapper                                                                                                                                                                                           |
| Tauri IPC commands   | `src-tauri/src/download/commands.rs` | Thin dispatch layer                                                                                                                                                                                    |
| Shared types         | `src-tauri/src/download/types.rs`    | AppSettings, enums, serde camelCase                                                                                                                                                                    |
| Frontend entry       | `src/main.ts`                        | Vue app mount                                                                                                                                                                                          |
| Download queue UI    | `src/components/downloader/`         | Queue table, inspector, composer                                                                                                                                                                       |
| Settings UI          | `src/components/settings/`           | 7 panels, dirty tracking                                                                                                                                                                               |
| Vue composables      | `src/composables/`                   | useDownloader, useDownloadList, usePolling, etc.                                                                                                                                                       |
| Tauri bridge         | `src/lib/tauri/`                     | download-api.ts, settings-api.ts, dialog-api.ts                                                                                                                                                        |

## CONVENTIONS

### Rust

- **Toolchain**: stable (edition 2024, stabilized in Rust 1.85). Pinned via `rust-toolchain.toml` (`channel = "stable"`).
- **Error handling**: Commands return `Result<T, String>`. Domain errors via `thiserror`. Propagation via `anyhow`.
- **IO**: `tokio::fs` for async, `fs4` for file locking.
- **Hashing**: `blake3` (integrity), `sha2` (pieces), `xxhash-rust` xxh3 (dedup).
- **Serde**: `#[serde(rename_all = "camelCase")]` on all shared types for TS interop.
- **Allocator**: `mimalloc` (not default).
- **Build flags**: CPU-specific flags removed from `.cargo/config.toml` to avoid SIGILL on older hardware. Set `RUSTFLAGS` env var for release builds.

### Frontend

- **All `<script setup lang="ts">`** — Composition API only. No Options API.
- **Props/Emits**: `defineProps<T>()` / `defineEmits<{ event: [arg: Type] }>()` — type-only generics.
- **CSS**: Scoped styles (`<style scoped>`). Use CSS custom properties (`var(--color-*)`, `var(--space-*)`).
- **Icons**: UnoCSS `i-ri-*` (Remix Icons via `@iconify-json/ri`).
- **State**: No Pinia. `useDownloader()` composable passed via props/emits.
- **Lint/Format**: `oxlint` + `oxfmt` (not ESLint/Prettier).
- **Toggle switches**: Toggle text MUST describe the **feature** the toggle controls, NOT the current state or action. The toggle's visual on/off indicator already communicates state. Dynamic text that changes between "Enable X" / "Disable X" creates ambiguity — is it describing current state or the action to take? Use static feature names (e.g., `"DHT Network"` not `"Enable DHT"` / `"Disable DHT"`).

### Cross-cutting

- **Package manager**: Bun (`bun install`, `bun run`, `bunx`). Lockfile: `bun.lock`.
- **CI**: Linux-only, nightly Rust, no `cargo test` in CI, no Windows/macOS matrix.
- **serde naming**: Uses `camelCase` (TS side) and `snake_case` (Rust enum variants serialized as strings).

## ANTI-PATTERNS (THIS PROJECT)

- ~~**manager.rs god object**~~ — RESOLVED. manager.rs is now ~1211 lines (down from ~2470). HTTP execution, chunk management, retry logic, persistence, settings normalization, and scheduling have all been extracted into dedicated modules. See `src-tauri/src/download/AGENTS.md` for details.
- **TaskId enum migration** — largely complete. `is_bt_task_id()`/`is_sftp_task_id()` eliminated from external use; internal BT pending-task routing still uses string prefixes (encapsulated in `torrent.rs`).
- **No bare `.unwrap()`** — use `.unwrap_or()` variants with fallbacks. `lock_or_recover()` macro in `mod.rs` for poison-safe mutex access.
- **Minimal tests** — 2 vitest files exist (smoke + type shape) but cover ~0% of business logic. Zero component/composable tests.
- **Monolithic components** — `SettingsPage.vue` (644 lines, down from 965 after `useSettingsForm` extraction), `DownloadQueueTable.vue` (945 lines).
- **CI runs tests** — `cargo test --workspace` and `bun run test` are in CI pipeline (`ci.yml`).
- **`rust-toolchain.toml`** pins stable channel (edition 2024 stabilized in Rust 1.85).

## COMMANDS

```bash
# Frontend (root)
bun run dev          # Vite dev server (port 1420)
bun run build        # vue-tsc --noEmit && vite build
bun run lint         # oxlint
bun run format       # oxfmt
bun run tauri dev    # Full Tauri desktop dev mode
bun run tauri build  # Production Tauri build

# Rust (src-tauri/)
cargo check          # Type-check (fast)
cargo clippy -- -D warnings  # Lint (same as CI)
cargo test           # Run all tests
cargo build          # Debug build
```

## NOTES

### Windows: Rust/Cargo Environment

Before using `cargo`, `rustc`, or any Rust tooling in PowerShell, load the MSVC environment:

```powershell
& "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
```

This is required for the MSVC linker (`link.exe`) to be available. Skipping this will cause linker errors.

### Post-Change Verification

After making Rust changes:

1. `cargo fmt` — format (if rustfmt is configured)
2. `cargo clippy -- -D warnings` — lint with strict warnings
3. `cargo check` — verify compilation

For large-scale or architecturally complex changes, **consult Oracle for independent review** before merging.
