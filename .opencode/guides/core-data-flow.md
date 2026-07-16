# Core Data Flow: HTTP Download

> Full path from user clicking "Start Download" to file written on disk. Crosses 6 subsystems. Read this before modifying any download-related code.

## Overview

```
用户点击下载
  ↓
DownloadComposer.vue → download-api.ts → invoke("download_start")
  ↓
commands::download_start() → TaskId::parse() 按前缀路由
  ↓ (http: 前缀)
DownloadManager::start()
  ├─ 创建 ManagedDownload { core, aimd, runtime }
  ├─ 插入 downloads HashMap
  └─ spawn run_download()
       ├─ probe(url) → RemoteMetadata
       ├─ 构建 Manifest + chunk plan
       ├─ Database::insert_download()
       ├─ [支持 Range] download_chunked()
       │    └─ JoinSet workers
       │         ├─ claim_next_chunk()
       │         ├─ HTTP Range → DownloadBuffer::buffer_chunk()
       │         └─ Database::save_chunk()
       └─ finalize_download()
            ├─ DownloadBuffer::flush_all()
            ├─ calculate_checksum()
            ├─ 重命名 .part → final
            └─ 更新 SQLite + emit "download-updated"
```

## Phase 1 — Frontend → Backend

1. User fills form in `DownloadComposer.vue` → emits `StartDownloadRequest` (JSON, camelCase fields)
2. `src/lib/tauri/download-api.ts` calls `invoke("download_start", request)` → Tauri IPC bridge
3. `commands::download_start()` (Rust) dispatches via `TaskId::parse()`:
   - `http:` prefix → `DownloadManager::start()`
   - `bt:` prefix → `OwnBtBackend::start()`

**Key types**: `StartDownloadRequest { url, destination_dir, file_name?, thread_mode?, thread_count?, max_retries?, checksum?, start_paused?, mirror_urls? }`

## Phase 2 — Probe & Plan

4. `DownloadManager::start()`: creates `ManagedDownload { core: DownloadCore { snapshot, manifest }, aimd, runtime }` → inserts into `downloads: HashMap<String, Arc<ManagedDownload>>` → spawns `run_download()` via `tauri::async_runtime::spawn`
5. `http_executor::run_download()`: calls `probe(url, user_agent)` (HEAD request, falls back to GET Range: 0-0)
   - Returns `RemoteMetadata { total_bytes: Option<u64>, supports_ranges: bool, etag: Option<String>, file_name: Option<String>, final_url: String }`
6. Builds `Manifest` with chunk plan: 1 chunk for single-threaded, N chunks for multi-segment (chunk size from settings or adaptive)
7. Inserts manifest + chunks into SQLite via `Database::insert_download()`
8. Resolves disk type via `disk_detect.rs` (Win32 `IOCTL_STORAGE_QUERY_PROPERTY`) → `DiskType::Ssd` or `DiskType::Hdd`

**Decision point**: `supports_parallelism(total_bytes, supports_ranges, chunk_size)` determines single vs multi-segment.

## Phase 3 — Download Execution

### Multi-segment (supports_ranges && total_bytes > threshold)
9. Spawns `download_chunked()` → creates `JoinSet<ChunkWorkerOutcome>`
10. For each worker thread (up to `allocated_thread_count`):
    - Calls `claim_next_chunk(manifest, worker_id)` → marks chunk as claimed → gets `ChunkManifest`
    - Sends HTTP `Range: bytes=start-end` request → streams response body
    - Writes data to `DownloadBuffer::buffer_chunk(offset, data)`:
      - **HDD**: pool-backed double-buffer → `BufferPool::acquire_slot()` → ping-pong between two `DashMap` halves → background flush via `spawn_blocking` when half-full or every 2s
      - **SSD**: local write-combining (`DownloadBuffer::new_local()`, 4 MiB limit) → synchronous flush
    - On chunk complete: `Database::save_chunk()`, mark chunk `completed = true`
    - On chunk failure: `mark_chunk_released()` for retry by another worker
11. Worker count changes are signaled via `rebalance_notify` → the `download_chunked` main loop detects the change and calls `shutdown_chunk_workers()` to terminate excess workers

### Single-stream (no Range support or small file)
12. `download_single()`: sequential GET request with `Range: bytes=downloaded-` header for resumption
13. Data flows directly to `DownloadBuffer` → periodic flush → `Database::update_download_progress()`

