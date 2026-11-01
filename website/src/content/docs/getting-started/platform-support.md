---
title: 平台支持
description: limedl 桌面客户端与无头服务端支持的操作系统、CPU 架构、分发格式、运行依赖与自动更新范围
---

# 平台支持

limedl 的发布产物按 **桌面客户端** 与 **无头服务端** 两条线构建。两者共用同一套 Rust 下载引擎，但支持的平台、架构与更新方式不同。

:::caution[先看结论]
- 桌面端只提供 **Windows x86_64、macOS Apple Silicon、Linux x86_64** 三种产物。
- **没有** Windows ARM64、Intel Mac、Linux ARM64 桌面版，也没有 32 位版本。
- 桌面端 x86_64 构建使用 `x86-64-v3` 指令集（Haswell 2013 及更新，需 AVX2）；无头服务端 x86_64 构建放宽到 `x86-64-v2`（Nehalem 2008+）。
:::

---

## 桌面客户端

| 平台 | 架构 | 最低系统 | 分发格式 | 自动更新 |
| :--- | :--- | :--- | :--- | :--- |
| **Windows** | x86_64（x86-64-v3） | Windows 10 1809+ / Windows 11 | Setup.exe、便携 zip、MSIX | Setup 与便携版支持应用内更新；MSIX 走 Microsoft Store |
| **macOS** | Apple Silicon（aarch64，`apple-m1` 基线） | macOS 11.0+ | `.app` 的 `.tar.gz` | 便携包支持应用内原地替换并重新做 ad-hoc 签名 |
| **Linux** | x86_64（x86-64-v3） | glibc ≥ 2.17 | AppImage、`.deb`、便携 `.tar.gz` | 便携包支持应用内更新；AppImage 与 `.deb` 请用对应分发方式更新 |

### Windows

- **系统版本**：打包清单声明的最低版本为 `10.0.17763.0`，即 Windows 10 1809。更老的 Windows 10 版本与 Windows 7/8 不受支持。
- **CPU**：桌面构建启用 `x86-64-v3`（AVX2 / BMI / FMA），2013 年 Haswell 及之后的 x86_64 处理器可用。更老的 x86_64 CPU 可能无法启动。
- **不提供**：Windows ARM64 原生包。ARM64 Windows 可以尝试系统自带的 x86_64 模拟，但不在官方支持范围内。
- **协议关联**：Setup.exe 会把 `magnet:` 与 `limedl://` 注册到系统。
- **更新**：Setup 版下载新安装包并以 `/P /R` 静默升级；便携版原地替换可执行文件；MSIX 由 Microsoft Store 管理，应用内更新会跳过 Store 安装。

### macOS

- **芯片**：仅 Apple Silicon（M1/M2/M3/M4）。发布工作流只构建 `darwin-aarch64`，**没有 Intel Mac 构建**。
- **系统版本**：`Info.plist` 的 `LSMinimumSystemVersion` 为 `11.0`，即 macOS Big Sur 及以上。
- **签名**：构建采用 ad-hoc 签名且未做 Apple 公证。首次从浏览器下载后，需要右键选择“打开”，或执行 `xattr -cr /Applications/limedl.app` 移除隔离属性。
- **更新**：便携 `.app` 支持应用内更新；更新器替换二进制后会重新签名整个 bundle。若应用位于 `/Applications` 且当前用户无写权限，更新会失败并提示安装位置不可写。

### Linux

- **glibc**：发布构建面向 `x86_64-unknown-linux-gnu.2.17`，CI 会用 `scripts/check-glibc-floor.sh` 阻止依赖抬高这个下限。Debian 10+、Ubuntu 18.04+、CentOS 7+ 等满足该下限。
- **CPU**：与 Windows 桌面端一样启用 `x86-64-v3`。**glibc 够新不代表 CPU 够新**，2013 年以前的 x86_64 机器可能因缺少 AVX2 而无法运行。
- **桌面依赖**：
  - 文件选择器通过 `xdg-desktop-portal` + D-Bus 工作，Wayland 与沙箱环境同样适用；平铺窗口管理器需要安装 `xdg-desktop-portal-gtk` 等后端。
  - 托盘使用 StatusNotifierItem；GNOME 需要启用 AppIndicator 扩展，KDE/XFCE 自带宿主。
- **更新**：便携 `.tar.gz` 可应用内替换；AppImage 内部无法原地替换挂载的 bundle，请直接下载新 AppImage；`.deb` 请通过包管理器升级。
- **不提供**：Linux ARM64 桌面版。

### 数据目录

| 平台 | 默认数据目录 |
| :--- | :--- |
| Windows | `%LOCALAPPDATA%\limedl\` |
| macOS | `~/Library/Application Support/limedl/` |
| Linux | `~/.local/share/limedl/` |

设置 `LIMEDL_DATA_DIR` 可以覆盖数据目录；指向 U 盘等相对/可移动位置即可实现便携运行。

---

## 无头服务端（limedl-server）

| 平台 | 架构 | 运行环境 | 分发格式 |
| :--- | :--- | :--- | :--- |
| Linux | x86_64（x86-64-v2） | musl 全静态，无需 glibc | `.tar.gz`、Docker 镜像 |
| Linux | aarch64 | musl 全静态 | `.tar.gz`、Docker 镜像 |

- 服务端构建使用 `x86-64-v2`（Nehalem 2008+），比桌面端更宽松，面向老 NAS 与软路由。
- 静态 musl 二进制不依赖系统 glibc，可用于 Alpine、OpenWrt、各类 NAS 与精简容器。
- 服务端只暴露 Aria2 JSON-RPC 2.0 接口，不包含 Slint 图形界面；请搭配 AriaNg、Motrix 或脚本使用。
- 服务端资产**不参与**桌面自更新签名链；下载后请用 GitHub Release 显示的 SHA-256 摘要核对完整性。

---

## 常见不支持场景

| 场景 | 状态 | 建议 |
| :--- | :--- | :--- |
| Intel Mac（x86_64） | 不支持 | 使用其他平台，或自行从源码构建（不在发布保障内） |
| Windows on ARM | 无原生包 | 可尝试 x86_64 模拟；不作为官方支持组合 |
| Linux ARM64 桌面 | 不支持 | 使用 `limedl-server` 的 aarch64 构建做无头下载 |
| 32 位 x86 | 不支持 | — |
| 无 AVX2 的老 x86_64 CPU | 桌面端可能无法运行 | 使用服务端版本，或自行以更低 `target-cpu` 构建 |
| glibc < 2.17 | 不支持 | 升级系统，或改用静态 musl 服务端 |
| macOS < 11.0 | 不支持 | 升级系统 |

---

## 相关文档

- [安装指南](/getting-started/installation/)：各平台安装、Gatekeeper 放行与验签步骤。
- [安全与验签](/security/)：发布资产签名、公钥与更新信任链。
- [无头服务端部署](/ecosystem/server-daemon/)：NAS、软路由、Docker 与 systemd。
- [更新日志](/changelog)：版本变化与平台相关修复。
