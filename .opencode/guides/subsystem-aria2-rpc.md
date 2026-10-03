# Subsystem: Aria2 RPC Server

## 模块职责

提供 aria2 JSON-RPC 2.0 兼容的 HTTP + WebSocket 服务器（默认端口 6800），使本下载器能被 AriaNg、Motrix 等 aria2 客户端连接和控制。内部下载任务被映射为 aria2 GID。

核心类型：Aria2RpcServer（Axum WebSocket + HTTP JSON-RPC 服务器）、RpcContext（内部上下文，含 registry、dispatcher、auth、event_bus、gid_cache、session_id）、AuthConfig（Disabled / Shared / PerClient 三态鉴权）。

位于 `aria2-rpc` feature 下，可选编译。

## 涉及文件

- `crates/limedl-core/src/aria2_rpc/` — Aria2RpcServer 完整实现，按职责拆分：`mod.rs`（装配 + `pub use` 导出）、`protocol.rs`（JSON-RPC 线格式与 aria2 状态映射）、`context.rs`（RpcContext / AuthConfig / 集中的 token 校验 / gid 缓存 / 事件广播）、`token.rs`（令牌生成、Argon2id 哈希、恒定时间校验、验证缓存）、`dispatch.rs`（方法路由 + 集中的 token 校验）、`download.rs`（addUri/addTorrent/pause/unpause/remove/purge）、`query.rs`（tellStatus/tellActive/getFiles/getPeers/session）、`options.rs`（getOption/changeGlobalOption + aria2 option 解析）、`system.rs`（shutdown/multicall/listMethods/临时文件清理）、`transport.rs`（HTTP + WebSocket）、`server.rs`（Router 与生命周期）；单测 `tests.rs`，E2E `e2e_tests.rs`（handler 矩阵、system.multicall、单密钥与 per-client 鉴权）
- `crates/limedl-native/src/main.rs` — 桌面接线：`settings.aria2_rpc.enabled` → `Aria2RpcServer::new(core.registry, &settings.aria2_rpc, event_bus)` → `serve(rx, vec![])`；启动失败仅 log 不阻塞。

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

