# Core Data Flow: HTTP Download

> 从用户点击"开始下载"到文件写入磁盘的完整路径。涉及 6 个子系统。

## 模块职责

描述 HTTP 下载的跨子系统数据流：前端 → invoke → 协议路由 → 探测 → 分块 → 并发下载 → 缓冲/刷盘 → 速率限制 → AIMD 调速 → 校验和 → 最终化。

## 涉及文件

- `src/lib/tauri/download-api.ts` — 前端 invoke 调用
- `src-tauri/src/download/commands.rs` — Tauri 命令层
- `crates/limedl-core/src/manager.rs` + `http_executor.rs` — 下载编排与执行
- `crates/limedl-core/src/scheduler.rs` + `aimd.rs` — 调度与自适应调速
- `crates/limedl-core/src/buffer_pool.rs` — 磁盘写缓冲
- `crates/limedl-core/src/database.rs` — 持久化
- `crates/limedl-core/src/rate_limiter/` — 全局限速
- `crates/limedl-core/src/checksum/` — 校验和验证
- `crates/limedl-core/src/retry.rs` — 指数退避重试
- `crates/limedl-core/src/file_ops/` — 文件创建/预分配/最终化

## 数据流向

```
Phase 1 — 前端 → 后端:
  用户填写表单 → StartDownloadRequest (JSON, camelCase)
    → download-api.ts → invoke("download_start")
    → commands.rs → classify_kind() → BackendRegistry.by_kind()
    → HTTP URL → DownloadManager::start() → TaskId::Http(uuid)

Phase 2 — 探测 & 规划:
  DownloadManager::start() → 创建 ManagedDownload → spawn run_download()
    → HttpExecutor::probe(url) → HEAD 请求（或 GET Range:0-0 回退）
    → RemoteMetadata（total_bytes, supports_ranges, etag, file_name）
    → 构建 Manifest + chunk plan
    → Database::insert_download()
    → resolve_disk_type() → DiskType::Ssd / Hdd

Phase 3 — 下载执行:
  [多流] supports_ranges && total_bytes > threshold:
    → download_chunked() → JoinSet<ChunkWorkerOutcome>
    → 每个 worker: claim_next_chunk() → HTTP Range 请求
    → DownloadBuffer::buffer_chunk(offset, data)
      ├─ HDD: 池双缓冲 → IoWorker 异步刷盘
      └─ SSD: 本地写合并 → 同步/异步刷盘
    → 全局 RateLimiter::consume()（累计 ~256KB 或 8 chunk 消费一次）
  [单流] 不支持 Range 或小文件:
    → download_single() → 顺序 GET + Range: bytes=downloaded- 续传

Phase 4 — 调度 & 自适应:
  scheduler 后台循环（SCHEDULER_TICK = 2s）:
    → update_adaptive_targets(): AIMD 采样吞吐 → 增减期望线程数
    → rebalance_allocations(): 重分配线程预算
      ├─ Traditional: FIFO + max_parallel_tasks 上限
      └─ Automatic: 按剩余字节排序（大文件优先），分配 max_parallel_threads 预算

Phase 5 — 最终化:
  所有 chunk 完成 → finalize_download()
    → DownloadBuffer::drain_background() + flush_all()
    → calculate_checksum()（Blake3 / SHA256 / XXH3-128）
    → 重命名 .tmp → 最终路径
    → Database: state=completed
    → EventBus::publish(Updated) → lib.rs 后台任务 → app_handle.emit("download-updated")
```

## 设计决策与约定

- 事件推送：DownloadManager::emit_single_summary 和 emit_progress 通过 EventBus::publish 发送。Progress 事件在周期性 persist 路径有 500ms 节流（每 500ms 最多发一次），终态立即发送。
- 崩溃恢复：重启时 load_downloads_from_db() 读取 SQLite → 重建 ManagedDownload → 最后持久化的 chunk 状态续传。未刷盘的缓冲数据丢失。
- 错误恢复：瞬态故障指数退避重试（max_retries 限制）；checksum 不匹配仅重下受影响 chunk（非整个文件）。
- 磁盘空间检查在 Phase 2 进行，预留 10% buffer。
- RateLimiter 批量 consume：累计 ~256KB 或 8 chunk（取先到）才调一次，不逐 chunk 消费。AIMD 采样窗口 2s 不受影响。
- 序列化：所有数据传输类型用 camelCase（Rust `#[serde(rename_all = "camelCase")]`），enum variant 用 snake_case。
- Subsystem 交互：Phase 1 (commands/protocol)、Phase 2 (manager/http_executor/manifest/database/file_ops)、Phase 3 (http_executor/buffer_pool/database/rate_limiter)、Phase 4 (scheduler/aimd/rate_limiter)、Phase 5 (http_executor/buffer_pool/checksum/database/retry/file_ops)。
