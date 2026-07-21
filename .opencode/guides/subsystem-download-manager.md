# Subsystem: DownloadManager

## 模块职责

HTTP 下载的完整生命周期编排：接收下载请求 → 探测远程文件元数据 → 规划分块策略 → 执行单流或多流下载 → 校验和验证 → 最终化文件。同时管理自适应调速（AIMD）、调度器（scheduler）和任务生命周期（TaskLifecycle）。

**涉及文件**：

- `crates/limedl-core/src/manager.rs` — 主管理器，下载 CRUD + 4×子结构体 + 3×actor 字段
- `crates/limedl-core/src/http_executor.rs` — `HttpExecutor` 独立 actor：HTTP 探测、单流/多流执行
- `crates/limedl-core/src/scheduler.rs` — `Scheduler` 独立 actor：后台调度循环 + 线程重分配
- `crates/limedl-core/src/task_lifecycle.rs` — `TaskLifecycle` 独立 actor：任务状态转换、文件操作、等待协调、进度记录、事件发射
- `crates/limedl-core/src/aimd.rs` — AIMD 吞吐量状态机
- `crates/limedl-core/src/manifest.rs` — Manifest/ChunkManifest 类型
- `crates/limedl-core/src/retry.rs` — 指数退避重试
- `crates/limedl-core/src/file_ops/mod.rs` — 文件创建、预分配、整理、磁盘检测
- `crates/limedl-core/src/checksum/mod.rs` — Blake3/SHA256/XXH3 校验和
- `crates/limedl-core/src/rate_limiter.rs` — 全局令牌桶速率限制
- `crates/limedl-core/src/protocol.rs` — DownloadBackend trait

## 关键结构体

### AppState (pub)

Tauri 命令注入的全局状态：

```rust
pub struct AppState {
    pub registry: Arc<BackendRegistry>,
    pub event_bus: Arc<EventBus>,
    pub cdn_service: Arc<CdnService>,
    pub rpc_shutdown: Arc<parking_lot::Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
}
```

### DownloadManager (pub)

字段按内聚关系分组到 4 个子结构体 + 3 个 actor，降低顶层字段散乱度：

```rust
pub struct DownloadManager {
    pub http: HttpClientInfra,           // HTTP client, CDN cache, CDN accelerator
    pub dirs: StateDirs,                 // state_dir, settings_path
    pub settings: Arc<RwLock<AppSettings>>,
    pub downloads: Arc<RwLock<HashMap<String, Arc<ManagedDownload>>>>,
    pub db: Arc<Database>,
    pub event_bus: Arc<EventBus>,
    pub(crate) rate_limiter: Arc<RateLimiter>,
    pub buffer_pool: Arc<BufferPool>,
    /// Dedicated I/O worker thread for file flush operations (mpsc + std::thread).
    /// Spawned once in DownloadManager::new(), cloned into each DownloadBuffer
    /// via http_executor.
    pub io_worker: IoWorker,
    pub controls: RuntimeControls,       // shutdown_token, rebalance_notify
    pub limits: ConcurrencyLimits,       // active_http_count, active_bt_count, max_concurrent_*, overclock_mode
    pub http_executor: Arc<HttpExecutor>, // HTTP 下载执行 actor
    pub scheduler: Arc<Scheduler>,       // 调度循环 actor
    pub task_lifecycle: Arc<TaskLifecycle>, // 任务生命周期 actor
}
```

#### 子结构体清单

**`ConcurrencyLimits`** — HTTP/BT 下载并发限制和计数器：
```rust
pub struct ConcurrencyLimits {
    pub active_http_count: Arc<AtomicUsize>,
    pub active_bt_count: Arc<AtomicUsize>,
    pub max_concurrent_http: AtomicUsize,
    pub max_concurrent_bt: Arc<AtomicUsize>,
    pub overclock_mode: AtomicBool,
}
```

**`RuntimeControls`** — 运行时控制信号（关闭、重平衡）：
```rust
pub struct RuntimeControls {
    pub shutdown_token: CancellationToken,
    pub rebalance_notify: Arc<Notify>,
}
```

**`HttpClientInfra`** — HTTP 客户端基础设施（含 CDN）：
```rust
pub struct HttpClientInfra {
    client: Arc<RwLock<Client>>,
    pub cdn_client_cache: Arc<ParkingRwLock<HashMap<(String, Ipv4Addr), Client>>>,
    cdn_accelerator: Arc<RwLock<Option<Arc<super::cdn::CdnAccelerator>>>>,
}
```

**`StateDirs`** — 文件系统路径：
```rust
pub struct StateDirs {
    pub(crate) state_dir: PathBuf,
    pub(crate) settings_path: PathBuf,
}
```

### HttpExecutor (pub)

