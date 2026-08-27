# Subsystem: Settings / Configuration

## 模块职责

管理应用配置的加载、验证、持久化和分发。配置以 JSON 格式存储在 settings.json（原子写入：先写 .json.tmp，再 rename），通过 AppSettings 结构体封装所有设置分类。

核心类型：AppSettings（根结构体，含 appearance / proxy / scheduler / download / bt / logging / aria2_rpc / cdn_acceleration / github_mirror / url_rewrite / global_speed_limit_bps / notifications / io_baseline / autostart / setup_completed 等字段）。各子设置结构体定义在 types.rs。

关键枚举：ThreadMode（Fixed / Adaptive）、AdaptiveProfile（Conservative / Balanced / Aggressive）、ChecksumMode（None / Blake3 / Sha256 / Xxh3128）、SchedulerMode（Traditional / Automatic）、ProxyMode（Disabled / System / Manual）、DiskType（Ssd / Hdd）、ColorMode（Light / Dark / System）。

## 涉及文件

- `crates/limedl-core/src/settings.rs` — load_settings / normalize_settings / persist_settings / resolve_user_agent
- `crates/limedl-core/src/types.rs` — AppSettings 及所有子设置结构体定义

## 数据流向

```
应用启动 → DownloadManager::new() → load_settings(settings_path)
  ├─ 尝试读取 settings.json → 反序列化 AppSettings
  │   └─ 若字段齐全（appearance/proxy/scheduler 等任意一个存在）→ 正常解析 + normalize
  │   └─ 若仅有旧格式 ProxySettings（legacy 回退）→ 构造默认 AppSettings + 注入 proxy
  ├─ normalize_settings() → 验证裁剪范围
  └─ 存入 DownloadManager.settings (Arc<RwLock<AppSettings>>)

用户修改设置 → settings_save Tauri 命令
  ├─ DownloadManager::update_settings()
  │   ├─ normalize_settings() 验证
  │   ├─ 更新内存中的 settings
  │   ├─ 分发到各子系统：
  │   │   ├─ BufferPool::update_limits()（io_baseline）
  │   │   ├─ IrontideBtBackend::update_settings()（bt）
  │   │   ├─ RateLimiter::set_rate()（global_speed_limit_bps）
  │   │   └─ CdnAccelerator::init_from_settings()（cdn_acceleration）
  │   └─ persist_settings() → 写入 settings.json.tmp → rename
  └─ 返回完整 AppSettings 给前端更新 UI
```

## 设计决策与约定

- 配置通过 JSON 文件持久化，不是 SQLite（下载任务数据用 SQLite）。
- normalize_settings 是关键验证点，所有从外部进入的设置必须经过此函数。
- load_settings 有一条 legacy proxy 回退分支：当 JSON 仅含 ProxySettings 旧格式字段时，自动构造完整 AppSettings 并注入 proxy 设置。
- HTTP 客户端在设置变更时需要重建（代理、UA 变更由 HttpClientFactory 负责）。
- disk_type_overrides 允许用户强制指定某个目录的磁盘类型，覆盖自动检测结果。
- 添加新设置字段时：在 types.rs 增加字段 → normalize_settings 增加验证 → 前端 settings 增加 UI。
- 序列化约定：所有 struct 用 `#[serde(rename_all = "camelCase")]`，枚举用 `#[serde(rename_all = "snake_case")]`。
