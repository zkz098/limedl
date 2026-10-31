# limedl

Fast multi-protocol download manager — HTTP, BitTorrent, with CDN acceleration. Native desktop app for Windows, macOS and Linux.

## Features

- **HTTP downloads** — chunked parallel downloading with adaptive concurrency (AIMD), automatic retry, mirror failover, and resumable transfers
- **BitTorrent** — full-featured BT client via librqbit engine (DHT, UPnP, PEX, LSD, magnet links, .torrent files)
- **CDN acceleration** — Cloudflare IP probing and DNS rewriting for faster downloads
- **Aria2 RPC** — Aria2-compatible JSON-RPC 2.0 API enables integration with AriaNg, Motrix, and other frontends
- **Buffer pool** — HDD double-buffer / SSD write-combining tuned to your disk type
- **Rate limiting** — configurable global speed limits
- **Multi-platform** — same engine powers all targets

### BitTorrent engine status

limedl's BT backend is built on **librqbit** (`9.0.1`), an actively maintained, high-performance BitTorrent engine written in Rust (Apache-2.0).

## Platforms

| Target   | Frontend                | Backend                 | Build                                                              |
| -------- | ----------------------- | ----------------------- | ------------------------------------------------------------------ |
| Windows  | Slint (native)          | `crates/limedl-native/` | `cargo build -p limedl-native` (see below)                         |
| macOS    | Slint (native)          | `crates/limedl-native/` | `cargo run -p limedl-native`; bundle via `scripts/package-macos.sh` |
| Linux    | Slint (native)          | `crates/limedl-native/` | `cargo run -p limedl-native`; bundle via `scripts/package-linux.sh` |

The download engine (`limedl-core`) is pure Rust with zero UI dependencies. Releases ship the
Slint desktop client for all three platforms.

For NAS / soft-router / headless use, the workspace also builds **`limedl-server`** — a
daemon with no GUI that exposes the same Aria2 JSON-RPC API, so AriaNg and other clients
can drive it remotely. Releases publish static musl binaries for `x86_64` and `aarch64`
(plus a systemd unit and a multi-arch `ghcr.io/zkz098/limedl-server` image); see
[`docs/server-daemon.md`](docs/server-daemon.md).

## Quick Start

### Desktop (Slint)

All platforms:

```bash
# MiSans VF is embedded at compile time and is not in git (font license)
cargo xtask fetch-font
cargo run -p limedl-native
```

The app keeps everything in the OS local data directory (Windows `%LOCALAPPDATA%\limedl`;
override with `LIMEDL_DATA_DIR`) and updates itself in-app (portable / NSIS / MSIX channels —
see `crates/limedl-native/src/update/mod.rs`).

## Configuration

limedl stores settings as JSON. The default location depends on the platform:

| Platform | Path                                                    |
| -------- | ------------------------------------------------------- |
| Windows  | `%LOCALAPPDATA%\limedl\settings.json`                    |
| macOS    | `~/Library/Application Support/limedl/settings.json`     |
| Linux    | `~/.local/share/limedl/settings.json`                    |

Override with `LIMEDL_DATA_DIR` environment variable.

Key settings:

```json
{
  "download": {
    "defaultDownloadDir": "/absolute/path/to/Downloads",
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

Paths in `settings.json` must be absolute — the loader drops a relative `defaultDownloadDir`.

## Development

```bash
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --manifest-path crates/limedl-core/Cargo.toml --features "test-utils,aria2-rpc"
cargo nextest run --manifest-path crates/limedl-native/Cargo.toml
cargo nextest run --manifest-path xtask/Cargo.toml
```

See [`.wiki/pages/`](.wiki/pages/) for architecture and subsystem documentation and [`docs/`](docs/) for operational runbooks.

## License

[GNU General Public License v3.0 or later](LICENSE)
