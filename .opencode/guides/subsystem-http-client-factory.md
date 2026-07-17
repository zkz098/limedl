# Subsystem: HttpClientFactory

## 模块职责

统一构建所有 `reqwest::Client` 实例。从 `settings` 子系统中独立出来，提供共享的客户端构建器配置（代理、User-Agent、超时、重定向策略），供 DownloadManager、BT Backend、CDN Accelerator 使用。

**涉及文件**：
- `src-tauri/src/download/http_client_factory/mod.rs` (~50 行) — build_http_client + configure_client_builder + normalize_user_agent

## 关键结构体

本子系统无自定义结构体。核心输出是 `reqwest::Client` 和 `reqwest::ClientBuilder`。

## 关键方法

```rust
// 构建完整的 reqwest::Client（标准下载用）
pub(crate) fn build_http_client(settings: &AppSettings) -> Result<Client>

// 配置 ClientBuilder 的共享参数（代理、UA、超时、重定向）
// 供 CDN accelerated client 和 throughput test client 复用
pub(crate) fn configure_client_builder(builder: ClientBuilder, settings: &AppSettings) -> Result<ClientBuilder>
```

**configure_client_builder 配置项目**：
| 配置 | 值 |
|---|---|
| 重定向策略 | `Policy::limited(10)` |
| TCP_NODELAY | true |
| 读超时 | 15 秒 |
| User-Agent | 从 settings.download.default_user_agent 解析 |
| 代理 | 根据 settings.proxy.mode 设置 (Disabled/System/Manual) |

## 数据流向

```
各子系统需要 HTTP 客户端
  ↓
├─ DownloadManager::new()
│    └─ build_http_client(&settings) → reqwest::Client（标准下载用）
│
├─ BT Backend (session.rs)
│    └─ build_http_client(&settings) → reqwest::Client（获取 .torrent 文件用）
│
├─ CdnAccelerator (resolver.rs)
│    └─ configure_client_builder(builder, settings) + resolve_to_addrs → accelerated Client
│
└─ CdnAccelerator (speed_test.rs)
     └─ configure_client_builder(builder, settings) + resolve_to_addrs → throughput test client

设置变更时（update_settings）：
  DownloadManager 重新调用 build_http_client(&new_settings) 重建客户端
```

**重要约定**：
- `configure_client_builder` 返回 `ClientBuilder` 而非 `Client`，允许调用方追加额外配置（如 DNS 重写、自定义 header）
- User-Agent 从设置读取，空值时回退到 Chrome 内置 UA（通过 `normalize_user_agent`）
- ProxyMode::System 时 reqwest 自动使用系统代理（不设置 `no_proxy` 也不显式设置 proxy）
- 此模块不依赖任何其他子系统（仅依赖 types.rs 和 error.rs）
- 客户端实例在 `DownloadManager.client: Arc<RwLock<Client>>` 中缓存，设置变更时通过 `RwLock::write` 替换
