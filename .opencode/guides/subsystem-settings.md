# Subsystem: Settings / Configuration + SettingsService

## 模块职责

管理应用配置的加载、验证、持久化和分发。配置以 JSON 格式存储在 settings.json（原子写入：先写 .json.tmp，再 rename）。通过 `SettingsService` 维护内存与磁盘的单一事实源（Single Source of Truth），通过 `AppSettings` 结构体封装所有设置分类。

核心类型：

- `SettingsService`：提供 `get()`、`get_blocking()`、`update(&AppSettings)`、`update_with(FnOnce(&mut AppSettings) -> Result<()>)`、`factory_reset()`、`default_download_dir()`。`update`/`update_with` 共用一把 `update_lock`，把读-改-写串行化，两个并发保存不会各自从同一份旧值出发而互相覆盖。
- `AppSettings`（根结构体，含 appearance / proxy / scheduler / download / bt / logging / aria2_rpc / cdn_acceleration / github_mirror / url_rewrite / global_speed_limit_bps / notifications / io_baseline / autostart / setup_completed 等字段）。各子设置结构体定义在 `types.rs`。

关键枚举：ThreadMode（Fixed / Adaptive）、AdaptiveProfile（Conservative / Balanced / Aggressive）、ChecksumMode（None / Blake3 / Sha256 / Sha512）、SchedulerMode（Traditional / Automatic）、ProxyMode（Disabled / System / Manual）、DiskType（Ssd / Hdd / Network）、ColorMode（Light / Dark / System）。

## 涉及文件

- `crates/limedl-core/src/services/settings_service.rs` — SettingsService 单一事实源服务
- `crates/limedl-core/src/settings/mod.rs` — load_settings / normalize_settings / persist_settings / resolve_user_agent
- `crates/limedl-core/src/types.rs` — AppSettings 及所有子设置结构体定义

## 数据流向

```
应用启动 → SystemContext::new() → SettingsService::new(settings_path)
  ├─ 尝试读取 settings.json → 直接反序列化 AppSettings
  │   （每个字段都带 `#[serde(default)]`，缺失/未知键均可容忍；不再有 legacy 回退）
  ├─ normalize_settings() → 验证裁剪范围
  └─ 存入 SettingsService (Arc<RwLock<AppSettings>>)

用户修改设置 → 桌面 / aria2 RPC 一律经 Dispatcher
  └─ dispatcher.save_settings_with(|settings| { ...修改...; Ok(()) })
       ├─ settings_service.update_with()
       │   ├─ 取得 update_lock（串行化读-改-写）
       │   ├─ 在当前值上执行闭包 → normalize_settings() 验证
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
- **写设置一律走 `Dispatcher::save_settings_with`**（或它的整体替换包装 `save_settings`）：闭包拿到的是**最新**的 `AppSettings`，在事务内修改。不要再写“读一份 clone → 改 → 整体 `save_settings`”的读-改-写，那会丢掉并发写入。桌面端曾有一份 `AppContext::current_settings` 影子副本（九个回写点），已删除；读设置统一用 `Dispatcher::get_settings_blocking()`（native 封装为 `AppContext::settings()`）。
- `normalize_settings` 是关键验证点，所有从外部进入的设置必须经过此函数。
- `load_settings` 只做「读文件 → `serde_json::from_str::<AppSettings>` → `normalize_settings`」。`AppSettings` 每个字段都有 `#[serde(default)]`，缺失字段与未知键（如已废弃的 `githubMirror`）都被容忍；旧的裸 `ProxySettings`、`sha1` checksum、`githubMirror` 迁移均已删除。
- HTTP 客户端在设置变更时需要重建（代理、UA 变更由 HttpClientFactory 负责）。
- disk_type_overrides 允许用户强制指定某个目录的磁盘类型，覆盖自动检测结果。键是目录，查找是规范化路径的**前缀**匹配（最长键优先），所以 `D:\dl` 也会命中 `D:\dl\sub\a.bin`；匹配规则与分类器在 `file_ops/media.rs`（`normalize_media_path` / `MediaOverrides` / `is_network_filesystem`）。
- 编辑器在 **设置 → IO 实验室 → 目录介质覆盖**（`tab_io.slint`，行状态与限速计划同构：保存在 UI model，Save 时由 `parse_disk_type_overrides` 解析），校验拒绝空、非绝对与重复路径；保存后 `DownloadManager::apply_settings` 把覆盖推给 `DiskDeviceManager`，所以设备队列的写线程数也会跟着变。
- 序列化约定：所有 struct 用 `#[serde(rename_all = "camelCase")]`，枚举用 `#[serde(rename_all = "snake_case")]`。
- **Aria2 RPC 鉴权设置**：`aria2Rpc.authMode`（`single` / `per_client`，默认 `single` 保证升级向后兼容）与 `aria2Rpc.clients`（`Vec<Aria2Client>`：`id` / `name` / `tokenHash` / `createdAtMs`）。`tokenHash` 是 Argon2id PHC 串，**绝不存明文令牌**。`normalize_aria2_rpc_settings` 会裁掉 `tokenHash` 为空的条目（永远无法验证通过，只会成为无效 UI 行）并 trim 名称/哈希。旧的仅含 `secret` 的 settings.json 反序列化后仍是 `single` 模式。详见 `subsystem-aria2-rpc.md`。
