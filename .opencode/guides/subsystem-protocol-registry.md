# Subsystem: BackendRegistry + DownloadBackend + Dispatcher

## 模块职责

协议抽象层：定义 DownloadBackend trait 作为 HTTP 和 BT 下载的统一接口，通过 BackendRegistry 按 TaskId 前缀路由（http: → DownloadManager、bt: → IrontideBtBackend）。Dispatcher 在此之上提供统一的调度入口和事件发射，消除 Tauri 命令层和 WebSocket RPC 层的重复调度逻辑。

核心类型：DownloadBackend trait（统一 API：start/pause/resume/cancel/remove/purge/open_in_explorer/status/list/update_settings/shutdown）、BackendRegistry（路由表，支持 by_kind、dispatch、get_typed、list_all）、Dispatcher（调度层，封装 registry + 统一 DownloadEvent::Updated emit）。

## 涉及文件

- `crates/limedl-core/src/protocol.rs` — DownloadBackend trait
- `crates/limedl-core/src/backend_registry.rs` — BackendRegistry 路由表
- `crates/limedl-core/src/dispatcher.rs` — Dispatcher 调度层

## 数据流向

```
commands.rs (Tauri IPC) / rpc.rs (WebSocket JSON-RPC)
  └─ 薄壳层：参数解析 + 错误转换
       └─ dispatcher.*()
            └─ registry.dispatch() → backend.*()
            └─ + 统一的 DownloadEvent::Updated emit

各操作的数据流：
  download.start → URL 校验 → dispatcher.start(request)
       → registry.by_kind(kind) → backend.start(request)
       → 边界层可选 emit（start 自身不 emit，避免 BT 双发）

  download.pause / resume / cancel / remove / purge:
       → dispatcher.*(&task_id) → registry.dispatch(&task_id)
       → backend.*(&task_id) → dispatcher 自动 emit Updated

  BT 特有操作（getPeers / getTrackers 等）:
       → dispatcher.bt_*(&task_id) → 内部解构 TaskId::Bt + get_typed
```

## 设计决策与约定

- BackendRegistry 使用 `register()` 注册后端；`dispatch()` 通过 `task_id.kind()` 路由。
- `get_typed::<T>()` 返回原始类型引用，用于访问协议特有方法。
- `list_all()` 合并所有注册后端的列表并排序。
- Dispatcher 对 pause/resume/cancel/remove/purge 操作自动 emit DownloadEvent::Updated，修复了旧代码漏 emit 的 bug。
- start 操作不自动 emit（BT 后端已通过 emit_pending_summary 自动 emit，HTTP 由边界层 emit）。
- status / list 为只读操作，不 emit。
- 各自边界层保留的独有逻辑：mirror URL 填充（commands.rs）、URL 长度/格式校验（rpc.rs）、open_in_explorer、settings.get/save、CDN routes 等不在 Dispatcher 范围内。