零大小 actor 类型，专门负责 HTTP 下载执行。所有方法都接收 `&DownloadManager` 或 `Arc<DownloadManager>` 参数来访问共享状态，避免与 `DownloadManager`（持有 `Arc<HttpExecutor>`）形成循环引用。

```rust
pub struct HttpExecutor; // ZST — 无自己的状态
```

### Scheduler (pub)

零大小 actor 类型，专门负责后台调度循环和自适应重平衡。同样通过方法参数接收 `&DownloadManager` 或 `Arc<DownloadManager>` 来访问共享状态。

```rust
pub struct Scheduler; // ZST — 无自己的状态
```

### TaskLifecycle (pub)

零大小 actor 类型，专门负责下载任务的生命周期操作。从 `DownloadManager` 提取而来，包含状态转换内部方法、文件清理、等待协调、进度记录和事件发射。同样通过方法参数接收 `&DownloadManager` 或 `Arc<DownloadManager>` 来访问共享状态。

```rust
pub struct TaskLifecycle; // ZST — 无自己的状态
```

### ManagedDownload (pub(crate))

每个下载任务的核心包装：

```rust
pub(crate) struct ManagedDownload {
    pub(crate) core: Mutex<DownloadCore>,
    pub(crate) runtime: Mutex<Option<CancellationToken>>,
    pub(crate) aimd: Mutex<AimdState>,
    pub(crate) stop_notify: Notify,
}
```

### DownloadCore (pub(crate))

```rust
pub(crate) struct DownloadCore {
    pub(crate) snapshot: DownloadSnapshot,
    pub(crate) manifest: Manifest,
}
```

## 关键方法

### DownloadManager — 构造 & 生命周期

```rust
pub fn new(state_dir: PathBuf, rate_limiter: Arc<RateLimiter>, event_bus: Arc<EventBus>) -> Result<Self>
```

`new()` 构造时创建 `HttpExecutor`、`Scheduler` 和 `TaskLifecycle` 的 Arc 实例。

`shutdown()` 方法已委托给 `self.task_lifecycle.shutdown(self)`，保留了 `DownloadBackend` trait 中的 `shutdown`（含 buffer pool drain 逻辑）。

### DownloadManager — 设置 & 注入

```rust
pub async fn settings(&self) -> Result<AppSettings>
pub fn initial_settings(&self) -> AppSettings
pub async fn apply_settings(&self, settings: AppSettings) -> Result<AppSettings>
pub fn set_cdn_accelerator(&self, acc: Arc<CdnAccelerator>)
```

`apply_settings()` 内部委托给 `self.scheduler.rebalance_allocations(self)`。

### DownloadManager — 下载 CRUD（全 pub）

```rust
pub async fn start(&self, request: StartDownloadRequest) -> Result<Uuid>
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

所有方法内部委托 actor 完成具体工作：
- `pause`/`resume`/`cancel` 内部调用 `self.task_lifecycle.get()`/`wait_until_stopped()`/`build_snapshot()` 等
- `remove`/`purge` 委托 `self.task_lifecycle.remove_internal()`
- `start`/`resume` 委托 `self.task_lifecycle.spawn_download()`
- 状态变更后通过 `self.scheduler.rebalance_allocations(self)` 触发线程重分配

### DownloadManager — 游戏模式 & 超频

```rust
pub fn game_mode(&self) -> bool
pub fn set_game_mode(&self, enabled: bool)
pub fn set_overclock_mode(&self, enabled: bool)
pub fn overclock_mode(&self) -> bool
```

### TaskLifecycle — 任务生命周期

```rust
// 按 ID 查找下载
pub(crate) async fn get(&self, dm: &DownloadManager, download_id: &str) -> Result<Arc<ManagedDownload>>

// 关闭所有运行中的下载
pub(crate) async fn shutdown(&self, dm: &DownloadManager)

// 主编排方法：启动后台任务（镜像重试循环 + HTTP 执行 + 状态持久化 + 裁剪）
pub(crate) async fn spawn_download(&self, dm: Arc<DownloadManager>,
    managed: Arc<ManagedDownload>, max_retries: u32, slot: DownloadSlotGuard) -> Result<()>

// 移除/清除内部实现
pub(crate) async fn remove_internal(&self, dm: &DownloadManager,
    download_id: &str, purge_file: bool) -> Result<DownloadSnapshot>

// 等待工作器退出
pub(crate) async fn wait_until_stopped(&self, dm: &DownloadManager, managed: &Arc<ManagedDownload>)

// 等待工作器激活（获取线程分配）
pub(crate) async fn wait_until_active(&self, dm: &DownloadManager,
    managed: &Arc<ManagedDownload>, token: &CancellationToken) -> WaitState

