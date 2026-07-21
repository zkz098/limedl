# Subsystem: DownloadManager

## 模块职责

HTTP 下载的完整生命周期编排：接收下载请求 → 探测远程文件元数据 → 规划分块策略 → 执行单流或多流下载 → 校验和验证 → 最终化文件。同时管理自适应调速（AIMD）、调度器（scheduler）和任务生命周期（TaskLifecycle）。

核心类型：DownloadManager（HTTP 下载主管理器，已从 god object 拆分为 4 个子结构体 + 3 个零大小 actor 类型）。子结构体：ConcurrencyLimits（HTTP/BT 并发计数）、RuntimeControls（关闭信号、重平衡通知）、HttpClientInfra（HTTP 客户端 + CDN 缓存）、StateDirs（文件系统路径）。Actor：HttpExecutor（ZST，HTTP 探测/执行）、Scheduler（ZST，后台调度循环）、TaskLifecycle（ZST，任务状态转换/文件操作/进度记录/事件发射）。

ManagedDownload（每个下载任务的核心包装，含 DownloadCore + runtime + AimdState + stop_notify）。

## 涉及文件

- `crates/limedl-core/src/manager.rs` — DownloadManager 主结构体、CRUD 方法、辅助函数
- `crates/limedl-core/src/http_executor.rs` — HttpExecutor actor：HTTP 探测、单流/多流执行
- `crates/limedl-core/src/scheduler.rs` — Scheduler actor：后台调度循环 + AIMD 线程重分配
- `crates/limedl-core/src/task_lifecycle.rs` — TaskLifecycle actor：状态转换、文件清理、等待协调、进度记录、事件发射
- `crates/limedl-core/src/aimd.rs` — AIMD 吞吐量状态机
- `crates/limedl-core/src/manifest.rs` — Manifest / ChunkManifest 类型
- `crates/limedl-core/src/retry.rs` — 指数退避重试

## 数据流向

完整的跨子系统流程见 **`core-data-flow.md`**。

简图：

```
用户点击下载 → invoke("download_start")
  ↓
DownloadManager::start()
  ├─ 创建 ManagedDownload + slot 获取
  ├─ self.task_lifecycle.spawn_download() → tokio::spawn 后台任务
  │    └─ 镜像重试循环内：
  │         ├─ resolve_client() → CDN 加速客户端
  │         ├─ self.http_executor.run_download()
  │         │    ├─ probe() → RemoteMetadata
  │         │    ├─ download_chunked() / download_single()
  │         │    └─ finalize_download() → checksum → verify → rename
  │         └─ self.task_lifecycle.evict_completed()
  └─ self.scheduler.rebalance_allocations()
```

## 设计决策与约定

- DownloadManager 内部分为两个可见性层级：`pub async fn`（CRUD：start/pause/resume/cancel/remove/purge/status/list...）和 `pub(crate)` actor 方法。
- 三个 actor（HttpExecutor / Scheduler / TaskLifecycle）均为 ZST，不持有 DownloadManager 引用，通过方法参数接收 `&DownloadManager` 或 `Arc<DownloadManager>`，避免循环引用。
- start/pause/resume/cancel/remove/purge 等 CRUD 方法内部委托 actor 完成具体工作。状态变更后通过 scheduler.rebalance_allocations() 触发线程重分配。
- 所有事件通过 EventBus::publish() 统一发布。EventBus 不自动转发到前端——前端发射由 lib.rs 的独立订阅任务完成。
- AIMD 状态变更在 scheduler.rs 中，不在 workers 中；workers 只报告下载字节数。
- rebalance_allocations 修改 allocated_thread_count，worker 通过 CancellationToken 感知变更。
