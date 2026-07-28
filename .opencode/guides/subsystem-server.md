# Subsystem: Server (limedl-server)

## 模块职责

NAS/无头部署的完整服务端：CLI 入口（daemon / download 子命令）、axum HTTP + WebSocket 服务器、JSON-RPC 2.0 调度、静态文件服务、HTTP Basic 认证、安全响应头、TLS 支持、服务端限速。

同时提供库接口（`lib.rs`）供外部集成。

## 涉及文件

- `crates/limedl-server/src/main.rs` — CLI 入口（clap） + 守护进程启动 + 单次下载
- `crates/limedl-server/src/lib.rs` — 库入口（pub mod 声明）
- `crates/limedl-server/src/rpc.rs` — WebSocket JSON-RPC 2.0 调度 + EventBus→WebSocket 事件转发
- `crates/limedl-server/src/auth.rs` — HTTP Basic Auth 中间件
- `crates/limedl-server/src/config.rs` — ServerConfig + JSON 加载 + CLI 覆写
- `crates/limedl-server/src/security.rs` — CSP + 安全响应头
- `crates/limedl-server/src/rate_limiter.rs` — 服务端 IP 限速

## 数据流向

```
CLI 入口（main.rs）
  ├─ limedl daemon [--config <path>] [--port <n>] [--user <u> --pass <p>]
  │     → ServerConfig::load(config_path) → apply_cli_overrides(port, user, pass)
  │     → bootstrap(state_dir) → CoreSystems（全部核心子系统）
  │     → 构建 RpcState { dispatcher, settings, event_bus, registry, cdn_service, security }
  │     → axum Router
  │         ├─ 安全中间件（security_headers_layers）
  │         ├─ 认证中间件（basic_auth_middleware，可选）
  │         ├─ GET  /ws → WebSocket 升级 → RPC dispatch + event relay
  │         ├─ POST /jsonrpc → HTTP JSON-RPC（复用 WS handler）
  │         ├─ POST /api/* → REST API（下载操作）
  │         └─ GET  /*    → 静态文件服务（NAS WebUI dist 目录）
  │
  └─ limedl download <url> [-o <path>]
        → bootstrap(temp_dir) → CoreSystems（临时会话）
        → DownloadManager::start() → progress 打印到 stdout → 完成退出

WebSocket 连接 → rpc.rs
  ├─ 接收 JSON-RPC 请求 → dispatch_method(method, params) → handler → JSON 响应
  └─ 后台事件 relay 任务 → subscribe EventBus → 匹配 WS_EVENTS → notification 推送到客户端
```

## 子系统详情

### Config (`config.rs`)

| 类型 | 字段 | 说明 |
|------|------|------|
| `ServerConfig` | `host` (默认 `0.0.0.0`) | 监听地址 |
| | `port` (默认 `9090`) | 监听端口 |
| | `data_dir` | 下载数据目录 |
| | `auth: Option<AuthConfig>` | 认证配置 |
| | `web_dir` (默认 `./dist`) | 前端静态文件目录 |
| | `tls: TlsConfig` | TLS 配置 |
| `AuthConfig` | `username`, `password` | 凭证 |
| `TlsConfig` | `enabled`, `cert_path`, `key_path` | TLS 证书 |

- `ServerConfig::load(path)` 从 JSON 文件读取，失败时回退到默认值。
- `apply_cli_overrides(port, user, pass)` 用 CLI 参数覆盖文件配置。

### Auth (`auth.rs`)

- Axum 中间件：`Authorization: Basic <base64>` 头或 `?token=` 查询参数。
- 未配置认证时放行所有请求。
- 使用**常数时间字符串比较**防止时序攻击。
- 从请求扩展（`Extensions`）中读取配置。

### Security (`security.rs`)

| 函数 | 作用 |
|------|------|
| `nas_csp_header()` | 动态生成 CSP：`connect-src` 允许当前 WebSocket 源 |
| `security_headers_layers()` | 返回 4 个 `SetResponseHeaderLayer` |

> 注意：Tauri desktop 的 CSP 设为 `null`（`tauri.conf.json`），但 NAS WebUI 使用严格的 CSP。两者通过不同的适配路径处理，不相交。

四个安全头：
- `Content-Security-Policy`（动态，含 WS 源）
- `X-Content-Type-Options: nosniff`
- `X-Frame-Options: DENY`
- `Referrer-Policy: strict-origin-when-cross-origin`

### RPC (`rpc.rs`)

- `RpcState` 持有 `Dispatcher`、`AppSettings`、`EventBus`、`BackendRegistry`、`CdnService` 等。
- WebSocket JSON-RPC 2.0：支持 ~32 个命令（下载 CRUD + BT 查询 + CDN + 设置 + Aria2 兼容）。
- HTTP POST `/jsonrpc` 共用同一套 dispatch 逻辑。
- 事件转发：每个 WebSocket 连接有一个独立的后台任务订阅 `EventBus`，匹配 `WS_EVENTS` manifest 后作为 JSON-RPC notification 推送到客户端。
- 编译期一致性测试：`ws_manifest.rs` 验证所有声明的命令都在 rpc.rs 中有 handler 分支。

### Rate Limiter (`rate_limiter.rs`)

- 服务端 IP 级别的请求速率限制（独立于 `limedl-core` 的下载带宽限速）。
- 保护 WebSocket 和 HTTP 端点免受滥用。

## 设计决策与约定

- **双模式 CLI**：`daemon` 启动持久化服务器，`download` 启动临时会话做单次下载。两者共用 `bootstrap()`。
- **前端共用**：NAS WebUI 使用与 Tauri 桌面**完全相同的 Vue 3 代码**，仅通信层通过 `#invoke` / `#event` import alias 切换为 WebSocket。
- **认证范围**：仅 WebSocket 升级路径（`/ws`）和 `/api/*` 需要认证；静态文件 `/` 在认证中间件之前已返回（公开访问）。
- **TLS**：生产环境建议在反向代理（nginx/Caddy）层面配置 TLS，`TlsConfig` 仅用于开发/简单部署场景。
- **安全头与 Tauri 的 CSP 不存在冲突**——Tauri 桌面不使用 axum 服务，NAS 不使用 Tauri 的 CSP 配置。
