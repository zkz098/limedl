---
title: Configuration Reference
description: Comprehensive reference for all limedl desktop settings, configuration options, and tuning parameters
---

# Configuration Reference

limedl provides a fine-grained, robust configuration architecture. All configuration parameters are persisted as standard JSON in `settings.json` within your user data directory, managed atomically via Rust transactions and atomic file renaming (`.json.tmp` → `.json`) to guard against corruption.

To open the settings panel in the desktop application, click the **Settings** gear icon in the left navigation sidebar or press the shortcut key.

---

## 1. General & Appearance

| Option | Values / Types | Default | Details |
| :--- | :--- | :--- | :--- |
| **Language** | 简体中文 / 繁體中文 / English | System Locale | Instantly updates the application's interface language. |
| **Color Mode** | Dark / Light / Follow System | Follow System | High-contrast native themes with automatic switching matching your OS dark/light mode. |
| **Close Window Behavior** | Minimize to Tray / Exit App | Minimize to Tray | Determines whether clicking the window close button (`X`) minimizes limedl into the system tray or terminates the process. |
| **Start on Boot** | Enabled / Disabled | Disabled | Registers autostart entries with `--hidden` to launch silently in the tray on login. |
| **Suppress System Sleep** | Enabled / Disabled | Enabled | Acquires OS power locks to prevent system sleep during active downloads; releases locks upon completion. |
| **List View Mode** | Card View / Compact Table View | Card View | The compact table view provides detailed sorting columns for managing large quantities of tasks. |

---

## 2. Network & Proxy

| Option | Values / Types | Default | Details |
| :--- | :--- | :--- | :--- |
| **Proxy Mode** | Direct / System / Manual | System Proxy | **Direct**: No proxy.<br>**System**: Reads the OS static proxy configuration (Windows Internet Options / macOS network settings, including the bypass list). PAC/WPAD auto-config scripts are not supported — use **Manual** for those.<br>**Manual**: User-specified custom proxy host and port. |
| **Manual Protocol** | HTTP / SOCKS5 | HTTP | Supports forwarding traffic through local or LAN proxies (e.g., `127.0.0.1:7890`). |
| **Default User-Agent** | String | Modern Chrome UA | Identifies the client during HTTP transfers. Useful when University or open-source mirrors reject unrecognized or outdated user-agents with HTTP 403. |
| **Connection Timeout** | Seconds (10 ~ 120s) | 30s | Handshake and idle read/write socket timeout thresholds. |

---

## 3. Downloads & Scheduler

| Option | Values / Types | Default | Details |
| :--- | :--- | :--- | :--- |
| **Default Directory** | Absolute Path | OS "Downloads" folder | Default filesystem destination for incoming downloads. |
| **Scheduler Mode** | Traditional / Automatic | Automatic | **Traditional**: Strict FIFO queue respecting max concurrent tasks.<br>**Automatic**: Prioritizes remaining bytes, dynamically reallocating thread pool budgets across tasks. |
| **Max Concurrent Tasks** | 1 ~ 32 | 5 tasks | Maximum number of downloads permitted in active state simultaneously. |
| **Global Max Threads** | 4 ~ 128 | 32 threads | Upper bound for all active chunk worker threads across all running downloads. |
| **AIMD Profile** | Conservative / Balanced / Aggressive | Balanced | **Conservative**: Slow ramp-up and sharp backoff for sensitive hosts.<br>**Balanced**: Smooth additive increase and multiplicative decrease.<br>**Aggressive**: Rapid bandwidth acquisition for dedicated high-speed pipelines. |
| **Checksum Mode** | None / Blake3 / SHA-256 / SHA-512 | Blake3 (when provided) | Automatically verifies data integrity post-download, re-fetching only damaged chunks on mismatches. |

---

## 4. Rate Limiting & Speed Schedule

