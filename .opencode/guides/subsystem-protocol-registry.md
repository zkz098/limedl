# Subsystem: BackendRegistry + DownloadBackend

## 模块职责

协议抽象层：定义 `DownloadBackend` trait 作为 HTTP 和 BT 下载的统一接口，通过 `BackendRegistry` 消除 `commands.rs` 中的硬编码协议路由。提供协议无关的 `start/pause/resume/cancel/remove/purge/open/status/list` 操作。

**涉及文件**：

- `crates/limedl-core/src/protocol.rs` — DownloadBackend trait + HTTP 适配器
- `crates/limedl-core/src/backend_registry.rs` — BackendRegistry 路由表

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
}
```

### BackendRegistry (pub)

```rust
pub struct BackendRegistry {
    by_kind: HashMap<TaskKind, Arc<dyn DownloadBackend>>,
    by_type: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    all: Vec<(TaskKind, Arc<dyn DownloadBackend>)>,
}
```

## 关键方法

```rust
impl BackendRegistry {
    pub fn new() -> Self
    pub fn register<T: DownloadBackend + 'static>(&mut self, kind: TaskKind, backend: T)
    pub fn dispatch(&self, task_id: &TaskId) -> &dyn DownloadBackend
    pub fn by_kind(&self, kind: TaskKind) -> Result<&dyn DownloadBackend, DownloadError>
    pub fn get_typed<T: 'static>(&self) -> Option<&T>
    pub fn list_all(&self) -> Vec<DownloadSummary>
    pub fn iter(&self) -> BackendIterator<'_>
    pub async fn update_all_settings(&self, settings: &AppSettings)  // 向所有后端广播设置
    pub async fn shutdown_all(&self)                                  // 优雅关闭所有后端
}
```

## 数据流向

```
commands.rs 接收 Tauri IPC / rpc.rs 接收 WebSocket JSON-RPC
  ↓
├─ download.start: classify_kind(url) → TaskKind
│    └─ registry.by_kind(kind) → &dyn DownloadBackend
│         └─ backend.start(request) → 返回 TaskId ({kind, id})
│
└─ 其他命令: TaskId::from_legacy_string(download_id)
     └─ registry.dispatch(&task_id) → &dyn DownloadBackend
          ├─ HTTP → Backend impl 解构 TaskId::Http(uuid) → 内部 &Uuid
          └─ BT   → Backend impl 解构 TaskId::Bt(info_hash) → 内部 Id20
```

**重要约定**：

- BackendRegistry 使用 `register()` 注册协议；`dispatch()` 通过 `task_id.kind()` 路由
- `start()` 方法返回 `TaskId`（序列化为 `{kind, id}`），`kind` 字段标识协议，`id` 为原始 UUID/hex
- `list_all()` 合并所有注册后端的列表并排序
- `get_typed::<T>()` 返回原始类型引用，用于访问协议特有方法（如 DownloadManager 的 `buffer_pool`、`settings_default_download_dir`）
- Tauri 和 WebSocket RPC 共享同一个 BackendRegistry
