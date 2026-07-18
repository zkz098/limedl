# Subsystem: BackendRegistry + DownloadBackend

## 模块职责

协议抽象层：定义 `DownloadBackend` trait 作为 HTTP 和 BT 下载的统一接口，通过 `BackendRegistry` 消除 `commands.rs` 中的硬编码协议路由。提供协议无关的 `start/pause/resume/cancel/remove/purge/open/status/list` 操作。

**涉及文件**：

- `crates/flareget-core/src/protocol.rs` — DownloadBackend trait + HTTP 适配器
- `crates/flareget-core/src/backend_registry.rs` — BackendRegistry 路由表

## 关键结构体

### DownloadBackend trait (pub)

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

### BackendRegistry (pub)

```rust
pub struct BackendRegistry {
    http: Arc<dyn DownloadBackend>,
    bt: Option<Arc<dyn DownloadBackend>>,
}
```

## 关键方法

```rust
impl BackendRegistry {
    pub fn new() -> Self
    pub fn register(&mut self, kind: TaskKind, backend: Arc<dyn DownloadBackend>)
    pub fn dispatch(&self, task_id: &TaskId) -> &dyn DownloadBackend
    pub fn by_kind(&self, kind: TaskKind) -> &dyn DownloadBackend
    pub fn get_typed<T: 'static>(&self) -> Option<&T>
    pub fn list_all(&self) -> Vec<DownloadSummary>
    pub fn iter(&self) -> BackendIterator<'_>
}
```

## 数据流向

```
commands.rs 接收 Tauri IPC / rpc.rs 接收 WebSocket JSON-RPC
  ↓
├─ download.start: classify_kind(url) → TaskKind
│    └─ registry.by_kind(kind) → &dyn DownloadBackend
│         └─ backend.start(request) → 返回 prefixed ID
│
└─ 其他命令: TaskId::parse(download_id)
     └─ registry.dispatch(&task_id) → &dyn DownloadBackend
          ├─ HTTP → DownloadBackend impl (manager.rs, TaskId::http_inner)
          └─ BT   → DownloadBackend impl (bt_backend_own/mod.rs)
```

**重要约定**：

- BackendRegistry 使用 `register()` 注册协议；`dispatch()` 通过 TaskId 前缀路由
- `start()` 方法返回的 ID 已包含协议前缀（`"http:"` 或 `"bt:"`），调用方无需再添加
- `get_typed::<T>()` 返回原始类型引用，用于访问协议特有方法（如 `set_speed_limit`）
- `list_all()` 合并所有注册后端的列表并排序
- Tauri 和 WebSocket RPC 共享同一个 BackendRegistry
