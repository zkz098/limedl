# Subsystem: BitTorrent Backend (IrontideBtBackend)

## 模块职责

通过 irontide 库管理 BitTorrent 下载的完整生命周期：会话管理、torrent 元数据解析、对等节点连接、文件选择、上传策略、进度/状态查询。bt 任务使用 TaskId::Bt(Id20)，ID 为原始 info_hash hex 字符串。

核心类型：IrontideBtBackend（持有 session handle、task_map、alert_task、upload_policy_task 等字段）。

自身有 `active_bt_count` 原子槽位（与 DownloadManager 的 DownloadSlotGuard 独立）。

## 涉及文件

- `crates/limedl-core/src/bt_backend_own/mod.rs` — IrontideBtBackend 结构体定义 + DownloadBackend trait 实现
- `crates/limedl-core/src/bt_backend_own/lifecycle.rs` — 生命周期方法（start/pause/resume/cancel/remove/purge）
- `crates/limedl-core/src/bt_backend_own/session.rs` — irontide Session 初始化/关闭
- `crates/limedl-core/src/bt_backend_own/snapshot.rs` — 从 irontide stats 构建 DownloadSnapshot
- `crates/limedl-core/src/bt_backend_own/queries.rs` — 对等节点/区块/tracker/文件状态查询
- `crates/limedl-core/src/bt_backend_own/alerts.rs` — irontide 告警事件桥接
- `crates/limedl-core/src/bt_backend_own/uploads.rs` — 上传策略循环
- `crates/limedl-core/src/bt_backend_own/tests.rs`

## 数据流向

```
用户提交 BT 任务（magnet link 或 .torrent 文件）
  ↓
commands / rpc → BackendRegistry → IrontideBtBackend::start()
  ├─ 解析 URL → 获取 .torrent 元数据（magnet link 通过 DHT 获取，文件直接下载）
  ├─ SessionHandle::add_torrent() → 加入 irontide 会话
  ├─ task_map.insert(download_id → info_hash)
  └─ setup_alert_bridge() → 后台循环接收 irontide 告警
       ├─ stats_alert → stats_to_snapshot() → EventBus::publish(Updated/Progress)
       ├─ metadata_received → 文件列表可用
       └─ torrent_finished → EventBus::publish(Aria2Notification/Progress/Updated)

上传策略循环（spawn_upload_policy_loop）：
  ├─ 每 N 秒检查全局上传速率
  ├─ 超过限制 → 暂停部分 torrent 上传
  └─ 低于限制 → 恢复上传
```

## 设计决策与约定

- BT 任务的 download_id 为原始 info_hash hex 字符串（40 字符十六进制），不含 `bt:` 前缀。
- task_map 维护 download_id → Id20(info_hash) 的映射关系。
- irontide 通过告警系统异步推送状态变更，不轮询 session。
- stats_to_snapshot 是状态转换的核心桥梁。
- 上传策略循环独立于下载，通过 `paused_by_limit: DashMap<Id20, ()>` 控制。
- 所有告警和状态变更通过 EventBus::publish() 统一发布，不直接持有 event_tx 或 app_handle。