// 文件操作
pub(crate) fn cleanup_files(&self, dm: &DownloadManager, managed: &Arc<ManagedDownload>) -> Result<()>
fn cleanup_destination_file(&self, dm: &DownloadManager, managed: &Arc<ManagedDownload>) -> Result<()>
pub(crate) fn prepare_fresh_temp_file(&self, dm: &DownloadManager, managed: &Arc<ManagedDownload>) -> Result<()>
pub(crate) fn reset_progress(&self, dm: &DownloadManager, managed: &Arc<ManagedDownload>, force_single_stream: bool)

// 裁剪已完成条目
pub(crate) async fn evict_completed(&self, dm: &DownloadManager) -> usize

// 构建快照和事件发射
pub(crate) fn build_snapshot(&self, dm: &DownloadManager, managed: Arc<ManagedDownload>) -> DownloadSnapshot
pub(crate) fn emit_single_summary(&self, dm: &DownloadManager, managed: &Arc<ManagedDownload>)
pub(crate) fn emit_progress(&self, dm: &DownloadManager, managed: &Arc<ManagedDownload>)
pub(crate) fn record_progress(&self, dm: &DownloadManager, managed: &Arc<ManagedDownload>,
    chunk_index: Option<usize>, bytes: u64)
```

### HttpExecutor — HTTP 下载执行

```rust
// 探测远程文件元数据（HEAD 或 GET Range:0-0 请求）
async fn probe(&self, dm: &DownloadManager, url: &str, user_agent: &str) -> Result<RemoteMetadata>

// 主运行循环，决定单流还是多流
pub(crate) async fn run_download(&self, dm: Arc<DownloadManager>, managed: Arc<ManagedDownload>,
    client: Client, token: CancellationToken, max_retries: u32) -> Result<()>

// 单流顺序下载（不支持 Range 时使用）
async fn download_single(&self, dm: Arc<DownloadManager>, managed: Arc<ManagedDownload>,
    client: Client, token: CancellationToken, max_retries: u32) -> Result<RunOutcome>

// 多流并行下载（JoinSet<ChunkWorkerOutcome>）
async fn download_chunked(&self, dm: Arc<DownloadManager>, managed: Arc<ManagedDownload>,
    client: Client, token: CancellationToken, max_retries: u32) -> Result<RunOutcome>

// 最终化：flush → 校验和 → 完整性验证 → 重命名 → 更新 DB
async fn finalize_download(&self, dm: Arc<DownloadManager>, managed: Arc<ManagedDownload>,
    token: CancellationToken) -> Result<RunOutcome>
```

每个方法第一个参数为 `&DownloadManager` 或 `Arc<DownloadManager>`，内部通过 `dm.field` 或 `dm.task_lifecycle.method()` 访问 DM 状态。

### Scheduler — 调度和重平衡

```rust
// 启动后台调度循环（2s tick 或 rebalance_notify 唤醒）
pub fn start_scheduler_loop(self: Arc<Self>, dm: Arc<DownloadManager>)

// 更新自适应（AIMD）线程目标
pub async fn update_adaptive_targets(&self, dm: &DownloadManager) -> Result<()>

// 重平衡线程分配
pub async fn rebalance_allocations(&self, dm: &DownloadManager) -> Result<()>
```

`start_scheduler_loop` 接收 `self: Arc<Self>` 以在后台任务中保活 scheduler。

### 完整性校验（Integrity Verification）

`finalize_download()` 在最终化中增加了可选的完整性校验步骤：

1. **`expected_checksum` 字段**：`StartDownloadRequest` 和 `Manifest` 均包含 `pub expected_checksum: Option<String>`。
2. **校验流程**：计算 → 比对 → 匹配则 Completed，不匹配则 Failed。

### 工作器内部枚举

定义在 `manager.rs`：

```rust
pub(crate) enum RunOutcome { Finished, Paused, Canceled }
pub(crate) enum ChunkWorkerOutcome { Finished, RestartSingle, Paused, Canceled }
pub(crate) enum WaitState { Running, Paused, Canceled }
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

### DownloadBackend trait

```rust
#[async_trait]
pub trait DownloadBackend: Send + Sync + 'static {
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
// DownloadManager 和 IrontideBtBackend 都实现了此 trait
```

## Actor 拆分架构

`DownloadManager` 曾聚合 17+ 字段和 50+ 方法，是已知 god object。分阶段演进：

| 阶段 | 内容 |
|------|------|
| Stage 4 | 字段分组到 4 子结构体（`ConcurrencyLimits`、`RuntimeControls`、`HttpClientInfra`、`StateDirs`） |
| Stage 5 | 将 `http_executor.rs` 和 `scheduler.rs` 中的扩展方法提炼为独立 actor type |
| Stage 9 (本阶段) | 将生命周期方法（spawn_download、remove_internal、wait_*、cleanup_*、reset_progress、record_progress、emit_*、build_snapshot、evict_completed、shutdown）提炼为 `TaskLifecycle` actor |

