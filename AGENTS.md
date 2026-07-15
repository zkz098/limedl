# AGENTS.md — downloader

> Compact instruction file for OpenCode sessions. Only includes what an agent would likely miss.

## MSVC environment (Windows builds)

**Before any Rust build/check/test/clippy on Windows**, initialize the MSVC environment first:

```powershell
& "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
```

Skipping this will cause linker errors (`LINK : fatal error LNK1181`).

## Package manager & tooling

- **Package manager**: Bun (`bun.lock`, not `package-lock.json`). Use `bun install --frozen-lockfile`, not `npm install`.
- **Lint**: `bun run lint` (oxlint, correctness/suspicious = error, perf = warn)
- **Format**: `bun run format` (oxfmt — not prettier)
- **Type-check**: `bunx vue-tsc --noEmit` (Vue type-checking, separate from build)
- **Test (frontend)**: `bun run test` (Vitest + jsdom)
- **Test (Rust)**: `cargo test --workspace` from repo root
- **Build order**: `vue-tsc --noEmit` then `vite build` (enforced by `bun run build`)

## Architecture

### Tauri v2 desktop download manager

- **Frontend**: Vue 3 + TypeScript + UnoCSS (`src/`)
- **Backend**: Rust + Tauri v2 (`src-tauri/`)
- **Rust edition**: 2024
- **Rust lib name**: `downloader_lib` (suffixed `_lib` to avoid Windows name collision with binary — see `Cargo.toml` comment and [cargo#8519](https://github.com/rust-lang/cargo/issues/8519))

### Task ID routing

Download tasks use prefixed IDs to route to the correct protocol executor:

- `http:` prefix → HTTP download path
- `bt:` prefix → BitTorrent path

The `DownloadProtocol` trait (`src-tauri/src/download/protocol.rs`) abstracts both.

### Key subsystems

| Component          | Location                                          | Role                                               |
| ------------------ | ------------------------------------------------- | -------------------------------------------------- |
| `DownloadManager`  | `src-tauri/src/download/manager.rs` (1200+ lines) | Core HTTP download orchestration                   |
| `TorrentManager`   | `src-tauri/src/download/torrent.rs` (1600+ lines) | BitTorrent via librqbit                            |
| `CdnAccelerator`   | `src-tauri/src/download/cdn/`                     | Cloudflare IP range probing & DNS rewriting        |
| Aria2 RPC server   | `src-tauri/src/download/aria2_rpc.rs`             | Axum WebSocket server emulating aria2 RPC protocol |
| SQLite persistence | `src-tauri/src/download/database.rs`              | rusqlite with `bundled` feature                    |

### Startup & shutdown

- App entry: `src-tauri/src/lib.rs` → `run()`
- On startup: state dirs → RateLimiter → DownloadManager → logging → TorrentManager → CdnAccelerator → optional Aria2 RPC
- On window close: TorrentManager is gracefully shut down before exit (intercepted close event)
- Allocator: `mimalloc` set as global allocator in `main.rs`

## Tauri dev workflow

- `bun run tauri dev` — starts Vite dev server (port 1420) then opens Tauri window
- `bun run tauri build` — builds frontend then compiles Rust + bundles
- CSP is disabled (`"csp": null` in `tauri.conf.json`)

## Serialization conventions

- Rust structs: `#[serde(rename_all = "camelCase")]` — fields are camelCase in JSON/JS
- Rust enums: `#[serde(rename_all = "snake_case")]` — variants are snake_case in JSON
- Frontend TypeScript interfaces mirror these exact casing conventions

## Testing

- Frontend tests: `src/__tests__/`, Vitest + jsdom, mock Tauri API via `src/__tests__/mocks/tauri-mock.ts`
- Rust unit tests: inline `#[cfg(test)]` modules
- Rust integration tests: `src-tauri/src/download/tests/`
- CI runs both `bun run test` and `cargo test --workspace`

## CI (`.github/workflows/ci.yml`)

- Ubuntu-latest only
- Frontend: `bun install --frozen-lockfile` → `bun run lint` → `bunx vue-tsc --noEmit` → `bun run test`
- Rust: `cargo check --workspace` → `cargo clippy --workspace -- -D warnings` → `cargo test --workspace`
- Rust clippy denies all warnings

## Frontend UI

### Design system & component catalog

**Before writing any frontend UI code**, read the guides:

- **`.opencode/guides/ui-design-guide.md`** — Design tokens (colors, typography, spacing, radii, shadows), icon system, CSS conventions, theme/dark-mode, accessibility minimums, responsive breakpoints
- **`.opencode/guides/ui-component-guide.md`** — Complete catalog of every shared UI component with props, slots, emits, and usage examples. Always check this before creating new UI — a component likely already exists.

Key rules:
- Never hardcode colors, spacing, or radii — use `var(--token)` from `src/styles.css`
- Use `i-ri-*` classes for icons (Remix Icon via UnoCSS), always add `aria-hidden="true"`
- Use `<style scoped>` in `.vue` SFCs; non-scoped CSS only for page-level shared layout classes
- `:focus-visible` required on every interactive element
- Empty states: use `UiEmptyState` component, never ad-hoc markup
- Settings/labs panels: use `SettingsSection` + `SettingsField` components
- Dialogs: `UiDialog` or `ConfirmDialog`; fullscreen: `ModalOverlay`

### Frontend stack

- **Framework**: Vue 3 Composition API + `<script setup lang="ts">`
- **CSS**: UnoCSS (`presetUno` + `presetIcons`) + scoped CSS with design tokens
- **State**: Composables (`use*` pattern in `src/composables/`), no Pinia/Vuex
- **i18n**: `useI18n()` → `{ t, language, languageOptions, setLanguage, supportedLanguages }` from `src/i18n/`
- **Tauri bridge**: `src/lib/tauri/*-api.ts` typed wrappers (never call `invoke` directly)

## Build flags

- `.cargo/config.toml` notes that `target-cpu=x86-64-v3` flags were removed to avoid SIGILL. For release builds, set `RUSTFLAGS` manually if desired (see comments in file).