- **Global Download Speed Limit**: Configurable in `KB/s` or `MB/s` (`0` means unlimited). Powered by an in-memory Token Bucket algorithm with nanosecond-scale overhead.
- **Global Upload Speed Limit**: Regulates BitTorrent seeding bandwidth to prevent saturating uplink channels.
- **Time-Based Speed Schedules**:
  Define custom windows by day of week and time. For example:
  - `Weekdays 09:00 - 18:00`: Throttle download speed to `2000 KB/s` (reserving bandwidth for meetings and work);
  - `Night 00:00 - 08:00`: Unlimited full-speed night downloading.

---

## 5. BitTorrent Settings

Full control over the embedded **Irontide** P2P engine:

| Option | Recommended | Details |
| :--- | :--- | :--- |
| **Listen Port** | 6881 (or dynamic) | TCP and UDP listen port for incoming peer connections. |
| **UPnP / NAT-PMP** | Enabled | Automatically requests port forwarding on your router to maximize peer connectability. |
| **Mainline DHT** | Enabled | Enables trackerless swarm discovery via decentralized hash table. |
| **Peer Exchange (PEX)**| Enabled | Exchanged known peers directly between connected swarms. |
| **Local Discovery (LSD)**| Enabled | Broadcasts locally to discover LAN peers for gigabit multi-megabyte transfers. |
| **Encryption** | Preferred | Protects traffic against ISP deep packet inspection (DPI) while preserving backward compatibility. |
| **Seeding Ratio Limit**| 1.0 ~ 2.0 | Halts seeding automatically once upload ratio reaches the target multiplier (`0` = seed indefinitely). |
| **Public Trackers** | List of URLs | Automatically appends healthy public trackers to every new torrent or magnet download. |

---

## 6. Aria2 RPC Server

limedl embeds a compliant Aria2 JSON-RPC 2.0 server running by default on startup:

- **Enable Aria2 RPC**: Enabled by default.
- **Port**: Default is `6800` (can be changed if another Aria2 daemon is running).
- **Secret Token**: Optional authentication token for secure or remote environments.
- **CORS Support**: Pre-configured out of the box for web control dashboards like AriaNg.

---

## 7. Storage & Buffer Pool

- **Automatic Medium Detection**:
  On Windows, limedl queries `DeviceIoControl` with `IOCTL_STORAGE_QUERY_PROPERTY` to check seek penalties, identifying whether a target directory is on an SSD or an HDD:
  - **SSD Mode**: Enables Write Combining, bundling small chunk writes into aligned bulk writes.
  - **HDD Mode**: Enables the Double-Buffering Pool, using a dedicated background thread to flush sequentially.
- **Network Locations and WSL**:
  UNC shares, mapped network drives and network mounts on Linux/macOS are reported as **Network share — media unknown**: scheduled like an SSD and listed in the settings panel instead of being silently claimed as an SSD (the local seek-penalty probe says nothing about them).
  `\\wsl$\<distro>` is *not* network storage: the 9p/virtiofs server behind it serves a local `ext4.vhdx`, so limedl resolves the distro's virtual disk through the registry (`HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss`) and schedules by the media of the host volume that actually stores it.
- **Manual Overrides (`disk_type_overrides`)**:
  Add a directory under **Settings → IO Lab → Directory Media Overrides** and pin it to SSD or HDD.
  Matching is a **normalized path-prefix match** (case-insensitive and trailing-separator-insensitive on Windows), so `D:\Downloads` also covers everything inside it; the longest matching entry wins. Each row shows the media auto-detection currently reports for that path, so you can see whether the override is still needed.
  Changes take effect on save: not only the buffering mode but also the device I/O queue's writer-thread count is rebuilt for the new media (one serialized thread for HDD, four parallel channels for SSD/network) — no restart required.

---

## 8. Labs

Experimental high-performance features:

- **Cloudflare CDN Probe**: Screens latency and bandwidth across worldwide edge nodes to rewrite transport DNS.
- **URL Rewrite Rules**: Regular expression mapping to redirect slow URLs (e.g. GitHub Releases) to high-speed mirror endpoints.
