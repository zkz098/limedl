---
title: CLI & Protocol Handlers
description: Command-line arguments, silent startup, single-instance IPC handover, and native protocol association integrations
---

# CLI & Protocol Handlers

While limedl is primarily a native desktop graphical client, it provides a comprehensive command-line interface (CLI) and operating system protocol hooks (Deep Links).

You can dispatch downloads to limedl directly from shell scripts, cron jobs, automation pipelines, or third-party web apps.

---

## Executable Locations

Depending on your operating system, the binary is located at:

- **Windows**:
  - Installer: `C:\Users\<User>\AppData\Local\Programs\limedl-native\limedl-native.exe`
  - Portable: `limedl-native.exe` inside your unpacked directory.
- **macOS**:
  - `/Applications/limedl.app/Contents/MacOS/limedl-native`
- **Linux**:
  - Portable: `./limedl-native`
  - AppImage: `./limedl-native-v0.3.14-linux-x86_64.AppImage`
  - DEB: Globally accessible via `/usr/bin/limedl-native`

---

## Command-Line Syntax

```bash
limedl-native [OPTIONS] [PAYLOAD]
```

### Supported Options

| Flag | Description | Typical Use Case |
| :--- | :--- | :--- |
| `--hidden` | Silent launch. Minimizes directly to the system tray without raising the main window. | Autostart on boot, headless background daemons, or NAS usage. |
| `-h`, `--help` | Prints help documentation. | Quick reference for CLI usage. |
| `-V`, `--version` | Displays current client version. | CI scripts and version checks. |

### Supported Payload Formats

You can append any valid download resource directly to the command:

#### 1. HTTP/HTTPS Download Link
```powershell
# Windows
limedl-native.exe "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso"

# Linux
limedl-native "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso"
```

#### 2. BitTorrent Magnet Link
```bash
limedl-native "magnet:?xt=urn:btih:d2b0e9a72c38e21e64c81979360cb53527655f02&dn=Ubuntu"
```

#### 3. Local .torrent File Path
```bash
limedl-native "D:\Downloads\linuxmint-22-cinnamon-64bit.iso.torrent"
```

#### 4. Native limedl:// Deep Link
```bash
limedl-native "limedl://download?url=https://example.com/file.zip&filename=file.zip"
```

---

## Single Instance & IPC Handover

Executing `limedl-native` repeatedly from terminal scripts **will never spawn duplicate, conflicting processes**.

limedl features platform-native single-instance IPC handovers:
- **Windows**:
  Leverages a Win32 Named Mutex to detect an existing running instance. If found, the secondary process forwards the command payload to the primary window via the `WM_COPYDATA` messaging channel and brings the window to focus before exiting immediately.
- **macOS & Linux**:
  Uses an authenticated local loopback TCP socket handshake to transmit incoming URLs or torrent paths to the primary event loop.

This architecture ensures seamless integration with external scripts: all downloads converge into a single centralized window.

---

## Protocol Handlers & Deep Links

During installation, limedl registers system protocol handlers:

### 1. Magnet Links (`magnet:`)
Clicking any `magnet:?xt=...` link in web browsers prompts the OS to open limedl, which raises the window and starts DHT peer discovery.

### 2. Native Deep Links (`limedl://`)
Allows custom websites or internal intranets to trigger limedl directly.

**URL Scheme Structure**:
```text
limedl://download?url=<ENCODED_URL>&filename=<OPTIONAL_NAME>&referer=<OPTIONAL_REFERER>
```

**Query Parameters**:
- `url` *(Required)*: URL-encoded direct download link.
- `filename` *(Optional)*: Suggested target filename.
- `referer` *(Optional)*: Custom HTTP Referer header for protected hosts.

**HTML Integration Example**:
```html
<a href="limedl://download?url=https%3A%2F%2Fexample.com%2Fbigfile.zip&filename=demo.zip">
  Download with limedl
</a>
```

---

## Environment Variables

| Variable | Default Value | Description |
| :--- | :--- | :--- |
| `LIMEDL_DATA_DIR` | `%LOCALAPPDATA%\limedl` | Overrides the primary data directory. Point this to a relative path on a flash drive for a 100% self-contained portable install. |
| `LIMEDL_TAURI_DATA_DIR` | `%LOCALAPPDATA%\com.zkz20.limedl` | Overrides the migration source path for legacy Tauri installs. |
| `RUST_LOG` | `info` | Adjusts logging verbosity (e.g. `limedl_core=debug,limedl_native=trace`). |
