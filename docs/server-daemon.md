# limedl-server — headless daemon (NAS / 软路由)

`limedl-server` runs the same download engine as the desktop client with no GUI
and exposes the **Aria2 JSON-RPC** API, so a web console (AriaNg, Motrix) or an
aria2 script can drive it. Use it on a NAS, a soft router, a container, or any
machine that has no desktop session.

The daemon is a thin wrapper around the library: it calls the same
`bootstrap()` the desktop uses, then serves `Aria2RpcServer`. Everything the
desktop engine does (HTTP chunking, BitTorrent, scheduler, rate limits) is
available.

## Get the binary

Tagged releases publish static musl tarballs for the two supported targets:

```
limedl-server-v<version>-x86_64-linux-musl.tar.gz
limedl-server-v<version>-aarch64-linux-musl.tar.gz
```

Each tarball contains the `limedl-server` binary plus the deployment assets
described below (`limedl-server.service`, `limedl-server.env.example`). They are
fully static — no glibc, no shared libraries — so they run on Alpine,
OpenWrt/ImmortalWrt (with enough flash/RAM), Synology/QNAP and similar. Server
artifacts are **not** part of the desktop self-update manifest; they are plain
release assets.

A multi-architecture container image is published to GHCR:

```
docker pull ghcr.io/zkz098/limedl-server:latest
```

Building from source (either target):

```sh
# Native build
cargo build --release -p limedl-server

# Cross-compile without Docker. `cargo zigbuild` bundles the musl C toolchain
# that aws-lc-sys needs; the release pipeline uses the same command.
rustup target add aarch64-unknown-linux-musl
cargo install cargo-zigbuild
cargo zigbuild --release --target aarch64-unknown-linux-musl -p limedl-server

# Or build the container image from source
docker build -f packaging/server/Dockerfile -t limedl-server .
```

## Quick start

```sh
# Loopback only (the default) — for a reverse proxy on the same host.
limedl-server --download-dir /srv/downloads

# Direct LAN access. A non-loopback bind requires authentication, so
# --rpc-secret (or per-client tokens in settings.json) is mandatory.
limedl-server \
  --rpc-listen 0.0.0.0 \
  --rpc-secret "$(head -c 32 /dev/urandom | base64)" \
  --download-dir /srv/downloads
```

Point AriaNg at `http://<host>:6800/jsonrpc` (HTTP or WebSocket) and enter the
same secret as the RPC key. The default port is `6800`.

## Options

| Flag | Meaning |
| --- | --- |
| `--data-dir <PATH>` | Base directory holding `settings.json`, `downloads.db` and torrent state. Falls back to `$LIMEDL_DATA_DIR`, then the platform data directory. |
| `--rpc-listen <ADDR>` | Listener address. `0.0.0.0` exposes it on the network. Default `127.0.0.1`. |
| `--rpc-port <PORT>` | Listener port. Default `6800`. |
| `--rpc-secret <TOKEN>` | Shared secret; every request must carry `token:<TOKEN>`. Also read from `$LIMEDL_RPC_SECRET`. |
| `--rpc-allow-origin-all` | Emit `Access-Control-Allow-Origin: *`. Needed when AriaNg is served from a *different* origin. |
| `--rpc-allow-origin <ORIGIN>` | Add one allowed origin; repeatable. |
| `--download-dir <PATH>` | Default download directory. Persisted to `settings.json`, because some aria2 clients omit `dir`. |
| `--log-level <LEVEL>` | `trace` / `debug` / `info` / `warn` / `error`. |

Every flag overrides `settings.json` for that run only, except `--download-dir`,
which is persisted. The RPC endpoint is always enabled in the daemon (it is the
only interface) and `aria2.shutdown` stops the process (aria2 semantics) — the
desktop deliberately does neither.

## Security

**A non-loopback listener is refused unless authentication is configured.**
The RPC `dir` option is not confined to a download root, so an anonymous
network-reachable endpoint can write files anywhere the process can. Pass
`--rpc-secret`, or configure per-client tokens in `settings.json`
(`aria2Rpc.authMode: "per_client"`); the daemon refuses to start otherwise.

Prefer the reverse-proxy setup below over exposing port 6800 directly: the RPC
token travels in the JSON body, so plain LAN HTTP/WS is cleartext.

### Recommended: reverse proxy on the same host (TLS, no CORS)

Keep the default loopback bind and let Caddy serve AriaNg and proxy the RPC on
one origin. The browser then talks to `https://…/jsonrpc` (same origin, no CORS),
and the token never leaves the encrypted connection:

```caddyfile
aria.example.lan {
    # AriaNg static build (this also gives you a trusted certificate).
    root * /srv/ariang
    handle /jsonrpc {
        reverse_proxy 127.0.0.1:6800
    }
    handle {
        file_server
    }
}
```

