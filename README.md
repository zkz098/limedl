# limedl

Fast multi-protocol download manager — HTTP, BitTorrent, with CDN acceleration. Runs as a desktop app (Windows / macOS / Linux) or a headless daemon (NAS / server).

## Features

- **HTTP downloads** — chunked parallel downloading with adaptive concurrency (AIMD), automatic retry, mirror failover, and resumable transfers
- **BitTorrent** — full-featured BT client via irontide engine (DHT, UPnP, PEX, LSD, magnet links, .torrent files)
- **CDN acceleration** — Cloudflare IP probing and DNS rewriting for faster downloads
- **Aria2 RPC** — Aria2-compatible JSON-RPC 2.0 API enables integration with AriaNg, Motrix, and other frontends
- **Buffer pool** — HDD double-buffer / SSD write-combining tuned to your disk type
- **Rate limiting** — configurable global speed limits
- **Multi-platform** — same engine powers all targets

## Platforms

| Target       | Frontend            | Backend         | Build                                     |
| ------------ | ------------------- | --------------- | ----------------------------------------- |
| Desktop      | Vue 3 via Tauri IPC | `src-tauri/`    | `pnpm tauri dev` / `pnpm tauri build`     |
| NAS / Server | Vue 3 via WebSocket | `limedl-server` | `cargo build -p limedl-server`            |
| CLI          | N/A                 | `limedl-server` | `limedl download <url>` / `limedl daemon` |

The frontend is shared across desktop and NAS targets. The download engine (`limedl-core`) is pure Rust with zero UI dependencies.

## Quick Start

### Desktop (Tauri)

Requires Node.js 24+, pnpm, and Rust.

```bash
pnpm install --frozen-lockfile
pnpm tauri dev        # dev mode with hot reload
pnpm tauri build      # production bundle
```

### NAS / Headless Server

```bash
cargo build --release -p limedl-server
./target/release/limedl daemon --addr 0.0.0.0:8080 --data-dir /var/lib/limedl
```

Open `http://<server-ip>:8080` in a browser. Use `--auth-user` / `--auth-pass` for HTTP Basic Auth.

#### TLS (HTTPS)

Enable TLS with the `tls` feature:

```bash
cargo build --release -p limedl-server --features tls
```

Configure in `settings.json`:

```json
{
  "tls": {
    "enabled": true,
    "certPath": "/etc/limedl/cert.pem",
    "keyPath": "/etc/limedl/key.pem"
  }
}
```

Without TLS, run behind a reverse proxy (nginx, Caddy) for production deployments.

### CLI

```bash
limedl download "https://example.com/file.zip"
limedl download --output ./downloads "https://example.com/file.iso"
```

## Configuration

limedl stores settings as JSON. The default location depends on the platform:

| Platform | Path                                                 |
| -------- | ---------------------------------------------------- |
| Windows  | `%APPDATA%\limedl\settings.json`                     |
| macOS    | `~/Library/Application Support/limedl/settings.json` |
| Linux    | `~/.local/share/limedl/settings.json`                |

Override with `LIMEDL_DATA_DIR` environment variable.

Key settings:

```json
{
  "download": {
    "defaultDownloadDir": "~/Downloads",
    "defaultMaxRetries": 3,
    "defaultChecksum": "none"
  },
  "scheduler": { "mode": "automatic" },
  "bt": {
    "dhtEnabled": true,
    "maxDownloadSpeed": 0,
    "maxUploadSpeed": 0
  },
  "aria2Rpc": { "enabled": true, "port": 6800 },
  "downloadLimits": { "maxConcurrentHttp": 5, "maxConcurrentBt": 3 },
  "maxInMemoryDownloads": 200
}
```

## Development

```bash
# Frontend
pnpm install --frozen-lockfile
pnpm run lint          # oxlint
pnpm exec vue-tsc --noEmit  # type-check
pnpm run test          # vitest

# Rust
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo fmt --check
```

See [`.opencode/guides/`](.opencode/guides/) for architecture and subsystem documentation.

## License

[GNU General Public License v3.0 or later](LICENSE)
