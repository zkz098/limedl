# Subsystem: Settings / Configuration

## 模块职责

管理应用配置的加载、验证、持久化和分发。配置以 JSON 格式存储在 `settings.json`（原子写入），通过 `AppSettings` 结构体封装所有设置分类。同时负责构建 reqwest HTTP 客户端（代理、UA、超时等）。

**涉及文件**：
- `src-tauri/src/download/settings.rs` (365 行) — 设置加载/验证/持久化 + HTTP 客户端构建
- `src-tauri/src/download/types.rs` (970 行) — AppSettings 及所有子设置结构体定义

## 关键结构体

### AppSettings (pub)
所有设置的根结构体：
```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub appearance: AppearanceSettings,
    pub proxy: ProxySettings,
    pub scheduler: SchedulerSettings,
    pub download: DownloadDefaultsSettings,
    pub bt: BtSettings,
    pub logging: LogSettings,
    pub aria2_rpc: Aria2RpcSettings,
    pub cdn_acceleration: CdnAccelerationSettings,
    pub github_mirror: GitHubMirrorSettings,
    pub global_speed_limit_bps: u64,
    pub notifications: NotificationSettings,
    pub io_baseline: IoBaselineSettings,
}
```

### 子设置结构体（节选关键字段）
```rust
pub struct ProxySettings {
    pub mode: ProxyMode,         // Disabled | System | Manual
    pub manual_url: String,
}

pub struct SchedulerSettings {
    pub mode: SchedulerMode,     // Traditional | Automatic
    pub traditional: TraditionalSchedulerSettings,   // max_parallel_tasks
    pub automatic: AutomaticSchedulerSettings,       // max_parallel_threads, max/min_threads_per_task, adaptive_profile
    pub chunk_size_strategy: ChunkSizeStrategy,      // Adaptive | Fixed
}

pub struct DownloadDefaultsSettings {
    pub default_download_dir: String,
    pub default_max_retries: u32,
    pub default_checksum: ChecksumMode,      // None | Blake3 | Sha256 | Xxh3128
    pub default_user_agent: String,
}

pub struct IoBaselineSettings {
    pub buffer_limit_mb: u64,
    pub game_mode_buffer_mb: u64,
    pub game_mode: bool,
    pub max_parallel_hdd: u32,
    pub game_mode_max_parallel: u32,
    pub disk_type_overrides: HashMap<String, DiskType>,  // 目录 → DiskType 覆盖
}
```

### 关键枚举
```rust
pub enum ThreadMode { Fixed, Adaptive }        // default: Adaptive
pub enum AdaptiveProfile { Conservative, Balanced, Aggressive }  // default: Balanced
pub enum ChecksumMode { None, Blake3, Sha256, Xxh3128 }  // default: Blake3
pub enum SchedulerMode { Traditional, Automatic }        // default: Automatic
pub enum ProxyMode { Disabled, System, Manual }         // default: Disabled
pub enum DiskType { Ssd, Hdd }                           // default: Ssd
pub enum ColorMode { Light, Dark, System }               // default: System
```

## 关键方法

### 设置加载/持久化 (settings.rs)
```rust
// 从 settings.json 加载，失败回退到 AppSettings::default()
pub(crate) fn load_settings(settings_path: &Path) -> Result<AppSettings>

// 原子写入：先写 .json.tmp，再 rename，确保完整性
pub(crate) async fn persist_settings(settings_path: &Path, settings: &AppSettings) -> Result<()>

// 验证并规范化所有设置字段（范围裁剪、默认值填充）
pub(crate) fn normalize_settings(settings: AppSettings) -> Result<AppSettings>
```

### HTTP 客户端构建 (settings.rs)
```rust
// 根据 AppSettings 构建标准 reqwest::Client
pub(crate) fn build_http_client(settings: &AppSettings) -> Result<Client>

// 共享的 builder 配置逻辑（标准客户端 + CDN 加速客户端共用）
pub(crate) fn configure_client_builder(mut builder: ClientBuilder, settings: &AppSettings) -> Result<ClientBuilder>
```

### User-Agent 解析 (settings.rs)
```rust
// 解析最终使用的 UA：请求中的 UA > 默认 UA > Chrome 内置回退 UA
pub(crate) fn resolve_user_agent(request_user_agent: Option<&str>, default_user_agent: &str) -> Result<String>
```

## 数据流向

```
应用启动
  ↓
DownloadManager::new() → load_settings(settings_path)
  ├─ 尝试读取 settings.json → 反序列化 AppSettings
  ├─ normalize_settings() → 验证裁剪
  └─ 存入 DownloadManager.settings (Arc<RwLock<AppSettings>>)

用户修改设置（前端 SettingsPage.vue）
  ↓
settings_save() Tauri 命令
  ├─ DownloadManager::update_settings()
  │    ├─ normalize_settings() 验证
  │    ├─ 更新内存中的 settings
  │    ├─ 分发到各子系统：
  │    │    ├─ BufferPool::update_limits()  (io_baseline)
  │    │    ├─ OwnBtBackend::update_settings()  (bt)
  │    │    ├─ RateLimiter::update_limit()  (global_speed_limit_bps)
  │    │    └─ CdnAccelerator::init_from_settings()  (cdn_acceleration)
  │    └─ persist_settings() → 写入 settings.json.tmp → rename
  └─ 返回完整 AppSettings 给前端更新 UI
```

**重要约定**：
- 配置通过 JSON 文件持久化，不是 SQLite（只有下载任务数据用 SQLite）
- `normalize_settings()` 是关键验证点，所有从外部进入的设置必须经过此函数
- HTTP 客户端在设置变更时需要重建（代理、UA 变更）
- `disk_type_overrides` 允许用户强制指定某个目录的磁盘类型，覆盖自动检测结果
- 添加新设置字段时：在 types.rs 增加字段 → normalize_settings 增加验证 → 前端 settings 增加 UI