**Global rate limiting**: `RateLimiter` (token bucket) enforces `global_speed_limit_bps` across all downloads. Workers check rate limiter before each read.

## Phase 4 — Scheduling & Adaptation

14. `scheduler.rs` background loop (`SCHEDULER_TICK = 2s`):
    - `update_adaptive_targets()`: for each downloading task in Adaptive mode → `AimdState::sample_throughput()` → compute throughput → decide increase/decrease `desired_thread_count`
    - `rebalance_allocations()`: redistributes thread budget across all tasks
      - **Traditional**: FIFO, `max_parallel_tasks` cap
      - **Automatic**: sort by remaining bytes (largest first), distribute `max_parallel_threads` budget, enforce `min_threads_per_task` floor
15. Thread modes:
    - **Fixed**: respects `requested_thread_count` from settings
    - **Adaptive**: uses `AimdState` with profile-dependent logic — `reduce_threads(current, profile, min)` and cooldown periods
16. `CancellationToken` cascade: scheduler → workers. When `pause()` is called, token is cancelled → workers stop gracefully.

## Phase 5 — Finalization

17. All chunks complete → `finalize_download()`:
    - `DownloadBuffer::drain_background()` → await any in-flight background flushes
    - `DownloadBuffer::flush_all()` → final flush to disk
    - `calculate_checksum(file, mode)` via `checksum.rs` (Blake3 / SHA256 / XXH3-128)
    - Rename `destination_path.tmp` → `destination_path`
    - Update SQLite: `state = "completed"`, `updated_at_ms`, final checksum
    - Emit `"download-updated"` Tauri event via `app_handle.emit()`
18. If checksum mismatch: re-download affected chunks (retry loop with exponential backoff via `retry.rs`)
19. On cancel/pause mid-download: `CancellationToken` propagates → workers call `DownloadBuffer::drain_background()` → `download_chunked()` returns `ChunkWorkerOutcome::Paused` / `Canceled` → partial state persisted to SQLite

## Subsystem Interaction Map

| Phase | Subsystems involved |
|---|---|
| Phase 1 | `commands.rs`, `protocol.rs` (TaskId routing) |
| Phase 2 | `manager.rs`, `http_executor.rs`, `manifest.rs`, `database.rs`, `disk_detect.rs` |
| Phase 3 | `http_executor.rs`, `buffer_pool.rs`, `database.rs`, `rate_limiter.rs` |
| Phase 4 | `scheduler.rs`, `aimd.rs`, `rate_limiter.rs` |
| Phase 5 | `http_executor.rs`, `buffer_pool.rs`, `checksum.rs`, `database.rs`, `retry.rs`, `file_alloc.rs` |

## Key Data Types in Flight

```
StartDownloadRequest  (前端表单)
  → 下载过程中持续更新 DownloadSnapshot  (完整详情, DownloadInspector 使用)
  → 列表视图使用 DownloadSummary       (snapshot 的子集)
  → 高频推送使用 DownloadProgress      (最小字段集, Tauri 事件)
```

**Event emission**:
- `DownloadManager::emit_single_summary()` → `app_handle.emit("download-updated", DownloadSummary)` (Tauri event, JSON)
- `DownloadManager::emit_progress()` → `app_handle.emit("download-progress", DownloadProgress)` (high-frequency update)
- Aria2 RPC: listens on `broadcast::channel` for `"download-updated"` string events → converts to WebSocket push

**Serialization note**: All types use `#[serde(rename_all = "camelCase")]` — fields are `camelCase` in JSON/frontend, `snake_case` enum variants in JSON.

## Error Recovery

- Transient failures (network, timeout): `retry.rs` exponential backoff, up to `max_retries` attempts per chunk
- Checksum mismatch: re-download affected chunks only (not entire file)
- Crash recovery: on restart, `persistence::load_downloads_from_db()` (impl DownloadManager in `persistence.rs`) reads SQLite → rebuilds `ManagedDownload` → resumes from last persisted chunk state. Un-flushed buffer data is lost.
- Insufficient disk space: `DownloadError::InsufficientDiskSpace { available, required }` — checked before download starts (10% buffer added to required)

## Other Protocol Flows

- **BitTorrent**: See `subsystem-bt-backend.md` — session lifecycle, alert bridge, upload policy loop
- **CDN acceleration**: See `subsystem-cdn-accelerator.md` — IP range fetching → screening → throughput measurement → DNS rewriting
