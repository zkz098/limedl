# Subsystem: FileOps

## 模块职责

文件系统操作基础设施层，合并了文件创建/预分配/写入/最终化（原 file_alloc）和磁盘类型检测（disk_detect）两个模块。提供跨平台的下载文件创建、分块写入、预分配、原子最终化、磁盘空间检查、磁盘类型检测功能。

核心类型：DiskType 枚举（Ssd / Hdd，定义在 types.rs）。非 Windows 平台默认返回 Ssd。

## 涉及文件

- `crates/limedl-core/src/file_ops/mod.rs` — 文件操作 + 磁盘检测完整实现

## 数据流向

```
下载开始（Phase 2 探测后）
  ↓
resolve_disk_type(dir)
  ├─ 检查磁盘类型覆盖（settings.io_baseline.disk_type_overrides）
  └─ detect_disk_type(dir) → DiskType::Ssd / Hdd
       ↓ 决定 BufferPool 模式

下载文件创建 → open_download_file(path, total_size)
  ├─ 创建父目录 → 打开文件 → 预分配空间（file.allocate 或 file.set_len 回退）
  └─ check_disk_space(dir, total) 验证空间充足（预留 10% buffer）

下载数据写入 → buffer_pool → write_all_at(file, buffer, offset)

下载最终化（Phase 5） → finalize_temp_file(temp_path, destination_path)
  ├─ 主路径：原子 rename（同文件系统）
  └─ 回退：跨设备复制（CrossesDevices）→ staging path → hard_link
```

## 设计决策与约定

- 文件预分配使用 `file.allocate()`，若 OS 不支持（errno 38/45/95/524）则回退 `file.set_len()`。
- write_all_at 是 `pub(super)` 可见性——仅 buffer_pool 和 manager 内部使用。
- finalize_temp_file 处理同名文件冲突：内容相同则接受（幂等重试），不同则报 AlreadyExists。
- 跨设备复制使用 256 KB 栈分配缓冲区（vs stdlib 默认 8 KB），提升性能。
- 磁盘检测失败（权限不足等）静默回退为 DiskType::Ssd。
- check_disk_space 在下载开始前调用（Phase 2），预留 10% buffer 防止下载过程中空间耗尽。
- 磁盘类型通过 Win32 `CreateFileW(\\.\C:)` + `DeviceIoControl(IOCTL_STORAGE_QUERY_PROPERTY)` 检测 `STORAGE_DEVICE_SEEK_PENALTY_PROPERTY`。
