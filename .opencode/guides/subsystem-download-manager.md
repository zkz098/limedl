# Subsystem: DownloadManager

## 模块职责

HTTP 下载的完整生命周期编排：接收下载请求 → 探测远程文件元数据 → 规划分块策略 → 执行单流或多流下载 → 全局限速 → 自适应调速（AIMD） → 缓冲/刷盘 → 校验和验证 → 最终化文件。

核心类型：DownloadManager（含 4 个子结构体 ConcurrencyLimits / RuntimeControls / HttpClientInfra / StateDirs + 3 个 ZST actor：HttpExecutor / Scheduler / TaskLifecycle）。ManagedDownload 为每个下载任务的核心包装。

## 涉及文件

- `crates/limedl-core/src/manager.rs` — DownloadManager 主结构体、CRUD 方法
- `crates/limedl-core/src/http_executor.rs` — HTTP 探测、单流/多流执行
- `crates/limedl-core/src/scheduler.rs` — 后台调度循环 + AIMD 线程重分配
- `crates/limedl-core/src/task_lifecycle.rs` — 状态转换、文件清理、进度记录、事件发射
- `crates/limedl-core/src/aimd.rs` — AIMD 吞吐量状态机
- `crates/limedl-core/src/manifest.rs` — Manifest / ChunkManifest
- `crates/limedl-core/src/retry.rs` — 指数退避重试
- `crates/limedl-core/src/checksum/mod.rs` — 校验和（Blake3 / SHA-256 / XXH3-128）
- `crates/limedl-core/src/rate_limiter/mod.rs` — 全局令牌桶速率限制器
- 前端入口：`src/lib/tauri/download-api.ts` → `src-tauri/src/download/commands.rs`

## 数据流向

### Phase 1 — 前端 → 后端

```
用户提交下载 → StartDownloadRequest (JSON, camelCase)
  → download-api.ts → invoke("download_start")
  → commands.rs → classify_kind() → BackendRegistry.by_kind()
  → HTTP URL → DownloadManager::start() → TaskId::Http(uuid)
```

### Phase 2 — 探测 & 规划

```
DownloadManager::start() → 创建 ManagedDownload → spawn run_download()
  → HttpExecutor::probe(url) → HEAD 请求（或 GET Range:0-0 回退）
  → RemoteMetadata（total_bytes, supports_ranges, etag, file_name）
  → 构建 Manifest + chunk plan
  → Database::insert_download()
  → resolve_disk_type() → DiskType::Ssd / Hdd
  → check_disk_space() 验证空间（预留 10% buffer）
  → open_download_file() + 预分配空间（file.allocate / set_len 回退）
```

### Phase 3 — 下载执行

```
[多流] supports_ranges && total_bytes > threshold:
  → download_chunked() → JoinSet<ChunkWorkerOutcome>
  → 每个 worker: claim_next_chunk() → HTTP Range 请求
  → DownloadBuffer::buffer_chunk(offset, data)
    ├─ HDD: 全局双缓冲池 → IoWorker 异步刷盘
    └─ SSD: 本地写合并 → 同步/异步刷盘
  → RateLimiter::consume()（累计 ~256KB 或 8 chunk 消费一次）

[单流] 不支持 Range 或小文件:
  → download_single() → 顺序 GET + Range: bytes=downloaded- 续传
```

### Phase 4 — 调度 & AIMD 自适应

```
Scheduler 后台循环（SCHEDULER_TICK = 2s）:
  → update_adaptive_targets(): AIMD 采样吞吐 → 增减期望线程数
  → rebalance_allocations(): 重分配线程预算
    ├─ Traditional: FIFO + max_parallel_tasks 上限
    └─ Automatic: 按剩余字节排序（大文件优先），分配 max_parallel_threads
```

### Phase 5 — 最终化

```
所有 chunk 完成 → finalize_download()
  → DownloadBuffer::drain_background() + flush_all()
  → calculate_checksum() → Blake3 / SHA-256 / XXH3-128（spawn_blocking, 1MiB 缓冲）
  → 校验和不匹配 → 仅重下受影响 chunk（非整个文件）
  → finalize_temp_file(): 原子 rename → 跨设备回退到 hard_link（256KB 缓冲区复制）
  → Database: state=completed
  → EventBus::publish(Updated) → app_handle.emit("download-updated")
```

## 设计决策与约定

### 架构

- DownloadManager 拆分为 3 个 ZST actor（HttpExecutor / Scheduler / TaskLifecycle），不持有 DownloadManager 引用，通过方法参数接收，避免循环引用。
- CRUD 方法（start/pause/resume/cancel/remove/purge）委托 actor 完成具体工作。
- 所有事件通过 EventBus::publish() 统一发布，前端发射由 lib.rs 的独立订阅任务完成。
- AIMD 状态变更在 scheduler.rs 中，worker 只报告下载字节数。

### 校验和

- ChecksumHasher 枚举封装三种算法（Blake3 / SHA256 / XXH3-128）。mode 为 None 时不调用（直接返回 Err）。
- 使用 `spawn_blocking` 避免阻塞 tokio 运行时。
- 输出格式：Blake3 用 `to_hex()`，SHA-256 用 `format!("{:x}")`，XXH3-128 用 `format!("{:032x}")`。
- `hash_slices()` 为同步函数，用于内存缓冲场景的快速校验。

### 速率限制

- RateLimiter 是全局令牌桶（`Arc<Mutex<Inner>>`），速率 0 = 无限制。令牌桶容量 = max(2 × rate, 1)。
- HTTP chunk worker 累积 ~256KB 或 8 chunk（取先到）才调一次 `consume()`，不逐 chunk 消费。
- `consume()` 异步（tokio::time::sleep），`consume_blocking()` 同步（std::thread::sleep）。
- 锁仅用于简短算术操作，不跨 await 点持有。
- AIMD 采样窗口 2s 不受批量消费影响。

### 崩溃恢复 & 错误处理

- 重启时 `load_downloads_from_db()` 从 SQLite 重建 ManagedDownload，最后持久化的 chunk 状态续传。
- 瞬态故障指数退避重试（max_retries 限制）；checksum 不匹配仅重下受影响 chunk。
- 磁盘空间检查在 Phase 2 进行，预留 10% buffer。
- Progress 事件在周期性 persist 路径有 500ms 节流（终态立即发送）。

### GitHub 镜像重写 (`mirror.rs`)

在受限网络环境中，将 GitHub 下载 URL 重写为配置的镜像地址（如 ghproxy、mirror.gh）。

**公开 API**：

| 函数                       | 作用                                                                              |
| -------------------------- | --------------------------------------------------------------------------------- |
| `is_github_url(url)`       | 判定 URL 是否为 GitHub（`github.com` 或子域名）                                   |
| `active_mirrors(settings)` | 返回已启用、非空的镜像列表（按 `order` 排序）                                     |
| `rewrite(url, settings)`   | 生成尝试 URL 列表：`{mirror_base}/{url_encoded_original}` + 原始 URL 作为最后回退 |

- 镜像未启用或 URL 非 GitHub → 返回单元素向量（仅原始 URL）。
- 镜像为空 → 同上。
- URL 使用 `url_encode()` 编码后拼接到镜像 base URL 后（base 末尾斜杠自动去重）。
- 原始 URL 始终附加在列表末尾，作为最终回退。
- `GitHubMirrorSettings` 和 `MirrorEntry` 类型定义在 `types.rs`，settings 由 `settings.rs` 管理。
