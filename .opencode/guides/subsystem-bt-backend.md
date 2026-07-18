# Subsystem: BitTorrent Backend (OwnBtBackend)

## 模块职责

通过 irontide 库管理 BitTorrent 下载的完整生命周期。负责会话管理、torrent 元数据解析、对等节点连接、文件选择、上传策略，以及进度/状态查询。bt 任务使用 `bt:` 前缀的 TaskId。

**涉及文件**：

- `src-tauri/src/download/bt_backend_own/mod.rs` (90 行) — OwnBtBackend 结构体定义
- `src-tauri/src/download/bt_backend_own/protocol.rs` (302 行) — 下载操作实现（start/pause/resume/cancel...）
- `src-tauri/src/download/bt_backend_own/session.rs` (148 行) — irontide Session 初始化/关闭
- `src-tauri/src/download/bt_backend_own/snapshot.rs` (187 行) — 从 irontide stats 构建 DownloadSnapshot
- `src-tauri/src/download/bt_backend_own/queries.rs` (283 行) — 对等节点/区块/tracker/file 状态查询
- `src-tauri/src/download/bt_backend_own/alerts.rs` (291 行) — irontide 告警事件桥接
- `src-tauri/src/download/bt_backend_own/uploads.rs` (111 行) — 上传策略循环
- `src-tauri/src/download/bt_backend_own/tests.rs` (438 行)

## 关键结构体

### OwnBtBackend (pub(crate))

```rust
pub struct OwnBtBackend {
    pub(crate) session: irontide::session::SessionHandle,
    pub(crate) state_dir: PathBuf,
    pub(crate) default_output_dir: PathBuf,
    pub(crate) bt_settings: Arc<Mutex<BtSettings>>,
    pub(crate) event_bus: Arc<EventBus>,                      // 统一事件总线
    pub(crate) task_map: Arc<DashMap<String, Id20>>,          // download_id → info_hash 映射
    pub(crate) alert_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub(crate) upload_policy_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub(crate) http_client: Option<reqwest::Client>,           // 用于获取 .torrent 文件
    pub(crate) global_speed_limit_bps: u64,
    pub(crate) paused_by_limit: Arc<DashMap<Id20, ()>>,
    pub(crate) runtime_handle: tokio::runtime::Handle,
}
```

## 关键方法

### 构造 & 生命周期

```rust
pub async fn new(settings: &AppSettings, state_dir: PathBuf, default_output_dir: PathBuf, event_bus: Arc<EventBus>) -> Result<Self>
pub async fn shutdown(&self)
pub fn update_settings(&self, settings: &AppSettings)
```

### 下载操作（实现 DownloadProtocol trait）

```rust
pub async fn start(&self, request: StartDownloadRequest) -> Result<String>
pub async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot>
pub async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot>
pub async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot>
pub async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot>
pub async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot>
pub async fn open_in_explorer(&self, download_id: &str) -> Result<()>
pub async fn status(&self, download_id: &str) -> Result<DownloadSnapshot>
pub async fn list(&self) -> Result<Vec<DownloadSummary>>
```

### BT 特有查询

```rust
pub fn set_speed_limit(&self, download_id: &str, download_limit_bps: Option<u64>, upload_limit_bps: Option<u64>)
pub async fn preview_torrent(&self, source: &str) -> Result<Vec<TorrentFileEntry>>
pub fn get_peers(&self, download_id: &str) -> Result<Vec<BtPeerInfo>>
pub fn get_trackers(&self, download_id: &str) -> Result<Vec<BtTrackerInfo>>
pub fn get_pieces(&self, download_id: &str) -> Result<Vec<BtPieceInfo>>
pub fn get_torrent_files(&self, download_id: &str) -> Result<Vec<BtFileStatus>>
pub async fn update_torrent_files(&self, download_id: &str, included_indices: Vec<usize>) -> Result<()>
pub fn runtime_status(&self) -> BtRuntimeStatus
pub fn emit_pending_summary(&self, pending_id: &str)
```

### 内部辅助

```rust
pub fn spawn_upload_policy_loop(self: Arc<Self>)
pub(crate) fn stats_to_snapshot(&self, task_id: &str, info_hash: &Id20, stats: &irontide::session::TorrentStats) -> DownloadSnapshot
pub(crate) fn parse_info_hash(download_id: &str) -> Result<Id20>
pub(crate) async fn fetch_url_bytes(&self, url: &str) -> Result<Vec<u8>>
```

## 数据流向

```
用户提交 BT 任务（magnet link 或 .torrent 文件）
  ↓
commands::bt_start() → TaskId::parse("bt:...")
  ↓
OwnBtBackend::start()
  ├─ 解析 URL → 获取 .torrent 元数据（magnet link 通过 DHT 获取，文件直接下载）
  ├─ SessionHandle::add_torrent() → 加入 irontide 会话
  ├─ task_map.insert(download_id → info_hash)
  └─ setup_alert_bridge()
       └─ 后台循环接收 irontide 告警
            ├─ stats_alert → stats_to_snapshot() → EventBus::publish(Updated/Progress)
            ├─ metadata_received → 文件列表可用
            └─ torrent_finished → 标记 completed → EventBus::publish(Aria2Notification/Progress/Updated)

事件发送：所有告警和状态变更通过 EventBus.publish() 统一发布。EventBus 自动将事件转发到
Tauri 前端（emit）和内部订阅者（Aria2 RPC 桥接）。不再直接持有 app_handle 或 event_tx。

上传策略循环（spawn_upload_policy_loop）：
  ├─ 每 N 秒检查全局上传速率
  ├─ 超过限制 → 暂停部分 torrent 上传（paused_by_limit）
  └─ 低于限制 → 恢复上传
```

**重要约定**：

- BT 任务的 download_id 格式为 `bt:{info_hash_hex}`（40 字符十六进制 info hash），不是 UUID
- `task_map` 维护 download_id → Id20(info_hash) 的映射关系
- irontide 通过告警系统异步推送状态变更，不要轮询 session 获取状态
- `stats_to_snapshot()` 是状态转换的核心桥梁
- 上传策略循环独立于下载，通过 `paused_by_limit` DashMap 控制
