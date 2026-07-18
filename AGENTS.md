# AGENTS.md — flareget

> Compact instruction file for OpenCode sessions. Only includes what an agent would likely miss.

## MSVC environment (Windows builds)

**Before any Rust build/check/test/clippy on Windows**, initialize the MSVC environment first:

```powershell
& "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
```

Skipping this will cause linker errors (`LINK : fatal error LNK1181`).

## Package manager & tooling

- **Package manager**: pnpm v11 (managed by corepack, `packageManager` field in `package.json`). Use `pnpm install --frozen-lockfile`, not `bun install`.
- **Lint**: `pnpm run lint` (oxlint, correctness/suspicious = error, perf = warn)
- **Format**: `pnpm run format` (oxfmt — not prettier)
- **Type-check**: `pnpm exec vue-tsc --noEmit` (Vue type-checking, separate from build)
- **Test (frontend)**: `pnpm run test` (Vitest + jsdom)
- **Test (Rust - workspace)**: `cargo test --workspace`
- **Test (Rust - core only)**: `cargo test --manifest-path crates/flareget-core/Cargo.toml`
- **Test (Rust - Tauri)**: `cargo test --manifest-path src-tauri/Cargo.toml --features test-utils`
- **Build order**: `vue-tsc --noEmit` then `vite build` (enforced by `pnpm run build`)

## Architecture

### Workspace layout (3 crates)

```
flareget/
├── crates/
│   ├── flareget-core/       # Pure download engine (zero UI deps)
│   └── flareget-server/     # axum HTTP/WS server + CLI binary
├── src-tauri/               # Tauri v2 desktop app (thin shell)
├── src/                     # Vue 3 frontend (shared across targets)
└── Cargo.toml               # workspace [members]
```

- **`flareget-core`**: Download engine — manager, scheduler, buffer pool, CDN, BT backend, settings, database, event bus, checksum, rate limiter. All modules live in `crates/flareget-core/src/`.
- **`flareget-server`**: NAS/headless daemon. Axum HTTP server + WebSocket JSON-RPC 2.0 + CLI (`daemon` / `download` subcommands) + HTTP Basic Auth + static file serving. Entry: `crates/flareget-server/src/main.rs`.
- **`src-tauri`**: Tauri v2 desktop app. Thin commands layer (`commands.rs`, `commands_cdn.rs`, `aria2_rpc.rs`) that dispatches to `flareget_core::BackendRegistry`. Entry: `src-tauri/src/lib.rs`.

### Multi-platform support

| Target | Frontend | Backend | Build |
|--------|----------|---------|-------|
| Tauri Desktop | Vue 3 via Tauri IPC (`#invoke` → `@tauri-apps/api/core`) | `src-tauri/` | `pnpm run tauri dev` / `pnpm run tauri build` |
| NAS WebUI | Same Vue 3 via WebSocket (`#invoke` → `ws-invoke.ts`) | `flareget-server` | `pnpm run build:nas` |
| CLI | N/A | `flareget-server` | `flareget daemon` / `flareget download <url>` |

### Frontend dual-mode (`#invoke` / `#event`)

The Vue frontend uses import aliases so the same code runs on both Tauri IPC and WebSocket:

- **Tauri mode**: `#invoke` → `@tauri-apps/api/core`, `#event` → `@tauri-apps/api/event`
- **NAS mode**: `#invoke` → `src/lib/ws/ws-invoke.ts`, `#event` → `src/lib/ws/ws-event.ts`

Switched via `vite.config.ts` resolve.alias based on `mode === "nas"`.

**Never call `invoke` directly** — use the typed wrappers in `src/lib/tauri/*-api.ts`. These import from `#invoke`.

### Event system (EventBus)

`EventBus` (`crates/flareget-core/src/event_bus/mod.rs`) is a pure `tokio::sync::broadcast::channel<DownloadEvent>` with zero UI dependency. Each adapter subscribes independently:

- **Tauri**: `src-tauri/src/lib.rs` spawns a background task that subscribes to EventBus and calls `app_handle.emit()` for each event type
- **NAS WebSocket**: `rpc.rs` spawns a per-connection task that subscribes to EventBus and relays events over WebSocket
- **Aria2 RPC**: subscribed directly via `event_bus.subscribe()`

### Protocol routing (BackendRegistry + DownloadBackend)

`DownloadBackend` trait (`crates/flareget-core/src/protocol.rs`) defines the unified API for all download protocols. `BackendRegistry` (`crates/flareget-core/src/backend_registry.rs`) routes by `TaskId` prefix:

- `http:` prefix → `DownloadManager` (HTTP downloads)
- `bt:` prefix → `IrontideBtBackend` (BitTorrent)

Tauri commands and WebSocket RPC both dispatch through the same registry.

### Startup & shutdown

**Tauri** (`src-tauri/src/lib.rs`):
state dirs → RateLimiter → EventBus → DownloadManager → IrontideBtBackend → CdnAccelerator → BackendRegistry → optional Aria2 RPC → app.manage(AppState)

