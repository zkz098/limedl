---
title: 系统架构与技术内幕
description: 深入了解 limedl 的纯 Rust 单轨化架构、协议路由、EventBus 事件总线与密码学自更新设计
---

# 系统架构与技术内幕

limedl 采用了纯 **Rust 2024** 单轨化（Single-Track）设计。在架构演化中，项目主动淘汰了早期繁重、迟缓且存在安全隐患的混合架构（Electron / WebView），将整个系统提炼为无额外运行时依赖的现代化高性能桌面应用。

---

## 整体工作空间架构

limedl 代码库由三个高度解耦的子项目构成：

```
limedl/
├── crates/limedl-core/     # 纯 Rust 多协议下载引擎 (无任何 GUI 依赖)
├── crates/limedl-native/   # 基于 Slint 的原生桌面应用 (Skia 硬件渲染)
└── xtask/                  # 构建期与发布期工具：Minisign 密钥、签名防篡改门禁
```

### 1. `limedl-core`（核心下载引擎）
- **职责**：任务编排、自适应并发调度、HTTP 探测与动态分块、BitTorrent P2P 蜂窝连接、Cloudflare CDN 探针测速、磁盘缓冲池 I/O、SQLite 任务持久化与 Aria2 JSON-RPC 2.0 服务端。
- **特性**：纯异步（基于 Tokio 异步运行时），完全无界面相关依赖。

### 2. `limedl-native`（原生交互桌面客户端）
- **职责**：基于现代声明式 GUI 框架 **Slint** 打造，使用 Skia 原生渲染后端。负责窗口几何持久化、单实例互斥与 IPC 接管、系统托盘、操作系统电源休眠抑制、通知与密码学自更新。
- **与引擎通信**：客户端与 `limedl-core` 处于**同一个操作系统进程中**。所有业务调用直接通过内存函数分发，彻底消除了跨进程或基于本地 HTTP 的序列化开销。

### 3. `xtask`（安全与构建守卫）
- **职责**：在发布流水线中生成 Minisign 密钥对，并对所有跨平台产物执行密码学签名；在 CI 阶段通过 `cargo xtask guard` 严格校验签名私钥与客户端内置公钥的一致性，防止软件供应链污染。

---

## 核心子系统与协议路由

```
                   用户交互 / 外部协议唤醒 / RPC 远程调用
                                     │
                                     ▼
                      ┌────────────────────────────┐
                      │    核心门面 Dispatcher      │
                      └──────────────┬─────────────┘
                                     │
                                     ▼
                      ┌────────────────────────────┐
                      │ BackendRegistry 协议路由器  │
                      └──────┬──────────────┬──────┘
                             │              │
       [TaskId 以 "http:" 开头]│              │[TaskId 以 "bt:" 开头]
                             ▼              ▼
                ┌──────────────────┐  ┌──────────────────┐
                │ DownloadManager  │  │IrontideBtBackend │
                │ (HTTP/HTTPS 并发)│  │ (BitTorrent 内核) │
                └────────┬─────────┘  └────────┬─────────┘
                         │                     │
                         └──────────┬──────────┘
                                    │
                                    ▼
                ┌────────────────────────────────────────┐
                │ 共享底层基础设施:                       │
                │ • BufferPool (SSD 写合并 / HDD 双缓冲) │
                │ • Database (SQLite WAL 模式原子存储)   │
                │ • RateLimiter (全局高精度令牌桶)       │
                │ • SettingsService (单一真实源配置)     │
                │ • EventBus (广播通道)                  │
                └────────────────────────────────────────┘
```

### 1. 协议路由机制 (BackendRegistry)
limedl 统一定义了 `DownloadBackend` trait：
- 系统生成的每个任务均拥有全局唯一的 `TaskId`；
- `TaskId` 自带语义前缀：以 `http:` 开头的任务交由 `DownloadManager` 处理，以 `bt:` 开头的任务路由至 `IrontideBtBackend`；
- 外层 `Dispatcher` 统一暴露启动、暂停、恢复、取消、移除等生命周期方法，屏蔽了具体底层协议差异。

### 2. ZST Actor 无锁任务编排
为了避免在复杂的多任务异步编排中出现循环引用或死锁，`DownloadManager` 拆分为了 3 个零尺寸类型（ZST）Actor：
- **HttpExecutor**：纯函数式处理元数据探测、Range 请求头生成与网络流提取；
- **Scheduler**：独立的后台调度协程，每 2 秒运行一次，评估 AIMD 吞吐量状态机并重新分配线程预算；
- **TaskLifecycle**：负责文件清理、进度计算与数据库状态推进。

---

## 响应式事件系统 (EventBus)

limedl 摒弃了传统的轮询检查机制，在系统核心维护了一个 `tokio::sync::broadcast` 全局事件通道：

```
                        核心任务状态流转
                               │
                               ▼
        ┌─────────────────────────────────────────────┐
        │        EventBus::publish(DownloadEvent)     │
        └──────────────────────┬──────────────────────┘
                               │
            ┌──────────────────┴──────────────────┐
            ▼                                     ▼
┌───────────────────────────────┐ ┌───────────────────────────────┐
│ limedl-native 桌面订阅协程    │ │ Aria2RpcServer 广播中继       │
│ • 解析穷尽变体 (Exhaustive)   │ │ • 过滤特定 GID 状态变更       │
│ • 计算变化差量                │ │ • 向已连接 WebSocket 推送   │
│ • slint::invoke_from_event_loop│ │   aria2.onDownloadStart 等通知│
│ • 更新 UI 模型并触发微秒级重绘│ └───────────────────────────────┘
└───────────────────────────────┘
```

- **类型安全与穷尽匹配**：`limedl-native` 的主事件循环对 `DownloadEvent` 的所有变体进行**穷尽匹配（Exhaustive Match，无 `_ => {}` 兜底）**，确保引擎底层新增任何事件时，编译器都会强制要求前端进行显式处理。
- **无锁响应**：UI 线程只需接收计算后的视图差量，即使引擎在千兆带宽下处理上万次数据块流转，主窗口界面始终保持 60+ FPS 丝滑顺畅。

---

## Minisign 密码学防篡改自更新架构

为了防止分发更新包在 CDN 传输或跨国网络镜像中被中间人攻击（MITM）篡改，limedl 设计了端到端的密码学安全自更新流水线：

```
1. GitHub Actions 流水线构建跨平台 Release 资产
2. 使用专用私钥为每个二进制生成 Minisign 签名 (.sig)
3. 生成全局更新元清单 latest-native.json 并上传
                     │
                     ▼ HTTPS 安全拉取
4. 客户端向官方最新 Release 获取 latest-native.json
5. 使用客户端内固化的公钥 (PUBKEY_B64) 核验清单签名
6. 下载匹配当前平台的二进制安装包 / 归档
7. 验证下载包的 SHA-256 哈希与 Minisign 签名
8. 调用 self_replace 库在进程原地原子替换旧版文件并安全重启
```

- **内置硬编码公钥**：客户端内部硬编码了唯一的发布公钥，任何未通过私钥合法签名的恶意更新包均会被客户端瞬间拒绝并告警；
- **通道适配覆盖**：
  - Windows 安装版：拉取 `setup.exe` 并调用 `/P /R` 静默升级；
  - Windows 便携版：通过 `self_replace` 原地无缝替换 `limedl-native.exe`；
  - macOS 归档包：原地替换 `.app` 包体内的执行文件并重新执行 Ad-hoc 签名；
  - Linux AppImage：自动识别并安全提示下载新版。
