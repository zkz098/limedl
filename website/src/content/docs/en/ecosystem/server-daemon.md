---
title: Headless Daemon (limedl-server)
description: Deploying the headless Aria2 JSON-RPC download daemon on NAS, routers, Linux servers, and Docker
---

# Headless Daemon (limedl-server)

`limedl-server` is the headless daemon edition of limedl. It runs the exact same pure Rust download engine as the desktop client without any GUI dependencies, exposing a compliant **Aria2 JSON-RPC 2.0** API.

Deploy it on a **NAS (Synology, QNAP, TrueNAS)**, **soft router (OpenWrt, ImmortalWrt)**, **Linux server**, or in **Docker**, and drive it remotely using AriaNg, Motrix, or automation scripts.

---

## Get the Binary & Container Image

### 1. Static musl Tarballs
Tagged GitHub releases publish fully static musl archives for two targets:

- `limedl-server-v<version>-x86_64-linux-musl.tar.gz`
- `limedl-server-v<version>-aarch64-linux-musl.tar.gz`

Each tarball contains the standalone `limedl-server` binary and systemd deployment templates (`limedl-server.service`, `limedl-server.env.example`). Since they are statically linked, they have zero glibc dependencies and run out of the box on Alpine, OpenWrt, CentOS 7+, and NAS distributions.

### 2. Multi-Architecture Container Image
A multi-arch container image with pre-installed `ca-certificates` is published to GHCR:

```bash
docker pull ghcr.io/zkz098/limedl-server:latest
```

---

## Quick Start

### Loopback Only (Recommended for Reverse Proxy Setup)
```bash
limedl-server --download-dir /srv/downloads
```

### Direct LAN Access (Authentication Mandatory)
```bash
# Non-loopback listening requires authentication to prevent unauthorized remote writes
limedl-server \
  --rpc-listen 0.0.0.0 \
  --rpc-secret "your-strong-secret-token" \
  --download-dir /srv/downloads
```

Open [AriaNg Web](http://ariang.mayswind.net/) on any browser, point the RPC configuration to `http://<server-ip>:6800/jsonrpc`, supply the secret token, and start managing downloads immediately.

---

## Command-Line Options

| Option | Environment Variable | Description |
| :--- | :--- | :--- |
| `--data-dir <PATH>` | `LIMEDL_DATA_DIR` | Base directory holding `settings.json`, `downloads.db`, and torrent state. |
| `--rpc-listen <ADDR>` | - | Listener address. Default `127.0.0.1`. Set to `0.0.0.0` for LAN access. |
| `--rpc-port <PORT>` | - | Listener port. Default `6800`. |
| `--rpc-secret <TOKEN>` | `LIMEDL_RPC_SECRET` | Shared secret token. **Mandatory** when binding to non-loopback addresses. |
| `--rpc-allow-origin-all` | - | Emits `Access-Control-Allow-Origin: *` for cross-origin web consoles. |
| `--rpc-allow-origin <ORIGIN>` | - | Allows a specific origin for CORS (repeatable). |
| `--download-dir <PATH>` | - | Default download directory (persisted to `settings.json`). |
| `--log-level <LEVEL>` | `RUST_LOG` | Log verbosity: `trace` / `debug` / `info` / `warn` / `error`. |

:::caution[Security Fail-Closed Stance]
A non-loopback listener is **refused** unless `--rpc-secret` (or per-client tokens in `settings.json`) is configured. Because the RPC API allows arbitrary file destination paths, an unprotected network endpoint presents a serious vulnerability.
:::

---

## Docker Compose Deployment

For home servers and NAS appliances, Docker Compose offers the easiest deployment:

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

Launch the service:
```bash
docker compose up -d
```

- The entrypoint uses `su-exec` to safely drop privileges to the specified `PUID` / `PGID` while adjusting volume permissions;
- Bundled `ca-certificates` ensure HTTPS downloads work seamlessly out of the box.

---

## Running as a systemd Service

The release tarball includes configured systemd service assets:

```bash
# 1. Install binary
sudo install -m 0755 limedl-server /usr/local/bin/limedl-server

# 2. Create service user and directories
sudo useradd --system --home-dir /var/lib/limedl --shell /usr/sbin/nologin limedl
sudo install -d -o limedl -g limedl /var/lib/limedl /srv/downloads

# 3. Install secret environment file and unit file
sudo install -m 0600 limedl-server.env.example /etc/limedl-server.env
sudo nano /etc/limedl-server.env  # configure LIMEDL_RPC_SECRET
sudo install -m 0644 limedl-server.service /etc/systemd/system/limedl-server.service

# 4. Enable and start
sudo systemctl daemon-reload
sudo systemctl enable --now limedl-server
sudo journalctl -u limedl-server -f
```

The systemd unit enforces isolation (`ProtectSystem=strict`, `ProtectHome=true`, `PrivateTmp=true`), restricting file write access strictly to data and download directories.

---

## Recommended Production Setup: Reverse Proxy with TLS

For secure external access, keep the default `127.0.0.1` loopback listener and let Caddy or Nginx provide TLS encryption and proxy `/jsonrpc`:

```caddyfile
# Caddyfile example
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

Caddy automatically obtains TLS certificates and proxies WebSocket upgrades, keeping the authentication token fully encrypted end-to-end.
