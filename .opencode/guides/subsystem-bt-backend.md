# Subsystem: BitTorrent Backend (IrontideBtBackend)

## 模块职责

通过 irontide 库管理 BitTorrent 下载的完整生命周期：会话管理、torrent 元数据解析、对等节点连接、文件选择、上传策略、进度/状态查询。bt 任务使用 TaskId::Bt(Id20)，ID 为原始 info_hash hex 字符串。

核心类型：IrontideBtBackend（持有 session handle、task_map、alert_task、upload_policy_task、anti_leech_task、banned_leechers、anti_leech_slot_state、applied_blocklist_key、torrent_created_at、bt_slot_guards 等字段）。

自身有 `active_bt_count` 原子槽位（与 DownloadManager 的 DownloadSlotGuard 独立）。`max_concurrent_bt` 由 DownloadManager 和 IrontideBtBackend 共享，通过 `DownloadSlotGuard` 协调。

## 涉及文件

- `crates/limedl-core/src/bt_backend/mod.rs` — IrontideBtBackend 结构体定义 + DownloadBackend trait 实现 + Clone impl
- `crates/limedl-core/src/bt_backend/lifecycle.rs` — 生命周期方法（start/pause/resume/cancel/remove/purge） + `build_canceled_snapshot()`
- `crates/limedl-core/src/bt_backend/session.rs` — irontide Session 初始化/关闭/设置热重载
- `crates/limedl-core/src/bt_backend/snapshot.rs` — 从 irontide stats 构建 DownloadSnapshot + 状态映射 + ETA 估算
- `crates/limedl-core/src/bt_backend/queries.rs` — 对等节点/piece/tracker/文件状态查询 + 限速 + 预览 + `emit_pending_summary`
- `crates/limedl-core/src/bt_backend/alerts.rs` — irontide 告警事件桥接（唯一 Aria2 事件发射源） + `extract_info_hash`
- `crates/limedl-core/src/bt_backend/uploads.rs` — 上传策略循环（按上传量和分享率限制）
- `crates/limedl-core/src/bt_backend/anti_leech.rs` — 反吸血策略循环（识别并处理只下载不回报的对等端）
- `crates/limedl-core/src/bt_backend/tests.rs`

## 数据流向

```
用户提交 BT 任务（magnet link 或 .torrent 文件）
  ↓
commands / rpc → Dispatcher → BackendRegistry → IrontideBtBackend::start()
  ├─ 解析 URL → 获取 .torrent 元数据（magnet link 通过 DHT 获取，文件直接下载）
  ├─ 获取并发槽位（try_acquire_bt_slot → DownloadSlotGuard）
  ├─ SessionHandle::add_torrent() → 加入 irontide 会话
  ├─ task_map.insert(info_hash, info_hash)
  ├─ torrent_created_at.insert(info_hash, now_ms())  ← 记录创建时间戳
  ├─ bt_slot_guards.insert(info_hash, slot_guard)
  └─ emit_pending_summary() → EventBus::publish(Updated)  → 前端立即显示排队任务

Alert 桥接循环（setup_alert_bridge，唯一 Aria2 事件源）：
  后台循环接收 irontide 告警
  ├─ TorrentAdded    → Aria2Notification(aria2.onDownloadStart)
  ├─ TorrentPaused   → Aria2Notification(aria2.onDownloadPause)
  ├─ TorrentResumed  → Aria2Notification(aria2.onDownloadStart)
  ├─ TorrentFinished → Aria2Notification(onDownloadComplete/onBtDownloadComplete)
  │                  + Progress + Updated（含最终统计信息）
  ├─ TorrentError    → Aria2Notification(onDownloadError) + Updated
  └─ 2 秒周期性定时器 → 遍历 task_map → Progress 事件（所有活跃 torrent）

上传策略循环（spawn_upload_policy_loop）：
  ├─ 每 5 秒检查每个 torrent 的上传量和分享率
  ├─ 超过限制且 pause_upload_when_limit_reached → 暂停上传（set_upload_limit → 1 byte/s）
  └─ 限制解除后 → 恢复上传（set_upload_limit → 0，无限制）

反吸血策略循环（spawn_anti_leech_loop，每 10 秒）：
  ├─ 对每个活跃 torrent 调 get_peer_info + peer_unchoke_durations
  ├─ peer_is_leecher() 识别吸血对等端（见下方策略判断）
  ├─ Ban 动作：ban_peer(ip) + banned_leechers 记录到期时间，到期自动 unban
  ├─ LimitSlots 动作：set_max_uploads 收紧上传槽位，吸血者消失后恢复原值
  └─ 功能关闭时 cleanup_disabled() 撤销所有自身触发的 ban/槽位限制
```

## 设计决策与约定

### 事件发射策略

- **Alert bridge 是 Aria2 事件的唯一发射源**。lifecycle 的 `pause()`/`resume()`/`start()` 不直接发射 Aria2 事件，确保不会产生重复通知。
- lifecycle 的 `cancel()`/`remove()`/`purge()` 的 `Updated` 事件由 Dispatcher 层统一发射（`dispatcher.rs`）。
- `MetadataReceived` 告警仅记录日志，前端在下一次 2 秒周期性 Progress tick 时获取元数据更新。
- 事件发射映射表见 `alerts.rs` 中 `alert_bridge_loop` 的 doc comment。

### 任务标识与状态

