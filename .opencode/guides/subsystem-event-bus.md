# Subsystem: EventBus

## 模块职责

统一的事件发布/订阅总线。纯 `tokio::sync::broadcast` 封装，所有下载子系统通过它发布状态变更。EventBus 自身不持有 Tauri AppHandle，不负责前端发射——Tauri 前端的 `app_handle.emit()` 调用由 `src-tauri/src/lib.rs` 中一个独立的后台订阅任务完成。WebSocket 推送则由 `crates/limedl-server/src/rpc.rs` 中另一个独立订阅任务完成。

核心类型：EventBus（仅含 `broadcast::Sender<DownloadEvent>` 一个字段）、DownloadEvent（6 个 variant：Updated / Progress / Aria2Notification / CdnProgress / CdnComplete / Warning）。EventBus 可 Clone（Arc 内部的 Sender 句柄）。

## 涉及文件

- `crates/limedl-core/src/event_bus/mod.rs` — EventBus 结构体 + DownloadEvent 枚举定义
- `src-tauri/src/lib.rs` — Tauri adapter：后台任务 subscribe → `app_handle.emit()`（约 110–180 行区域）
- `crates/limedl-server/src/rpc.rs` — WebSocket adapter：后台任务 subscribe → 通过 WebSocket 推送

## 数据流向

```
各子系统（DownloadManager / HttpExecutor / IrontideBtBackend / CdnService）
  ↓
EventBus::publish(event) → broadcast::Sender::send()
  ↓
                          ┌─ Tauri subscriber (lib.rs):
                          │     match event → app_handle.emit("download-updated"/"download-progress"/"cdn-test-*"/"download-warning")
                          │
                          ├─ WebSocket subscriber (rpc.rs):
                          │     match event → 通过 WebSocket 向已连接客户端推送
                          │
                          └─ 其他订阅者（aria2 RPC 等）
```

## 设计决策与约定

- `publish()` 只做 `tx.send(event)`，**不做**任何前端转发。前端发射由各 adapter 的独立订阅任务负责。
- 生产环境容量 8192（`bootstrap.rs` 中创建），单元测试内部单独使用 1024。
- broadcast 的 lagged-receiver 检测：当订阅者消费速度落后于发布速度时，`recv()` 返回 `RecvError::Lagged(n)`。lib.rs 中处理此情况时会调用 `emit_all_downloads()` 做兜底全量推送。
- DownloadEvent 使用 `#[serde(tag = "type", content = "payload", rename_all = "camelCase")]`，序列化为 `{"type":"updated","payload":{...}}` 格式。
- 新增事件 variant：在 DownloadEvent 中加新变体，然后分别在 lib.rs 和 rpc.rs 的 match 分支中添加对应 emit/推送逻辑。一致性由 ws_manifest.rs 中的编译期测试保证。
- Progress 事件在发送端（http_executor）有 500ms 节流——周期性 persist 路径每 500ms 最多发一次 progress；终态（completed/failed/canceled）立即发送，不节流。
