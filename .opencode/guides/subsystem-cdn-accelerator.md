# Subsystem: CdnAccelerator / CdnService

## 模块职责

通过探测 Cloudflare IP 范围中延迟最低的节点，创建 DNS 重写后的 `reqwest::Client` 加速 HTTP 下载。**当前硬编码 Cloudflare，未抽象为多 CDN 架构。如需支持其他 CDN（Akamai、Fastly 等），需重构此模块。**

**涉及文件**：

- `crates/limedl-core/src/cdn/mod.rs` — 模块导出
- `crates/limedl-core/src/cdn/accelerator.rs` — CdnAccelerator 状态机
- `crates/limedl-core/src/cdn/service.rs` — **CdnService 统一抽象层（新增）**
- `crates/limedl-core/src/cdn/ip_ranges.rs` — Cloudflare IP 范围抓取/解析
- `crates/limedl-core/src/cdn/resolver.rs` — DNS 重写 + 加速 HTTP 客户端构建
- `crates/limedl-core/src/cdn/speed_test.rs` — 速度测试逻辑
- `src-tauri/src/download/commands_cdn.rs` — Tauri CDN 命令（通过 CdnService 委派）
- `crates/limedl-server/src/rpc.rs` (CDN handlers 区) — NAS WebSocket CDN 命令（通过 CdnService 委派）

## 关键结构体

### CdnService (pub) — 统一抽象层

```rust
pub struct CdnService {
    accelerator: Arc<CdnAccelerator>,   // 内部状态机
}
```

`CdnService` 包装 `CdnAccelerator`，对外暴露 CDN 操作的一组稳定方法。**Tauri 桌面和 NAS WebSocket 两种前端都通过此服务调用 CDN 操作**，消除了 commands_cdn.rs 和 rpc.rs 之间的重复实现。

### CdnTestOutcome (pub)

```rust
pub struct CdnTestOutcome {
    pub state: AccelState,
    pub active_ip: Option<Ipv4Addr>,
    pub active_speed_mbps: Option<f64>,
    pub candidates: Vec<SpeedTestResult>,
    pub default_node: Option<DefaultNodeResult>,
}
```

由 `monitor_test()` 返回，供调用方（Tauri/NAS handler）持久化结果到 settings。

### CdnAccelerator (pub)

```rust
pub struct CdnAccelerator {
    state: RwLock<AccelState>,
    active_ip: RwLock<Option<Ipv4Addr>>,
    active_speed_mbps: RwLock<Option<f64>>,
    cancel_token: RwLock<Option<CancellationToken>>,
    accelerated_client: RwLock<Option<reqwest::Client>>,   // DNS 重写后的 HTTP 客户端
    /// Atomic phase indicator: 0=FetchingRanges, 1=Screening,
    /// 2=MeasuringThroughput, 0xFF=None. Written from sync progress
    /// callbacks without spawning.
    phase_atomic: AtomicU8,
    phase_progress_current: AtomicU64,
    phase_progress_total: AtomicU64,
    all_candidates: RwLock<Vec<SpeedTestResult>>,
    default_node: RwLock<Option<DefaultNodeResult>>,
}
```

### AccelState (pub)

```rust
pub enum AccelState { Idle, Testing, Ready, Error(String) }
```

- `Idle`: 未启用或已清除
- `Testing`: 正在测试各 IP 节点（FetchingRanges → Screening → MeasuringThroughput）
- `Ready`: 已有选中的加速 IP，`accelerated_client` 可用
- `Error(String)`: 测试失败，含错误信息

## 关键方法

### CdnService (统一接口)

```rust
// 构造
pub fn new() -> Self
pub fn from_accelerator(accelerator: Arc<CdnAccelerator>) -> Self
pub fn accelerator(&self) -> &Arc<CdnAccelerator>

// 生命周期
pub async fn start_test(self: &Arc<Self>, settings: AppSettings) -> anyhow::Result<()>
pub fn cancel_test(&self)
pub async fn apply_ip(&self, ip: Ipv4Addr, speed_mbps: f64, settings: &AppSettings) -> anyhow::Result<()>
pub async fn clear(&self)
pub async fn init_from_settings(self: &Arc<Self>, settings: &AppSettings)

// 查询
pub async fn status(&self) -> AccelState
pub async fn active_ip(&self) -> Option<Ipv4Addr>
pub async fn active_speed_mbps(&self) -> Option<f64>
pub async fn phase(&self) -> Option<CdnTestPhase>
pub async fn phase_progress(&self) -> (u64, u64)
pub async fn candidates(&self) -> Vec<SpeedTestResult>
pub async fn default_node(&self) -> Option<DefaultNodeResult>
pub async fn get_client(&self) -> Option<reqwest::Client>

// 监控（在 background task 中调用，自动发布进度/完成事件到 EventBus）
pub async fn monitor_test(self: &Arc<Self>, event_bus: Arc<EventBus>) -> CdnTestOutcome
```

### CdnAccelerator（内部状态机，通常不直接使用）

