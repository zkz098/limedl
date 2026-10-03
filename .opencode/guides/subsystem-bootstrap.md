# Subsystem: Bootstrap (CoreSystems)

## 模块职责

**所有核心子系统的唯一初始化入口**。桌面客户端（Slint）调用 `bootstrap()`，按正确依赖顺序初始化全部子系统，返回统一的 `CoreSystems` 句柄集合。

> 新增子系统时，在此处添加初始化逻辑，不要在 `limedl-native/src/main.rs` 里单独写——入口只调用 `bootstrap()` 并传递返回的 Arc 句柄。

核心类型：CoreSystems（聚合所有 Arc<子系统> 句柄的扁平结构体）。

## 涉及文件

- `crates/limedl-core/src/bootstrap.rs` — 唯一实现文件（84 行）
- 调用方：`crates/limedl-native/src/main.rs`（桌面客户端）

## 初始化顺序（强依赖链）

```
bootstrap(state_dir)
  ├─ 1. RateLimiter::default()          ← 无依赖，先建
  ├─ 2. EventBus::new(8192)             ← 无依赖，先建
  ├─ 3. DownloadManager::new(state_dir, rate_limiter, event_bus)
  │      └─ start_scheduler_loop()      ← 后台调度启动
  ├─ 4. initial_settings()              ← 从 DownloadManager 获取 AppSettings
  ├─ 5. IrontideBtBackend::new(settings, state_dir, output_dir, event_bus, ...)
  │      ├─ spawn_upload_policy_loop()  ← 上传策略循环
  │      └─ setup_alert_bridge()        ← irontide → EventBus 告警桥接
  ├─ 6. BackendRegistry::new()
  │      ├─ register_arc(TaskKind::Http, download_manager)
  │      └─ register_arc(TaskKind::Bt, bt_backend)
  └─ 7. CdnService::new()               ← 独立初始化
```

## CoreSystems 字段

| 字段               | 类型                     | 说明             |
| ------------------ | ------------------------ | ---------------- |
| `download_manager` | `Arc<DownloadManager>`   | HTTP 下载管理器  |
| `bt_backend`       | `Arc<IrontideBtBackend>` | irontide BT 后端 |
| `registry`         | `Arc<BackendRegistry>`   | 协议路由注册表   |
| `event_bus`        | `Arc<EventBus>`          | 统一事件总线     |
| `rate_limiter`     | `Arc<RateLimiter>`       | 全局令牌桶限速器 |
| `settings`         | `AppSettings`            | 应用设置快照     |
| `cdn_service`      | `Arc<CdnService>`        | CDN 加速服务     |

## 设计决策与约定

- **单例初始化**：`bootstrap()` 是整个应用中唯一创建核心子系统的地方。桌面客户端和 NAS 均调用此函数，确保初始化逻辑一致。
- **Arc 共享**：`BackendRegistry::register_arc()` 直接存储调用方传入的 `Arc`，而非克隆新 Arc——保证 `CoreSystems.download_manager` 和 registry 内部的 DownloadManager 指向同一对象。
- **Bootstrap 不创建 Aria2RpcServer**：Aria2 RPC 为可选功能（`aria2-rpc` feature），由 `limedl-native/src/main.rs` 在 `bootstrap()` 返回后按需构建。
- **state_dir 目录**：首次调用自动创建，下载数据、数据库、torrent 状态均在其子目录下。
