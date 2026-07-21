# Subsystem: Buffer Pool

## 模块职责

管理磁盘 I/O 写缓冲，针对 HDD 和 SSD 采用不同策略以最大化写入吞吐量。HDD 使用全局共享的双缓冲池（减少磁盘寻道），SSD 使用本地写合并（无需全局池）。同时配合游戏模式动态缩减内存占用。

核心类型：BufferPool（全局共享的池，含 semaphore、内存用量追踪、游戏模式标志）、DownloadBuffer（每个下载任务独立的写缓冲，按 DiskType 分 HDD 双缓冲 / SSD 本地缓冲模式）、SlotGuard（RAII 守卫，drop 时自动归还信号量许可）、IoWorker（专用 I/O worker 线程，通过 mpsc::unbounded_channel 串行化所有文件 flush 请求）。

DiskType 枚举（Ssd / Hdd）定义在 types.rs，由 file_ops/mod.rs 中通过 Win32 IOCTL_STORAGE_QUERY_PROPERTY 检测。

## 涉及文件

- `crates/limedl-core/src/buffer_pool.rs` — BufferPool + IoWorker + DownloadBuffer + SlotGuard
- `crates/limedl-core/src/file_ops/mod.rs`（disk_detect 部分）

## 数据流向

```
下载开始 → resolve_disk_type(destination_dir)
  ├─ HDD → acquire_slot(pool) → DownloadBuffer::new_with_worker(...)
  └─ SSD → DownloadBuffer::new_local_with_worker(...)

Worker 下载数据块 → buffer_chunk(offset, data)
  ├─ [HDD 模式] 写入当前活跃半缓冲 → 半缓冲满 OR 2s 定时器 → 切换半缓冲
  │      → IoWorker::write_batch() 异步刷盘
  └─ [SSD 模式] 写入本地 BTreeMap → 缓冲满 → IoWorker::write_batch()

取消/暂停 → drain_background() → 等待后台 flush 完成
下载完成 → flush_all() → SlotGuard drop → 归还池槽位
```

## 设计决策与约定

- HDD 是全局稀缺资源，slot semaphore 会导致新下载任务排队等待。半缓冲大小 = effective_limit / effective_max_parallel / 2，最小 64 KiB。
- 游戏模式仅影响 HDD 池（缩减内存和并发），SSD 不受影响。
- 所有文件 flush 操作提交到专用 IoWorker 线程（`IoWorker::spawn()` 在 `DownloadManager::new()` 时调用一次）。单 worker 线程串行化所有文件 flush，BufferPool 的 ping-pong 提供天然背压。`spawn_blocking` 保留为 fallback 路径（单元测试 / 无 IoWorker 上下文）。
- HDD 双缓冲内部使用 `Mutex<BTreeMap<u64, Bytes>>` 替代原来的 DashMap，BTreeMap 的 `into_iter()` 天然按 offset 升序，移除了 4 处 `sort_by_key` 调用。dashmap crate 在 bt_backend_own/ 中仍保留使用。
- crash recovery 不依赖缓冲池状态——只从 SQLite 中已持久化的 chunks 恢复进度。
- buffer_chunk 返回 Err 说明后台 flush 失败（磁盘满、权限等）。
