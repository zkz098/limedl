# Subsystem: Buffer Pool & FileOps

## 模块职责

管理磁盘 I/O 写缓冲（针对 HDD/SSD 差异化策略）+ 底层文件系统操作（创建、预分配、写入、最终化、磁盘空间检查、磁盘类型检测）。

### BufferPool

HDD 使用全局共享双缓冲池减少磁盘寻道，SSD 使用本地写合并（无需全局池）。配合游戏模式动态缩减内存占用。

核心类型：BufferPool（含 semaphore、内存追踪、游戏模式标志）、DownloadBuffer（per-task 写缓冲）、SlotGuard（RAII，drop 归还信号量）、IoWorker（专用 I/O 线程，通过 mpsc::unbounded_channel 串行化所有 flush 请求）。

### FileOps

跨平台文件系统操作基础设施：open_download_file（创建+预分配）、write_all_at（分块写入）、finalize_temp_file（原子最终化，跨设备回退）、check_disk_space（空间验证）、resolve_disk_type（磁盘类型检测）。

核心类型：DiskType 枚举（Ssd / Hdd，定义在 types.rs）。非 Windows 默认 Ssd。

## 涉及文件

- `crates/limedl-core/src/buffer_pool.rs` — BufferPool + IoWorker + DownloadBuffer + SlotGuard
- `crates/limedl-core/src/file_ops/mod.rs` — 文件操作 + 磁盘检测完整实现

## 数据流向

```
下载开始 → resolve_disk_type(destination_dir)
  ├─ 检查 disk_type_overrides（settings）
  └─ detect_disk_type(dir) → DiskType::Ssd / Hdd (Win32 IOCTL_STORAGE_QUERY_PROPERTY)
       ↓ 决定 BufferPool 模式

下载文件创建 → open_download_file(path, total_size)
  ├─ 创建父目录 → 打开文件
  ├─ 预分配空间（file.allocate → set_len 回退 on errno 38/45/95/524）
  └─ check_disk_space(dir, total) + 10% buffer

Worker 下载数据块 → buffer_chunk(offset, data)
  ├─ [HDD] 写入当前活跃半缓冲 → 半缓冲满 OR 2s 定时器 → 切换
  │      → IoWorker::write_batch() 异步刷盘
  └─ [SSD] 写入本地 BTreeMap → 缓冲满 → IoWorker::write_batch()

取消/暂停 → flush_all() → 等待后台 flush 完成并持久化已计费数据（drain_background 仅测试辅助）
下载完成 → flush_all() → SlotGuard drop → 归还池槽位

最终化 → finalize_temp_file(temp_path, destination_path)
  ├─ 主路径：原子 rename（同文件系统）
  └─ 回退：跨设备复制（CrossesDevices）→ staging path → hard_link（256KB 缓冲）
```

## 设计决策与约定

### BufferPool

- HDD 半缓冲大小 = effective_limit / effective_max_parallel / 2，最小 64 KiB。
- 游戏模式仅影响 HDD 池（缩减内存和并发），SSD 不受影响。
- 所有 flush 提交到专用 IoWorker 线程（单线程串行化），`spawn_blocking` 仅作 fallback。
- HDD 双缓冲内用 `Mutex<BTreeMap<u64, Bytes>>`（替代 DashMap），BTreeMap 天然按 offset 升序，移除了 sort_by_key 调用。
- Crash recovery 不依赖缓冲池状态——仅从 SQLite 已持久化 chunks 恢复。

### FileOps

- 文件预分配优先 `file.allocate()`，OS 不支持时回退 `file.set_len()`。
- `write_all_at` 为 `pub(super)` 可见——仅 buffer_pool 和 manager 使用。
- 同名文件冲突：内容相同接受（幂等重试），不同报 AlreadyExists。
- 跨设备复制使用 256KB 栈分配缓冲区（vs stdlib 默认 8KB）。
- 磁盘检测失败静默回退 DiskType::Ssd。
- Windows 磁盘检测通过 `CreateFileW(\\.\C:)` + `DeviceIoControl(IOCTL_STORAGE_QUERY_PROPERTY)` + `STORAGE_DEVICE_SEEK_PENALTY_PROPERTY`。