- BT 任务的 download_id 为原始 info_hash hex 字符串（40 字符十六进制），不含 `bt:` 前缀。
- `task_map: DashMap<Id20, Id20>` 维护 download_id → info_hash 的映射关系。
- `torrent_created_at: DashMap<Id20, u64>` 记录每个 torrent 的创建时间戳（start 时写入，stats_to_snapshot 时查询，cancel/remove/purge 时清理）。对于从 resume state 恢复的 torrent（未经过 start），回退到调用时的当前时间。

### irontide 集成

- irontide 通过告警系统异步推送状态变更，不轮询 session。
- `stats_to_snapshot` 是状态转换的核心桥梁。使用 `downloaded`（所有 payload 字节）而非 `total_done`（仅已验证 piece）以提供平滑进度显示。
- `get_pieces()` 优先使用 `session.get_piece_states()` 获取精确的 per-piece 完成位图（支持乱序下载如 rarest-first）。回退到 `torrent_stats` 的 `pieces_total`/`pieces_have` 统计值（仅适用于顺序下载场景）。

### 并发控制

- `try_acquire_bt_slot()` 使用 CAS 循环竞争 `active_bt_count`，成功返回 `DownloadSlotGuard`。`start()` 中的 `_guard` 在 `add_to()` 失败时通过 Drop 自动释放。
- `bt_slot_guards` 保存成功添加的 torrent 的 slot guard，在 cancel/remove/purge 时移除以释放槽位。
- `setup_alert_bridge()` 在生成新任务前会 abort 任何正在运行的旧 alert bridge 任务（与 `spawn_upload_policy_loop` 模式一致）。

### 上传策略

- 上传策略循环独立于下载，通过 `paused_by_limit: DashMap<Id20, ()>` 跟踪被限制上传的 torrent。
- 仅在限制条件**不再满足**时恢复上传，避免 per-tick 振荡（pause → unpause → pause 循环）。
- 全局上传限制在 irontide session settings 中设定，per-torrent 限速通过 `set_upload_limit(info_hash, bps)` 实现。

### 反吸血（anti-leech）

- 与上传策略循环**相互独立**：上传策略按用户设定的整体上传量/分享率限制整 torrent；反吸血只针对**单个不回报的对等端**。
- `peer_is_leecher()` 是纯函数（可单测），判断规则：
  1. 跳过 `upload_only`（BEP 21 seeder）、我们正在 choke 的 peer、未过宽限期（`grace_secs`）及 `progress >= 1.0` 的 peer。
  2. 信号 A：peer 一直 choke 我们（`peer_choking`）而我们已放开上传超过宽限期且回传约 0 → 吸血。
  3. 信号 B：即使未 choke，回传/取用比例低于 `ratio` → 吸血（`ratio=0` 时关闭此判断）。
- `banned_leechers: DashMap<IpAddr, u64>` 记录 ban 到期时间戳；每轮 sweep 到期自动 `unban_peer` 宽容处理（避免误伤共享 NAT/VPN）。
- `anti_leech_slot_state: DashMap<Id20, usize>` 记录 LimitSlots 前的原槽位数以便恢复。
- 动作通过 `DownloadEvent::Warning` 通知前端（复用现有事件，不改 ws_manifest）。
- 后台任务句柄保存在 `anti_leech_task`，shutdown 时 abort（与 alert/upload 一致）；bootstrap 中 `spawn_anti_leech_loop()` 启动。
- 相关 BtSettings 字段：`anti_leech_enabled/action/grace_secs/ratio/ban_secs/max_upload_slots`。

### 设置热重载

- `apply_settings()` 复制 BtSettings 到 `Arc<Mutex<>>`，并通过 `build_engine_settings()` 把引擎调参 + 全局速率限制应用到 irontide session，无需重启 session。
- 注意：热重载采用 **spawn 到运行时异步应用**（而非 `block_in_place`），因此既能在 Tauri 同步 handler 中使用，也能在 current-thread 测试运行时中安全调用。

### 引擎调参（透传）

- BtSettings 暴露一组引擎调参字段，经 `build_engine_settings()` 映射为 irontide `Settings`，在启动与热重载时应用：
  - `seed_choking_algorithm`（含 `AntiLeech`：做种时优先回报最高的 peer）
  - `choking_algorithm`（FixedSlots / RateBased）
  - `max_upload_slots_per_torrent`、`max_peers_per_torrent`
  - `smart_ban_max_failures` / `smart_ban_parole`（hash 失败智能封禁）
  - `eviction_ban_duration_secs`（被驱逐 peer 的重新连接封禁时长）
  - `data_contribution_timeout_secs`（不贡献数据的 peer 被断开）

### IP 黑名单

- BtSettings 的 `blocklist_enabled` / `blocklist_path` 控制会话级 IP 过滤。
- `apply_blocklist()`（启动与热重载时）读取文件，按扩展名 **`.dat` → `parse_dat`，否则 `parse_p2p`**，得到 `irontide IpFilter` 后 `set_ip_filter()` 替换会话过滤。
- 本地/私网地址自动豁免，替换不会误伤内网。
- `applied_blocklist_key: Mutex<Option<String>>` 记录上次成功加载的 `enabled:path`，避免每次保存设置都重读大文件；解析失败不缓存，便于用户改文件后重存生效。

### 关闭流程

1. 保存 session state（`save_session_state`）
2. Abort 所有后台任务（upload_policy_task、alert_task）
3. 逐个保存每个活跃 torrent 的 resume data
4. 按比例宽限期（每 torrent 500ms，1-5s 区间），等待磁盘写入完成
5. 调用 `session.shutdown()` 关闭 irontide session
