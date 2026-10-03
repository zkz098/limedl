# Subsystem: EventBus

## 模块职责

统一的事件发布/订阅总线。纯 `tokio::sync::broadcast` 封装，所有下载子系统通过它发布状态变更。EventBus 自身不持有任何 UI 句柄，也不负责 UI 发射：桌面（Slint）客户端在 `crates/limedl-native/src/event_stream/bus.rs` 里用独立订阅任务更新 UI 模型；Aria2 RPC 的 WebSocket adapter（`crates/limedl-core/src/aria2_rpc/transport.rs`）订阅同一个总线，只挑 `Aria2Notification` 推给 aria2 客户端。

核心类型：EventBus（仅含 `broadcast::Sender<DownloadEvent>` 一个字段）、DownloadEvent（6 个 variant：Updated / Progress / Aria2Notification / CdnProgress / CdnComplete / Warning）。EventBus 可 Clone（Arc 内部的 Sender 句柄）。

## 涉及文件

- `crates/limedl-core/src/event_bus/mod.rs` — EventBus 结构体 + DownloadEvent 枚举定义
- `crates/limedl-core/src/aria2_rpc/transport.rs` — aria2 WebSocket adapter：只转发 `Aria2Notification`
- `crates/limedl-native/src/event_stream/bus.rs` — 桌面 adapter：订阅任务 → 更新 Slint 模型

## 数据流向

```
各子系统（DownloadManager / HttpExecutor / IrontideBtBackend / CdnService）
  ↓
EventBus::publish(event) → broadcast::Sender::send()
  ↓
                          ┌─ Slint desktop subscriber (native/event_stream/bus.rs):
                          │     match event → 更新列表/详情模型并标记 UI 重绘
                          │
                          └─ aria2 WebSocket subscriber (aria2_rpc/transport.rs):
                               只处理 Aria2Notification → 转发给 AriaNg / Motrix
```

## 设计决策与约定

- `publish()` 只做 `tx.send(event)`，**不做**任何前端转发。前端发射由各 adapter 的独立订阅任务负责。
- **负载是强类型**：`Updated { summary: DownloadSummary }` / `Progress { progress: DownloadProgress }`。EventBus 不再承载 JSON 线协议——需要 JSON 的边界（aria2 转换、SQLite `summary_json` 列）各自在边界处 `serde_json::to_value`。不要新增 `serde_json::Value` 负载；旧代码里那种「只带一两个字段的补丁 JSON」消费端无法反序列化，会被静默丢弃。
- 生产环境容量 8192（`bootstrap.rs` 中创建），单元测试内部单独使用 1024、UI 测试使用 64。
- **lag 恢复在订阅端**：订阅者消费落后时 `recv()` 返回 `RecvError::Lagged(n)`。`while let Ok(...)` 会让任务永久退出——必须显式 match。桌面订阅端在 Lagged 时调用 `Dispatcher::list()` 并用结果 `replace_all`（见 `native/event_stream/bus.rs`）；aria2 订阅端只需要 continue（它只关心后续通知）。没有任何事件承载"全量快照"，恢复由 adapter 主动拉取。
- 新增事件 variant：在 DownloadEvent 中加新变体，并在**每个** adapter 的 match 分支添加对应逻辑（桌面 + aria2）。没有编译期穷尽性测试，新增 variant 后请全仓 `rg 'DownloadEvent::'` 检查遗漏。
- Progress 事件在发送端（http_executor）有 500ms 节流——周期性 persist 路径每 500ms 最多发一次 progress；终态（completed/failed/canceled）立即发送，不节流。
