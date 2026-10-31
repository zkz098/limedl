---
title: 项目简介
description: 了解 limedl 的设计理念、架构特点、核心优势与性能对比
---

# 关于 limedl

**limedl** 是一款为极致性能与流畅体验而生的次世代多协议下载管理器。基于纯 **Rust** 核心引擎打造，并搭载轻量级现代原生 GUI 框架 **Slint**。

项目致力于彻底解决传统下载工具“臃肿迟钝、内存高企、商业广告骚扰、付费限速”以及传统开源客户端“基于 Electron 动辄数百兆内存常驻、高并发下磁盘 I/O 阻塞假死”的行业痛点。

:::tip[一句话概括]
limedl 将**原生极速的 Rust 引擎**与**毫秒级冷启动的 Slint 原生界面**完美结合，提供 HTTP AIMD 动态并发、完整 BitTorrent 协议族、独创 CDN 探针优选，以及对 Aria2 生态 100% 的兼容性。
:::

---

## 核心设计哲学

### 1. 告别臃肿，回归原生
拒绝 Chromium/Electron 庞大的多进程模型。limedl 采用纯 Rust 编写的 Slint 框架配合底层 FemtoVG 硬件加速渲染：
- **启动时间**：冷启动在 100 毫秒以内，窗口秒开，交互无延迟。
- **内存占用**：托盘常驻空闲内存仅约 **30~50 MB**，仅为同类 Electron 下载器的 1/6。
- **单进程架构**：核心引擎与原生 UI 运行于同一进程内，通过内部异步事件总线（EventBus）零拷贝交互，免除了繁琐的 IPC 网络序列化开销。

### 2. 拥塞自适应，保护网络
传统下载器往往固定死板的线程数（如无脑开启 32 或 64 线程），这在弱网或防刷严格的服务端极易触发 HTTP 429 / 503 封锁或严重的路由器排队丢包。

limedl 借鉴现代网络传输算法，实现了 **AIMD（Additive Increase Multiplicative Decrease）** 动态并发调度机制：
- **平滑起步**：根据延迟与分块耗时渐进加法提速；
- **智能退避**：当感知到网络抖动或服务端限流时，乘法降速并平滑回退；
- **镜像容灾**：当首选镜像超时或失败时，无缝透明切换备用镜像节点。

### 3. 硬件感知，呵护磁盘
在千兆甚至万兆宽带环境下，未经优化的多线程并发写入会导致严重的磁盘 I/O 拥塞甚至闪存磨损：
- **SSD 固态硬盘**：针对闪存擦除块物理特性，提供**写合并机制（Write Combining）**，将分散的小块聚合为最佳尺寸后批量异步落盘，降低写入放大（WAF），延长固态寿命。
- **HDD 机械硬盘**：采用**双缓冲池（Double Buffering）**与严格的物理文件偏移排序，前台异步接收网络数据，后台单线程顺序刷盘，彻底根绝高并发下载导致的磁头高频随机寻道与系统假死。

### 4. 拥抱开放，无缝融入 Aria2 生态
无需改变你已有的使用习惯。limedl 内置了标准 **Aria2 JSON-RPC 2.0** 服务端：
- 支持 Chrome / Edge / Firefox 上的各类主流下载插件（如 Aria2 Explorer、Camtd）；
- 支持网盘助手与油猴（Tampermonkey）脚本直接派发下载任务；
- 支持接入 AriaNg 等现代化 Web 控制台进行局域网与远程管理。

---

## 架构拓扑概览

limedl 采用多 crate 的 Rust 工作空间（Workspace），严格分离引擎、前端与工具：

```
limedl Workspace
├── crates/limedl-types/      # 领域类型、设置、事件、manifest、Backend trait
├── crates/limedl-disk/       # 文件操作、缓冲池、I/O 调度
├── crates/limedl-store/      # SQLite 持久化、Alt-Svc 缓存
├── crates/limedl-http/       # HTTP 客户端、限速器
├── crates/limedl-engine/     # HTTP executor、scheduler、CDN、Metalink、生命周期
├── crates/limedl-bt/         # librqbit BitTorrent 后端
├── crates/limedl-rpc/        # Aria2 JSON-RPC / WebSocket 服务
├── crates/limedl-testkit/    # 测试用 HTTP 夹具（仅测试）
├── crates/limedl-core/       # 门面：Dispatcher / BackendRegistry / bootstrap
├── crates/limedl-native/     # Slint + FemtoVG 桌面客户端
├── crates/limedl-server/     # 无头守护进程 (NAS / 软路由 / Docker)
└── xtask/                    # 密钥、双签名、构建与发布门禁
```

