# Subsystem: RateLimiter

## 模块职责

全局令牌桶速率限制器，控制所有下载任务的总带宽消耗。提供异步（tokio）和同步（spawn_blocking）两种消费接口。

核心类型：RateLimiter（线程安全的令牌桶，包装 `Arc<Mutex<Inner>>`）。Inner 包含 rate（字节/秒，0=无限制）、capacity（令牌桶容量 = 2*rate，至少 1）、tokens、last_refill。Clone 通过 Arc 实现，整个应用共享单个实例。

## 涉及文件

- `crates/limedl-core/src/rate_limiter/mod.rs` — RateLimiter + 测试

## 数据流向

```
启动时 → DownloadManager::new(rate_limiter) → 存储在 manager.rate_limiter 字段

HTTP 下载（http_executor）
  ├─ chunk worker 接收数据块后累计 bytes_since_consume
  ├─ 累计达到 ~256KB 或 8 chunk（取先到）时调 rate_limiter.consume()
  ├─ loop 正常退出前 flush 剩余累加字节
  └─ 超过限速时 tokio::time::sleep 等待令牌恢复

BT 下载（Phase 8 规划）→ alert bridge 检测到流量 → rate_limiter.consume()

设置变更 → manager.update_settings() → rate_limiter.set_rate(global_speed_limit_bps)
```

## 设计决策与约定

- 速率 0 表示无限速，consume() 和 consume_blocking() 立即返回。
- set_rate() 在切换速率时保留已积累的令牌（按新容量封顶）。
- 令牌桶容量 = max(2 * rate, 1)，提供初始突发能力。
- consume() 内部使用 tokio::time::sleep（异步），consume_blocking() 使用 std::thread::sleep。
- 锁仅用于简短算术操作，不会跨 await 点持有。
- http_executor 不再每 ~16KB chunk 调一次 consume，改为累积到 ~256KB 或 8 chunk（取先到）才调一次。AIMD 收敛不受影响（采样窗口 2s 远大于批量时间窗口）。
- Phase 8 将统一 HTTP 和 BT 的限速：BT 的 upload_policy_loop 和下载循环都将通过 RateLimiter 控制。
