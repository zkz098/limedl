# Subsystem: ProtocolRegistry + DownloadProtocol

## 模块职责

协议抽象层：定义 `DownloadProtocol` trait 作为 HTTP 和 BT 下载的统一接口，通过 `ProtocolRegistry` 消除 `commands.rs` 中的硬编码协议路由。提供协议无关的 `start/pause/resume/cancel/remove/purge/open/status/list` 操作。

**涉及文件**：

- `src-tauri/src/download/protocol.rs` (100+ 行) — DownloadProtocol trait + HTTP 适配器
- `src-tauri/src/download/protocol_registry.rs` — ProtocolRegistry 路由表

## 关键结构体

### DownloadProtocol trait (pub(crate))

```rust
#[async_trait]
pub(crate) trait DownloadProtocol: Send + Sync {
    async fn start(&self, request: StartDownloadRequest) -> Result<String>;
    async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn open_in_explorer(&self, download_id: &str) -> Result<()>;
    async fn status(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn list(&self) -> Result<Vec<DownloadSummary>>;
}
```

### ProtocolRegistry (pub(crate))

```rust
pub(crate) struct ProtocolRegistry {
    http: Arc<dyn DownloadProtocol>,
    bt: Arc<dyn DownloadProtocol>,
}
```

## 关键方法

```rust
impl ProtocolRegistry {
    pub(crate) fn new(http: Arc<dyn DownloadProtocol>, bt: Arc<dyn DownloadProtocol>) -> Self
    pub(crate) fn for_task(&self, task_id: &TaskId) -> &dyn DownloadProtocol
    pub(crate) async fn start(&self, kind: DownloadSourceKind, request: StartDownloadRequest) -> Result<String>
}
```

## 数据流向

```
commands.rs 接收 Tauri IPC
  ↓
├─ download_start: classify_download_source(url) → DownloadSourceKind
│    └─ registry.start(kind, request) → protocol.start() → 返回 prefixed ID
│
└─ 其他命令: TaskId::parse(download_id)
     └─ registry.for_task(&task_id) → &dyn DownloadProtocol
          ├─ HTTP → DownloadProtocol impl (protocol.rs, 前缀适配)
          └─ BT   → DownloadProtocol impl (bt_backend_own/mod.rs)
```

**重要约定**：

- ProtocolRegistry 使用具体字段（http/bt）而非 HashMap — 避免泛型分发复杂度，新协议只需加字段
- `start()` 方法返回的 ID 已包含协议前缀（`"http:"` 或 `"bt:"`），调用方无需再添加
- trait 中所有方法接受带前缀的 download_id（外部格式），HTTP 适配器负责 `strip_http_prefix` + `prefix_http_snapshot`
- `list()` 方法在 commands.rs 中单独调用（不走 registry），因为需要合并两个协议的列表
- 添加新协议（如 ftp:）时：在 types.rs 增加 `DownloadSourceKind` 变体 → ProtocolRegistry 加字段 → 实现 DownloadProtocol trait
