# Subsystem: CdnAccelerator

## 模块职责

通过探测 Cloudflare IP 范围中延迟最低的节点，创建 DNS 重写后的 `reqwest::Client` 加速 HTTP 下载。**当前硬编码 Cloudflare，未抽象为多 CDN 架构。如需支持其他 CDN（Akamai、Fastly 等），需重构此模块。**

**涉及文件**：
- `src-tauri/src/download/cdn/mod.rs` (9 行) — 模块导出
- `src-tauri/src/download/cdn/accelerator.rs` (408 行) — CdnAccelerator 状态机
- `src-tauri/src/download/cdn/commands.rs` (282 行) — Tauri CDN 命令
- `src-tauri/src/download/cdn/ip_ranges.rs` (369 行) — Cloudflare IP 范围抓取/解析
- `src-tauri/src/download/cdn/resolver.rs` (195 行) — DNS 重写 + 加速 HTTP 客户端构建
- `src-tauri/src/download/cdn/speed_test.rs` (662 行) — 速度测试逻辑

## 关键结构体

### CdnAccelerator (pub(crate))
```rust
pub(crate) struct CdnAccelerator {
    state: RwLock<AccelState>,
    active_ip: RwLock<Option<Ipv4Addr>>,
    active_speed_mbps: RwLock<Option<f64>>,
    cancel_token: RwLock<Option<CancellationToken>>,
    accelerated_client: RwLock<Option<reqwest::Client>>,   // DNS 重写后的 HTTP 客户端
    phase: RwLock<Option<CdnTestPhase>>,
    phase_progress: RwLock<(u64, u64)>,
    all_candidates: RwLock<Vec<SpeedTestResult>>,
    default_node: RwLock<Option<DefaultNodeResult>>,
}
```

### AccelState (pub(crate))
```rust
pub(crate) enum AccelState { Idle, Testing, Ready, Error(String) }
```
- `Idle`: 未启用或已清除
- `Testing`: 正在测试各 IP 节点（FetchingRanges → Screening → MeasuringThroughput）
- `Ready`: 已有选中的加速 IP，`accelerated_client` 可用
- `Error(String)`: 测试失败，含错误信息

## 关键方法

### CdnAccelerator
```rust
pub(crate) fn new() -> Self

// 从持久化设置恢复（启动时调用）
pub(crate) async fn init_from_settings(self: &Arc<Self>, settings: &AppSettings)

// 开始测试流程：FetchingRanges → Screening → MeasuringThroughput → Ready/Error
pub(crate) async fn start_test(self: &Arc<Self>, settings: AppSettings) -> anyhow::Result<()>

// 取消正在进行的测试
pub(crate) fn cancel_test(&self)

// 应用选中的 IP 节点（构建 DNS 重写客户端）
pub(crate) async fn apply_ip(&self, ip: Ipv4Addr, speed_mbps: f64, settings: &AppSettings) -> anyhow::Result<()>

// 清除加速状态，丢弃 accelerated_client
pub(crate) async fn clear(&self)

// 查询
pub(crate) async fn status(&self) -> AccelState
pub(crate) async fn get_client(&self) -> Option<reqwest::Client>
pub(crate) async fn active_ip(&self) -> Option<Ipv4Addr>
pub(crate) async fn active_speed_mbps(&self) -> Option<f64>
pub(crate) async fn phase(&self) -> Option<CdnTestPhase>
pub(crate) async fn phase_progress(&self) -> (u64, u64)
pub(crate) async fn candidates(&self) -> Vec<SpeedTestResult>
pub(crate) async fn default_node(&self) -> Option<DefaultNodeResult>
```

## 数据流向

```
用户打开 CDN 加速面板（LabsCdnAccelerationPanel.vue）
  ↓
点击"开始测试"
  ↓
cdn_fetch_ranges() → ip_ranges.rs 抓取 Cloudflare IP 范围列表
  ↓
cdn_test() → accelerator.start_test()
  ├─ Phase: FetchingRanges → 获取所有 Cloudflare IP 段
  ├─ Phase: Screening → ICMP ping / TCP connect 筛选低延迟 IP
  ├─ Phase: MeasuringThroughput → 对每个候选 IP 做速度测试
  └─ 完成 → AccelState::Ready + 排序 candidates
  ↓
用户选择 IP → cdn_apply()
  ├─ resolver.rs 构建 DNS 重写客户端
  │    └─ 使用 TrustDNS Resolver 将域名解析到选中的 CF IP
  └─ accelerated_client 存入 CdnAccelerator
  ↓
DownloadManager 使用 accelerated_client 发起下载请求
  ├─ set_cdn_accelerator() 注入
  └─ 下载时: 若 accelerated_client 存在则使用，否则使用标准 client
```

**重要约定**：
- CDN 加速仅对使用 Cloudflare CDN 的下载链接有效
- `accelerated_client` 是普通的 `reqwest::Client`，DNS 解析在底层被改写
- 测试流程可被 `cancel_test()` 中断，会重置为 Idle
- 启动时 `init_from_settings()` 从持久化设置恢复之前选择的 IP
- 此模块当前**未设计多 CDN 抽象**，扩展需重构 `ip_ranges.rs` 和 `resolver.rs`
