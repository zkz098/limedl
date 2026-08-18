# Architecture Overview

## 模块职责

limedl 的整体架构描述：工作空间布局、三目标平台（Tauri Desktop / NAS WebUI / CLI）、前端双模式（#invoke / #event import alias 切换 Tauri IPC ↔ WebSocket）、协议路由（BackendRegistry 按 TaskId 前缀分发）、事件系统（EventBus → 各 adapter 独立订阅发射）。

## 涉及文件

** workspace 层 **：

- `Cargo.toml` — workspace root，members 包括 `crates/*` 和 `src-tauri`
- `package.json` — pnpm workspace（frontend only），`packageManager` 字段指定 pnpm v11

** 核心库 **：

- `crates/limedl-core/src/` — 22+ 模块：event_bus、types、protocol、manager、http_executor、scheduler、task_lifecycle、bt_backend/、cdn/、database、buffer_pool、rate_limiter/、checksum/、file_ops/、settings、http_client_factory/、backend_registry、dispatcher、manifest、retry、aria2_rpc、ws_manifest
- lib 名称：`limedl_core`

** 服务端 / CLI **：

- `crates/limedl-server/src/main.rs` — CLI 入口（clap 子命令：daemon | download）+ axum 服务器
- `crates/limedl-server/src/rpc.rs` — WebSocket JSON-RPC 2.0 dispatch + event relay
- `crates/limedl-server/src/auth.rs` — HTTP Basic Auth middleware
- `crates/limedl-server/src/config.rs` — 服务器配置（JSON + CLI 覆写）

** Tauri 桌面 **：

- `src-tauri/src/lib.rs` — Tauri 入口、EventBus→Tauri bridge 后台任务
- `src-tauri/src/download/commands.rs` — Tauri IPC 命令（薄壳，经 Dispatcher 委派）
- `src-tauri/src/download/commands_cdn.rs` — CDN 命令
- `src-tauri/src/download/aria2_rpc.rs` — Aria2 RPC 集成
- lib 名称：`limedl_lib`

** 前端 **：

- `src/` — Vue 3 + TypeScript，跨 Tauri/NAS 共享
- `src/lib/tauri/*-api.ts` — 类型安全 invoke 包装（导入 `#invoke`）
- `src/lib/ws/ws-invoke.ts` — WebSocket invoke 实现（NAS 模式）
- `src/lib/ws/ws-event.ts` — WebSocket event 实现（NAS 模式）
- `src/lib/ws/generated/ws-commands.ts` — 由 ws_manifest.rs 自动生成
- `src/lib/ws/generated/ws-events.ts` — 由 ws_manifest.rs 自动生成
- `src/types/generated/types.ts` — 由 ts-rs 自动生成

** 构建配置 **：

- `vite.config.ts` — resolve.alias 中根据 `mode === "nas"` 切换 `#invoke` / `#event` 指向
- `.cargo/config.toml` — x86_64 target 设置 `target-cpu=x86-64-v3`

## 数据流向

```
Tauri Desktop:
  Vue UI → #invoke → @tauri-apps/api/core → Tauri IPC
    → commands.rs → Dispatcher → BackendRegistry → DownloadManager / IrontideBtBackend
    → EventBus::publish() → broadcast
    → Tauri bridge (lib.rs 后台任务) → app_handle.emit() → Vue UI

NAS WebUI:
  Vue UI → #invoke → ws-invoke.ts → WebSocket JSON-RPC
    → rpc.rs → Dispatcher → BackendRegistry → DownloadManager / IrontideBtBackend
    → EventBus::publish() → broadcast
    → rpc.rs event relay → WebSocket → ws-event.ts → Vue UI

CLI daemon:
  limedl daemon → main.rs → axum 服务器（同 NAS 后端）+ 静态文件服务 + WebSocket RPC

CLI 单次下载:
  limedl download <url> → main.rs → DownloadManager（临时 state dir）→ EventBus → stdout progress
```

## 设计决策与约定

- 前端代码共享：同一套 Vue 3 组件/路由/composable 在 Tauri 和 NAS 上完全复用，仅通信层通过 import alias 切换。
- 协议路由：BackendRegistry 按 TaskId 前缀（http:/bt:）将操作分派到对应的 DownloadBackend 实现。
- 事件系统：EventBus 是纯 broadcast channel，不感知前端。Tauri 前端发射在 lib.rs 中由独立后台任务完成，WebSocket 推送在 rpc.rs 中完成。
- 事件名映射由 ws_manifest.rs 声明，编译期一致性测试验证 rpc.rs 和 lib.rs 中 handler 分支完整性。
- 序列化约定：Rust struct 用 `#[serde(rename_all = "camelCase")]`，enum 用 `#[serde(rename_all = "snake_case")]`。TypeScript 接口镜像相同命名。
- 所有 crate 使用 Rust edition 2024。
- 认证（NAS 模式）：HTTP Basic Auth on WebSocket upgrade path（/ws），配置通过 config.json 或 CLI flags。
