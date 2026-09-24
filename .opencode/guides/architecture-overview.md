# Architecture Overview

## 模块职责

limedl 的整体架构描述：工作空间布局、目标平台（Slint 桌面客户端）、协议路由（BackendRegistry 按 TaskId 前缀分发）、事件系统（EventBus → 桌面客户端与 Aria2 RPC 独立订阅）。

## 涉及文件

** workspace 层 **：

- `Cargo.toml` — workspace root，members 包括 `crates/limedl-core`, `crates/limedl-native`, `xtask`

** 核心库 **：

- `crates/limedl-core/src/` — 核心下载引擎模块：event_bus、types、protocol、manager、http_executor、scheduler、task_lifecycle、bt_backend/、cdn/、database、buffer_pool、rate_limiter/、checksum/、file_ops/、settings、http_client_factory/、backend_registry、dispatcher、manifest、retry、aria2_rpc
- lib 名称：`limedl_core`

** 桌面客户端（Slint）**：

- `crates/limedl-native/src/main.rs` — 进程入口：`bootstrap()` → `Dispatcher`、托盘/窗口/单实例、EventBus 订阅循环
- `crates/limedl-native/src/bridge.rs` — Rust 数据 → Slint 模型的纯函数映射（可单测）
- `crates/limedl-native/src/update.rs` / `autostart.rs` / `migrate.rs` — 自更新、自启、数据迁移
- `crates/limedl-core/src/aria2_rpc.rs` — Aria2 RPC 集成（经 `limedl-core` 的 `aria2-rpc` feature；桌面在 bootstrap 后根据设置启动，用于接收浏览器插件下载拦截）

** 构建与工具 **：

- `xtask/` — 仓库运维工具：更新通道密钥生成、签名与发布防篡改守卫（`cargo xtask`）
- `.cargo/config.toml` — x86_64 target 设置 `target-cpu=x86-64-v3`

## 数据流向

```
Slint 桌面（limedl-native）:
  Slint UI → main.rs 回调 → Dispatcher → BackendRegistry → DownloadManager / IrontideBtBackend
    → EventBus::publish() → broadcast
    → main.rs 订阅任务 → 更新 Slint 模型 → UI 重绘

Aria2 RPC（本地 6800 端口，可选）:
  浏览器插件 → JSON-RPC 2.0 → Aria2RpcServer → Dispatcher → BackendRegistry
    → EventBus::publish() → 桌面端 UI 自动同步
```

## 设计决策与约定

- 架构单轨化：纯 Rust 实现，消除了前端 Web/Node.js 生态。桌面客户端使用 Slint（Skia 渲染），直接在进程内与 `limedl-core` 的 `Dispatcher` 交互，零网络开销与跨语言序列化。
- 协议路由：BackendRegistry 按 TaskId 前缀（`http:` / `bt:`）将操作分派到对应的 DownloadBackend 实现。
- 事件系统：EventBus 是纯 broadcast channel。Slint 桌面在 `limedl-native/src/main.rs` 的订阅任务中更新 UI 模型。
- 序列化约定：Rust struct 用 `#[serde(rename_all = "camelCase")]`，enum 用 `#[serde(rename_all = "snake_case")]`。
- 所有 crate 使用 Rust edition 2024。
