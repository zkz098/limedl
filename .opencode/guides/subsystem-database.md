# Subsystem: SQLite Persistence (Database)

## 模块职责

使用 rusqlite (bundled SQLite) 持久化下载任务的状态、分块进度和元数据。取代了早期的 JSON 文件方案。数据库文件位于 state_dir 下，通过 `Database` 结构体提供 CRUD 操作。

**涉及文件**：

- `crates/flareget-core/src/database.rs` (1499 行) — 数据库层完整实现
- `crates/flareget-core/src/migration.rs` (231 行) — JSON → SQLite 迁移逻辑
- `crates/flareget-core/src/persistence.rs` (154 行) — 从 DB 加载下载任务到内存

## 关键结构体

### Database (pub(crate))

```rust
pub(crate) struct Database {
    conn: Mutex<Connection>,  // rusqlite::Connection，Mutex 保护并发访问
}
```

### Manifest (pub(crate)) — 主要持久化单元

```rust
// 存储在 downloads 表中，对应下载任务的完整元数据
pub(crate) struct Manifest {
    pub id: String,
    pub url: String,
    pub final_url: String,
    pub user_agent: String,
    pub destination_dir: String,
    pub file_name: String,
    pub file_name_locked: bool,
    pub destination_path: String,
    pub temp_path: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub supports_ranges: bool,
    pub connection_count: usize,
    pub thread_mode: ThreadMode,
    pub requested_thread_count: Option<usize>,
    pub desired_thread_count: Option<usize>,
    pub allocated_thread_count: Option<usize>,
    pub adaptive_profile_snapshot: Option<AdaptiveProfile>,
    pub thread_note: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub state: DownloadState,
    pub checksum_mode: ChecksumMode,
    pub checksum: Option<String>,
    pub error: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub chunk_size: u64,
    pub cdn_accelerated: bool,
    pub mirror_url: Option<String>,
    pub mirror_urls: Vec<String>,
    pub current_mirror_index: usize,
    pub chunks: Vec<ChunkManifest>,
}
```

## 关键方法

### Database

```rust
pub(crate) fn open(path: &Path) -> Result<Self>
#[cfg(test)]
pub(crate) fn open_in_memory() -> Result<Self>   // 测试用内存数据库

// 创建表结构（幂等，含 ALTER TABLE 迁移）
pub(crate) fn create_tables(&self) -> Result<()>

// 写入
pub(crate) fn insert_download(&self, manifest: &Manifest) -> Result<()>
pub(crate) fn update_download_progress(
    &self, id: &str, downloaded_bytes: u64, dirty_chunks: &[ChunkManifest],
    state: &str, updated_at_ms: u64,
) -> Result<()>

// 读取
pub(crate) fn get_download(&self, id: &str) -> Result<Option<Manifest>>
pub(crate) fn list_downloads(&self) -> Result<Vec<Manifest>>
pub(crate) fn list_download_headers(&self) -> Result<Vec<Manifest>>  // 不含 chunks 的轻量查询
pub(crate) fn load_chunks(&self, download_id: &str) -> Result<Vec<ChunkManifest>>

// 删除
pub(crate) fn delete_download(&self, id: &str) -> Result<()>

// 统计
pub(crate) fn count_downloads(&self) -> Result<usize>
```

### ChunkManifest — 分块状态

```rust
// 存储在 chunks 表中
pub(crate) struct ChunkManifest {
    pub index: usize,              // SQL 列名: chunk_index
    pub start: u64,                // SQL 列名: start_byte
    pub end: u64,                  // SQL 列名: end_byte
    pub downloaded: u64,
    pub completed: bool,
    pub claimed_by: Option<usize>, // worker ID
    pub dirty: bool,               // 需增量持久化标志
}
// 注意：ChunkManifest 无 download_id 字段；download_id 是 SQL 复合主键的一部分，由外层的 Manifest/Database 方法维护
```

## SQLite 表结构

### downloads 表

