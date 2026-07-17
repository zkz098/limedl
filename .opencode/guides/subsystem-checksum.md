# Subsystem: Checksum

## 模块职责

文件校验和计算，支持 Blake3、SHA-256、XXH3-128 三种哈希算法。提供流式哈希器（`ChecksumHasher`）和完整的文件校验方法（`calculate_checksum`）。在下载最终化阶段（Phase 5）被调用，验证下载文件完整性。

**涉及文件**：
- `src-tauri/src/download/checksum/mod.rs` (104 行) — ChecksumHasher + calculate_checksum

## 关键结构体

### ChecksumHasher (pub(crate))
流式哈希器枚举，封装三种哈希实现：
```rust
pub(crate) enum ChecksumHasher {
    Blake3(Box<blake3::Hasher>),
    Sha256(sha2::Sha256),
    Xxh3_128(Box<xxhash_rust::xxh3::Xxh3>),
}
```

### ChecksumMode (pub, 定义在 types.rs)
```rust
pub enum ChecksumMode { None, Blake3, Sha256, Xxh3128 }
```

## 关键方法

### ChecksumHasher
```rust
impl ChecksumHasher {
    pub(crate) fn new(mode: ChecksumMode) -> Result<Self>
    pub(crate) fn update(&mut self, bytes: &[u8])
    pub(crate) fn finalize(self) -> String
}
```

### 自由函数
```rust
// 从有序字节切片计算校验和（内存缓冲场景）
pub fn hash_slices(mode: ChecksumMode, slices: &[&[u8]]) -> String

// 异步计算文件校验和（spawn_blocking IO）
pub(crate) async fn calculate_checksum(path: PathBuf, mode: ChecksumMode) -> Result<String>
```

## 数据流向

```
下载最终化（Phase 5）
  ↓
http_executor::finalize_download()
  ├─ 调用 calculate_checksum(temp_path, mode)
  │    └─ spawn_blocking → 打开文件 → ChecksumHasher::new + update 循环（1 MiB 缓冲区）→ finalize()
  └─ 与期望 checksum 比较 → 匹配则继续，不匹配则触发分块重试
```

**重要约定**：
- `checksum_mode: None` 时不应调用 `ChecksumHasher::new()` —— 会返回 `Err`
- 文件校验使用 `spawn_blocking` 避免阻塞 tokio 运行时
- Blake3 输出 hex 使用 `blake3::Hash::to_hex()`，SHA-256 使用 `format!("{:x}")`，XXH3-128 使用 `format!("{:032x}")`
- `hash_slices` 是同步函数且 `#[allow(dead_code)]`，用于内存缓冲场景的快速校验（如 chunk 数据预校验）
- 与 `Database` 和 `Settings` 子系统通过 `ChecksumMode` 枚举耦合（类型定义在 types.rs）
