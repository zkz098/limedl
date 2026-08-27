# Subsystem: BackendRegistry + DownloadBackend + Dispatcher + SystemContext

## 模块职责

协议抽象层：定义 DownloadBackend trait 作为 HTTP 和 BT 下载的统一接口，通过 BackendRegistry 按 TaskId 前缀路由（http: → DownloadManager、bt: → IrontideBtBackend）。

Dispatcher 作为统一调度门面（Unified Facade），封装了 BackendRegistry、SystemContext（SettingsService, ConcurrencyManager, DiskIoService）、CdnService 和 EventBus。边界适配层（Tauri IPC 和 WebSocket RPC）完全通过 Dispatcher 交互，彻底消除了跨协议下溯类型转换（get_typed Downcast）与边界层重复逻辑。

核心类型：

- `DownloadBackend` trait（统一 API：start/pause/resume/cancel/remove/purge/open_in_explorer/status/list/update_settings/shutdown）
- `BackendRegistry`（路由表，支持 register_arc、dispatch、list_all、shutdown_all）
- `Dispatcher`（统一调度层，聚合生命周期、设置、磁盘IO、超频与BT专用操作）
- `SystemContext`（全局系统基础容器，统一持有 Database, EventBus, RateLimiter, BufferPool, ConcurrencyManager, SettingsService, DiskIoService）

## 涉及文件

- `crates/limedl-core/src/protocol.rs` — DownloadBackend trait
- `crates/limedl-core/src/backend_registry.rs` — BackendRegistry 路由表
- `crates/limedl-core/src/dispatcher.rs` — Dispatcher 统一调度层
- `crates/limedl-core/src/context.rs` — SystemContext 基础容器
- `crates/limedl-core/src/services/` — ConcurrencyManager, SettingsService, DiskIoService

## 数据流向

```
commands.rs (Tauri IPC) / rpc.rs (WebSocket JSON-RPC)
  └─ 薄壳层：参数反序列化 + 错误转换
       └─ dispatcher.*()
            ├─ 生命周期操作 (start / pause / resume / cancel / remove / purge / status / list)
            │    └─ registry.dispatch() → backend.*()
            │    └─ 统一 DownloadEvent::Updated 自动发射
            ├─ 系统设置操作 (get_settings / save_settings / factory_reset)
            │    └─ settings_service.update() + registry.update_all_settings() + cdn_service 同步
            ├─ 磁盘与并发操作 (detect_disk_type / get_io_status / toggle_overclock_mode)
            │    └─ disk_io.*() / concurrency.*()
            └─ BT 协议专用操作 (bt_get_peers / bt_get_trackers / bt_preview_torrent 等)
                 └─ registry.get_typed::<IrontideBtBackend>()
```

## 设计决策与约定

- 边界层零 Downcast 约定：Tauri `commands.rs` 和 Server `rpc.rs` 严禁调用 `get_typed::<DownloadManager>()`，所有操作全部经由 `state.dispatcher` 统一分发。
- 单一真实源（SSOT）：设置读取与变更统一由 `SettingsService` 维护并持久化，消除了各后端与 UI 状态的分裂与漂移。
- 托盘与多协议聚合：托盘状态检查使用 `dispatcher.has_active_downloads()` 跨 HTTP/BT 全局聚合，避免遗漏 BT 任务。
- 自动事件发射：Dispatcher 对 pause/resume/cancel/remove/purge 以及 start 后状态自动触发事件，确保前后端实时同步。
