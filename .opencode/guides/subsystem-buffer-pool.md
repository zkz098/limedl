# Subsystem: Buffer Pool

## 模块职责

管理磁盘 I/O 写缓冲，针对 HDD 和 SSD 采用不同策略以最大化写入吞吐量。HDD 使用全局共享的双缓冲池（减少磁盘寻道），SSD 使用本地写合并（无需全局池）。同时配合游戏模式动态缩减内存占用。

**核心机制**：所有文件 flush 操作提交到专用 I/O worker 线程（`IoWorker`），通过 `mpsc::unbounded_channel` + `oneshot` 完成异步——单 worker 串行化所有写盘请求，BufferPool 的 ping-pong 提供天然背压（队列深度 ≤ 活跃下载数）。`spawn_blocking` 保留为 fallback 路径（单元测试 / 无 IoWorker 上下文）。

**涉及文件**：

- `crates/limedl-core/src/buffer_pool.rs` — BufferPool + IoWorker + DownloadBuffer + SlotGuard
- `crates/limedl-core/src/file_ops/mod.rs` (disk_detect 部分) — Win32 IOCTL 磁盘类型检测

## 关键结构体

### BufferPool (pub)

全局共享的内存管理池，限制并发写操作数量和总内存占用：

```rust
pub struct BufferPool {
    total_limit_mb: AtomicU64,              // 正常模式内存上限 (MB)
    game_mode: AtomicBool,                  // 游戏模式标志
    game_mode_limit_mb: AtomicU64,          // 游戏模式内存上限
    max_parallel: AtomicU32,                // 最大并发缓冲槽位数
    game_mode_max_parallel: AtomicU32,      // 游戏模式最大并发槽位
    slot_semaphore: Arc<Semaphore>,         // 公平信号量控制并发
    current_usage: AtomicU64,              // 当前总内存使用 (bytes)
    active_count: AtomicU32,               // 当前活跃槽位数
}
```

### IoWorker (pub)

专用 I/O worker 线程，串行执行所有文件 flush 请求：

```rust
pub struct IoWorker {
    tx: mpsc::UnboundedSender<IoCommand>,
}

impl IoWorker {
    pub fn spawn() -> Self                      // 创建 mpsc channel + std::thread("limedl-io-worker")
    pub async fn write_batch(                   // 提交一批有序条目到 worker 线程并 await 完成
        &self, file: Arc<File>, entries: Vec<(u64, Bytes)>
    ) -> Result<(), DownloadError>
}
```

- `spawn()` 在 `DownloadManager::new()` 时调用一次，整个应用共享单个 `IoWorker`
- `write_batch()` 内部通过 `mpsc::UnboundedSender` 发送 `IoCommand::WriteBatch`，用 `oneshot::channel` 等待 worker 线程完成写盘
- 所有 `IoWorker` 实例通过 clone 共享同一个 channel sender
- 所有 sender drop 后 channel 自动关闭，worker 线程自然退出

### SlotGuard (pub)

RAII 守卫，持有信号量许可。drop 时自动归还：

```rust
pub struct SlotGuard {
    permit: Option<OwnedSemaphorePermit>,
}
```

### DownloadBuffer (pub)

每个下载任务独立的写缓冲：

```rust
pub struct DownloadBuffer {
    mode: BufferMode,
    // HDD 模式: BufferMode::Double { half_a: Arc<Mutex<BTreeMap<u64, Bytes>>>, half_b: ..., flush_handle, ... }
    // SSD 模式: BufferMode::Local { buffer: Arc<Mutex<BTreeMap<u64, Bytes>>>, limit: u64, file: Arc<File> }
    io_worker: Option<IoWorker>,            // None → spawn_blocking fallback
}
```

双构造函数设计：

| 构造函数 | 用途 | Flush 路径 |
|----------|------|-----------|
| `new()` / `new_local()` | 单元测试 / 无 IoWorker 上下文 | `spawn_blocking` fallback |
| `new_with_worker()` / `new_local_with_worker()` | 生产代码 | `IoWorker::write_batch()` |

### DiskType (pub)

```rust
pub enum DiskType { Ssd, Hdd }
```

定义在 `types.rs`，公开类型。
由 `file_ops/mod.rs` 中的 `detect_disk_type()` 在下载开始时通过 Win32 `IOCTL_STORAGE_QUERY_PROPERTY` 检测。

## 关键方法

### BufferPool

```rust
pub fn new(total_limit_mb: u64, game_mode_limit_mb: u64, max_parallel: u32, game_mode_max_parallel: u32) -> Self

// 计算每个半缓冲的大小: effective_limit / effective_max_parallel / 2，最小 64 KiB
pub fn half_size(&self) -> u64

// 异步获取槽位（信号量 acquire，公平调度）
pub async fn acquire_slot(&self) -> SlotGuard

// 释放槽位（SlotGuard drop 时自动调用）
pub fn release_slot(&self)

// 内存使用追踪
pub fn add_usage(&self, bytes: u64)
pub fn sub_usage(&self, bytes: u64)

// 查询（均受 game_mode 影响）
pub fn effective_limit(&self) -> u64
pub fn effective_max_parallel(&self) -> u32
pub fn current_usage(&self) -> u64
pub fn active_slots(&self) -> u32
pub fn max_slots(&self) -> u32
pub fn queued_count(&self) -> u32          // 等待槽位的任务数

// 游戏模式
pub fn game_mode(&self) -> bool
pub fn set_game_mode(&self, enabled: bool)
pub fn update_limits(&self, total_limit_mb: u64, game_mode_limit_mb: u64, max_parallel: u32, game_mode_max_parallel: u32)
```

