# Subsystem: Aria2 RPC Server

## 模块职责

提供 aria2 JSON-RPC 2.0 兼容的 HTTP + WebSocket 服务器（默认端口 6800），使本下载器能被 AriaNg、Motrix 等 aria2 客户端连接和控制。内部下载任务被映射为 aria2 GID。

核心类型：Aria2RpcServer（Axum WebSocket + HTTP JSON-RPC 服务器）、RpcContext（内部上下文，含 registry、dispatcher、secret、event_bus、gid_cache、session_id）。

位于 `aria2-rpc` feature 下，可选编译。

## 涉及文件

- `crates/limedl-core/src/aria2_rpc.rs` — Aria2RpcServer 完整实现（构造 + serve + dispatch_method + 各个 handler）
- `src-tauri/src/lib.rs` — 桌面接线（约 430-444 行）：`settings.aria2_rpc.enabled` → `Aria2RpcServer::new(core.registry, &settings.aria2_rpc, event_bus)` → `serve(rx, vec![])`，`tx` 存入 `AppState.rpc_shutdown`；启动失败仅 log 不阻塞。
- `crates/limedl-server/src/main.rs` — NAS/守护进程接线（`run_daemon`，bootstrap 之后）：与桌面相同的模式，CORS 传 `settings.aria2_rpc.cors_allowed_origins`（NAS 需要真实 CORS 源），watch Sender 接入 shutdown_signal 实现优雅停机。limedl-server 通过 `limedl-core` 的 `aria2-rpc` feature 编译。

## 数据流向

```
AriaNg / Motrix 客户端
  ↓ HTTP POST /jsonrpc 或 WebSocket 连接 ws://127.0.0.1:6800/jsonrpc
  ↓
Axum Router → dispatch_method(method, params)
  ├─ 解析 JSON-RPC 请求 → 路由到对应 handler（aria2.addUri, aria2.tellStatus 等）
  ├─ handler 调用 DownloadManager / IrontideBtBackend 方法
  └─ 内部状态转换为 aria2 格式的 JSON 响应

事件通知（WebSocket 推送）
  ├─ 订阅 EventBus，过滤 DownloadEvent::Aria2Notification
  ├─ 转换为 aria2 事件格式（JsonRpcNotification）
  └─ 通过 WebSocket 推送到已连接的客户端
```

## 设计决策与约定

- GID 由 XXH3(TaskId) 计算得出。HTTP 下载的 TaskId 为 UUID（持久化在 SQLite 中），BT 下载的 TaskId 为 info hash（从种子/磁力链接提取，确定性）。两者重启后均保持稳定，因此 GID 在重启后不变。`gid_cache` 仅作为反向查找缓存优化性能，重启后通过扫描所有任务重建。
- secret 令牌若配置，客户端请求必须包含 `token:` 前缀的参数。
- WebSocket 和 HTTP POST 共用同一套 handler 逻辑。
- 此实现经过 AriaNg / Motrix 实际测试验证兼容性。
- 辅助函数 `cleanup_old_aria2_temp_files` 清理旧的 aria2 临时文件。
- 支持约 20 个 aria2 方法（addUri, addTorrent, pause/unpause/remove, tellStatus, tellActive, tellWaiting, tellStopped, getGlobalStat, getGlobalOption, changeGlobalOption, getVersion, getFiles, getUris, getPeers, shutdown, system.listMethods, system.listNotifications 等）。