`reverse_proxy` upgrades WebSocket automatically, so AriaNg can use the
WebSocket transport. nginx needs the `Upgrade`/`Connection` headers set
explicitly.

### Direct LAN exposure

Only if you cannot run a proxy: bind `0.0.0.0`, set a secret, and reach it from
a browser session on the same LAN. AriaNg served from a different origin needs
`--rpc-allow-origin-all` (or the exact origin via `--rpc-allow-origin`). This is
plaintext HTTP/WS; treat the token as exposed to anyone who can capture LAN
traffic. Built-in TLS for the endpoint is not implemented yet.

## Running as a service (systemd)

The release tarball ships `limedl-server.service` and
`limedl-server.env.example`; a typical install is:

```sh
# 1. Binary
install -m 0755 limedl-server /usr/local/bin/limedl-server

# 2. Dedicated account and directories
useradd --system --home-dir /var/lib/limedl --shell /usr/sbin/nologin limedl
install -d -o limedl -g limedl /var/lib/limedl /srv/downloads

# 3. Secret + unit
install -m 0600 limedl-server.env.example /etc/limedl-server.env
$EDITOR /etc/limedl-server.env          # set LIMEDL_RPC_SECRET
install -m 0644 limedl-server.service /etc/systemd/system/limedl-server.service

systemctl daemon-reload
systemctl enable --now limedl-server
journalctl -u limedl-server -f
```

The unit binds `--rpc-listen 0.0.0.0` and reads the secret from
`/etc/limedl-server.env`; it also applies `ProtectSystem=strict`,
`ProtectHome=true`, `PrivateTmp=true` and restricts `ReadWritePaths` to the data
and download directories. Two consequences follow:

- if you move `--download-dir`, add it to `ReadWritePaths` (and keep it out of
  `/home`, which `ProtectHome` hides);
- a missing/invalid `/etc/limedl-server.env` makes the daemon refuse to start on
the non-loopback bind, which is the intended fail-closed behavior.

Switch `--rpc-listen` to `127.0.0.1` when a reverse proxy on the same host owns
the TLS endpoint.

`SIGTERM` and `SIGINT` shut the engine down cleanly (manifests flushed, WAL
checkpointed, BT session released), so `systemctl stop` is safe.

## Docker / Compose

The published image runs the same daemon and entrypoint convention as most NAS
containers: start it as root and set `PUID`/`PGID`, and the entrypoint fixes the
mounted volume ownership then drops privileges with `su-exec`.

```yaml
# packaging/server/docker-compose.yml
services:
  limedl-server:
    image: ghcr.io/zkz098/limedl-server:latest
    restart: unless-stopped
    environment:
      LIMEDL_RPC_SECRET: "change-me"
      PUID: "1000"
      PGID: "1000"
    ports:
      - "6800:6800"
    volumes:
      - ./data:/var/lib/limedl
      - ./downloads:/downloads
```

```sh
docker compose -f packaging/server/docker-compose.yml up -d
```

- The image includes `ca-certificates`, so HTTPS downloads work out of the box.
- Build it yourself with `docker build -f packaging/server/Dockerfile .`; the
  release pipeline instead assembles the image from the prebuilt static musl
  binaries (`packaging/server/Dockerfile.release`), so no Rust toolchain runs
inside the image build.
- The container command already binds `0.0.0.0`, so `LIMEDL_RPC_SECRET` is
required; the redirection of `6800` is what exposes it to the LAN. Put it behind
  a reverse proxy (or only publish to `127.0.0.1`) when the host is not trusted.

## Files and logs

| Path | Contents |
| --- | --- |
| `<data-dir>/settings.json` | Configuration (shared format with the desktop). |
| `<data-dir>/downloads/downloads.db` | SQLite task/history database. |
| `<data-dir>/downloads/torrents/` | BitTorrent resume state. |
| `<data-dir>/downloads/logs/limedl.log` | Default log file (override with `logging.filePath`). |
| `<data-dir>/limedl-server.lock` | Advisory single-instance lock. |

**Do not point the daemon and a running desktop client at the same data
directory.** The lock only guards against a second *daemon*; a desktop instance
shares the same SQLite database, torrent state and RPC port. Stop one before
starting the other, or use a separate `--data-dir`.

## Current limitations

- No built-in HTTPS/WSS for the RPC endpoint (aria2's `--rpc-secure`); use a
  reverse proxy. HTTP GET/JSONP transports are likewise not served (POST +
  WebSocket only).
- The aria2 surface has documented gaps (`getGlobalOption`/`getOption` key
  subsets, `addMetalink`, GID prefix matching). See
  [`aria2-interop-testing.md`](aria2-interop-testing.md).
- `--rpc-listen` binds one address; dual-stack needs two instances or `::`.
