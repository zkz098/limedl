# Subsystem: SQLite Persistence (Database)

## 模块职责

使用 rusqlite（bundled SQLite）持久化下载任务的状态、分块进度和元数据。数据库文件位于 state_dir 下。

核心类型：Database（双连接：`write_conn` + `read_conn`，均 `Arc<Mutex<Connection>>`）、Manifest（下载任务完整元数据）、ChunkManifest（分块状态）。

## 涉及文件

- `crates/limedl-core/src/database.rs` — Database 结构体、CRUD 方法、建表/迁移、PRAGMA 配置
- `crates/limedl-core/src/manifest.rs` — Manifest / ChunkManifest 类型定义
- `crates/limedl-core/src/migration.rs` — 旧 JSON 文件 → SQLite 迁移逻辑（新安装不触发）
- `crates/limedl-core/src/persistence.rs` — 从 SQLite 加载下载任务到内存（`load_downloads_from_db`）

## 数据流向

```
应用启动 → Database::open(state_dir / "downloads.db")
  ├─ write_conn：PRAGMA journal_mode=WAL, wal_autocheckpoint=4096,
  │               foreign_keys=ON, busy_timeout=5000, synchronous=NORMAL,
  │               cache_size=-32000
  ├─ read_conn：PRAGMA query_only=1, busy_timeout=5000
  └─ create_tables() → 幂等建表 + ALTER TABLE 迁移

下载创建（Phase 2） → Database::insert_download(manifest)
  └─ INSERT INTO downloads + INSERT INTO chunks

下载进行中（Phase 3） → Database::update_download_progress()
  └─ UPDATE downloads + UPDATE chunks

崩溃恢复 → Database::list_download_headers() + load_chunks()
  └─ 重建 ManagedDownload，从最后持久化的 chunk 状态恢复

删除任务 → Database::delete_download(id) → ON DELETE CASCADE 自动删除 chunks
```

## 设计决策与约定

- 双连接设计：WAL 模式下 reader 不被 writer 阻塞，实现读写并发。`read_conn` 设 `query_only=1` 防止意外修改。
- `open_in_memory()`（测试用）让 write_conn 和 read_conn 共享同一 `Arc<Mutex<Connection>>`——in-memory SQLite 不允许多个 `:memory:` 连接。**不启 WAL**，否则导致 "database is locked"。
- 写方法 `lock_write()`，读方法 `lock_read()`，所有 DB 操作同步阻塞，由 tokio 的 `spawn_blocking` 包装。
- `prepare_cached` 在两个连接上各自独立缓存 prepared statement，不能跨连接共享。
- 枚举值以 snake_case 字符串存储（`"downloading"`、`"blake3"` 等）。
- Migration 机制：用 `PRAGMA table_info` 检测列是否存在，按需 `ALTER TABLE` 添加。版本号通过 `PRAGMA user_version` 维护。
- Manifest 的 Serialize/Deserialize 用于 JSON 序列化（aria2 RPC 响应），与 SQLite 列存储是两套映射。
- 辅助函数 `insert_manifest_row` / `update_manifest_row` 使用 `prepare_cached` + `named_params!` 直接传引用，零 String 克隆。
