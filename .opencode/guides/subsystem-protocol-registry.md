# Subsystem: BackendRegistry + DownloadBackend + Dispatcher

## 模块职责

协议抽象层：定义 `DownloadBackend` trait 作为 HTTP 和 BT 下载的统一接口，通过 `BackendRegistry` 提供协议路由。
`Dispatcher` 在此之上提供统一的调度入口，消除 Tauri 命令层和 WebSocket RPC 层的重复调度逻辑。

**涉及文件**：

- `crates/limedl-core/src/protocol.rs` — DownloadBackend trait + HTTP 适配器
- `crates/limedl-core/src/backend_registry.rs` — BackendRegistry 路由表
- `crates/limedl-core/src/dispatcher.rs` — Dispatcher 调度层（新）

## 关键结构体

### DownloadBackend trait (pub)

```rust
#[async_trait]
pub trait DownloadBackend: Send + Sync {
    async fn start(&self, request: StartDownloadRequest) -> Result<TaskId>;
    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()>;
    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot>;
    async fn list(&self) -> Result<Vec<DownloadSummary>>;
    async fn update_settings(&self, settings: &AppSettings) -> Result<()>;
    async fn shutdown(&self);
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

### Dispatcher (pub)

```rust
pub struct Dispatcher {
    registry: Arc<BackendRegistry>,
    event_bus: Arc<EventBus>,
}
```

## 关键方法

### BackendRegistry

```rust
impl BackendRegistry {
    pub fn new() -> Self
    pub fn register<T: DownloadBackend + 'static>(&mut self, kind: TaskKind, backend: T)
    pub fn dispatch(&self, task_id: &TaskId) -> Result<&dyn DownloadBackend, DownloadError>
    pub fn by_kind(&self, kind: TaskKind) -> Result<&dyn DownloadBackend, DownloadError>
    pub fn get_typed<T: DownloadBackend + 'static>(&self) -> Option<&T>
    pub fn list_all(&self) -> Vec<DownloadSummary>
    pub fn iter(&self) -> impl Iterator<Item = &dyn DownloadBackend>
    pub async fn update_all_settings(&self, settings: &AppSettings)  // 向所有后端广播设置
    pub async fn shutdown_all(&self)                                  // 优雅关闭所有后端
}
```

### Dispatcher

```rust
impl Dispatcher {
    pub fn new(registry: Arc<BackendRegistry>, event_bus: Arc<EventBus>) -> Self

    // 核心下载生命周期
    pub async fn start(&self, request: StartDownloadRequest) -> Result<TaskId>
    pub async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot>     // + emit Updated
    pub async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot>     // + emit Updated
    pub async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot>     // + emit Updated
    pub async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot>     // + emit Updated
    pub async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot>      // + emit Updated
    pub async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot>     // 只读，不 emit
    pub async fn list(&self) -> Result<Vec<DownloadSummary>>                      // 只读，不 emit

    // BT 特有操作（类型安全，内部解构 TaskId::Bt）
    pub fn bt_runtime_status(&self) -> Result<BtRuntimeStatus>
    pub fn bt_set_speed_limit(&self, task_id: &TaskId, dl: Option<u64>, ul: Option<u64>) -> Result<()>
    pub async fn bt_preview_torrent(&self, source: &str) -> Result<Vec<TorrentFileEntry>>
    pub fn bt_get_peers(&self, task_id: &TaskId) -> Result<Vec<BtPeerInfo>>
    pub fn bt_get_trackers(&self, task_id: &TaskId) -> Result<Vec<BtTrackerInfo>>
    pub fn bt_get_pieces(&self, task_id: &TaskId) -> Result<Vec<BtPieceInfo>>
    pub fn bt_get_files(&self, task_id: &TaskId) -> Result<Vec<BtFileStatus>>
    pub async fn bt_update_files(&self, task_id: &TaskId, indices: Vec<usize>) -> Result<()>

    // 手动触发事件（供 start 后调用，因为 start() 自身不 emit）
    pub fn emit_updated(&self, snapshot: &DownloadSnapshot)
}
```

## 数据流向

### 重构前

```
commands.rs Tauri IPC ──────────────────────┐
                                            ├─ registry.dispatch() → backend.*()
rpc.rs WebSocket JSON-RPC ──────────────────┘   （各自实现相同逻辑，重复 36 处）
```

### 重构后

```
commands.rs Tauri IPC（薄壳：参数解析 + 错误转换）──┐
                                                    ├─ dispatcher.*()
rpc.rs WebSocket JSON-RPC（薄壳：params 反序列化 ───┘
       + URL 校验 + 错误编码）
                          │
                          └─ dispatcher 内部：
                               registry.dispatch() → backend.*()
                               + 统一的 DownloadEvent::Updated emit
```

### 各操作的数据流

```
download.start:
  边界层 → URL 校验 + mirror URL 注入 → dispatcher.start(request)
       → registry.by_kind(kind) → backend.start(request)
       → 边界层可选 emit (start 自身不 emit，避免 BT 双发)

download.pause / resume / cancel / remove / purge:
  边界层 → 解析 downloadId → TaskId → dispatcher.*(&task_id)
       → registry.dispatch(&task_id) → backend.*(&task_id)
       → dispatcher 自动 emit DownloadEvent::Updated

download.status / list:
  边界层 → dispatcher.*()
       → registry.dispatch / list_all → 返回数据（不 emit）

BT 特有操作 (getPeers / getTrackers / getPieces 等):
  边界层 → 解析 taskId → TaskId → dispatcher.bt_*(&task_id)
       → 内部解构 TaskId::Bt + registry.get_typed() 调用
```

## 事件 emit 策略

| 操作 | Dispatcher emit? | 说明 |
|------|------------------|------|
| `start` | 否 | BT 后端已通过 `emit_pending_summary` 自动 emit；HTTP 由边界层 emit |
| `pause` | 是 | 统一 emit，确保前端状态同步 |
| `resume` | 是 | 同上 |
| `cancel` | 是 | **修复了旧代码 cancel/remove/purge 漏 emit 的 bug** |
| `remove` | 是 | 同上 |
| `purge` | 是 | 同上 |
| `status` | 否 | 只读操作 |
| `list` | 否 | 只读操作 |

## 各边界层保留的逻辑（不在 Dispatcher 内）

| 逻辑 | 所属层 | 原因 |
|------|--------|------|
| mirror URL 填充 | commands.rs (Tauri) | 需要 DownloadManager::mirror_urls_for |
| URL 长度/格式校验 | rpc.rs (Server) | RPC 层特有的防御性校验 |
| `open_in_explorer` | 各边界层 | OS 特有操作 |
| `settings.get/save` | 各边界层 | 设置管理，非下载生命周期 |
| `toggleGameMode/getIoStatus` | 各边界层 | 缓冲池管理，非下载生命周期 |
| CDN routes | 各边界层 | 独立子系统，不在 Dispatcher 范围 |

## 重要约定

- BackendRegistry 使用 `register()` 注册协议；`dispatch()` 通过 `task_id.kind()` 路由
- `start()` 方法返回 `TaskId`（序列化为 `{kind, id}`），`kind` 字段标识协议，`id` 为原始 UUID/hex
- `list_all()` 合并所有注册后端的列表并排序
- `get_typed::<T>()` 返回原始类型引用，用于访问协议特有方法（如 DownloadManager 的 `buffer_pool`、`settings_default_download_dir`）
- Tauri 和 WebSocket RPC 共享同一个 BackendRegistry 和 Dispatcher 抽象
