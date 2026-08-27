# Subsystem: Settings / Configuration + SettingsService

## 模块职责

管理应用配置的加载、验证、持久化和分发。配置以 JSON 格式存储在 settings.json（原子写入：先写 .json.tmp，再 rename）。通过 `SettingsService` 维护内存与磁盘的单一事实源（Single Source of Truth），通过 `AppSettings` 结构体封装所有设置分类。

核心类型：

- `SettingsService`：提供 `get()`、`get_blocking()`、`update(&AppSettings)`、`factory_reset()`、`default_download_dir()`。
- `AppSettings`（根结构体，含 appearance / proxy / scheduler / download / bt / logging / aria2_rpc / cdn_acceleration / github_mirror / url_rewrite / global_speed_limit_bps / notifications / io_baseline / autostart / setup_completed 等字段）。各子设置结构体定义在 `types.rs`。

关键枚举：ThreadMode（Fixed / Adaptive）、AdaptiveProfile（Conservative / Balanced / Aggressive）、ChecksumMode（None / Blake3 / Sha256 / Xxh3128）、SchedulerMode（Traditional / Automatic）、ProxyMode（Disabled / System / Manual）、DiskType（Ssd / Hdd）、ColorMode（Light / Dark / System）。

## 涉及文件

- `crates/limedl-core/src/services/settings_service.rs` — SettingsService 单一事实源服务
- `crates/limedl-core/src/settings.rs` — load_settings / normalize_settings / persist_settings / resolve_user_agent
- `crates/limedl-core/src/types.rs` — AppSettings 及所有子设置结构体定义

## 数据流向

```
应用启动 → SystemContext::new() → SettingsService::new(settings_path)
  ├─ 尝试读取 settings.json → 反序列化 AppSettings
  │   └─ 若字段齐全（appearance/proxy/scheduler 等任意一个存在）→ 正常解析 + normalize
  │   └─ 若仅有旧格式 ProxySettings（legacy 回退）→ 构造默认 AppSettings + 注入 proxy
  ├─ normalize_settings() → 验证裁剪范围
  └─ 存入 SettingsService (Arc<RwLock<AppSettings>>)

用户修改设置 → settings_save (Tauri IPC / WS RPC)
  └─ dispatcher.save_settings(&settings)
       ├─ settings_service.update()
       │   ├─ normalize_settings() 验证
       │   ├─ persist_settings() → 写入 settings.json.tmp → rename
       │   └─ 更新内存中的 settings 单一真实源
       ├─ registry.update_all_settings() 分发到各后端：
       │   ├─ DownloadManager::apply_settings()
       │   │   ├─ BufferPool::update_limits()（io_baseline）
       │   │   ├─ RateLimiter::set_rate()（global_speed_limit_bps）
       │   │   └─ HttpClientFactory / proxy 重新初始化
       │   └─ IrontideBtBackend::update_settings()（bt）
       └─ cdn_service 同步与清理
```

## 设计决策与约定

- 配置通过 JSON 文件持久化，不是 SQLite（下载任务数据用 SQLite）。
- `SettingsService` 为全局设置的唯一权威源，禁止任何直接读写裸 `settings.json` 或私存副本造成漂移。
- `normalize_settings` 是关键验证点，所有从外部进入的设置必须经过此函数。
- load_settings 有一条 legacy proxy 回退分支：当 JSON 仅含 ProxySettings 旧格式字段时，自动构造完整 AppSettings 并注入 proxy 设置。
- HTTP 客户端在设置变更时需要重建（代理、UA 变更由 HttpClientFactory 负责）。
- disk_type_overrides 允许用户强制指定某个目录的磁盘类型，覆盖自动检测结果。
- 序列化约定：所有 struct 用 `#[serde(rename_all = "camelCase")]`，枚举用 `#[serde(rename_all = "snake_case")]`。
