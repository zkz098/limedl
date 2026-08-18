# AGENTS.md — limedl

> Compact instruction file for OpenCode sessions. What an agent would miss from source alone.

## Environment

**Windows**: initialize MSVC before any Rust command:

```powershell
& "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
```

## Toolchain

| Purpose | Command |
|---------|---------|
| Install deps | `pnpm install --frozen-lockfile` |
| Frontend dev | `pnpm run tauri dev` |
| Lint | `pnpm run lint` (oxlint) |
| Format | `pnpm run format` (oxfmt) |
| Type-check | `pnpm exec vue-tsc --noEmit` |
| Test (frontend) | `pnpm run test` |
| Test (Rust) | `cargo test --workspace` |
| Build | `pnpm run build` (vue-tsc → vite build) |
| Version bump | `node scripts/bump-version.mjs patch` |

## Architecture

### Workspace

```
limedl/
├── crates/limedl-core/   # Pure download engine (lib: limedl_core)
├── crates/limedl-server/ # axum HTTP/WS server + CLI
├── src-tauri/            # Tauri v2 desktop shell (lib: limedl_lib)
└── src/                  # Vue 3 frontend (shared across targets)
```

All Rust crates use edition 2024.

### Multi-platform

| Target | Frontend | Backend | Build |
|--------|----------|---------|-------|
| Tauri Desktop | Vue 3 via Tauri IPC | `src-tauri/` | `pnpm run tauri dev` |
| NAS WebUI | Same Vue via WebSocket | `limedl-server` | `pnpm run build:nas` |
| CLI | N/A | `limedl-server` | `limedl daemon` / `limedl download <url>` |

### Frontend dual-mode

Vue uses import aliases so the same code runs on both Tauri IPC and WebSocket:

- `#invoke` → `@tauri-apps/api/core` (Tauri) or `src/lib/ws/ws-invoke.ts` (NAS)
- `#event` → `@tauri-apps/api/event` (Tauri) or `src/lib/ws/ws-event.ts` (NAS)

Switched via `vite.config.ts` resolve.alias. **Never call `invoke` directly** — use typed wrappers in `src/lib/tauri/*-api.ts`.

### Event system

`EventBus` = `tokio::sync::broadcast::channel<DownloadEvent>`. Each adapter subscribes independently:
- Tauri: `src-tauri/src/lib.rs` background task → `app_handle.emit()`
- NAS: `rpc.rs` per-connection task → WebSocket push
- Aria2 RPC: direct `event_bus.subscribe()`

### Protocol routing

`DownloadBackend` trait (unified API) → `BackendRegistry` routes by TaskId prefix:
- `http:` → `DownloadManager`
- `bt:` → `IrontideBtBackend`

Tauri commands and WebSocket RPC both dispatch through the same `Dispatcher` → `BackendRegistry`.

## Conventions

- Rust structs: `#[serde(rename_all = "camelCase")]`. Enums: `#[serde(rename_all = "snake_case")]`.
- Frontend UI: use `var(--token)` for colors/spacing, `i-ri-*` icons, `<style scoped>` only, `:focus-visible` on all interactive elements. Read `.opencode/guides/ui-design-guide.md` and `ui-component-guide.md` before writing UI code.
- Build: `.cargo/config.toml` sets `target-cpu=x86-64-v3`.
- CSP: explicit CSP is defined in `tauri.conf.json` (Tauri); NAS WebUI applies a strict CSP via server headers.

## Code generation

After modifying any Rust type serialized to the frontend, or adding WS commands/events, regenerate all generated `.ts` files:

```powershell
cargo test --manifest-path crates/limedl-core/Cargo.toml --features ts -- export_typescript_bindings
git diff --stat src/types/generated/ src/lib/ws/generated/
```

Full workflow: see `.opencode/guides/code-generation.md`.

## Guides

Read the relevant guide **before** modifying any subsystem. Update it **after**.

| Core guides | Rust subsystem guides |
|-------------|----------------------|
| `.opencode/guides/architecture-overview.md` | `subsystem-download-manager.md` (HTTP + checksum + rate limiter + data flow) |
| `.opencode/guides/code-generation.md` | `subsystem-bt-backend.md` |
| `.opencode/guides/troubleshooting.md` | `subsystem-cdn-accelerator.md` |
| `.opencode/guides/testing-guide.md` | `subsystem-aria2-rpc.md` |
| `.opencode/guides/ui-design-guide.md` | `subsystem-database.md` |
| `.opencode/guides/ui-component-guide.md` | `subsystem-buffer-pool.md` (includes file_ops) |
| | `subsystem-settings.md` |
| | `subsystem-event-bus.md` |
| | `subsystem-protocol-registry.md` |
| | `subsystem-http-client-factory.md` |

## Pre-commit verification gate (MANDATORY)

Never commit while any check is red. CI runs every check on the whole workspace
and fails if **any** test fails, warning is emitted, or error is raised —
regardless of whether your own diff caused it.

Therefore: **fix all failures, warnings, and errors before committing, even if
they pre-date your change or were not introduced by you.** Leaving a broken test
or warning "for later" blocks the entire pipeline and hides real regressions.

Run the full gate locally (Windows: init MSVC first):

```powershell
# Rust — clippy/build/test under -D warnings
& "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
$env:RUSTFLAGS="-D warnings"
$env:CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse"
cargo clippy --workspace --all-targets
cargo test --workspace

# Frontend
pnpm exec vue-tsc --noEmit
pnpm run lint
pnpm run test
```

Only commit once every check above is green. If a failure is environmental
(e.g. a Linux-only script on Windows), fix the code so it is platform-neutral or
otherwise reruns green in CI rather than committing around it.

## Dependency discipline

After `cargo update`/`cargo add`/`cargo remove`, commit changed lockfiles:

```powershell
git diff --stat Cargo.lock pnpm-lock.yaml
git add Cargo.lock pnpm-lock.yaml
```

Uncommitted lockfile changes cause CI cache misses and stale dependency resolution.
