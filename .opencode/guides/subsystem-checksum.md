# Subsystem: Checksum

## 模块职责

文件校验和计算，支持 Blake3、SHA-256、XXH3-128 三种哈希算法。提供流式哈希器和完整的文件校验方法。在下载最终化阶段（Phase 5）被调用，验证下载文件完整性。

核心类型：ChecksumHasher（封装三种哈希实现的枚举）、ChecksumMode 枚举（None / Blake3 / Sha256 / Xxh3128，定义在 types.rs）。

## 涉及文件

- `crates/limedl-core/src/checksum/mod.rs` — ChecksumHasher + calculate_checksum + hash_slices

## 数据流向

```
下载最终化（Phase 5）
  ↓
finalize_download() → calculate_checksum(temp_path, mode)
  └─ spawn_blocking → 打开文件 → ChecksumHasher::new + update 循环（1 MiB 缓冲区）→ finalize()
  ↓
与期望 checksum 比较 → 匹配则继续，不匹配则触发分块重试
```

## 设计决策与约定

- checksum_mode 为 None 时不调用 ChecksumHasher::new（会返回 Err）。
- 文件校验使用 `spawn_blocking` 避免阻塞 tokio 运行时。
- 各算法的 hex 输出格式：Blake3 使用内置 `to_hex()`，SHA-256 使用 `format!("{:x}")`，XXH3-128 使用 `format!("{:032x}")`。
- hash_slices 是同步函数，用于内存缓冲场景的快速校验（如 chunk 数据预校验）。
- 与 Database 和 Settings 子系统通过 ChecksumMode 枚举耦合（类型定义在 types.rs）。
