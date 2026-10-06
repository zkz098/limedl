---
title: 无头服务端与 NAS 部署 (limedl-server)
description: 如何在 NAS、软路由、Linux 服务器与 Docker 容器中部署无头 Aria2 RPC 守护进程
---

# 无头服务端与 NAS 部署 (limedl-server)

`limedl-server` 是 limedl 的无头（Headless）守护进程版本。它移除了 Slint 桌面图形界面，运行与桌面端完全相同的 pure Rust 下载引擎内核，并对外暴露标准 **Aria2 JSON-RPC 2.0** 接口。

你可以将它部署在 **NAS（群晖/威联通/TrueNAS）**、**软路由（OpenWrt/ImmortalWrt）**、**Linux 服务器** 或 **Docker 容器** 中，并通过网页端（如 AriaNg、Motrix）或各类自动化脚本远程驱动下载。

---

## 获取二进制与容器镜像

### 1. 静态 musl 二进制包
官方 GitHub Releases 为以下两个主流架构提供了全静态链接的 musl 归档包：

- `limedl-server-v<version>-x86_64-linux-musl.tar.gz`
- `limedl-server-v<version>-aarch64-linux-musl.tar.gz`

每个压缩包内包含 `limedl-server` 单一可执行文件，以及 systemd 部署资产（`limedl-server.service`、`limedl-server.env.example`）。由于为纯静态链接，无需 glibc 等动态库，可在 Alpine、OpenWrt、CentOS 7+ 及各类 NAS 系统上即解即用。

### 2. Docker 多架构容器镜像
官方提供已预装 `ca-certificates` 的多架构容器镜像：

```bash
docker pull ghcr.io/zkz098/limedl-server:latest
```

---

## 快速起步

### 仅本地回环监听 (配合同机反向代理)
```bash
limedl-server --download-dir /srv/downloads
```

### 局域网直接暴露 (必须设置密钥)
```bash
# 非回环地址绑定强制要求配置密钥以防未授权调用
limedl-server \
  --rpc-listen 0.0.0.0 \
  --rpc-secret "your-strong-secret-token" \
  --download-dir /srv/downloads
```

随后在任意设备打开 [AriaNg 网页版](http://ariang.mayswind.net/)，在 RPC 设置中指向 `http://<服务器IP>:6800/jsonrpc`，填入设置的 Token 即可立即连接管理。

---

## 命令行参数一览

| 参数 | 环境变量 | 说明 |
| :--- | :--- | :--- |
| `--data-dir <PATH>` | `LIMEDL_DATA_DIR` | 基础数据目录，存储 `settings.json`、`downloads.db` 与种子状态。 |
| `--rpc-listen <ADDR>` | - | 监听地址。默认 `127.0.0.1`，设为 `0.0.0.0` 开放局域网访问。 |
| `--rpc-port <PORT>` | - | 监听端口。默认 `6800`。 |
| `--rpc-secret <TOKEN>` | `LIMEDL_RPC_SECRET` | 共享密钥鉴权 Token。非回环地址监听时**强制要求**。 |
| `--rpc-allow-origin-all` | - | 输出 `Access-Control-Allow-Origin: *`，允许异源 Web 控制台跨域调用。 |
| `--rpc-allow-origin <ORIGIN>` | - | 添加特定允许跨域 Origin（可多次指定）。 |
| `--download-dir <PATH>` | - | 默认下载保存路径（会持久化至 `settings.json`）。 |
| `--log-level <LEVEL>` | `RUST_LOG` | 日志级别：`trace` / `debug` / `info` / `warn` / `error`。 |

:::caution[安全须知]
当 `--rpc-listen` 设为非 `127.0.0.1` 时，如果未配置 `--rpc-secret` 或多客户端 Token，**守护进程将拒绝启动**。因为未受保护的网络端点可向系统写入任意文件，存在安全风险。
:::

---

## Docker Compose 容器化部署

针对家庭 NAS 或家用服务器，推荐使用 Docker Compose 快速部署：

```yaml
# docker-compose.yml
services:
  limedl-server:
    image: ghcr.io/zkz098/limedl-server:latest
    container_name: limedl-server
    restart: unless-stopped
    environment:
      LIMEDL_RPC_SECRET: "your-strong-secret-token"
      PUID: "1000"
      PGID: "1000"
    ports:
      - "6800:6800"
    volumes:
      - ./data:/var/lib/limedl
      - /path/to/downloads:/downloads
```

启动容器：
```bash
docker compose up -d
```

- 镜像内置入口脚本，支持通过 `PUID` 和 `PGID` 自动修正挂载目录权限，并自动降权为普通用户运行；
- 内置 `ca-certificates`，HTTPS 与 TLS 下载开箱即用。

---

## 作为 systemd 系统服务运行

发布包中包含配置好的服务模板：

```bash
# 1. 复制二进制文件
sudo install -m 0755 limedl-server /usr/local/bin/limedl-server

# 2. 创建系统专用运行用户与目录
sudo useradd --system --home-dir /var/lib/limedl --shell /usr/sbin/nologin limedl
sudo install -d -o limedl -g limedl /var/lib/limedl /srv/downloads

# 3. 配置密钥与服务文件
sudo install -m 0600 limedl-server.env.example /etc/limedl-server.env
sudo nano /etc/limedl-server.env  # 设置 LIMEDL_RPC_SECRET
sudo install -m 0644 limedl-server.service /etc/systemd/system/limedl-server.service

# 4. 启动并启用开机自启
sudo systemctl daemon-reload
sudo systemctl enable --now limedl-server
sudo journalctl -u limedl-server -f
```

`systemd` 单元文件默认启用了严格的安全沙箱防护（`ProtectSystem=strict`, `ProtectHome=true`, `PrivateTmp=true`），仅允许对配置的数据目录和下载目录执行写操作。

---

## 推荐生产架构：同机反向代理 (HTTPS & WebSocket)

若需要公网安全访问，推荐让 `limedl-server` 维持默认的 `127.0.0.1` 本地回环监听，由 Caddy 或 Nginx 提供 TLS 加密并反代 `/jsonrpc`：

```caddyfile
# Caddyfile 示例
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

Caddy 会自动申请 HTTPS 证书并自动将 WebSocket 升级代理，Token 在传输层得到全程 TLS 加密保护。
