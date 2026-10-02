# Subsystem: Buffer Pool & FileOps

## 模块职责

管理磁盘 I/O 写缓冲（针对 HDD/SSD 差异化策略）+ 底层文件系统操作（创建、预分配、写入、最终化、磁盘空间检查、磁盘类型检测）。

### BufferPool

HDD 使用全局共享双缓冲池减少磁盘寻道，SSD 使用本地写合并（无需全局池）。配合游戏模式动态缩减内存占用。

核心类型：BufferPool（含 semaphore、内存追踪、游戏模式标志）、DownloadBuffer（per-task 写缓冲）、SlotGuard（RAII，drop 归还信号量）、IoWorker（专用 I/O 线程，通过 mpsc::unbounded_channel 串行化所有 flush 请求）。

### FileOps

跨平台文件系统操作基础设施：open_download_file（创建+预分配）、write_all_at（分块写入）、finalize_temp_file（原子最终化，跨设备回退）、check_disk_space（空间验证）、resolve_disk_type（磁盘类型检测）。

核心类型：DiskType 枚举（Ssd / Hdd / Network，定义在 types.rs）、MediaOverrides（`disk_type_overrides` 的规范化快照，在 file_ops/media.rs）。非 Windows 默认 Ssd。

### 介质判定（disk_detect.rs + media.rs）

- **本地卷**：Windows 走 `IOCTL_STORAGE_QUERY_PROPERTY` + `STORAGE_DEVICE_SEEK_PENALTY_PROPERTY`；Linux 走 `/sys/block/<dev>/queue/rotational`；macOS 走 IOKit `IOMedia` 的 `Rotational`。
- **`DiskType::Network`**：UNC/映射网络盘（Windows）、网络或宿主转发文件系统（Linux/macOS 查 fstype）。本地探测对它们无意义，之前会静默报成 SSD。
- **WSL（`\\wsl$\<distro>` / `\\wsl.localhost\<distro>`）不是网络存储**：它背后的 9p/virtiofs 服务的是本地 `ext4.vhdx`，所以查 `HKCU\...\Lxss` 的 `BasePath` 并对 `<BasePath>\ext4.vhdx`（不存在时退回 `BasePath` 本身）做普通本地探测，得到宿主卷的真实介质。

## 涉及文件

- `crates/limedl-core/src/buffer_pool.rs` — BufferPool + IoWorker + DownloadBuffer + SlotGuard
- `crates/limedl-core/src/file_ops/mod.rs` — 文件操作 + 磁盘检测完整实现
- `crates/limedl-core/src/file_ops/disk_detect.rs` — 三个平台的介质探测（含 WSL 注册表解析）
- `crates/limedl-core/src/file_ops/media.rs` — `disk_type_overrides` 路径匹配 + 网络文件系统分类

## 数据流向