### 数据交互流向

```
┌─────────────────────────────────────────────────────────────┐
│              用户交互层 (Slint Native UI / limedl-server)     │
│      [任务列表]  [新建任务]  [设置中心]  [实验室 CDN]          │
└───────────────────────┬───────────────────▲─────────────────┘
                        │ API 调用          │ EventBus 订阅
                        ▼                   │ (广播通知)
┌─────────────────────────────────────────────────────────────┐
│            limedl-core 门面 (Dispatcher / Registry)          │
├─────────────────────────────────────────────────────────────┤
│   BackendRegistry (协议路由分发)                             │
│      ├─ [http: 前缀] ──► DownloadManager (limedl-core)       │
│      └─ [bt: 前缀]   ──► RqbitBtBackend (limedl-bt)          │
├─────────────────────────────────────────────────────────────┤
│   共享基础设施:                                              │
│   • limedl-disk  BufferPool / FileOps                       │
│   • limedl-store SQLite / Alt-Svc                           │
│   • limedl-http  客户端 / 令牌桶限速                         │
│   • limedl-types 设置 / 事件 / 领域类型                       │
└─────────────────────────────────────────────────────────────┘
```

---

## 性能与特性对比

下表对比了 limedl 与主流同类工具在实际使用场景中的综合表现：

| 维度 / 特性 | **limedl** | 传统商业下载工具 (如迅雷) | 常见开源工具 (如 Motrix) | 经典命令行 (Aria2) |
| :--- | :--- | :--- | :--- | :--- |
| **底层核心** | **纯 Rust 2024** | 专有 C++ 闭源引擎 | Node.js + Aria2 封装 | 纯 C++ 核心 |
| **界面技术** | **Slint (FemtoVG 原生渲染)** | 网页混合容器 (内嵌广告) | Electron (Chromium 内核) | 无原生界面 (纯 CLI) |
| **冷启动速度** | **< 100 ms (秒开)** | 3 ~ 6 秒 (含鉴权/弹窗) | 2 ~ 4 秒 (Chromium 白屏) | 瞬时 (命令行) |
| **空闲内存占用** | **约 35 ~ 50 MB** | 180 ~ 350 MB+ | 250 ~ 450 MB+ | < 20 MB |
| **并发策略** | **AIMD 动态自适应调度** | 固定线程 / 付费加速 | 固定连接数 | 固定连接数 (需手动配) |
| **磁盘 I/O 保护** | **SSD 写合并 + HDD 双缓冲** | 无明显介质优化 (常发热) | 基础顺序写，千兆易卡顿 | 基础缓冲配置 |
| **CDN 探针加速** | **Cloudflare / Fastly / 自定义 Provider** | 无 (依赖官方专有 P2P) | 无 | 无 |
| **BitTorrent 支持** | **librqbit (DHT/PEX/LSD)** | 商业专有 (部分协议受限) | Aria2 BT (DHT 性能一般) | 原生支持 |
| **Aria2 生态兼容** | **原生完整兼容 (6800)** | 不支持 | 自身作为前端包装 | 标准定义者 |
| **纯净与开源协议** | **GPL-3.0 无广告零遥测** | 商业闭源、开屏弹窗与广告 | MIT 开源 | GPL-2.0 开源 |

---

## 接下来

- [安装指南](/getting-started/installation/)：了解如何获取针对 Windows、macOS 与 Linux 的预编译安装包与校验指引。
- [配置手册](/getting-started/configuration/)：详细探索 limedl 的各项高级参数设置。
- [任务管理与并发策略](/guides/tasks/)：了解 AIMD 算法与分块下载的工作机制。
- [浏览器接管扩展](/ecosystem/browser/)：配置 Chrome / Edge 扩展以实现网页下载一键拦截。
