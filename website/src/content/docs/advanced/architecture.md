---
title: 系统架构与技术内幕
description: 深入了解 limedl 的纯 Rust 单轨化架构、协议路由、EventBus 事件总线与密码学自更新设计
---

# 系统架构与技术内幕

limedl 采用了纯 **Rust 2024** 单轨化（Single-Track）设计。在架构演化中，项目主动淘汰了早期繁重、迟缓且存在安全隐患的混合架构（Electron / WebView），将整个系统提炼为无额外运行时依赖的现代化高性能桌面应用。

---

## 整体工作空间架构

limedl 是一个多 crate 的 Rust 工作区。引擎按职责拆分，前端只通过门面访问：

```
limedl/
├── crates/limedl-types/    # 领域类型、设置、错误、事件、manifest、DownloadBackend
├── crates/limedl-disk/     # file_ops、buffer_pool、io_scheduler
├── crates/limedl-store/    # SQLite 任务/分片/manifest 与 Alt-Svc 缓存
├── crates/limedl-http/     # HTTP client factory、identity 编码、限速器
├── crates/limedl-engine/   # HTTP executor、scheduler、CDN、Metalink、task lifecycle
├── crates/limedl-bt/       # librqbit BT 后端（唯一命名 BT 引擎的 crate）
├── crates/limedl-rpc/      # Aria2 JSON-RPC + WebSocket 服务（位于引擎之上）
├── crates/limedl-testkit/  # 测试用 HTTP 夹具（仅测试依赖）
├── crates/limedl-core/     # 门面：Dispatcher、BackendRegistry、DownloadManager、bootstrap
├── crates/limedl-native/   # 基于 Slint 的桌面客户端（FemtoVG 渲染）
├── crates/limedl-server/   # 无头守护进程（NAS / 软路由 / Docker）
└── xtask/                  # 构建与发布工具：密钥、双签名、门禁
```

- `limedl-core` 是兼容门面：保留 `Dispatcher`、`DownloadManager`、`bootstrap` 与历史 `pub use` 路径，实际 HTTP 执行、调度、CDN、Metalink 在 `limedl-engine`，BT 在 `limedl-bt`。
- `limedl-rpc` 位于引擎/门面之上，`limedl-core` 不依赖它；桌面端与无头服务端各自决定是否启动 RPC。
- `limedl-native` 与 `limedl-server` 各自在自己的进程中直接链接引擎，通过 Rust 函数调用，没有跨进程序列化开销。

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
                │ DownloadManager  │  │  RqbitBtBackend  │
                │ (limedl-core)    │  │ (limedl-bt)      │
                └────────┬─────────┘  └────────┬─────────┘
                         │                     │
                         └──────────┬──────────┘
                                    │
                                    ▼
                ┌────────────────────────────────────────┐
                │ 共享基础设施:                          │
                │ • limedl-disk  BufferPool / FileOps    │
                │ • limedl-store SQLite / Alt-Svc        │
                │ • limedl-http  客户端 / 令牌桶限速     │
                │ • limedl-types 设置 / 事件 / 类型       │
                └────────────────────────────────────────┘
```

### 1. 协议路由机制 (BackendRegistry)
limedl 统一定义了 `DownloadBackend` trait：
- 系统生成的每个任务均拥有全局唯一的 `TaskId`；
- `TaskId` 自带语义前缀：以 `http:` 开头的任务交由 `DownloadManager` 处理，以 `bt:` 开头的任务路由至 `RqbitBtBackend`；
- 外层 `Dispatcher` 统一暴露启动、暂停、恢复、取消、移除等生命周期方法，屏蔽具体协议差异。

### 2. 三个并列 Actor
为了避免复杂异步编排中的循环引用与死锁，`DownloadManager` 将执行职责拆成三个并列 Actor：
- **HttpExecutor**：零尺寸类型，负责元数据探测、Range 请求与网络流提取；
- **Scheduler**：独立的后台调度协程，每 2 秒评估 AIMD 吞吐量并重新分配线程预算；
- **TaskLifecycle**：负责状态推进、文件清理、进度计算与数据库持久化。

Scheduler 与 TaskLifecycle 各自持有运行状态，不是零尺寸类型。

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
