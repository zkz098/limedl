# Subsystem: FileOps

## 模块职责

文件系统操作基础设施层，合并了原 `file_alloc`（文件创建/预分配/写入/最终化）和 `disk_detect`（磁盘类型检测 Win32 IOCTL）两个模块。提供跨平台的下载文件创建、分块写入、预分配、原子最终化、磁盘空间检查、磁盘类型检测功能。

**涉及文件**：

- `crates/limedl-core/src/file_ops/mod.rs` — 文件操作 + 磁盘检测

## 关键结构体

### DiskType (pub, 定义在 types.rs)

```rust
pub enum DiskType { Ssd, Hdd }
```

由 `detect_disk_type()` 通过 Win32 `IOCTL_STORAGE_QUERY_PROPERTY` 检测。非 Windows 平台默认返回 Ssd。

## 关键方法

### 文件创建 & 预分配

```rust
pub(super) fn open_download_file(path: &Path, total_size: Option<u64>) -> Result<File>
pub(super) fn reset_download_file(file: &File, total_size: Option<u64>) -> Result<()>
```

- `open_download_file`: 创建父目录 → 打开/创建文件 → 预分配磁盘空间
- `reset_download_file`: 截断文件为 0 → 重新预分配

### 分块写入

```rust
pub(super) fn write_all_at(file: &File, buffer: &[u8], offset: u64) -> Result<()>
```

- 支持 Windows (`seek_write`) 和 Unix (`write_at`) 的定位写入
- 循环写入直到整个 buffer 消费完成

### 文件最终化

```rust
pub(super) fn finalize_temp_file(temp_path: &Path, destination_path: &Path) -> Result<()>
```

- 主路径：原子 rename（同文件系统，O(1) 元数据操作）
- 回退路径：跨设备复制（CrossesDevices）→ staging path → hard_link
- 检测目标已存在：内容相同则接受，不同则报错

### 磁盘操作

```rust
pub(super) fn check_disk_space(destination_dir: &Path, required_bytes: u64) -> Result<()>
pub(crate) fn detect_disk_type(path: &Path) -> DiskType
```

- `check_disk_space`: 检查目标目录剩余空间（required + 10% buffer）
- `detect_disk_type`: Windows `CreateFileW(\\.\C:)` + `DeviceIoControl(IOCTL_STORAGE_QUERY_PROPERTY)` 检测 `STORAGE_DEVICE_SEEK_PENALTY_PROPERTY`

## 数据流向

```
下载开始（Phase 2 探测后）
  ↓
DownloadManager::resolve_disk_type(dir)
  ├─ 检查磁盘类型覆盖（settings.io_baseline.disk_type_overrides）
  └─ detect_disk_type(dir) → DiskType::Ssd / Hdd
       ↓
  决定 BufferPool 模式（HDD 双缓冲 vs SSD 写合并）

下载文件创建
  ↓
open_download_file(path, total_size)
  ├─ 创建目录 → 打开文件 → preallocate_file(file, total_size)
  │    └─ file.allocate(total_size) 或 file.set_len(total_size) 回退
  └─ 后续 check_disk_space(dir, total) 验证空间充足

下载数据写入
  ↓
buffer_pool.rs → write_all_at(file, buffer, offset)
  └─ write_once_at(file, buffer, offset) → OS 定位写入

下载最终化（Phase 5）
  ↓
finalize_temp_file(temp_path, destination_path)
  └─ rename / 跨设备复制 → 删除 temp 文件
```

**重要约定**：

- 文件预分配使用 `file.allocate()`，若 OS 不支持（errno 38/45/95/524）则回退 `file.set_len()`
- `write_all_at` 是 `pub(super)` 而非 `pub(crate)`——仅 buffer_pool 和 manager 内部使用
- `finalize_temp_file` 处理同名文件冲突：内容相同则接受（幂等重试），不同则报 AlreadyExists
- `copy_file_buffered` 使用 256 KB 栈分配缓冲区（vs stdlib 默认 8 KB），提升跨设备复制性能
- 磁盘检测失败（权限不足等）静默回退为 `DiskType::Ssd`
- `check_disk_space` 在下载开始前调用（Phase 2），预留 10% buffer 防止下载过程中空间耗尽
