# Subsystem: CdnAccelerator / CdnService

## 模块职责

通过探测 Cloudflare IP 范围中延迟最低的节点，创建 DNS 重写后的 reqwest::Client 加速 HTTP 下载。当前硬编码 Cloudflare，未抽象为多 CDN 架构。

核心类型：CdnService（统一抽象层，包装 CdnAccelerator）、CdnAccelerator（内部状态机）、AccelState（Idle / Testing / Ready / Error）、CdnTestOutcome（测试结果汇总）。

通过 EventBus 发布 CdnProgress / CdnComplete 事件。

## 涉及文件

- `crates/limedl-core/src/cdn/mod.rs` — 模块导出
- `crates/limedl-core/src/cdn/accelerator.rs` — CdnAccelerator 状态机（原子 phase 指示器、候选 IP 列表、加速客户端）
- `crates/limedl-core/src/cdn/service.rs` — CdnService 统一抽象层
- `crates/limedl-core/src/cdn/ip_ranges.rs` — Cloudflare IP 范围抓取/解析
- `crates/limedl-core/src/cdn/resolver.rs` — DNS 重写 + 加速 HTTP 客户端构建
- `crates/limedl-core/src/cdn/speed_test.rs` — 速度测试逻辑
- `src-tauri/src/download/commands_cdn.rs` — Tauri CDN 命令
- `crates/limedl-server/src/rpc.rs` (CDN handlers 区) — NAS WebSocket CDN 命令

## 数据流向

```
用户点击"开始测试"
  ↓
CdnService::start_test() → CdnAccelerator::start_test()
  ├─ Phase: FetchingRanges → 获取所有 Cloudflare IP 段
  ├─ Phase: Screening → TCP connect 筛选低延迟 IP
  ├─ Phase: MeasuringThroughput → 对每个候选 IP 做速度测试
  └─ 完成 → AccelState::Ready + 排序 candidates

CdnService::monitor_test() → 轮询 → EventBus::publish(CdnProgress / CdnComplete)
  ↓
Tauri / WebSocket event relay 将事件转发到前端

用户选择 IP → CdnService::apply_ip() → resolver.rs 构建 DNS 重写客户端
  ↓
DownloadManager 使用 accelerated_client 发起下载请求
```

## 设计决策与约定

- CDN 加速仅对使用 Cloudflare CDN 的下载链接有效。
- accelerated_client 是普通的 reqwest::Client，DNS 解析在底层被改写。
- 测试流程可被 cancel_test() 中断，重置为 Idle。
- 启动时 `init_from_settings()` 从持久化设置恢复之前选择的 IP。
- Tauri 和 NAS WebSocket 两种前端都通过 CdnService 调用 CDN 操作，消除了重复实现。
- CdnService 通过 `CoreSystems` 注入：Tauri 端在 `AppState.cdn_service`，NAS 端在 `RpcState.cdn_service`。
- phase 指示器使用 AtomicU8（而非 RwLock），speed test 回调中直接 atomic store，避免 tokio::spawn 开销。