```
下载开始 → resolve_disk_type(destination_dir)
  ├─ 检查 disk_type_overrides（settings；路径前缀匹配，最长键优先）
  └─ detect_disk_type(dir)
       ├─ WSL 传输 → 解析发行版 VHDX → 宿主卷介质
       ├─ UNC / 映射网络盘 / 网络挂载 → DiskType::Network
       └─ 本地卷 → DiskType::Ssd / Hdd
       ↓ 决定 BufferPool 模式

设置保存 → DownloadManager::apply_settings
  ├─ BufferPool::update_limits()
  └─ DiskIoService::apply_overrides() → DiskDeviceManager::set_overrides()
       ├─ 覆盖集变化时清空 resolution cache（否则旧路径永远拿旧答案）
       └─ 并清空 device queue（写线程数在构造时固定，只能重建）

下载文件创建 → open_download_file(path, total_size)
  ├─ 创建父目录 → 打开文件
  ├─ 预分配空间（file.allocate → set_len 回退 on errno 38/45/95/524）
  │    └─ 失败 → reservation_error()：卷装不下这个文件 / 盘真的满 / 其他
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
- ping-pong 翻转的不变量（`buffer_chunk_pingpong_impl`）：先把新后台 flush 的 handle 存进 `flush_handle`，再 `active_is_a.store(!is_a)`，最后把当前 chunk 插入新活动半区；顺序反了会让并发等待者看到“已翻转但无 handle”的中间态。重构时保持这个顺序与原子 Ordering 不变。
- HDD 双缓冲内用 `Mutex<BTreeMap<u64, Bytes>>`（替代 DashMap），BTreeMap 天然按 offset 升序，移除了 sort_by_key 调用。
- Crash recovery 不依赖缓冲池状态——仅从 SQLite 已持久化 chunks 恢复。

### FileOps

- 文件预分配优先 `file.allocate()`，OS 不支持时回退 `file.set_len()`。
- 预分配失败必须**翻译**，不能透出裸 OS 错误码：`check_disk_space` 只比剩余空间，所以「空间够但单文件上限不够」（FAT32 的 4 GiB、FAT16 的 2 GiB）只会在预分配这一步暴露。`reservation_error()` 把 `ErrorKind::FileTooLarge`（EFBIG / ERROR_FILE_TOO_LARGE）判为 `DownloadError::FileTooLarge`，并在「报 ENOSPC / ERROR_DISK_FULL 但卷明明放得下整个文件」时给出同一结论（部分卷就是这样回答超限预留的）；空间不足仍是 `InsufficientDiskSpace`，其余错误保持原始 io 错误——**不猜文件系统**：按文件大小猜出来的 FAT32 提示曾经在 NTFS/ext4/APFS 上刷屏，那就是这次修掉的 bug。
- `preallocate_file()` 返回原始 `io::Result`：只有调用方知道目标目录，因此只有它能拿到可用空间做上述判定。`reset_download_file()` 手上只有 `File`（拿不到路径），只按 `ErrorKind` 判定。
- `write_all_at` 为 `pub(super)` 可见——仅 buffer_pool 和 manager 使用。
- 同名文件冲突：内容相同接受（幂等重试），不同报 AlreadyExists。
- 跨设备复制使用 256KB 栈分配缓冲区（vs stdlib 默认 8KB）。
- 磁盘检测失败静默回退 `DiskType::Ssd`（未知 ≠ 远程，见下）。
- Windows 磁盘检测通过 `CreateFileW(\\.\C:)` + `DeviceIoControl(IOCTL_STORAGE_QUERY_PROPERTY)` + `STORAGE_DEVICE_SEEK_PENALTY_PROPERTY`。

### 介质覆盖与远程判定

- `disk_type_overrides` 的键是用户填的**目录**，查询值是下载目标（通常是其子孙），所以查找是
  **规范化路径的组件边界前缀匹配**，不是 `HashMap::get`：`D:\downloads` 命中 `D:\downloads\a.bin`，
  但不命中 `D:\downloads-old`；嵌套规则最长键优先。规范化规则（大小写、分隔符、尾部分隔符、
  `\\?\` 前缀）在 `media.rs::normalize_media_path`，是键与查询共用的唯一实现。
- 覆盖只在**两处**生效，缺一不可：`DiskIoService::resolve_disk_type`（决定缓冲模式，每次下载开始读）
  与 `DeviceTopology::resolve_device`（决定 `DeviceQueue` 的写线程数，每设备构造一次）。
- `DeviceQueue` 的通道数在构造时确定（HDD 1 条串行、SSD/Network 4 条并行），所以覆盖集变化时
  `DiskDeviceManager::set_overrides` 会丢弃整个 queue 表：在途写请求持有旧 `Arc` 并正常完成，
  下一次查找按新策略建队列。**不这样做就等于要重启才能生效**（历史 bug）。
- `DiskType::Network` 按 SSD 方式调度（写合并 + 多通道）：瓶颈在传输而非寻道，远端本来就会重排写入。
  它存在的意义是**可见**（设置面板会列出网络盘与 WSL 发行版），以及让「被误判成 SSD」这件事
  可以被 `disk_type_overrides` 修掉 —— 这才是这条路径真正的用途。
