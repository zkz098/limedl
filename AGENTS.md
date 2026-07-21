# AGENTS.md — limedl

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
- **Test (Rust - core only)**: `cargo test --manifest-path crates/limedl-core/Cargo.toml`
- **Test (Rust - Tauri)**: `cargo test --manifest-path src-tauri/Cargo.toml --features test-utils`
- **Build order**: `vue-tsc --noEmit` then `vite build` (enforced by `pnpm run build`)

## Architecture

### Workspace layout (3 crates)

```
limedl/
├── crates/
│   ├── limedl-core/       # Pure download engine (zero UI deps)
│   └── limedl-server/     # axum HTTP/WS server + CLI binary
├── src-tauri/               # Tauri v2 desktop app (thin shell)
├── src/                     # Vue 3 frontend (shared across targets)
└── Cargo.toml               # workspace [members]
```

- **`limedl-core`**: Download engine — manager, scheduler, buffer pool, CDN, BT backend, settings, database, event bus, checksum, rate limiter. All modules live in `crates/limedl-core/src/`.
- **`limedl-server`**: NAS/headless daemon. Axum HTTP server + WebSocket JSON-RPC 2.0 + CLI (`daemon` / `download` subcommands) + HTTP Basic Auth + static file serving. Entry: `crates/limedl-server/src/main.rs`.
- **`src-tauri`**: Tauri v2 desktop app. Thin commands layer (`commands.rs`, `commands_cdn.rs`, `aria2_rpc.rs`) that dispatches to `limedl_core::BackendRegistry`. Entry: `src-tauri/src/lib.rs`.

### Multi-platform support

| Target | Frontend | Backend | Build |
|--------|----------|---------|-------|
| Tauri Desktop | Vue 3 via Tauri IPC (`#invoke` → `@tauri-apps/api/core`) | `src-tauri/` | `pnpm run tauri dev` / `pnpm run tauri build` |
| NAS WebUI | Same Vue 3 via WebSocket (`#invoke` → `ws-invoke.ts`) | `limedl-server` | `pnpm run build:nas` |
| CLI | N/A | `limedl-server` | `limedl daemon` / `limedl download <url>` |

### Frontend dual-mode (`#invoke` / `#event`)

The Vue frontend uses import aliases so the same code runs on both Tauri IPC and WebSocket:

- **Tauri mode**: `#invoke` → `@tauri-apps/api/core`, `#event` → `@tauri-apps/api/event`
- **NAS mode**: `#invoke` → `src/lib/ws/ws-invoke.ts`, `#event` → `src/lib/ws/ws-event.ts`

Switched via `vite.config.ts` resolve.alias based on `mode === "nas"`.

**Never call `invoke` directly** — use the typed wrappers in `src/lib/tauri/*-api.ts`. These import from `#invoke`.

### Event system (EventBus)

`EventBus` (`crates/limedl-core/src/event_bus/mod.rs`) is a pure `tokio::sync::broadcast::channel<DownloadEvent>` with zero UI dependency. Each adapter subscribes independently:

- **Tauri**: `src-tauri/src/lib.rs` spawns a background task that subscribes to EventBus and calls `app_handle.emit()` for each event type
- **NAS WebSocket**: `rpc.rs` spawns a per-connection task that subscribes to EventBus and relays events over WebSocket
- **Aria2 RPC**: subscribed directly via `event_bus.subscribe()`

### Protocol routing (BackendRegistry + DownloadBackend)

`DownloadBackend` trait (`crates/limedl-core/src/protocol.rs`) defines the unified API for all download protocols. `BackendRegistry` (`crates/limedl-core/src/backend_registry.rs`) routes by `TaskId` prefix:

- `http:` prefix → `DownloadManager` (HTTP downloads)
- `bt:` prefix → `IrontideBtBackend` (BitTorrent)

Tauri commands and WebSocket RPC both dispatch through the same registry.

### Startup & shutdown

**Tauri** (`src-tauri/src/lib.rs`):
state dirs → RateLimiter → EventBus → DownloadManager → IrontideBtBackend → CdnService → BackendRegistry → optional Aria2 RPC → app.manage(AppState)

**NAS daemon** (`crates/limedl-server/src/main.rs`):
config load → state dirs → RateLimiter → EventBus → DownloadManager → CdnService → BackendRegistry → axum router (WebSocket RPC + static files) → serve

### Rust edition & lib names

- Edition: 2024 (all crates)
- `limedl-core` lib name: `limedl_core`
- `src-tauri` lib name: `limedl_lib` (suffixed `_lib` to avoid Windows name collision)

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

Six-job matrix across three platforms:

| Job | OS | Key steps |
|-----|----|-----------|
| **lint-typescript** | ubuntu-latest | `pnpm install --frozen-lockfile` → `pnpm run lint` → `vue-tsc --noEmit` → `pnpm run test` |
| **check-windows** | windows-latest | `cargo clippy --workspace -- -D warnings` → 3× per-crate `cargo test` (core + server + src-tauri) |
| **check-macos** | macos-14 | `cargo clippy --workspace -- -D warnings` → 3× per-crate `cargo test` (core + server + src-tauri) |
| **check-rust** | ubuntu-latest | `cargo clippy --workspace -- -D warnings` → ts-rs bindings freshness check (see below) → 3× per-crate `cargo test` |
| **bench-rust** | ubuntu-latest | `cargo bench` for `aimd` + `rate_limiter` benchmarks, with baseline comparison on `push` |
| **supply-chain** | ubuntu-latest | `cargo deny check` (bans + licenses + sources) + `cargo audit` |

Key constraints:

- **clippy denies all warnings** (`-- -D warnings`) across all Rust jobs.
- **ts-rs bindings freshness**: On `check-rust`, `cargo test --features ts export_typescript_bindings` regenerates `.ts` files, then `git diff --exit-code src/types/generated/ src/lib/ws/generated/` fails the job if generated files are out of sync with Rust structs/WS_COMMANDS. See [Frontend TS type generation](#frontend-ts-type-generation-ts-rs) and [WebSocket manifest 代码生成](#websocket-manifest-代码生成).
- **Per-crate test** (not `--workspace`): core + server + Tauri each run separately with appropriate features (`test-utils,aria2-rpc` for core, no extra features for server, `test-utils` for Tauri).
- **Windows build**: Uses `lld-link` linker for faster linking; ComCtl32 manifest is embedded via `build.rs` (harmless `LNK4078` warning — see [Known warnings](#known-warnings)).
- **macOS**: No system deps needed (WebKit bundled with OS).

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

## Frontend TS type generation (ts-rs)

> **After adding/modifying any Rust struct/enum that is serialized to the frontend**, regenerate TypeScript bindings:

```powershell
# Initialize MSVC env first (Windows)
& "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
# Regenerate
$env:TS_RS_EXPORT_DIR="."
cargo test --manifest-path crates/limedl-core/Cargo.toml --features ts -- export_typescript_bindings
```

This writes type definitions to `src/types/generated/types.ts`. The generated file must be committed alongside Rust changes.

**Architecture:**
- `crates/limedl-core/` has `ts-rs` as an **optional** dependency behind the `ts` feature (`ts-rs = { version = "12", optional = true }`; ts-rs v12 内置 serde-compat，Cargo.toml 中只需指定 version + optional = true，无需额外 feature 声明).
- Types annotated with `#[cfg_attr(feature = "ts", derive(TS))]` and `#[cfg_attr(feature = "ts", ts(export, export_to = "..."))]` auto-export when compiled with `--features ts`.
- The export test in `crates/limedl-core/src/tests/ts_export.rs` triggers export via `export_all()` calls.
- The frontend files `src/types/settings.ts`, `src/types/download.ts`, and `src/types/cdn.ts` re-export generated types and add only pure-frontend types not present in Rust.

**Rules:**
- Never manually edit TypeScript types that match Rust serialized structs — edit the Rust source and regenerate.
- After regenerating, run `git diff --stat src/types/generated/types.ts` to verify.
- The CI's `check-rust` job (`.github/workflows/ci.yml`) should run the following step after `cargo clippy`:
  ```yaml
  - name: Check ts-rs bindings are up-to-date
    run: |
      cargo test --manifest-path crates/limedl-core/Cargo.toml --features ts export_typescript_bindings
      git diff --exit-code src/types/generated/ src/lib/ws/generated/
  ```
  This ensures generated `.ts` files are committed alongside Rust changes. CI fails if the generated files are out of sync.

## WebSocket manifest 代码生成

> `src/lib/ws/ws-invoke.ts` 中的 `METHOD_MAP`（命令名映射）和 `applyTransform`（参数转换）不再手工维护。
> 所有 WebSocket JSON-RPC 命令的注册信息统一声明在 Rust 端，构建时自动生成 TypeScript 文件。

**Source of truth**: `crates/limedl-core/src/ws_manifest.rs`

- [`WsCommandSpec`] 结构体定义：`tauri_name`（snake_case 命令名）、`rpc_method`（JSON-RPC method）、`param_transform`（参数变换方式）
- [`ParamTransform`] enum：`Identity`（透传）、`Rename`（重命名单字段）、`UnwrapField`（展开嵌套对象）
- [`WS_COMMANDS`] 常量数组列出全部命令——当前 32 条

**生成时机**：`cargo test --features ts export_typescript_bindings`

输出文件：`src/lib/ws/generated/ws-commands.ts`
- `WsCommandSpec` TypeScript interface
- `WS_COMMANDS` 常量数组
- `METHOD_MAP` 便利查询表

**添加新 WS 命令的步骤**：

1. 在 `crates/limedl-core/src/ws_manifest.rs` 的 `WS_COMMANDS` 数组中添加一个 `WsCommandSpec` 条目
2. 在 `crates/limedl-server/src/rpc.rs` 的 `dispatch_method` 中添加对应 handler 分支（并且如果命令属于分组 dispatch（如 download action、BT details、CDN routes），还需要在对应 sub-handler 中添加分支）
3. 运行 `cargo test --features ts export_typescript_bindings` 自动生成 `src/lib/ws/generated/ws-commands.ts`
4. 如果新命令需要特殊的参数转换（非 Identity），在 `ParamTransform` enum 中添加变体并在 `applyTransform` 函数（`src/lib/ws/ws-invoke.ts`）中处理新 kind
5. 提交 ws_manifest.rs、rpc.rs 和生成的 ws-commands.ts

> ⚠️ **rpc.rs 一致性警告**：`crates/limedl-core/src/ws_manifest.rs` 中的一致性测试 `all_rpc_methods_have_dispatch_arms` 会在编译期读取 rpc.rs 源码并验证每个 `tauri_name` 字符串都出现在 dispatch handler 中。如果忘记更新 rpc.rs，该测试会 fail。

**不需要做的**：
- 不再手工编辑 `METHOD_MAP`（已从 ws-invoke.ts 删除）
- 不再手工添加 `transformParams` case 分支（已替换为通用 `applyTransform`）

**CI 校验**：`git diff --exit-code src/types/generated/ src/lib/ws/generated/` 确保生成文件与 Rust 源同步。

## WebSocket event manifest 代码生成

> `src/lib/ws/ws-invoke.ts` 中的 `mapEventType`（事件名映射）不再手工维护。
> 所有 WebSocket JSON-RPC notification 事件名的映射统一声明在 Rust 端，构建时自动生成 TypeScript 文件。

**Source of truth**: `crates/limedl-core/src/ws_manifest.rs`（与命令 manifest 同一文件）

- [`WsEventSpec`] 结构体定义：`ws_type`（RPC notification 的 type 字段值）、`tauri_event_name`（Tauri 前端事件名）
- [`WS_EVENTS`] 常量数组列出全部事件映射——当前 6 条（覆盖 6 个 `DownloadEvent` variant：
  - `updated` ↔ `download-updated`
  - `progress` ↔ `download-progress`
  - `aria2Notification` ↔ `aria2-notification`
  - `cdnProgress` ↔ `cdn-test-progress`
  - `cdnComplete` ↔ `cdn-test-complete`
  - `warning` ↔ `download-warning`）

**生成时机**：`cargo test --features ts export_typescript_bindings`

输出文件：`src/lib/ws/generated/ws-events.ts`
- `WsEventSpec` TypeScript interface
- `WS_EVENTS` 常量数组
- `EVENT_TYPE_MAP` 便利查询表（`ws_type` → `tauri_event_name`）

**添加新 DownloadEvent variant 的步骤**：

1. 在 `crates/limedl-core/src/event_bus/mod.rs` 的 `DownloadEvent` enum 中添加新 variant
2. 在 `crates/limedl-core/src/ws_manifest.rs` 的 `WS_EVENTS` 数组中添加一个 `WsEventSpec` 条目
3. 在 `src-tauri/src/lib.rs` 的 Tauri adapter event relay match 中添加对应 emit 分支
4. 在 `crates/limedl-server/src/rpc.rs` 的 RPC adapter event relay match 中添加对应 notification handler 分支
5. 运行 `cargo test --features ts export_typescript_bindings` 自动重新生成 `src/lib/ws/generated/ws-events.ts`
6. 提交 event_bus/mod.rs、ws_manifest.rs、lib.rs、rpc.rs 和生成的 ws-events.ts

> ⚠️ **一致性警告**：`ws_manifest.rs` 中的一致性测试 `ws_event_types_appear_in_rpc_adapter` 和 `ws_event_tauri_names_appear_in_lib_rs` 会在编译期读取 rpc.rs 和 lib.rs 源码，验证每个 `ws_type` 和 `tauri_event_name` 字符串分别出现在对应文件的 event relay handler 中。如果忘记更新任意一端，对应测试会 fail。
>
> 例外：`aria2Notification` 的 Tauri adapter 使用动态 event_name（直接透传 BT 后端的原始事件名），不在 lib.rs 中检查固定字符串。

**不需要做的**：
- 不再手工编辑 `mapEventType` 的 switch case（已替换为通用 `EVENT_TYPE_MAP` 查表）

**CI 校验**：`git diff --exit-code src/types/generated/ src/lib/ws/generated/` 已覆盖 ws-events.ts，无需额外配置。

## Known warnings

### `LNK4078` on Windows release builds

```
resource.lib : warning LNK4078: found multiple ".rsrc" sections with different attributes (40000040)
```

**Harmless.** Caused by `build.rs` manually embedding a ComCtl32 v6 manifest for the binary target, while `tauri_build::build()` also embeds one via `tauri-winres`. The manual embedding is intentional — it ensures the manifest is present in test binaries (`cargo test --workspace`), not just the release binary. Do not remove the custom `build.rs` manifest code.

### `cargo audit` reports quick-xml RUSTSEC-2026-0194 / RUSTSEC-2026-0195

`cargo audit` will always report 2 high-severity advisories against `quick-xml 0.39.4`:

- **RUSTSEC-2026-0194** — `BytesStart::attributes()` O(N²) duplicate-name check → CPU DoS
- **RUSTSEC-2026-0195** — `NsReader` unbounded namespace-declaration allocation → OOM

**Status: known, accepted, no remediation possible from limedl side.**

- quick-xml enters as a **build-time** dependency of `wayland-scanner 0.31.10` (proc-macro parsing host-preinstalled Wayland protocol XML at compile time). `grep` confirms limedl source has zero `quick_xml` / `BytesStart::attributes` / `NsReader` imports — the runtime never invokes the affected API surface.
- wayland-scanner 0.31.10 declares `quick-xml = "^0.39"` in its `Cargo.toml`, so `cargo update` cannot lift the version across the 0.39 → 0.41 major bump. crates.io has no newer `wayland-scanner` / `wl-clipboard-rs` / `arboard` / `tauri-plugin-clipboard-manager` release on this branch.
- Attack path described by the advisories (attacker-controlled XML reaching `NsReader` / `attributes()`) does not apply — limedl's build parses only trusted platform Wayland protocol DTDs, no external XML input.
- `deny.toml` explicitly `ignore`s both advisory IDs with rationale. `cargo deny check` therefore passes; `cargo audit` does not read `deny.toml`'s `ignore` list and will always surface these warnings — treat the audit-side noise as expected.
- **Revisit when** the wayland-rs repo publishes a release that bumps its quick-xml dependency (likely `wayland-scanner 0.31.11` or `0.32.0`), or when `tauri-plugin-clipboard-manager` / `arboard` / `wl-clipboard-rs` publish a release that picks it up transitively. At that point remove both IDs from `deny.toml [advisories].ignore` and re-run `cargo update`.

### `cargo deny` `multiple-versions` warning for `winreg`

`bans ok` is reported with one `warning[duplicate]`: `winreg 0.10.1` (via `auto-launch` ← `tauri-plugin-autostart`) and `winreg 0.55.0` (via `embed-resource` ← `tauri-winres` ← `tauri-build`). Two `winreg` major versions are incompatible at API level but do not coexist in the same module path at runtime, so there is no functional conflict. Supplied upstream via Tauri's plugin / build stack — only an upstream release can collapse the split. Don't add `skip = ["winreg"]` to `deny.toml` to silence it; the warning is informative.

## Local pre-push verification (required)

CI runs `cargo clippy --workspace -- -D warnings` (without `--all-targets`), which only lints the default build targets. Several test-mode clippy warnings (`len_without_is_empty`, `identity_op`, `duplicate_mod`, `needless_borrows_for_generic_args`, `to_string_in_format_args`) only surface under `--all-targets`. Before pushing, run the **stricter local trio** and ensure RC=0 — CI cannot catch regressions here without `--all-targets`:

```powershell
& "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64 | Out-Null
cargo clippy --manifest-path crates/limedl-core/Cargo.toml --all-targets --features test-utils,aria2-rpc -- -D warnings
cargo clippy --manifest-path crates/limedl-server/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features test-utils -- -D warnings
```

Supply-chain must also be clean locally (CI's `-- -W rejected` does not turn rejections into warnings for the local developer):

```powershell
cargo deny check                                                    # expects: advisories ok, bans ok, licenses ok, sources ok
cargo audit                                                         # expected: 2 vulns + 18 unmaintained warnings — see "quick-xml" section above
```

`cargo audit` non-zero RC caused by the quick-xml advisories alone is **acceptable** — both IDs are reviewed and `cargo deny check` (the gate that CI uses) passes.