### Actor 化设计原则

- **http_executor** → 独立 `HttpExecutor` struct，方法接收 `dm: &DownloadManager` / `dm: Arc<DownloadManager>`
- **scheduler** → 独立 `Scheduler` struct，方法接收 `dm: &DownloadManager` / `dm: Arc<DownloadManager>`
- **task_lifecycle** → 独立 `TaskLifecycle` struct，方法接收 `dm: &DownloadManager` / `dm: Arc<DownloadManager>`
- **manager** 本身保留对外 API 入口责任（`impl DownloadBackend`、settings 应用、状态查询），把执行/生命周期工作委派给三个 actor

### 循环引用避免

`DownloadManager` 持有 `Arc<HttpExecutor>`、`Arc<Scheduler>` 和 `Arc<TaskLifecycle>`。
三个 actor 均**不持有** DM 引用——它们通过方法参数接收 `&DownloadManager` 或 `Arc<DownloadManager>`。
这完全避免了循环引用问题。

### 模块声明

| 模块 | 声明位置 | 类型 |
|------|----------|------|
| `manager` | `lib.rs` — `pub mod manager;` | 主模块 |
| `http_executor` | `lib.rs` — `pub mod http_executor;` | 独立模块 |
| `scheduler` | `lib.rs` — `pub mod scheduler;` | 独立模块 |
| `task_lifecycle` | `lib.rs` — `pub mod task_lifecycle;` | 独立模块 |

### pub(crate) 可见性变更

各 actor 间通过 `pub(crate)` 方法互相访问，不突破 friend module 限制：

| Helper | 定义位置 | 用途 |
|--------|----------|------|
| `supports_parallelism` | manager.rs | 判断是否支持多流并行 |
| `resolve_thread_settings` | manager.rs | 根据设置和请求解析线程模式/数量 |
| `thread_note` | manager.rs | 生成线程模式的中文说明文本 |
| `sync_snapshot_from_manifest` | manager.rs | 将 manifest 字段同步到 snapshot。**COW 优化**：fast path（chunk 结构未变：数量相等且每对 index/start/end 一致）原地更新三个状态字段（downloaded/completed/claimed_by），零 alloc；slow path（结构变化或初始空 snapshot）全量 `Vec::with_capacity` 重建。`!is_empty()` 守卫防止空 snapshot + 空 manifest 误走 fast path。 |
| `record_progress_on_managed` | manager.rs | 记录 chunk 下载进度 |
| `unique_destination_path` | manager.rs | 生成不冲突的目标文件路径 |
| `log_background_error` | manager.rs | 记录后台错误日志 |
| `cancellation_outcome` | manager.rs | 根据 state 判断取消/暂停结果 |
| `cancellation_chunk_outcome` | manager.rs | chunk worker 的取消/暂停判定 |

所有 TaskLifecycle 方法均为 `pub(crate)`，可从 http_executor/scheduler 等兄弟模块访问。

## 数据流向

完整的跨子系统流程见 **`.opencode/guides/core-data-flow.md`**。

简图：

```
用户点击下载
  ↓
DownloadComposer.vue → invoke("download_start")
  ↓
DownloadManager::start()
  ├─ 创建 ManagedDownload + slot 获取
  ├─ self.task_lifecycle.spawn_download()  → tokio::spawn 后台任务
  │    └─ 镜像重试循环内:
  │         ├─ resolve_client() → CDN 加速客户端
  │         ├─ self.http_executor.run_download()
  │         │    ├─ probe() → RemoteMetadata
  │         │    ├─ download_chunked() / download_single()
  │         │    └─ finalize_download() → checksum → verify → rename
  │         └─ self.task_lifecycle.evict_completed()
  └─ self.scheduler.rebalance_allocations(self)
       └─ scheduler loop (2s) → AIMD → rebalance
```

**重要约定**：

- 修改 manager.rs 中的下载状态逻辑前，务必先读 `http_executor.rs` 理解 worker 生命周期
- `DownloadCore.snapshot` 在 workers 中通过 `ManagedDownload::lock_core()` 更新，注意 Mutex 竞争
- AIMD 状态变更在 scheduler.rs 中，不在 workers 中；workers 只报告下载字节数
- `rebalance_allocations()` 修改 `allocated_thread_count`，worker 通过 `CancellationToken` 感知变更
- 所有事件（前端更新 + 内部订阅）通过 `event_bus: Arc<EventBus>` 统一发布
- `EventBus::publish()` 自动将事件转发到 Tauri 前端和内部广播通道
- TaskLifecycle 方法与 HttpExecutor/Scheduler 模式一致：ZST + `&DM`/`Arc<DM>` 参数，无循环引用风险