### DownloadBuffer

```rust
// HDD 模式：创建池支持的双缓冲（spawn_blocking fallback，供测试用）
pub fn new(pool: Arc<BufferPool>, slot: SlotGuard, file: Arc<File>) -> Self

// HDD 模式：使用专用 I/O worker（生产代码使用）
pub fn new_with_worker(pool: Arc<BufferPool>, slot: SlotGuard, file: Arc<File>, worker: IoWorker) -> Self

// SSD 模式：创建本地写合并缓冲（spawn_blocking fallback，供测试用）
pub fn new_local(limit_bytes: u64, file: Arc<File>) -> Self

// SSD 模式：使用专用 I/O worker（生产代码使用）
pub fn new_local_with_worker(limit_bytes: u64, file: Arc<File>, worker: IoWorker) -> Self

// 缓冲一块数据到指定 offset。若底层 flush 失败返回 Err
pub async fn buffer_chunk(&self, offset: u64, data: Bytes) -> Result<(), DownloadError>

// 刷新所有缓冲数据到磁盘
pub async fn flush_all(&self) -> Result<(), DownloadError>

// 等待后台 flush 完成（取消/暂停时使用）
pub async fn drain_background(&self)

// 清空缓冲（不写盘）
pub fn clear(&self)
```

## HDD vs SSD 模式对比

| 特性         | HDD (DoubleBuffer)             | SSD (LocalBuffer) |
| ------------ | ------------------------------ | ----------------- |
| 内存来源     | 全局 BufferPool                | 本地 4 MiB 限制   |
| 并发控制     | semaphore acquire (可排队等待) | 无争用            |
| 缓冲结构     | 两个 Mutex<BTreeMap> ping-pong  | 单个 Mutex<BTreeMap> |
| Flush 方式   | IoWorker::write_batch（专用线程） / spawn_blocking fallback | IoWorker::write_batch / 同步 flush fallback |
| Flush 触发   | 半缓冲满 OR 2s 定时器          | 缓冲满时          |
| 游戏模式影响 | 缩减 limit/parallel            | 无影响            |
| 设计目的     | 批量顺序写减少寻道             | 本地写合并足够    |

> **DashMap → `Mutex<BTreeMap>` 变更**：`half_a`/`half_b`/`chunks` 从 `Arc<DashMap<u64, Bytes>>` 改为 `Arc<parking_lot::Mutex<BTreeMap<u64, Bytes>>>`。BTreeMap `into_iter()` 按 key 升序遍历，天然有序，移除了 4 处 `sort_by_key` 调用。Drain 用 `std::mem::take(&mut *map).into_iter().collect()`（BTreeMap::drain 是 nightly only）。BTreeMap move 语义避免 Bytes Arc::clone 调用；单 half 本质串行写，Mutex 不竞争。dashmap crate 保留（bt_backend_own/ 仍用）。

## 数据流向

```
下载开始 → resolve_disk_type(destination_dir)
  ├─ HDD → acquire_slot(pool) → DownloadBuffer::new_with_worker(pool, slot, file, io_worker)
  └─ SSD → DownloadBuffer::new_local_with_worker(4MB, file, io_worker)

Worker 下载数据块
  ↓
buffer_chunk(offset, data)
  ├─ [HDD 模式]
  │    ├─ 写入当前活跃半缓冲 (half_a 或 half_b)
  │    ├─ 半缓冲满 OR 2s 定时器触发 → 切换活跃半缓冲
  │    └─ 旧半缓冲 → tokio::spawn(async { IoWorker::write_batch(...).await })
  │         └─ 无 IoWorker → spawn_blocking fallback
  ├─ [SSD 模式]
  │    ├─ 写入本地 BTreeMap (Arc<Mutex<BTreeMap>>)
  │    └─ 缓冲满 → IoWorker::write_batch (当 worker 可用时) / 同步 write_all_at fallback

取消/暂停
  ↓
drain_background() → 等待后台 flush 完成
clear() → 丢弃未刷数据

下载完成
  ↓
flush_all() → 确保所有缓冲写盘
SlotGuard drop → release_slot() → 归还池槽位
```

**重要约定**：

- HDD 是全局稀缺资源，slot semaphore 会导致新下载任务排队等待
- `half_size()` 保证每个下载至少 64 KiB 缓冲，避免极端配置导致过小缓冲区
- 游戏模式仅影响 HDD 池（缩减内存和并发），SSD 不受影响
- `buffer_chunk()` 返回 Err 说明后台 flush 失败（磁盘满、权限等）
- 修改池参数前理解：`total_limit_mb` 是所有下载共享的全局上限，`max_parallel` 是同时写盘的任务数上限
- **IoWorker 专用线程**：`DownloadManager::new()` 时 `IoWorker::spawn()` 一次，`http_executor` 通过 `dm.io_worker.clone()` 传入 `DownloadBuffer`。单 worker 线程串行化所有文件 flush，BufferPool ping-pong 提供自然背压（队列深度 ≤ 活跃下载数）。`spawn_blocking` 保留为 fallback（单元测试 / 无 IoWorker 上下文）
- 崩溃恢复不依赖缓冲池状态——只从 SQLite 中已持久化的 chunks 恢复进度
