# Subsystem: DownloadManager

## 模块职责

HTTP 下载的完整生命周期编排：接收下载请求 → 探测远程文件元数据 → 规划分块策略 → 执行单流或多流下载 → 校验和验证 → 最终化文件。同时管理自适应调速（AIMD）和调度器（scheduler）。

**涉及文件**：
- `src-tauri/src/download/manager.rs` (1240 行) — 主管理器，下载 CRUD
- `src-tauri/src/download/http_executor.rs` (847 行) — HTTP 探测、单流/多流执行
- `src-tauri/src/download/scheduler.rs` (347 行) — 后台调度循环 + 线程重分配
- `src-tauri/src/download/aimd.rs` (249 行) — AIMD 吞吐量状态机
- `src-tauri/src/download/manifest.rs` (286 行) — Manifest/ChunkManifest 类型
- `src-tauri/src/download/retry.rs` (103 行) — 指数退避重试
- `src-tauri/src/download/file_alloc.rs` (346 行) — 文件创建、预分配、整理
- `src-tauri/src/download/checksum.rs` (95 行) — Blake3/SHA256/XXH3 校验和
- `src-tauri/src/download/rate_limiter.rs` (266 行) — 全局令牌桶速率限制
- `src-tauri/src/download/protocol.rs` (87 行) — DownloadProtocol trait

## 关键结构体

### AppState (pub)
Tauri 命令注入的全局状态：
```rust
pub struct AppState {
    pub manager: Arc<DownloadManager>,
    pub bt_backend: Arc<OwnBtBackend>,
    pub cdn_accelerator: Arc<CdnAccelerator>,
    pub app_handle: tauri::AppHandle,
    pub rpc_shutdown: Arc<Mutex<Option<watch::Sender<bool>>>>,
}
```

### DownloadManager (pub)
```rust
pub struct DownloadManager {
    client: Arc<RwLock<Client>>,                          // reqwest HTTP 客户端
    state_dir: PathBuf,                                    // 状态目录
    settings_path: PathBuf,                                // settings.json 路径
    pub(crate) settings: Arc<RwLock<AppSettings>>,
    pub(crate) downloads: Arc<RwLock<HashMap<String, Arc<ManagedDownload>>>>,
    pub(crate) db: Arc<Database>,
    pub(crate) rebalance_notify: Arc<Notify>,              // 调度器唤醒信号
    pub(crate) buffer_pool: Arc<BufferPool>,
    pub(crate) overclock_mode: AtomicBool,
    pub(crate) shutdown_token: CancellationToken,
    rate_limiter: Arc<RateLimiter>,
    // 内部字段: event_tx, cdn_accelerator, app_handle
}
```

### ManagedDownload (pub(crate))
每个下载任务的核心包装：
```rust
pub(crate) struct ManagedDownload {
    pub(crate) core: Mutex<DownloadCore>,
    pub(crate) runtime: Mutex<Option<CancellationToken>>,  // 运行时取消令牌
    pub(crate) aimd: Mutex<AimdState>,
    pub(crate) stop_notify: Notify,
}
```

### DownloadCore (pub(crate))
```rust
pub(crate) struct DownloadCore {
    pub(crate) snapshot: DownloadSnapshot,   // 实时下载状态（序列化到前端）
    pub(crate) manifest: Manifest,           // 分块计划 + 持久化元数据
}
```

### AimdState (pub(crate))
```rust
pub(crate) struct AimdState {
    pub(crate) last_sample_bytes: u64,
    pub(crate) last_sample_at: Option<Instant>,
    pub(crate) last_throughput: Option<f64>,
    pub(crate) cooldown_until: Option<Instant>,
    pub(crate) consecutive_good_samples: u32,
    pub(crate) consecutive_bad_samples: u32,
    pub(crate) recent_penalty: bool,
    pub(crate) throughput_sample_count: u32,
    pub(crate) throughput_sum: f64,
    pub(crate) peak_throughput: f64,
    pub(crate) penalty_count: u32,
}
```
**AIMD Profile 影响**：三个 profile 控制增减幅度和冷却时间：
| Profile | 初始线程 | 减少系数 | 冷却时间 |
|---|---|---|---|
| Conservative | 1 | 0.7x | 8s |
| Balanced | 2 | 0.5x | 6s |
| Aggressive | 4 | 0.5x | 4s |

## 关键方法

### DownloadManager — 构造 & 生命周期
```rust
pub fn new(state_dir: PathBuf, rate_limiter: Arc<RateLimiter>) -> Result<Self>
pub async fn shutdown(&self)
```