```sql
CREATE TABLE IF NOT EXISTS downloads (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    final_url TEXT NOT NULL DEFAULT '',
    user_agent TEXT NOT NULL,
    destination_dir TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_name_locked INTEGER NOT NULL DEFAULT 1,
    destination_path TEXT NOT NULL,
    temp_path TEXT NOT NULL,
    total_bytes INTEGER,
    downloaded_bytes INTEGER NOT NULL DEFAULT 0,
    supports_ranges INTEGER NOT NULL DEFAULT 0,
    connection_count INTEGER NOT NULL DEFAULT 0,
    thread_mode TEXT NOT NULL DEFAULT 'adaptive',
    requested_thread_count INTEGER,
    desired_thread_count INTEGER,
    allocated_thread_count INTEGER,
    adaptive_profile_snapshot TEXT,
    thread_note TEXT,
    etag TEXT,
    last_modified TEXT,
    state TEXT NOT NULL DEFAULT 'queued',
    checksum_mode TEXT NOT NULL DEFAULT 'blake3',
    checksum TEXT,
    error TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    -- 迁移列 (v2+)
    chunk_size INTEGER NOT NULL DEFAULT 4194304,
    -- 迁移列 (v3+)
    mirror_url TEXT,
    mirror_urls TEXT NOT NULL DEFAULT '[]',
    current_mirror_index INTEGER NOT NULL DEFAULT 0
);
```

### chunks 表

```sql
CREATE TABLE IF NOT EXISTS chunks (
    download_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    start_byte INTEGER NOT NULL,
    end_byte INTEGER NOT NULL,
    downloaded INTEGER NOT NULL DEFAULT 0,
    completed INTEGER NOT NULL DEFAULT 0,
    claimed_by INTEGER,
    PRIMARY KEY (download_id, chunk_index),
    FOREIGN KEY (download_id) REFERENCES downloads(id) ON DELETE CASCADE
);
```

### 索引

```sql
CREATE INDEX IF NOT EXISTS idx_downloads_state ON downloads(state);
CREATE INDEX IF NOT EXISTS idx_downloads_created ON downloads(created_at_ms);
CREATE INDEX IF NOT EXISTS idx_chunks_claimed ON chunks(download_id, claimed_by);
```

## 数据流向

```
应用启动
  ↓
DownloadManager::new() → Database::open(state_dir / "downloads.db")
  └─ Database::create_tables() → 幂等建表 + ALTER TABLE 迁移

下载创建（Phase 2）
  ↓
Database::insert_download(manifest) → INSERT INTO downloads + INSERT INTO chunks

下载进行中（Phase 3）
  ↓
每个 worker 完成一块后 → Database::update_download_progress()
  └─ UPDATE downloads SET downloaded_bytes=?, updated_at_ms=?, state=?
  └─ UPDATE chunks SET downloaded=?, completed=? WHERE (download_id, chunk_index)

退出/崩溃恢复
  ↓
DownloadManager::load_downloads_from_db()
  ├─ Database::list_download_headers() → 获取所有未完成的 Manifest
  ├─ Database::load_chunks() → 恢复分块进度
  └─ 重新创建 ManagedDownload，恢复下载

任务删除
  ↓
Database::delete_download(id) → DELETE FROM downloads WHERE id=?
  └─ ON DELETE CASCADE → 自动删除关联的 chunks
```

**重要约定**：

- `Connection` 被 `Mutex` 保护，所有 DB 操作是同步的（阻塞调用线程），由 tokio 的 `spawn_blocking` 包装
- 枚举值以 snake_case 字符串存储（`"downloading"`, `"blake3"` 等）
- `Manifest` 的 `Serialize`/`Deserialize` 用于 JSON 序列化（aria2 RPC 响应），与 SQLite 列存储是两套映射
- `migration.rs` 处理从旧 JSON 文件方案到 SQLite 的迁移，新安装不触发
- 不要直接访问 `conn`，始终通过 Database 的 pub(crate) 方法操作
