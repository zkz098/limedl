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
  ├─ BT Backend (session.rs) → build_http_client(&settings) 获取 .torrent 文件；apply_settings 时重建
  ├─ Dispatcher (bootstrap.rs) → configure_client_builder + 覆盖 UA/超时（tracker list、校验和探测）
  ├─ CdnAccelerator (resolver.rs) → configure_client_builder + DNS 重写
  └─ CdnAccelerator (speed_test.rs) → configure_client_builder + DNS 重写

设置变更时 → DownloadManager / BT Backend 重新调用 build_http_client 重建客户端
```

## 设计决策与约定

- **所有出网客户端都必须经由本模块构建**。用裸 `reqwest::Client::builder()` 构建会绕过 `settings.proxy`（直连或系统代理失效），所以 Dispatcher、BT Backend 等旁路客户端也统一走 `build_http_client` / `configure_client_builder`；调用方可在返回的 builder 上追加 UA、超时、`resolve_to_addrs` 等个性化配置。

- configure_client_builder 返回 ClientBuilder 而非 Client，允许调用方追加额外配置（如 DNS 重写）。
- User-Agent 从设置读取，空值时回退到内置 UA（`default_http_user_agent()`，当前为 Chrome/154 形态的浏览器 UA）。**该版本号需要随发布周期手动保持新鲜**：部分镜像边缘（如清华 TUNA）会把“声称是浏览器但版本过期/尚未发布”的 UA 判定为伪装软件并返回 403，实测 Chrome/124 被拒、140/141 通过。升级方式：修改 `crates/limedl-core/src/types/settings.rs` 的 `default_http_user_agent()`，并同步 `ui/components/settings/tab_download.slint` 提示文案与 `lang/*/LC_MESSAGES/limedl-native.po`。注意：已存在的 `settings.json` 会保留旧 UA（不做自动迁移）。
- ProxyMode::System 时 reqwest 自动使用系统代理，前提是 reqwest 的 `system-proxy` feature 开启（它已包含在 reqwest 默认 feature 集中，因此根 `Cargo.toml` **不再设 `default-features = false`**）。一旦有人重新关闭默认 features 又没显式列出 `system-proxy`，`hyper-util/client-proxy-system` 就不会编译，`ProxyMatcher::system()` 只剩环境变量（`HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY`），Windows 注册表（Internet 选项）与 macOS 系统配置的静态代理会被静默忽略——表现就是“系统代理不生效”；回归守卫见 `workspace_enables_reqwest_system_proxy`（`http_client_factory/tests.rs`）。注意 hyper-util 只读静态 `ProxyServer`/`ProxyOverride`，不解析 PAC/WPAD。
- HTTP/3：根 `Cargo.toml` 开启 reqwest 的 `http3`（拉入 `h3` / `h3-quinn` / `quinn`）。这是上游 **unstable** feature，必须同时设置 `--cfg reqwest_unstable`，否则 reqwest 直接 `compile_error!`；该 cfg 已加入 `.cargo/config.toml` 每个 `[target.*]` 的 rustflags（新增 target 时别忘同步）。
- h3 的分派依据是**请求的 HTTP version**：只有 `RequestBuilder::version(http::Version::HTTP_3)` 的请求走 QUIC，其他请求走 h1/h2。`http3_prior_knowledge()` 只设置客户端偏好，**单独用它不会把请求切到 h3**（PR #2929 / v0.13.2 “Fix HTTP/3 to send h3 ALPN” 之后按请求切换才可用；QUIC 的 `h3` ALPN 由 `build_h3_connector` 固定设置）。reqwest **没有 Alt-Svc 自动升级**（issue #1810 仍未实现），也没有 h3→h1/h2 回退，所以当前客户端工厂仍走 h1/h2。另外 h3 路径不经过代理连接器（`build_h3_connector` 只拿 resolver，不拿 `proxies`），启用前必须先处理“配了代理时不得走 h3”的泄漏问题。
- 共享配置项：重定向策略 `Policy::limited(10)`、TCP_NODELAY=true、读超时 15 秒。
- 此模块不依赖任何其他子系统（仅依赖 types.rs 和 error.rs）。
- 客户端实例在 DownloadManager 的 `client: Arc<RwLock<Client>>` 中缓存，设置变更时通过 RwLock::write 替换；BT Backend 的 `.torrent` 客户端同样是 `Arc<RwLock<Client>>`，在 `apply_settings` 中随代理 / UA 变更重建（见 `bt_backend/mod.rs` 的 `http_client` 字段）。
