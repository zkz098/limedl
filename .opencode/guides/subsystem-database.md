# Subsystem: SQLite Persistence (Database)

## 模块职责

使用 rusqlite（bundled SQLite）持久化下载任务的状态、分块进度和元数据。数据库文件位于 state_dir 下。

核心类型：Database（双连接：`write_conn` + `read_conn`，均 `Arc<Mutex<Connection>>`）、Manifest（下载任务完整元数据）、ChunkManifest（分块状态）。

## 涉及文件

- `crates/limedl-core/src/database/connection.rs` — Database 结构体、连接/PRAGMA/迁移入口
- `crates/limedl-core/src/database/schema.rs` — `CREATE_TABLES_SQL` + `MIGRATIONS`（版本化迁移列表）
- `crates/limedl-core/src/database/manifest_repo.rs` — downloads / chunks CRUD
- `crates/limedl-core/src/database/bt_task_repo.rs` — BT 任务索引（`bt_tasks`）读写，见 `subsystem-bt-backend.md`
- `crates/limedl-core/src/manifest.rs` — Manifest / ChunkManifest 类型定义
- `crates/limedl-core/src/migration/mod.rs` — 旧 JSON 文件 → SQLite 迁移逻辑（新安装不触发）
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
  └─ 重建 ManagedDownload，从最后持久化的 chunk 状态恢复；加载时清空 chunk 的
     `claimed_by`（残留 claim 属于已消失的进程，见 persistence.rs）

RPC 历史回查 → get_download_header(id) / list_download_headers() / count_terminal_downloads()
  └─ 终态任务被 max_in_memory_downloads 淘汰后，aria2 RPC 仍可查询/列表/计数，
     直到 purgeDownloadResult 或 removeDownloadResult 删除数据库行

删除任务 → Database::delete_download(id) → ON DELETE CASCADE 自动删除 chunks

BT 任务索引 → Database::replace_bt_tasks(summaries) / list_bt_tasks() / delete_bt_task(id)
  └─ bt_tasks(id, summary_json, created_at_ms)：轻量 BT 模式在引擎卸载时供 UI 读的任务缓存
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
- `bt_tasks` 是**缓存表**：`summary_json` 存序列化的 `DownloadSummary`（新增字段不需要迁移），整表替换（单事务）保证与引擎视图一致；损坏的 JSON 行读取时跳过并 warn，不会连累其余行。
