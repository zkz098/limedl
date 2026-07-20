# Subsystem: Aria2 RPC Server

## 模块职责

提供 aria2 JSON-RPC 2.0 兼容的 HTTP + WebSocket 服务器（默认端口 6800），使下载器能被 AriaNg、Motrix 等 aria2 客户端连接和控制。内部下载任务被映射为 aria2 GID。

**涉及文件**：

- `src-tauri/src/download/aria2_rpc.rs` — Axum WebSocket + HTTP JSON-RPC 服务器完整实现

## 关键结构体

### Aria2RpcServer (pub)

```rust
pub struct Aria2RpcServer {
    ctx: Arc<RpcContext>,
    addr: String,  // 监听地址，如 "127.0.0.1:6800"
}
```

### RpcContext（内部）

```rust
struct RpcContext {
    manager: Arc<DownloadManager>,
    bt_backend: Arc<IrontideBtBackend>,
    secret: Option<String>,                        // RPC 密钥
    event_bus: Arc<EventBus>,                      // 统一事件总线
    gid_cache: Mutex<HashMap<String, String>>,     // internal_id → GID 映射缓存
}
```

### JSON-RPC 2.0 类型（内部）

```rust
struct JsonRpcRequest {
    jsonrpc: String,       // "2.0"
    id: Option<Value>,     // 请求 ID
    method: String,        // 方法名
    params: Option<Value>, // 参数
}
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}
```

## 关键方法

### Aria2RpcServer

```rust
pub fn new(
    manager: Arc<DownloadManager>,
    bt_backend: Arc<IrontideBtBackend>,
    settings: &Aria2RpcSettings,
    event_bus: Arc<EventBus>,
) -> Self

// 启动服务器（阻塞直到 shutdown 信号）
pub async fn serve(self, mut shutdown: watch::Receiver<bool>) -> anyhow::Result<()>
```

### 公共辅助函数

```rust
// 将内部 download_id 转换为 16 字符 hex GID（XXH3 hash）
pub fn internal_id_to_gid(internal_id: &str) -> String
// 清理旧的 aria2 临时文件
pub(crate) fn cleanup_old_aria2_temp_files()
```

## 实现的 JSON-RPC 方法

| 方法                                     | 说明               |
| ---------------------------------------- | ------------------ |
| `aria2.addUri`                           | 添加 HTTP 下载     |
| `aria2.addTorrent`                       | 添加 BT 下载       |
| `aria2.pause` / `aria2.forcePause`       | 暂停下载           |
| `aria2.unpause`                          | 恢复下载           |
| `aria2.pauseAll` / `aria2.forcePauseAll` | 暂停所有任务       |
| `aria2.unpauseAll`                       | 恢复所有任务       |
| `aria2.remove` / `aria2.forceRemove`     | 删除下载           |
| `aria2.tellStatus`                       | 查询单个任务状态   |
| `aria2.tellActive`                       | 查询活跃任务列表   |
| `aria2.tellWaiting`                      | 查询等待中任务列表 |
| `aria2.tellStopped`                      | 查询已停止任务列表 |
| `aria2.getGlobalStat`                    | 全局统计信息       |
| `aria2.getGlobalOption`                  | 全局选项           |
| `aria2.changeGlobalOption`               | 修改全局选项       |
| `aria2.getVersion`                       | 版本信息           |
| `aria2.getFiles`                         | 获取文件列表       |
| `aria2.getUris`                          | 获取 URI 列表      |
| `aria2.getPeers`                         | 获取对等节点列表   |
| `aria2.shutdown`                         | 关闭               |
| `system.listMethods`                     | 列出所有方法       |
| `system.listNotifications`               | 列出所有通知       |

## 数据流向

```
AriaNg / Motrix 客户端
  ↓ HTTP POST /jsonrpc 或 WebSocket 连接 ws://127.0.0.1:6800/jsonrpc
  ↓
Axum Router
  ├─ POST /jsonrpc → handle_jsonrpc()
  └─ GET  /jsonrpc → handle_ws_upgrade()
  ↓
dispatch_method(method, params)
  ├─ 解析 JSON-RPC 请求 → 路由到对应 handler
  ├─ handler 调用 DownloadManager / IrontideBtBackend 方法
  └─ 内部状态转换为 aria2 格式的 JSON 响应
  ↓
事件通知（通过 WebSocket 推送）
  ├─ 订阅 EventBus，过滤 DownloadEvent::Aria2Notification
  ├─ 转换为 aria2 事件格式 (JsonRpcNotification)
  └─ 通过 WebSocket 推送到已连接的客户端
```

**重要约定**：

- 内部 download_id 与 aria2 GID 的映射通过 `internal_id_to_gid()` 实现（XXH3 hash 取前 16 hex 字符）
- 未持久化 GID 映射，重启后 GID 可能变化
- secret 令牌若配置，客户端请求必须包含 `token:` 前缀的参数
- WebSocket 和 HTTP POST 共用同一套 handler 逻辑
- 此实现经过 AriaNg / Motrix 实际测试验证兼容性
