# AGENTS.md — flareget

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
- **Rust lib name**: `flareget_lib` (suffixed `_lib` to avoid Windows name collision with binary — see `Cargo.toml` comment and [cargo#8519](https://github.com/rust-lang/cargo/issues/8519))

### Task ID routing

Download tasks use prefixed IDs to route to the correct protocol executor:

- `http:` prefix → HTTP download path
- `bt:` prefix → BitTorrent path

The `DownloadProtocol` trait (`src-tauri/src/download/protocol.rs`) abstracts both.

### Subsystem documentation

Detailed four-section docs (module responsibility, key structs, key methods, data flow) live in `.opencode/guides/`. **Before modifying any subsystem, read its guide first.**

| Guide | Source (Rust) | Role |
|---|---|---|
| `subsystem-download-manager.md` | `download/manager.rs` + `http_executor.rs` + `scheduler.rs` + `aimd.rs` | HTTP download lifecycle orchestration |
| `subsystem-bt-backend.md` | `download/bt_backend_own/` | BitTorrent via irontide engine |
| `subsystem-cdn-accelerator.md` | `download/cdn/` | Cloudflare IP probing & DNS rewriting (currently Cloudflare-only) |
| `subsystem-aria2-rpc.md` | `download/aria2_rpc.rs` | Axum WebSocket + HTTP JSON-RPC emulating aria2 protocol |
| `subsystem-database.md` | `download/database.rs` | rusqlite with `bundled` feature |
| `subsystem-buffer-pool.md` | `download/buffer_pool.rs` | HDD double-buffer / SSD write-combining memory pool |
| `subsystem-settings.md` | `download/settings.rs` + `types.rs` | JSON-based settings load/save, HTTP client builder |

**Cross-cutting docs**: `core-data-flow.md` — full HTTP download lifecycle across all subsystems.

### Documentation maintenance

> **After any add/modify/delete/refactor, update the corresponding subsystem guide.** Outdated docs are worse than no docs — they actively mislead. At minimum: check that struct fields, method signatures, and file paths still match the source. If a new subsystem or protocol is added, create its guide following the four-section template.

### Startup & shutdown

- App entry: `src-tauri/src/lib.rs` → `run()`
- On startup: state dirs → RateLimiter → DownloadManager → logging → OwnBtBackend → CdnAccelerator → optional Aria2 RPC
- On window close: DownloadManager.shutdown() → OwnBtBackend.shutdown() → exit
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

See **`.opencode/guides/testing-guide.md`** for test patterns, mock setup, and E2E configuration.

Quick ref: `bun run test` (frontend), `cargo test --workspace` (Rust), `e2e/` (Playwright, pending).

## Core data flow & buffer pool

Moved to standalone guides — read them before touching download pipeline or I/O code:

- **`.opencode/guides/core-data-flow.md`** — HTTP download full lifecycle through 6 subsystems
- **`.opencode/guides/subsystem-buffer-pool.md`** — HDD double-buffer vs SSD write-combining, game mode, slot management

## CI (`.github/workflows/ci.yml`)

- Ubuntu-latest only
- Frontend: `bun install --frozen-lockfile` → `bun run lint` → `bunx vue-tsc --noEmit` → `bun run test`
- Rust: `cargo check --workspace` → `cargo clippy --workspace -- -D warnings` → `cargo test --workspace`
- Rust clippy denies all warnings

## Frontend UI

**Before writing any frontend UI code**, read `.opencode/guides/ui-design-guide.md` (design tokens) and `.opencode/guides/ui-component-guide.md` (component catalog). Key rules: never hardcode colors/spacing (use `var(--token)`), use `i-ri-*` for icons with `aria-hidden="true"`, `<style scoped>` only, `:focus-visible` on all interactive elements.

### Frontend stack

- **Framework**: Vue 3 Composition API + `<script setup lang="ts">`
- **CSS**: UnoCSS (`presetUno` + `presetIcons`) + scoped CSS with design tokens
- **State**: Composables (`use*` pattern in `src/composables/`), no Pinia/Vuex
- **i18n**: `useI18n()` → `{ t, language, languageOptions, setLanguage, supportedLanguages }` from `src/i18n/`
- **Tauri bridge**: `src/lib/tauri/*-api.ts` typed wrappers (never call `invoke` directly)

## Build flags

- `.cargo/config.toml` sets `target-cpu=x86-64-v3` for x86_64 targets (app targets modern desktops — Haswell 2013+ / Excavator 2015+). macOS aarch64 is unaffected.

## Known warnings

### `LNK4078` on Windows release builds

```
resource.lib : warning LNK4078: found multiple ".rsrc" sections with different attributes (40000040)
```

**Harmless.** Caused by `build.rs` manually embedding a ComCtl32 v6 manifest for the binary target, while `tauri_build::build()` also embeds one via `tauri-winres`. The manual embedding is intentional — it ensures the manifest is present in test binaries (`cargo test --workspace`), not just the release binary. Do not remove the custom `build.rs` manifest code.
