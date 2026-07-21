# Subsystem: HttpClientFactory

## 模块职责

统一构建所有 reqwest::Client 实例。从 settings 子系统中独立出来，提供共享的客户端构建器配置（代理、User-Agent、超时、重定向策略），供 DownloadManager、BT Backend、CDN Accelerator 使用。

本子系统无自定义结构体。核心输出是 reqwest::Client 和 reqwest::ClientBuilder。

关键函数：build_http_client（构建完整 Client）、configure_client_builder（配置共享参数，返回 Builder 供调用方追加 DNS 重写等额外配置）、normalize_user_agent。

## 涉及文件

- `crates/limedl-core/src/http_client_factory/mod.rs` — 构建器函数

## 数据流向

```
各子系统需要 HTTP 客户端
  ├─ DownloadManager::new() → build_http_client(&settings)
  ├─ BT Backend (session.rs) → build_http_client(&settings) 获取 .torrent 文件
  ├─ CdnAccelerator (resolver.rs) → configure_client_builder + DNS 重写
  └─ CdnAccelerator (speed_test.rs) → configure_client_builder + DNS 重写

设置变更时 → DownloadManager 重新调用 build_http_client 重建客户端
```

## 设计决策与约定

- configure_client_builder 返回 ClientBuilder 而非 Client，允许调用方追加额外配置（如 DNS 重写）。
- User-Agent 从设置读取，空值时回退到 Chrome 内置 UA。
- ProxyMode::System 时 reqwest 自动使用系统代理。
- 共享配置项：重定向策略 `Policy::limited(10)`、TCP_NODELAY=true、读超时 15 秒。
- 此模块不依赖任何其他子系统（仅依赖 types.rs 和 error.rs）。
- 客户端实例在 DownloadManager 的 `client: Arc<RwLock<Client>>` 中缓存，设置变更时通过 RwLock::write 替换。
