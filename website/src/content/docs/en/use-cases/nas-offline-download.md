---
title: NAS / Soft-Router Downloads
description: Run limedl-server headlessly on Synology, QNAP, TrueNAS, OpenWrt, or a Linux server, and manage it remotely with AriaNg
---

# NAS / Soft-Router Downloads

## Who this is for

- You have an always-on NAS, soft router, or Linux server;
- You do not need a GUI and want downloads handled by a background daemon;
- You want to submit tasks from a browser or phone via AriaNg, Motrix, or scripts.

This scenario uses **`limedl-server`**: the same download engine as the desktop app, with the Slint UI removed and only the Aria2 JSON-RPC 2.0 API exposed.

---

## Architecture

```text
Browser / phone / script
        │  Aria2 JSON-RPC 2.0 (HTTP / WebSocket)
        ▼
   limedl-server  ──►  Download directory (local disk / SMB / NFS)
        │
        └── settings.json, downloads.db, torrent state
```

- The server has **no GUI**; a frontend such as AriaNg displays task state.
- Server binaries are fully static musl. x86_64 uses the `x86-64-v2` baseline; aarch64 has no extra CPU requirement.
- Server assets are not part of the desktop self-update signing chain; verify them against the SHA-256 digest on the GitHub Release page.

---

## Option 1: Docker Compose (recommended)

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

- `PUID` / `PGID` make the entrypoint fix mount ownership and drop privileges.
- The image includes `ca-certificates`, so HTTPS downloads work out of the box.
- Replace `/path/to/downloads` with the real NAS share.

## Option 2: Static binary + systemd

The release archive contains `limedl-server`, `limedl-server.service`, and `limedl-server.env.example`:

```bash
sudo install -m 0755 limedl-server /usr/local/bin/limedl-server
sudo useradd --system --home-dir /var/lib/limedl --shell /usr/sbin/nologin limedl
sudo install -d -o limedl -g limedl /var/lib/limedl /srv/downloads

sudo install -m 0600 limedl-server.env.example /etc/limedl-server.env
sudo nano /etc/limedl-server.env          # set LIMEDL_RPC_SECRET
sudo install -m 0644 limedl-server.service /etc/systemd/system/limedl-server.service

sudo systemctl daemon-reload
sudo systemctl enable --now limedl-server
sudo journalctl -u limedl-server -f
```

The unit enables `ProtectSystem=strict`, `ProtectHome=true`, and `PrivateTmp=true`, allowing writes only to the configured data and download directories.

---

## Connecting AriaNg

1. Open [AriaNg](http://ariang.mayswind.net/) or a self-hosted instance;
2. Set the RPC URL to `http://<NAS-IP>:6800/jsonrpc`;
3. Enter `LIMEDL_RPC_SECRET`;
4. Save — you can now add, pause, and inspect tasks.

> When binding a non-loopback address, `limedl-server` **requires** a token and refuses to start without one. Do not expose port 6800 directly to the internet.

---

## Remote access: reverse proxy on the same host

Keep `limedl-server` on loopback and let Caddy/Nginx terminate TLS:

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

Caddy obtains HTTPS certificates automatically and proxies WebSocket upgrades; the token stays inside TLS. Nginx needs explicit `Upgrade` / `Connection` headers.

---

## Storage and disk scheduling

- The download directory can be a local disk, an SMB/NFS mount, or a mapped drive.
- **Network shares are detected as network locations and scheduled like SSDs**, without the HDD double buffer, to avoid random-write amplification on network filesystems.
- On mixed SSD/HDD NAS storage, put the download directory on the appropriate volume and tune the buffer settings for that workload.
- Speed schedules use the server's local time and match by hour.

---

## Operations and security checklist

- [ ] Use a long random token; set a distinct token per exposed endpoint.
- [ ] Do not expose 6800 to the internet; use a TLS reverse proxy for remote access.
- [ ] Run the container with `PUID` / `PGID`; mount only the required download directories.
- [ ] Verify the server archive against the SHA-256 digest on the GitHub Release page.
- [ ] To upgrade, replace the binary and restart; the server does not self-update.
- [ ] For BitTorrent, allow the listening ports through the router/NAS firewall and consider UPnP.

---

## Related documentation

- [Headless server deployment](/en/ecosystem/server-daemon/): full flags, Docker, systemd, and Caddy examples.
- [Aria2 RPC](/en/ecosystem/aria2-rpc/): supported methods, authentication, and client setup.
- [Platform Support](/en/getting-started/platform-support/): x86_64/aarch64 and CPU baselines.
- [Security & Verification](/en/security/): why server assets are unsigned and how to verify them.
