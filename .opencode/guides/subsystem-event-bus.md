# Subsystem: EventBus

## 模块职责

统一的事件发布/订阅总线，取代当前 `broadcast::channel<String>` + `app_handle.emit()` 双通道模式。提供强类型事件发布、Tauri 前端自动转发、多订阅者支持。所有下载子系统通过 EventBus 发布状态变更，消费者通过 subscribe() 接收。

**涉及文件**：
- `src-tauri/src/download/event_bus/mod.rs` — EventBus + DownloadEvent 定义

## 关键结构体

### DownloadEvent (pub(crate))
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub(crate) enum DownloadEvent {
    Updated { id: String, summary_json: Value },
    Progress { id: String, progress_json: Value },
    Aria2Notification { event_name: String, gid: String },
    GameModeChanged { enabled: bool },
    SpeedLimitChanged { limit_bps: u64 },
}
```

### EventBus (pub(crate))
```rust
pub(crate) struct EventBus {
    tx: broadcast::Sender<DownloadEvent>,
    app_handle: Arc<RwLock<Option<tauri::AppHandle>>>,
}
```

## 关键方法

```rust
impl EventBus {
    pub(crate) fn new(capacity: usize) -> Self
    pub(crate) fn set_app_handle(&self, handle: tauri::AppHandle)
    pub(crate) fn publish(&self, event: DownloadEvent)
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<DownloadEvent>
    pub(crate) fn receiver_count(&self) -> usize
}
```

## 数据流向

```
各子系统
  ↓
EventBus::publish(DownloadEvent)
  ├─ 自动转发到 Tauri frontend (via app_handle.emit)
  └─ broadcast 到所有 subscribe() 的消费者
       ├─ Aria2 RPC (WebSocket 推送)
       ├─ BT alert bridge
       └─ 未来的 Webhook / 通知子系统

消费端通过 subscribe() 获取 broadcast::Receiver，异步迭代事件。
```

## 设计决策

- `DownloadEvent` 使用 `serde_json::Value` 作为 payload 而非泛型，避免 EventBus 依赖 `DownloadSummary`/`DownloadProgress` 的具体类型，保持子系统独立性
- `publish()` 同时处理 Tauri 前端发射和内部广播，调用方只需调用一次
- `set_app_handle()` 延迟注入，支持两阶段构造（与现有 `AppState` 模式兼容）
- 使用 `tokio::sync::broadcast` 而非自定义 channel，利用其 lagged-receiver 检测和内存效率
- capacity 参数建议设为 256（与当前 commands.rs 中的设置一致）

**重要约定**：
- EventBus 是可 Clone 的轻量句柄（Arc 内部），各子系统持有自己的克隆
- `subscribe()` 返回的 receiver 只能接收订阅后发布的事件，不回溯历史
- Aria2Notification 变体用于兼容 aria2 RPC 协议的事件名（aria2.onDownloadStart 等）
- 未来扩展新事件类型时，在 DownloadEvent enum 中增加变体，并在 publish() 中处理对应的 Tauri emit