```rust
pub fn new() -> Self
pub async fn init_from_settings(self: &Arc<Self>, settings: &AppSettings)
pub async fn start_test(self: &Arc<Self>, settings: AppSettings) -> anyhow::Result<()>
pub fn cancel_test(&self)
pub async fn apply_ip(&self, ip: Ipv4Addr, speed_mbps: f64, settings: &AppSettings) -> anyhow::Result<()>
pub async fn clear(&self)
pub async fn status(&self) -> AccelState
pub async fn get_client(&self) -> Option<reqwest::Client>
pub async fn active_ip(&self) -> Option<Ipv4Addr>
pub async fn active_speed_mbps(&self) -> Option<f64>
pub async fn phase(&self) -> Option<CdnTestPhase>       // 内部用 AtomicU8 load，无 RwLock 争用
pub async fn phase_progress(&self) -> (u64, u64)        // 内部从两个独立 AtomicU64 load
pub async fn candidates(&self) -> Vec<SpeedTestResult>
pub async fn default_node(&self) -> Option<DefaultNodeResult>
```

> **性能优化**：`phase_atomic`、`phase_progress_current`、`phase_progress_total` 使用 `AtomicU8`/`AtomicU64` 替代原来的 `RwLock<Option<CdnTestPhase>>` + `RwLock<(u64, u64)>`。`progress_cb` 闭包内直接做 atomic store，消除了每 speed test 55+ 次 `tokio::spawn` 的开销。`CdnTestPhase` 标记 `#[repr(u8)]`，`PHASE_NONE = 0xFF` 作为"无活跃阶段"的哨兵值。

## 数据流向（重构后）

```
用户打开 CDN 加速面板（LabsCdnAccelerationPanel.vue）
  ↓
点击"开始测试"
  ↓
cdn_fetch_ranges() → ip_ranges.rs 抓取 Cloudflare IP 范围列表
  ↓
cdn_test() → CdnService::start_test() → CdnAccelerator::start_test()
  ├─ Phase: FetchingRanges → 获取所有 Cloudflare IP 段
  ├─ Phase: Screening → TCP connect 筛选低延迟 IP
  ├─ Phase: MeasuringThroughput → 对每个候选 IP 做速度测试
  └─ 完成 → AccelState::Ready + 排序 candidates
  ↓
CdnService::monitor_test() → 轮询并发布 CdnProgress / CdnComplete 事件到 EventBus
  ↓
Tauri / WebSocket 事件适配层将 EventBus 事件转发到前端
  ↓
用户选择 IP → cdn_apply()
  ├─ CdnService::apply_ip() → resolver.rs 构建 DNS 重写客户端
  └─ accelerated_client 存入 CdnAccelerator
  ↓
DownloadManager 使用 accelerated_client 发起下载请求
  ├─ set_cdn_accelerator() 注入（内部仍然使用 CdnAccelerator）
  └─ 下载时: 若 accelerated_client 存在则使用，否则使用标准 client
```

### 统一服务注入（bootstrap.rs）

`CoreSystems` 包含 `cdn_service: Arc<CdnService>`，Tauri 和 NAS 各自通过 `core.cdn_service.clone()` 注入到自己的状态对象中：

- **Tauri**: `AppState.cdn_service`
- **NAS**: `RpcState.cdn_service`

`DownloadManager` 内部仍然通过 `set_cdn_accelerator()` 持有 `Arc<CdnAccelerator>`（从 `CdnService::accelerator()` 获取），用于下载时的客户端选择。

### NAS 端 CDN handler 变更（rpc.rs）

| 命令 | 之前 (重构前) | 之后 (重构后) |
|------|--------------|--------------|
| `cdn.fetchRanges` | 直接读取静态列表 | **不变** |
| `cdn.status` | 从 settings 读取 | **通过 CdnService 读取实时状态** |
| `cdn.detail` | 从 settings 读取 | **通过 CdnService 读取实时状态** |
| `cdn.test` | `-32001 not supported` | **实际调用 CdnService::start_test + 后台 monitor** |
| `cdn.apply` | `-32001 not supported` | **实际调用 CdnService::apply_ip** |
| `cdn.clear` | `-32001 not supported` | **实际调用 CdnService::clear** |
| `cdn.cancel` | `-32001 not supported` | **实际调用 CdnService::cancel_test** |
| `cdn.candidates` | `-32001 not supported` | **实际调用 CdnService::candidates** |

**重要约定**：

- CDN 加速仅对使用 Cloudflare CDN 的下载链接有效
- `accelerated_client` 是普通的 `reqwest::Client`，DNS 解析在底层被改写
- 测试流程可被 `cancel_test()` 中断，会重置为 Idle
- 启动时 `init_from_settings()` 从持久化设置恢复之前选择的 IP
- 此模块当前**未设计多 CDN 抽象**，扩展需重构 `ip_ranges.rs` 和 `resolver.rs`
- **NAS 模式现在完整支持 CDN 操作**，不再有限制。CDN 速度测试（纯 async TCP/HTTP 探测）无 OS 依赖，可在 Linux/Windows NAS 上运行。
