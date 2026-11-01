---
title: NAS / 软路由离线下载
description: 用 limedl-server 在群晖、威联通、TrueNAS、OpenWrt 或 Linux 服务器上做无头下载，并用 AriaNg 远程管理
---

# NAS / 软路由离线下载

## 适合谁

- 有一台常年开机的 NAS、软路由或 Linux 服务器；
- 不需要图形界面，希望把下载任务交给后台守护进程；
- 想用 AriaNg、Motrix 或脚本从浏览器/手机远程提交任务。

这个场景使用 **`limedl-server`**：它与桌面端共用下载引擎，但移除了 Slint 界面，只暴露 Aria2 JSON-RPC 2.0 接口。

---

## 架构

```text
浏览器 / 手机 / 脚本
        │  Aria2 JSON-RPC 2.0 (HTTP / WebSocket)
        ▼
   limedl-server  ──►  下载目录 (本地盘 / SMB / NFS)
        │
        └── settings.json、downloads.db、种子状态
```

- 服务端**没有** GUI，任务状态由前端（AriaNg 等）展示。
- 服务端二进制为 musl 全静态，x86_64 使用 `x86-64-v2` 基线，aarch64 无额外 CPU 要求。
- 服务端资产不参与桌面自更新签名链；下载后用 GitHub Release 的 SHA-256 摘要核对。

---

## 方案一：Docker Compose（推荐）

```yaml
# docker-compose.yml
services:
  limedl-server:
    image: ghcr.io/zkz098/limedl-server:latest
    container_name: limedl-server
    restart: unless-stopped
    environment:
      LIMEDL_RPC_SECRET: "replace-with-a-long-random-token"
      PUID: "1000"
      PGID: "1000"
    ports:
      - "6800:6800"
    volumes:
      - ./data:/var/lib/limedl
      - /path/to/downloads:/downloads
```

```bash
docker compose up -d
docker compose logs -f limedl-server
```

- `PUID` / `PGID` 会让入口脚本修正挂载目录权限并降权运行。
- 镜像内置 `ca-certificates`，HTTPS 下载开箱即用。
- 将 `/path/to/downloads` 换成 NAS 上的真实共享目录。

## 方案二：静态二进制 + systemd

发布包包含 `limedl-server`、`limedl-server.service` 与 `limedl-server.env.example`：

```bash
sudo install -m 0755 limedl-server /usr/local/bin/limedl-server
sudo useradd --system --home-dir /var/lib/limedl --shell /usr/sbin/nologin limedl
sudo install -d -o limedl -g limedl /var/lib/limedl /srv/downloads

sudo install -m 0600 limedl-server.env.example /etc/limedl-server.env
sudo nano /etc/limedl-server.env          # 设置 LIMEDL_RPC_SECRET
sudo install -m 0644 limedl-server.service /etc/systemd/system/limedl-server.service

sudo systemctl daemon-reload
sudo systemctl enable --now limedl-server
sudo journalctl -u limedl-server -f
```

单元文件默认启用了 `ProtectSystem=strict`、`ProtectHome=true`、`PrivateTmp=true`，只允许写配置的数据目录与下载目录。

---

## 连接 AriaNg

1. 打开 [AriaNg 网页版](http://ariang.mayswind.net/) 或自建实例；
2. RPC 地址填 `http://<NAS-IP>:6800/jsonrpc`；
3. 填入 `LIMEDL_RPC_SECRET`；
4. 保存后即可新建、暂停、查看任务。

> 非回环监听时，`limedl-server` **强制要求** Token，未配置会拒绝启动。不要把 6800 直接暴露到公网。

---

## 远程访问：同机反向代理

推荐让 `limedl-server` 保持回环监听，由 Caddy/Nginx 提供 TLS：

```text
aria.example.com {
    root * /srv/ariang
    handle /jsonrpc {
        reverse_proxy 127.0.0.1:6800
    }
    handle {
        file_server
    }
}
```

Caddy 会自动申请 HTTPS 证书并代理 WebSocket 升级；Token 全程走 TLS。Nginx 需要显式配置 `Upgrade` / `Connection` 头。

---

## 存储与磁盘调度

- 下载目录可以是本地盘、SMB/NFS 挂载或映射盘。
- **网络共享会被识别为“网络位置”并按 SSD 方式调度**，不会套用 HDD 双缓冲；这是为了避免网络文件系统上的随机写放大。
- 如果 NAS 上有 SSD 与 HDD 混合存储，建议把下载目录放在合适的卷上，并在桌面端/服务端按需调整缓冲池参数。
- 服务端同样支持定时限速计划；规则按服务器本地时间的小时匹配。

---

## 运维与安全清单

- [ ] 为服务端设置长随机 Token；如果改用桌面端提供 RPC，可再配置多客户端独立 Token。
- [ ] 不把 6800 暴露到公网；远程访问走 TLS 反向代理。
- [ ] 容器以 `PUID` / `PGID` 降权运行；只挂载必要的下载目录。
- [ ] 用 GitHub Release 的 SHA-256 摘要核对服务端压缩包。
- [ ] 升级服务端时替换二进制并重启；服务端不参与桌面自更新。
- [ ] 需要 BT 时，在路由器/NAS 防火墙放行监听端口并考虑 UPnP。

---

## 相关文档

- [无头服务端与 NAS 部署](/ecosystem/server-daemon/)：完整参数、Docker、systemd 与 Caddy 示例。
- [Aria2 RPC 对接](/ecosystem/aria2-rpc/)：兼容方法、鉴权与客户端配置。
- [平台支持](/getting-started/platform-support/)：x86_64/aarch64 与 CPU 基线。
- [安全与验签](/security/)：服务端资产为何不签名、如何核对。
