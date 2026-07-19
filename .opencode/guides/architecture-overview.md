# Architecture Overview

## Workspace structure

```
limedl/
├── Cargo.toml                 # workspace root [members: crates/*, src-tauri]
├── package.json               # pnpm workspace (frontend only)
├── src/                       # Vue 3 frontend (shared)
├── crates/
│   ├── limedl-core/         # Pure download engine
│   │   └── src/               # 22 modules: event_bus, types, protocol, manager, bt_backend_own, ...
│   └── limedl-server/       # NAS/headless daemon + CLI
│       └── src/
│           ├── main.rs        # CLI entry (clap: daemon | download)
│           ├── rpc.rs         # WebSocket JSON-RPC 2.0 dispatch
│           ├── auth.rs        # HTTP Basic Auth middleware
│           └── config.rs      # Server config (JSON + CLI override)
└── src-tauri/                 # Tauri v2 desktop app
    └── src/
        ├── lib.rs             # Tauri app entry, EventBus→Tauri bridge
        └── download/
            ├── mod.rs         # Re-exports from limedl-core
            ├── commands.rs    # Tauri IPC commands (thin dispatch)
            ├── commands_cdn.rs
            └── aria2_rpc.rs
```

## Data flow

### Tauri Desktop

```
Vue UI → #invoke → @tauri-apps/api/core → Tauri IPC
  → commands.rs → BackendRegistry → DownloadManager / IrontideBtBackend
  → EventBus.publish() → broadcast::channel
  → Tauri bridge (lib.rs) → app_handle.emit() → Vue UI
```

### NAS WebUI

```
Vue UI → #invoke → ws-invoke.ts → WebSocket JSON-RPC
  → rpc.rs → BackendRegistry → DownloadManager / IrontideBtBackend
  → EventBus.publish() → broadcast::channel
  → rpc.rs event relay → WebSocket → ws-event.ts → Vue UI
```

### CLI daemon

```
limedl daemon → main.rs
  → axum HTTP server (same as NAS WebUI backend)
  → serves Vue dist/ as static files
  → WebSocket RPC on /ws
```

### CLI single download

```
limedl download <url> → main.rs
  → DownloadManager (temp state dir) → HTTP GET → file
  → EventBus.subscribe() → stdout progress
```

## Key traits

### DownloadBackend (crates/limedl-core/src/protocol.rs)

```rust
#[async_trait]
pub trait DownloadBackend: Send + Sync {
    async fn start(&self, request: StartDownloadRequest) -> Result<String>;
    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()>;
    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn list(&self) -> Result<Vec<DownloadSummary>>;
    async fn update_settings(&self, settings: &AppSettings) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}
```

Implemented by `DownloadManager` (HTTP) and `IrontideBtBackend` (BitTorrent).

### BackendRegistry (crates/limedl-core/src/backend_registry.rs)

Routes operations by TaskId prefix:
- `dispatch(&TaskId)` → returns `&dyn DownloadBackend`
- `by_kind(TaskKind)` → returns `&dyn DownloadBackend`
- `get_typed::<T>()` → returns `Option<&T>` for protocol-specific methods
- `list_all()` → merged + sorted list from all backends

## RPC protocol

NAS WebUI uses **WebSocket + JSON-RPC 2.0**:

Request: `{"jsonrpc":"2.0","id":1,"method":"download.start","params":{...}}`
Response: `{"jsonrpc":"2.0","id":1,"result":{...}}`
Server push: `{"jsonrpc":"2.0","method":"event","params":{"type":"updated","payload":{...}}}`

Method names use dot.case (`download.start`, `settings.get`). The `ws-invoke.ts` client maps Tauri snake_case commands to dot.case methods via `METHOD_MAP`.

## Authentication (NAS only)

HTTP Basic Auth on WebSocket upgrade path (`/ws`). Credentials configured via `config.json` or CLI flags (`--user`/`--pass`). No auth = all requests pass through.

## Build targets

| Command | Target | Output |
|---------|--------|--------|
| `pnpm run build` | Tauri desktop | `src-tauri/target/` |
| `pnpm run build:nas` | NAS WebUI | `dist/` (copy to server) |
| `pnpm run tauri dev` | Tauri dev | hot-reload |
| `cargo build -p limedl-server` | CLI binary | `target/debug/limedl.exe` |

## Module index

| Module | Crate | Source |
|--------|-------|--------|
| event_bus | core | `crates/limedl-core/src/event_bus/mod.rs` |
| types | core | `crates/limedl-core/src/types.rs` |
| protocol (DownloadBackend) | core | `crates/limedl-core/src/protocol.rs` |
| backend_registry | core | `crates/limedl-core/src/backend_registry.rs` |
| manager (DownloadManager) | core | `crates/limedl-core/src/manager.rs` |
| bt_backend_own (IrontideBtBackend) | core | `crates/limedl-core/src/bt_backend_own/` |
| cdn (CdnAccelerator) | core | `crates/limedl-core/src/cdn/` |
| database | core | `crates/limedl-core/src/database.rs` |
| buffer_pool | core | `crates/limedl-core/src/buffer_pool.rs` |
| scheduler + aimd | core | `crates/limedl-core/src/scheduler.rs` + `aimd.rs` |
| rate_limiter | core | `crates/limedl-core/src/rate_limiter/` |
| checksum | core | `crates/limedl-core/src/checksum/` |
| file_ops | core | `crates/limedl-core/src/file_ops/` |
| settings | core | `crates/limedl-core/src/settings.rs` |
| Tauri commands | tauri | `src-tauri/src/download/commands.rs` |
| Aria2 RPC | tauri | `src-tauri/src/download/aria2_rpc.rs` |
| NAS server | server | `crates/limedl-server/src/main.rs` + `rpc.rs` |
| WebSocket client | frontend | `src/lib/ws/ws-invoke.ts` + `ws-event.ts` |