**NAS daemon** (`crates/flareget-server/src/main.rs`):
config load → state dirs → RateLimiter → EventBus → DownloadManager → BackendRegistry → axum router (WebSocket RPC + static files) → serve

### Rust edition & lib names

- Edition: 2024 (all crates)
- `flareget-core` lib name: `flareget_core`
- `src-tauri` lib name: `flareget_lib` (suffixed `_lib` to avoid Windows name collision)

### Documentation maintenance

> **After any add/modify/delete/refactor, update the corresponding subsystem guide.** Outdated docs are worse than no docs — they actively mislead. At minimum: check that struct fields, method signatures, and file paths still match the source. If a new subsystem or protocol is added, create its guide following the four-section template.

### Subsystem documentation

Detailed four-section docs (module responsibility, key structs, key methods, data flow) live in `.opencode/guides/`. **Before modifying any subsystem, read its guide first.**

| Guide | Source (Rust) | Role |
|-------|---------------|------|
| `subsystem-download-manager.md` | `manager.rs` + `http_executor.rs` + `scheduler.rs` + `aimd.rs` | HTTP download lifecycle |
| `subsystem-bt-backend.md` | `bt_backend_own/` | BitTorrent via irontide engine |
| `subsystem-cdn-accelerator.md` | `cdn/` | Cloudflare IP probing & DNS rewriting |
| `subsystem-aria2-rpc.md` | `aria2_rpc.rs` (Tauri crate: `src-tauri/src/download/`) | Axum WebSocket + HTTP JSON-RPC |
| `subsystem-database.md` | `database.rs` | rusqlite with `bundled` feature |
| `subsystem-buffer-pool.md` | `buffer_pool.rs` | HDD double-buffer / SSD write-combining |
| `subsystem-settings.md` | `settings.rs` + `types.rs` | JSON-based settings load/save |

## Tauri dev workflow

- `pnpm run tauri dev` — starts Vite dev server (port 1420) then opens Tauri window
- `pnpm run tauri build` — builds frontend then compiles Rust + bundles
- CSP is disabled (`"csp": null` in `tauri.conf.json`)

## Serialization conventions

- Rust structs: `#[serde(rename_all = "camelCase")]` — fields are camelCase in JSON/JS
- Rust enums: `#[serde(rename_all = "snake_case")]` — variants are snake_case in JSON
- Frontend TypeScript interfaces mirror these exact casing conventions

## Testing

See **`.opencode/guides/testing-guide.md`** for test patterns, mock setup, and E2E configuration.

Quick ref: `pnpm run test` (frontend), `cargo test --workspace` (Rust), `e2e/` (Playwright, pending).

## Core data flow & buffer pool

Moved to standalone guides — read them before touching download pipeline or I/O code:

- **`.opencode/guides/core-data-flow.md`** — HTTP download full lifecycle through 6 subsystems
- **`.opencode/guides/subsystem-buffer-pool.md`** — HDD double-buffer vs SSD write-combining, game mode, slot management

## CI (`.github/workflows/ci.yml`)

- Ubuntu-latest only
- Frontend: Node.js 24 + corepack → `pnpm install --frozen-lockfile` → `pnpm run lint` → `pnpm exec vue-tsc --noEmit` → `pnpm run test`
- Rust: `cargo check --workspace` → `cargo clippy --workspace -- -D warnings` → `cargo test --workspace`
- Rust clippy denies all warnings

## Frontend UI

**Before writing any frontend UI code**, read `.opencode/guides/ui-design-guide.md` (design tokens) and `.opencode/guides/ui-component-guide.md` (component catalog). Key rules: never hardcode colors/spacing (use `var(--token)`), use `i-ri-*` for icons with `aria-hidden="true"`, `<style scoped>` only, `:focus-visible` on all interactive elements.

### Frontend stack

- **Framework**: Vue 3 Composition API + `<script setup lang="ts">`
- **CSS**: UnoCSS (`presetUno` + `presetIcons`) + scoped CSS with design tokens
- **State**: Composables (`use*` pattern in `src/composables/`), no Pinia/Vuex
- **i18n**: `useI18n()` → `{ t, language, languageOptions, setLanguage, supportedLanguages }` from `src/i18n/`
- **Tauri bridge**: `src/lib/tauri/*-api.ts` typed wrappers. All import `invoke` from `#invoke` (NOT `@tauri-apps/api/core` directly). See `src/lib/ws/ws-invoke.ts` for the WebSocket-equivalent implementation.

## Build flags

- `.cargo/config.toml` sets `target-cpu=x86-64-v3` for x86_64 targets (app targets modern desktops — Haswell 2013+ / Excavator 2015+). macOS aarch64 is unaffected.

## Known warnings

### `LNK4078` on Windows release builds

```
resource.lib : warning LNK4078: found multiple ".rsrc" sections with different attributes (40000040)
```

**Harmless.** Caused by `build.rs` manually embedding a ComCtl32 v6 manifest for the binary target, while `tauri_build::build()` also embeds one via `tauri-winres`. The manual embedding is intentional — it ensures the manifest is present in test binaries (`cargo test --workspace`), not just the release binary. Do not remove the custom `build.rs` manifest code.
