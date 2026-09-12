# Architecture Overview

## 模块职责

limedl 的整体架构描述：工作空间布局、两大目标（Slint 桌面客户端 / NAS WebUI + CLI）、前端传输层（`#invoke` / `#event` import alias → WebSocket）、协议路由（BackendRegistry 按 TaskId 前缀分发）、事件系统（EventBus → 各 adapter 独立订阅发射）。

## 涉及文件

** workspace 层 **：

- `Cargo.toml` — workspace root，members 包括 `crates/*`
- `package.json` — pnpm workspace（frontend only），`packageManager` 字段指定 pnpm v11

** 核心库 **：

- `crates/limedl-core/src/` — 22+ 模块：event_bus、types、protocol、manager、http_executor、scheduler、task_lifecycle、bt_backend/、cdn/、database、buffer_pool、rate_limiter/、checksum/、file_ops/、settings、http_client_factory/、backend_registry、dispatcher、manifest、retry、aria2_rpc、ws_manifest
- lib 名称：`limedl_core`

** 服务端 / CLI **：

- `crates/limedl-server/src/main.rs` — CLI 入口（clap 子命令：daemon | download）+ axum 服务器
- `crates/limedl-server/src/rpc.rs` — WebSocket JSON-RPC 2.0 dispatch + event relay
- `crates/limedl-server/src/auth.rs` — HTTP Basic Auth middleware
- `crates/limedl-server/src/config.rs` — 服务器配置（JSON + CLI 覆写）

** 桌面客户端（Slint）**：

- `crates/limedl-native/src/main.rs` — 进程入口：`bootstrap()` → `Dispatcher`、托盘/窗口/单实例、EventBus 订阅循环
- `crates/limedl-native/src/bridge.rs` — Rust 数据 → Slint 模型的纯函数映射（可单测）
- `crates/limedl-native/src/update.rs` / `autostart.rs` / `migrate.rs` — 自更新、自启、Tauri 数据迁移
- `crates/limedl-core/src/aria2_rpc.rs` — Aria2 RPC 集成（经 `limedl-core` 的 `aria2-rpc` feature；桌面/NAS 均在 bootstrap 后接线）

** 前端 **：

- `src/` — Vue 3 + TypeScript，NAS/桌面 WebUI（浏览器）使用
- `src/lib/ipc/*-api.ts` — 类型安全 invoke 包装（导入 `#invoke`）
- `src/lib/ws/ws-invoke.ts` — WebSocket invoke 实现（JSON-RPC 2.0）
- `src/lib/ws/ws-event.ts` — WebSocket event 实现
- `src/lib/ws/generated/ws-commands.ts` — 由 ws_manifest.rs 自动生成
- `src/lib/ws/generated/ws-events.ts` — 由 ws_manifest.rs 自动生成
- `src/types/generated/types.ts` — 由 ts-rs 自动生成

** 构建配置 **：

- `vite.config.ts` / `vitest.config.ts` — resolve.alias 将 `#invoke` / `#event` 指向 `src/lib/ws/*`
- `.cargo/config.toml` — x86_64 target 设置 `target-cpu=x86-64-v3`

## 数据流向

```
Slint 桌面（limedl-native）:
  Slint UI → main.rs 回调 → Dispatcher → BackendRegistry → DownloadManager / IrontideBtBackend
    → EventBus::publish() → broadcast
    → main.rs 订阅任务 → 更新 Slint 模型 → UI 重绘

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

- 前端传输层：Vue 代码只有一套，通信层通过 import alias 固定指向 `src/lib/ws/*`（WebSocket），不再有编译期双模式。
- 协议路由：BackendRegistry 按 TaskId 前缀（http:/bt:）将操作分派到对应的 DownloadBackend 实现。
- 事件系统：EventBus 是纯 broadcast channel，不感知前端。Slint 桌面在 `limedl-native/src/main.rs` 的订阅任务中更新 UI 模型，WebSocket 推送在 `rpc.rs` 中完成。
- 事件名映射由 ws_manifest.rs 声明，编译期一致性测试验证 rpc.rs 中 handler 分支完整性。
- 序列化约定：Rust struct 用 `#[serde(rename_all = "camelCase")]`，enum 用 `#[serde(rename_all = "snake_case")]`。TypeScript 接口镜像相同命名。
- 所有 crate 使用 Rust edition 2024。
- 认证（NAS 模式）：HTTP Basic Auth on WebSocket upgrade path（/ws），配置通过 config.json 或 CLI flags。
