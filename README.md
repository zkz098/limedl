# limedl

Fast multi-protocol download manager — HTTP, BitTorrent, with CDN acceleration. Native desktop app for Windows, macOS and Linux.

## Features

- **HTTP downloads** — chunked parallel downloading with adaptive concurrency (AIMD), automatic retry, mirror failover, and resumable transfers
- **BitTorrent** — full-featured BT client via irontide engine (DHT, UPnP, PEX, LSD, magnet links, .torrent files)
- **CDN acceleration** — Cloudflare IP probing and DNS rewriting for faster downloads
- **Aria2 RPC** — Aria2-compatible JSON-RPC 2.0 API enables integration with AriaNg, Motrix, and other frontends
- **Buffer pool** — HDD double-buffer / SSD write-combining tuned to your disk type
- **Rate limiting** — configurable global speed limits
- **Multi-platform** — same engine powers all targets

### BitTorrent engine status

limedl's BT backend is built on **irontide**, pinned to one exact release (`irontide = "=1.7.0"`)
and locked by `Cargo.lock`.

- Upstream's git repository (`codeberg.org/alan090/irontide`) was **removed in 2026** and no public
git copy survives (Software Heritage has no origin for it), so crates.io is the only remaining
source of truth: the published 1.7.0 tarballs, checksummed in `Cargo.lock`.
- The author's current forge (`git.alangaudet.dev`) does not host it. His project page says the
engine was "reopened for a 2.0 correctness recovery" and that fixes on the recovery branch are
**unpublished**; the defects listed there that touch limedl are pure-v2 torrent identity, v2-swarm
handshakes on hybrid torrents, pad-file accounting and resume.
- The pin is exact so an unreviewed 1.8/2.0 cannot arrive through `cargo update`: the BT backend in
`crates/limedl-core/src/bt_backend/` depends on a wide slice of the engine API (session methods,
session settings, alert stream), and moving off 1.7.0 is a migration, not a version bump.
- **Status: no fork.** We build the published 1.7.0 as-is. Vendoring a source snapshot, patching the
engine locally, or replacing it wholesale (e.g. librqbit) are separate decisions to be taken
deliberately — the engine is frozen, so those options stay open rather than decay.

## Platforms

| Target   | Frontend                | Backend                 | Build                                                              |
| -------- | ----------------------- | ----------------------- | ------------------------------------------------------------------ |
| Windows  | Slint (native)          | `crates/limedl-native/` | `cargo build -p limedl-native` (see below)                         |
| macOS    | Slint (native)          | `crates/limedl-native/` | `cargo run -p limedl-native`; bundle via `scripts/package-macos.sh` |
| Linux    | Slint (native)          | `crates/limedl-native/` | `cargo run -p limedl-native`; bundle via `scripts/package-linux.sh` |

The download engine (`limedl-core`) is pure Rust with zero UI dependencies. Releases ship the
Slint desktop client for all three platforms.

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

See [`.opencode/guides/`](.opencode/guides/) for architecture and subsystem documentation.

## License

[GNU General Public License v3.0 or later](LICENSE)