- GID 由 XXH3(TaskId) 计算得出。HTTP 下载的 TaskId 为 UUID（持久化在 SQLite 中），BT 下载的 TaskId 为 info hash（从种子/磁力链接提取，确定性）。两者重启后均保持稳定，因此 GID 在重启后不变。`gid_cache` 仅作为反向查找缓存优化性能，重启后通过扫描所有任务重建。**生命周期**：`addUri` 与 `resolve_gid` 的扫描路径写入缓存，`aria2.remove` 与 `purgeDownloadResult` 必须删除对应条目——否则长会话里缓存只增不减。新增删除类 handler 时同步清理缓存。
- secret 令牌若配置，**所有**方法都必须携带 `token:` 参数。校验集中在 `dispatch.rs::dispatch_method`（先 `check_token` 再 `strip_token`），handler 只拿剥完的参数；新增方法自动受保护。`system.multicall` 的嵌套调用走 `dispatch_authorized`（外层已验证，只剥离不复查）。历史 bug 有两个：每个 handler 各写一份且顺序写反（先 strip 后 check，secret 一配就拒绝所有合法请求），而 `pauseAll`/`getVersion` 等根本不检查（匿名可调用）——所以校验必须留在路由层，不要下放回 handler。
- **鉴权模式**（`Aria2AuthMode`，默认 `single`）：`single` 沿用 `settings.secret` 单一共享密钥；`per_client` 使用 `settings.clients`（`Aria2Client { id, name, token_hash, created_at_ms }`），每个客户端独立令牌，`secret` 在该模式下被忽略。`RpcContext` 由 `AuthConfig::from_settings` 构造三态：`Disabled`（无密钥，放行）/ `Shared{secret}` / `PerClient(PerClientAuth)`。校验只接受 `token:` 前缀（与 aria2 线格式一致），`Shared` 走 `subtle::ConstantTimeEq`，`PerClient` 对每个客户端用 Argon2 验证。`check_token` 集中在 `context.rs`，`dispatch_method` 调用它，改动模式不影响 handler。
- **令牌存储**：settings.json 只保存 `token_hash`（Argon2id PHC 串），明文由 UI 生成后仅展示一次。`token.rs::generate_token` 产出 32 字节 URL-safe base64；`hash_token` 用 `rand` 生成盐 + `SaltString::encode_b64`（不启用 argon2 的 `rand` feature，避免引入第二个 rand_core 大版本）；`verify_token` 对畸形 PHC 串返回 false 而非报错。**验证缓存**：`PerClientAuth` 用 BLAKE3(令牌) 作为内存键缓存已验证结果（首次 Argon2 约 20-40ms，之后 O(1) 恒定时间比对）；缓存只在 Argon2 成功后写入、从不落盘、也从不单独作为凭据，因此 settings.json 泄漏无法直接重放。客户端列表为空时 **fail closed**（拒绝全部），不会退化为匿名访问。
- **热重载的端口交接**：保存设置会重启 RPC 服务器（`SettingsSync::sync_aria2_rpc`）。旧实例走 `axum::serve(...).with_graceful_shutdown(...)`，监听套接字是在**另一个任务**收到关闭信号后才 drop 的，因此新实例若立刻 `bind` 可能撞上 `AddrInUse`，`serve` 返回后端点就彻底没服务了（表现为 AriaNg 连不上）。`server.rs::bind_with_retry` 对 `AddrInUse` 在 5 秒窗口内按 25ms 重试，其它错误立即返回；端口被别的进程真正占用时仍会在窗口后失败，只记日志、不阻塞 UI。
- `system.multicall` 每个结果按 aria2 规范包成**单元素数组**：成功 `[value]`，失败 `[{"code","message"}]`。AriaNg/Motrix 取 `entry[0]`，写成 `[null, value]` 会被当成失败。
- WebSocket 和 HTTP POST 共用同一套 handler 逻辑。
- 此实现经过 AriaNg / Motrix 实际测试验证兼容性。
- 辅助函数 `cleanup_old_aria2_temp_files` 清理旧的 aria2 临时文件。
- 支持约 22 个 aria2 方法（addUri, addTorrent, changeGlobalOption, changeOption, getFiles, getGlobalOption, getOption, getPeers, getGlobalStat, getUris, getVersion, removeDownloadResult, pause/unpause, pauseAll/unpauseAll, purgeDownloadResult, remove, saveSession, shutdown, tellActive/tellStatus/tellStopped/tellWaiting, system.multicall/listMethods/listNotifications 等）。未实现且**不**写进 `listMethods` 的：`addMetalink`、`changePosition`、`changeUri`、`getServers`、`forceShutdown`。
- `aria2.addUri` 透传 aria2 请求选项：`header`（字符串数组或单字符串，格式 `Name: value`）、`referer`（`*` 表示使用下载 URL 本身）、`http-user`/`http-passwd`（合成 `Authorization: Basic`）、`checksum`（`TYPE=DIGEST`，仅支持 sha-256/sha-512/blake3；sha-1 及其它类型被忽略）、`user-agent`、`split`、`max-tries`、`out`、`dir`、`pause`。aria2 在 JSON-RPC 中把**所有选项都序列化为字符串**，所以 `pause` 同时接受 `"true"`（客户端实际发送形态）与布尔 `true`。`uris` 数组按 aria2 语义视为有序候选列表：第一个为主 URL，其余作为镜像写入 `mirror_urls`。`aria2.getOption` 返回任务真实的 `user-agent` / `referer` / `header`（从内存中的 manifest 读取）。
- **URI 分类**（`classify_aria2_uri`）：`aria2.addUri` 只把 `magnet:` 路由到 BT 后端；http(s) 与 `.torrent` URL 都走 HTTP（aria2 语义：解析型种子用 `addTorrent`）。**不要**把 `kind` 交给 `classify_kind` 的扩展名启发式，也不要用 `kind: None`，否则 `.torrent` 会被误路由到 BT。
- **BT 文件列表**：`tellStatus.files` / `getFiles` 对 `TaskId::Bt` 使用 `LazyBtBackend::get_torrent_files`（引擎索引 0 起 → aria2 索引 1 起的字符串），元数据未到时返回 `[]` 而不是伪造一条；HTTP 保持原有单条合成记录。`select-file` 为 1 起逗号分隔，经 `parse_select_file` 转成引擎的 0 起索引；返回的 `selected` 与引擎 file priority 一致（`Skip` → `false`），所以 `select-file` 的效果会立即反映到 `getFiles`/`tellStatus.files`。
- **`aria2.changeOption`**：只接受引擎运行期真能改的键——`pause`（两协议）、BT 的 `select-file`、`max-download-limit`、`max-upload-limit`；其余键（`split`、`dir`、`out`、`header` 等）返回带键名的错误，**不**静默忽略。
- **通知归属**：HTTP 的 start/pause/resume/stop 由 RPC handler 发（`broadcast_event`），complete/error 由 `http_executor::finalize` / `task_lifecycle` 发；BT 的 start/pause/resume/complete/btComplete/error 全部由 `bt_backend/alerts.rs` 发。因此 handler 只对 `TaskId::Http` 广播，否则 BT 会收到重复通知（历史上 `addTorrent`/`pause` 就是双发）。
- **`keys` 过滤**：tellStatus/tellActive/tellWaiting/tellStopped 支持 aria2 的 `keys` 数组（位置分别为 1/0/2/2），`filter_status_keys` 只保留指定字段。
- **`getUris`**：返回完整候选列表（主 URL + `mirror_urls`，去重），当前使用中的为 `used`，其余 `waiting`。
- **终态可见性**：`max_in_memory_downloads` 淘汰只影响内存；`tellStatus`/`tellStopped`/`getGlobalStat`/`removeDownloadResult` 通过 `db.get_download_header` / `list_download_headers` / `count_terminal_downloads` 回查数据库，直到 purge/逐条删除。`purgeDownloadResult` 也会清掉已淘汰行的数据库记录。
