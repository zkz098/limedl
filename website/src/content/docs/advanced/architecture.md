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
├── crates/limedl-native/   # 基于 Slint 的原生桌面应用 (FemtoVG 硬件渲染)
├── crates/limedl-server/   # 无头服务端守护进程 (面向 NAS / 软路由 / Docker 的 Aria2 RPC 服务)
└── xtask/                  # 构建期与发布期工具：后量子密钥、双签名防篡改门禁
```

### 1. `limedl-core`（核心下载引擎）
- **职责**：任务编排、自适应并发调度、HTTP 探测与动态分块、BitTorrent P2P 蜂窝连接、Metalink 4.0/3.0 镜像优选、Cloudflare CDN 探针测速、磁盘缓冲池 I/O、SQLite 任务持久化与 Aria2 JSON-RPC 2.0 服务端。
- **特性**：纯异步（基于 Tokio 异步运行时），完全无界面相关依赖。

### 2. `limedl-native`（原生交互桌面客户端）
- **职责**：基于现代声明式 GUI 框架 **Slint** 打造，使用 FemtoVG 原生渲染后端。负责窗口几何持久化、单实例互斥与 IPC 接管、系统托盘、操作系统电源休眠抑制、通知与密码学自更新。
- **与引擎通信**：客户端与 `limedl-core` 处于**同一个操作系统进程中**。所有业务调用直接通过内存函数分发，彻底消除了跨进程或基于本地 HTTP 的序列化开销。

### 3. `limedl-server`（无头服务端守护进程）
- **职责**：面向 NAS、软路由、Linux 服务器与 Docker 容器。以轻量纯异步守护进程运行，共享底层核心引擎并暴露标准 Aria2 JSON-RPC 2.0 接口，支持多客户端独立 Token 与优雅停机。

### 4. `xtask`（安全与构建守卫）
- **职责**：在发布流水线中管理 Minisign 与 ML-DSA-65 密钥对，并对所有跨平台产物执行密码学双签名；在 CI 阶段通过 `cargo xtask guard` 严格校验签名私钥与客户端内置公钥的一致性，防止软件供应链污染。

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

## 后量子混合双签名自更新架构 (Minisign + ML-DSA-65)

为了防范当前及未来量子计算环境下的中间人攻击（MITM）与软件供应链投毒，limedl 引入了 **经典密码学 (Ed25519) + 后量子密码学 (ML-DSA-65 / NIST FIPS 204)** 的端到端混合双签名防御体系：

```
1. GitHub Actions 流水线构建跨平台 Release 资产
2. xtask 工具执行双签名：生成 .sig (Minisign) 与 .pqc.sig (ML-DSA-65)
3. 生成全局更新元清单 latest-native.json 并分别生成两种签名
                     │
                     ▼ HTTPS 安全拉取
4. 客户端向官方最新 Release 获取 latest-native.json 及对应的双签名
5. 在反序列化 JSON 之前，同时使用固化公钥完成 Minisign 与 ML-DSA-65 双验签
6. 下载匹配当前平台的二进制安装包 / 归档
7. 校验下载文件的 SHA-256 哈希以及工件本身的独立双签名
8. 调用 self_replace 库在进程原地原子替换旧版文件并安全重启
```

- **域隔离保护 (Domain Separation)**：ML-DSA-65 签名强制注入了域上下文字符串（清单使用 `limedl-manifest`，安装包工件使用 `limedl-artifact`），彻底阻断跨上下文签名重放攻击。
- **发布门禁严格约束 (`cargo xtask guard`)**：CI 发布流水线会在打包后从密钥派生公钥，强制与客户端内固化的 `PUBKEY_B64` 和 `PQC_PUBKEY_B64` 比对；若有任何不匹配立即阻断发布。
- **通道适配覆盖**：
  - Windows 安装版：拉取 `setup.exe` 并调用 `/P /R` 静默升级；
  - Windows 便携版：通过 `self_replace` 原地无缝替换 `limedl-native.exe`；
  - macOS 归档包：原地替换 `.app` 包体内的执行文件并重新执行 Ad-hoc 签名；
  - Linux AppImage：自动识别并安全提示下载新版。
