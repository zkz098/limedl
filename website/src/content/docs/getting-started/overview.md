---
title: 项目简介
description: 了解 limedl 的设计理念、架构特点与核心优势
---

# 关于 limedl

**limedl** 是一款为极致性能与流畅体验而生的次世代多协议下载管理器。基于纯 **Rust** 核心引擎开发，并搭载轻量级现代原生 UI 框架 **Slint**。

它同时支持 **HTTP/HTTPS 动态自适应并发分块下载** 与 **BitTorrent（磁力链 / .torrent）** 完整协议，内建 **CDN 智能探针优选加速**，并无缝兼容 **Aria2 JSON-RPC 2.0** 协议标准。

## 核心设计哲学

- **告别臃肿，回归原生**：拒绝 Chromium/Electron 动辄数百兆的内存常驻。limedl 采用 Slint 原生渲染，冷启动毫秒级，空闲内存仅数十兆。
- **动态拥塞自适应**：HTTP 并发并非盲目开设大量线程，而是借鉴 TCP 的 AIMD（加法递增、乘法递减）自适应算法，在跑满带宽的同时保护系统网络稳定性。
- **保护硬件与磁盘**：智能检测存储介质类型（SSD / HDD），对 SSD 采用批量合并写入降低闪存磨损，对 HDD 采用双缓冲与预分配减少磁头频繁寻道与磁盘碎片。
- **开放与生态兼容**：无缝对接全网 Aria2 生态（浏览器扩展、油猴脚本、远程控制端），无需改变使用习惯即可享受 Rust 引擎带来的强劲速度。

## 架构概览

```
limedl 架构
├── crates/limedl-core/    # 纯 Rust 异步多协议下载引擎 (无 GUI 依赖)
│   ├── DownloadManager    # HTTP/HTTPS 自适应并发调度与分块
│   ├── IrontideBtBackend  # BitTorrent 内核 (DHT, PEX, UPnP)
│   ├── CdnAccelerator     # Cloudflare 探针测速与智能优化
│   ├── BufferPool         # SSD 写入合并 / HDD 双缓冲池
│   └── Aria2RpcServer     # Aria2 JSON-RPC 2.0 兼容服务
└── crates/limedl-native/  # Slint 原生桌面界面 (Windows / macOS / Linux)
```
