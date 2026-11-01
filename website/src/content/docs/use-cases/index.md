---
title: 使用场景
description: 按实际工作流选择 limedl：NAS 无头下载、跨境镜像加速、浏览器一键接管
---

# 使用场景

同一个下载引擎，可以放在三种完全不同的工作流里。先找到与你最接近的场景，再按对应页面配置。

| 场景 | 适合谁 | 你会用到的能力 | 入口 |
| :--- | :--- | :--- | :--- |
| **NAS / 软路由离线下载** | 群晖、威联通、TrueNAS、OpenWrt、Linux 服务器 | `limedl-server`、Aria2 RPC、Docker、systemd、AriaNg | [无头下载部署](/use-cases/nas-offline-download/) |
| **跨境与开源镜像加速** | 经常下载 GitHub Releases、HuggingFace、Linux 发行版镜像 | CDN 探针、URL 重写、Metalink 多镜像、代理 | [跨境镜像加速](/use-cases/cross-border-mirrors/) |
| **浏览器一键接管** | 日常在浏览器里点下载，希望自动交给 limedl | Aria2 RPC + Aria2 Explorer/Camtd、请求头透传、拦截规则 | [浏览器接管工作流](/use-cases/browser-handoff/) |

---

## 怎么选

- **机器没有桌面环境**：选 NAS/软路由场景，使用 `limedl-server`。
- **有桌面端，但下载源在海外或镜像站很慢**：选跨境镜像场景，在桌面端开启 CDN 探针或 URL 重写。
- **主要在网页上点下载**：选浏览器接管场景，让扩展把任务推给本地 limedl。

三种场景不互斥：例如 NAS 上的 `limedl-server` 同样可以配合浏览器扩展（通过局域网 RPC），只是需要额外处理 Token 与 TLS。

---

## 共同前提

- 先完成[安装](/getting-started/installation/)或[服务端部署](/ecosystem/server-daemon/)，并确认版本与[平台支持](/getting-started/platform-support/)匹配。
- 涉及 RPC 时，默认只监听 `127.0.0.1` 且默认关闭；开放到局域网前务必设置 Token，公网访问请走 TLS 反向代理。
- 发布资产的安全性见[安全与验签](/security/)；服务端二进制不参与桌面自更新签名链。
