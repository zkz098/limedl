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

| Target        | Frontend                | Backend                 | Build                                     |
| ------------- | ----------------------- | ----------------------- | ----------------------------------------- |
| Desktop       | Slint (native, Windows) | `crates/limedl-native/` | `cargo build -p limedl-native` (see below) |
| NAS / Server  | Vue 3 via WebSocket     | `limedl-server`         | `pnpm run build:nas` + `cargo build -p limedl-server` |
| CLI           | N/A                     | `limedl-server`         | `limedl download <url>` / `limedl daemon` |

The download engine (`limedl-core`) is pure Rust with zero UI dependencies and powers every
target. Releases ship the Slint desktop client (Windows) plus the headless NAS build with the
WebUI embedded; the Tauri desktop shell in `src-tauri/` is kept in-tree but is no longer built
or released (its updater manifest `latest.json` is gone, so Tauri installs stop updating).
macOS/Linux users are served by the NAS build (`limedl daemon` + browser) today.

## Quick Start

### Desktop (Slint, Windows)

```powershell
# MiSans VF is embedded at compile time and is not in git (font license)
pwsh scripts/fetch-misans.ps1
cargo run -p limedl-native
```

The app keeps everything in `%LOCALAPPDATA%\limedl` (override with `LIMEDL_DATA_DIR`), imports
settings/history from a previous Tauri install on first run, and updates itself in-app
(portable / NSIS / MSIX channels — see `crates/limedl-native/src/update.rs`).

### NAS / Headless Server

```bash
cargo build --release -p limedl-server
./target/release/limedl daemon --addr 0.0.0.0:8080 --data-dir /var/lib/limedl
```

Build the WebUI first (`pnpm run build:nas`) or build the server with `--features embed-frontend`
to bake `dist/` into the binary. Open `http://<server-ip>:8080` in a browser; use `--auth-user` /
`--auth-pass` for HTTP Basic Auth.

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
    "globalDownloadRateLimit": 0,
    "globalUploadRateLimit": 0
  },
  "aria2Rpc": { "enabled": true, "port": 6800 },
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
