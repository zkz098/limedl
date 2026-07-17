# Subsystem: RateLimiter

## 模块职责

全局令牌桶速率限制器，控制所有下载任务的总带宽消耗。提供异步（tokio）和同步（spawn_blocking）两种消费接口。Phase 8 将统一 HTTP 和 BT 的速率控制，替代 BT 当前的 `paused_by_limit: DashMap` 手动暂停机制。

**涉及文件**：
- `src-tauri/src/download/rate_limiter/mod.rs` (306 行) — RateLimiter 结构体 + 测试

## 关键结构体

### RateLimiter (pub)
```rust
pub struct RateLimiter {
    inner: Arc<Mutex<Inner>>,
}
```
线程安全的令牌桶。Clone 通过 Arc 实现，整个应用共享单个实例。

### Inner (私有)
```rust
struct Inner {
    rate: u64,           // 字节/秒限制 (0 = 无限制)
    capacity: u64,       // 令牌桶容量 = 2 * rate (至少 1)
    tokens: f64,         // 当前令牌数
    last_refill: Instant, // 上次补充时间
}
```

## 关键方法

```rust
impl RateLimiter {
    // 更新速率限制 (bytes/sec, 0 = unlimited)
    pub fn set_rate(&self, new_rate: u64)

    // 异步消费 n 字节令牌。限速时暂停当前 task，无限速时立即返回
    pub async fn consume(&self, n: usize)

    // 阻塞消费（供 spawn_blocking 使用）
    pub fn consume_blocking(&self, n: usize)
}
```

## 数据流向

```
启动时
  ↓
DownloadManager::new(rate_limiter: Arc<RateLimiter>)
  ↓ 存储在manager.rate_limiter字段

HTTP 下载（http_executor.rs）
  ├─ chunk worker 接收数据块后
  ├─ rate_limiter.consume(chunk.len()).await
  └─ 超过限速时 tokio::time::sleep 等待令牌恢复

BT 下载（Phase 8）
  ├─ alert bridge 检测到下载/上传流量
  ├─ rate_limiter.consume(bytes).await
  └─ 替代当前 paused_by_limit DashMap 手动暂停机制

设置变更
  ↓
manager.update_settings() → rate_limiter.set_rate(global_speed_limit_bps)
```

**重要约定**：
- 速率 0 表示无限速，`consume()` 和 `consume_blocking()` 立即返回
- `set_rate()` 在切换速率时保留已积累的令牌（按新容量封顶）
- 令牌桶容量 = `max(2 * rate, 1)`，提供初始突发能力
- `consume()` 内部使用 `tokio::time::sleep`（异步），`consume_blocking()` 使用 `std::thread::sleep`
- 锁仅用于简短算术操作，不会跨 await 点持有
- Clone 通过 Arc，零拷贝
- Phase 8 将统一 HTTP 和 BT 的限速：BT 的 upload_policy_loop 和下载循环都将通过 RateLimiter 控制