### DownloadManager — 设置 & 注入
```rust
pub async fn settings(&self) -> Result<AppSettings>
pub fn initial_settings(&self) -> AppSettings
pub async fn update_settings(&self, settings: AppSettings) -> Result<AppSettings>
pub fn set_event_tx(&self, tx: broadcast::Sender<String>)
pub fn set_app_handle(&self, handle: tauri::AppHandle)
pub fn set_cdn_accelerator(&self, acc: Arc<CdnAccelerator>)
```

### DownloadManager — 下载 CRUD（全 pub）
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
pub async fn get_summary(&self, download_id: &str) -> Option<DownloadSummary>
```

### DownloadManager — 游戏模式 & 超频
```rust
pub fn game_mode(&self) -> bool
pub fn set_game_mode(&self, enabled: bool)
pub fn set_overclock_mode(&self, enabled: bool)
pub fn overclock_mode(&self) -> bool
```

### DownloadManager — 调度器（pub(crate)）
```rust
pub(crate) fn start_scheduler_loop(self: Arc<Self>)
pub(crate) async fn update_adaptive_targets(&self) -> Result<()>
pub(crate) async fn rebalance_allocations(&self) -> Result<()>
```

### http_executor 内部方法（pub(super)，impl DownloadManager）
```rust
// 探测远程文件元数据（HEAD 或 GET Range:0-0 请求）
async fn probe(&self, url: &str, user_agent: &str) -> Result<RemoteMetadata>

// 主运行循环，决定单流还是多流
pub(super) async fn run_download(&self, managed: Arc<ManagedDownload>, client: Client, token: CancellationToken, max_retries: u32) -> Result<()>

// 单流顺序下载（不支持 Range 时使用）
async fn download_single(&self, managed: Arc<ManagedDownload>, client: Client, token: CancellationToken, max_retries: u32) -> Result<RunOutcome>

// 多流并行下载（JoinSet<ChunkWorkerOutcome>）
async fn download_chunked(&self, managed: Arc<ManagedDownload>, client: Client, token: CancellationToken, max_retries: u32) -> Result<RunOutcome>

// 最终化：flush → 校验和 → 重命名 → 更新 DB
async fn finalize_download(&self, managed: Arc<ManagedDownload>, token: CancellationToken) -> Result<RunOutcome>
```

### 工作器内部枚举
定义在 `manager.rs`（非 `http_executor.rs`）：
```rust
enum RunOutcome { Finished, Paused, Canceled }
enum ChunkWorkerOutcome { Finished, RestartSingle, Paused, Canceled }
```

### 调度器辅助函数
```rust
// 调度循环每 2s 唤醒一次（或由 rebalance_notify 触发）
// → update_adaptive_targets() → rebalance_allocations()

// AIMD 自适应线程调整（仅 Automatic 模式 + 无代理时生效）
// rebalance_allocations:
//   Traditional 模式: FIFO + max_parallel_tasks 上限
//   Automatic 模式: 按剩余字节排序（大文件优先），分配 max_parallel_threads 预算
```

### DownloadProtocol trait
```rust
#[async_trait]
pub(crate) trait DownloadProtocol: Send + Sync {
    async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn open_in_explorer(&self, download_id: &str) -> Result<()>;
    async fn status(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn list(&self) -> Result<Vec<DownloadSummary>>;
}
// DownloadManager 和 OwnBtBackend 都实现了此 trait
```

## 数据流向

> 完整的 5 阶段跨子系统流程见 **`.opencode/guides/core-data-flow.md`**。

简图：
```
用户点击下载
  ↓
DownloadComposer.vue → invoke("download_start")
  ↓
DownloadManager::start()
  ├─ spawn run_download()
  │    ├─ probe() → RemoteMetadata
  │    ├─ download_chunked() / download_single()
  │    └─ finalize_download() → checksum → rename → emit
  └─ scheduler loop (2s) → AIMD → rebalance
```

**重要约定**：
- 修改 manager.rs 中的下载状态逻辑前，务必先读 `http_executor.rs` 理解 worker 生命周期
- `DownloadCore.snapshot` 在 workers 中通过 `ManagedDownload::lock_core()` 更新，注意 Mutex 竞争
- AIMD 状态变更在 scheduler.rs 中，不在 workers 中；workers 只报告下载字节数
- `rebalance_allocations()` 修改 `allocated_thread_count`，worker 通过 `CancellationToken` 感知变更
