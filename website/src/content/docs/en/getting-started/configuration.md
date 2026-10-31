---
title: Configuration
description: Complete reference for limedl desktop settings, defaults, and recommended configurations
---

# Configuration

limedl keeps its configuration in `settings.json` inside the application data directory. Writes go through an in-memory transaction and an atomic rename (`.json.tmp` → `.json`) so concurrent reads and writes stay consistent.

Open **Settings** from the left toolbar for the graphical panel. A few engine-level switches (for example HTTP/3) live only in `settings.json`; they are called out below.

---

## 1. General & Appearance

| Setting | Values / Default | Notes |
| :--- | :--- | :--- |
| **Language** | Simplified Chinese / Traditional Chinese / English / Follow system (default) | Applies immediately. |
| **Color mode** | Light / Dark / Follow system (default) | Follow system tracks the OS light/dark preference. |
| **Accent color** | Amber / Sky / Lime (default) / Violet / Monochrome | Accent and theme styling. |
| **Background opacity** | Default / Acrylic / Frosted | Window background transparency/blur; off by default. |
| **Close main window** | Minimize to tray (default) / Exit | Keep running in the tray or quit. |
| **Start on login** | On / Off (default) | Registers a startup entry; launches with `--hidden` into the tray. |
| **Prevent sleep while downloading** | On (default) / Off | Takes a power lock only while tasks are active; the screen may still turn off. |
| **System notifications** | On / Off (default) | Desktop notification when a task completes or fails. |
| **Double-click completed task** | None (default) / Open file / Reveal in file manager / Open download directory | Action for completed tasks. |
| **Double-click uncompleted task** | None (default) / Toggle pause | Action for unfinished tasks. |
| **Compact view** | On / Off (default) | Off is the card layout; on is the compact table. |
| **Show detail info** | On (default) / Off | Secondary details such as speed and ETA in task cards. |
| **Visible columns** | File name, size, downloaded, status, progress, speed, ETA (default) | Table columns; the file-name column is always kept. |
| **Max in-memory downloads** | `200` (default) | `maxInMemoryDownloads`: completed/failed tasks kept in memory; older entries are evicted. Database rows are unaffected. |

---

## 2. Network & Proxy

| Setting | Values / Default | Notes |
| :--- | :--- | :--- |
| **Proxy mode** | Direct (default) / System / Manual | **Direct** bypasses proxies. **System** reads the OS static proxy configuration (Windows Internet Options / macOS network settings); PAC/WPAD is not supported. **Manual** uses the URL below. |
| **Manual proxy URL** | Empty | Full URL such as `http://127.0.0.1:7890` or `socks5://127.0.0.1:1080`. The scheme selects the protocol; there is no separate protocol dropdown. |
| **Default User-Agent** | Built-in modern Chrome UA | Used for HTTP downloads; customize when a mirror rejects the default UA. |
| **Auto-detect checksum** | On (default) / Off | Prefers response-header checksums (RFC 3230/9530, S3, GCS), then probes companion files such as `.sha256` and `SHA256SUMS`. |
| **Default retries** | `5` (default) | Maximum retries for retryable task errors. |
| **Connect / read timeout** | 30s / 30s (default) | Fixed at 30 seconds outside expert mode; expert mode allows 3–120s and 5–300s respectively. |

---

## 3. Downloads & Scheduler

| Setting | Values / Default | Notes |
| :--- | :--- | :--- |
| **Default download directory** | Local path | Default destination for new tasks; must be absolute. |
| **Scheduler mode** | Automatic (default) / Traditional | Automatic allocates a shared thread budget; Traditional activates tasks by priority, then creation time. |
| **Traditional max parallel tasks** | 1–32, default `3` | Traditional mode only: maximum tasks in the Downloading state. |
| **Global max worker threads** | 1–64, default `16` | Shared ceiling for HTTP chunk workers across all tasks. |
| **Per-task max threads** | 1–32, default `8` | Automatic mode: maximum threads one task may hold. |
| **Per-task min threads** | `0` (default) | 0 means no floor; a task can be reduced to zero and queued under budget pressure. |
| **AIMD profile** | Conservative / Balanced (default) / Aggressive | Controls additive increase and multiplicative decrease. |
| **Chunk strategy** | Adaptive (default) / Fixed | Fixed uses the expert custom chunk size (0 = derive from file size). |
| **Tail Sprint** | On / Off (default) | Splits/retries the last large chunks to reduce long tails. |
| **Connection warmup** | On (default) / Off | Pre-opens some connections to reduce time-to-first-byte. |
| **Automatic checksum mode** | None / Blake3 (default) / SHA-256 / SHA-512 | Post-download verification. A generic checksum mismatch fails the task and preserves `.part.corrupt`; it does **not** auto-redownload bad chunks. Metalink piece verification is a separate per-piece mechanism. |
| **HTTP/3 Alt-Svc** | `scheduler.http3AltSvcEnabled` in `settings.json`, on by default | Tries HTTP/3 (QUIC) for hosts that advertise Alt-Svc; failures trip a circuit breaker and fall back to TCP. UDP 443 must be available. Not exposed in the settings dialog. |

### Expert Mode

Enable **Settings → Downloads & Scheduler → Expert mode** to override the automatic strategy. Common knobs:

| Parameter | Default | Notes |
| :--- | :--- | :--- |
| Scheduler tick | `2000 ms` | Scheduler loop period. |
| Max connections per host | `16` | Caps per-host concurrency. |
| AIMD degrade / increase / severe-drop thresholds | `0.16` / `0.04` / `0.50` | Throughput deltas that trigger each action. |
| AIMD cooldown | `3 s` | Observation window after a decrease. |
| AIMD samples needed / probe stable cycles | `1` / `2` | Consecutive condition counts. |
| AIMD decrease factor / step ratio | `0.50` / `0.25` | Multiplicative decrease and additive step. |
| Oscillation threshold / hysteresis multiplier | `3` / `4` | Damp thread-count oscillation. |
| Custom chunk size | `0` (auto) | MiB; 0 derives the size from the file. |
| Tail-sprint stall window / min split | `8 s` / `1 MiB` | Tail-sprint triggers. |
| Work-steal min split | `2 MiB` | Smallest work-stealing split. |
| Connect / read timeout | `30 s` / `30 s` | Ranges: 3–120s and 5–300s. |
| Pool idle per host / TCP keepalive | `20` / `60 s` | Idle pool and keepalive. |
| Retry backoff base / max / jitter | `250 ms` / `4000 ms` / `25%` | Exponential backoff. |
| Queue sort strategy | Priority first (default) / Shortest job first / FIFO | Ordering key in automatic mode. |

---

## 4. Speed Limits & Schedule

- **Global download limit**: KB/s or MB/s, `0` means unlimited; enforced by a token bucket.
- **Global upload limit**: caps BitTorrent upload so ACKs are not starved.
- **Speed schedule**:
  - Rules match **local-time hours only**; there is no weekday/date dimension.
  - `start_hour` is inclusive and `end_hour` is exclusive (9–18 means 09:00 through 17:59).
  - `start_hour > end_hour` wraps midnight (for example 22–6).
  - Rules are evaluated in order and the first match wins; no match falls back to the global limit.
  - A limit of `0` means unlimited for that slot.

Examples:

- Every day `09:00 - 18:00`: limit downloads to `2000 KB/s`;
- Every day `00:00 - 08:00`: unlimited (`0`).

---

## 5. BitTorrent Settings

The BT backend is built on **librqbit** and supports DHT, UPnP, PEX, and LSD. PEX/LSD/transport encryption are not user toggles; the engine follows the protocol behavior.

| Setting | Values / Default | Notes |
| :--- | :--- | :--- |
| **Listen port** | Empty (OS-assigned) by default; e.g. `6881` | TCP/UDP port for inbound peers. |
| **UPnP port mapping** | On / Off (default) | Requests a mapping on UPnP-capable routers. |
| **DHT** | On (default) / Off | Mainline DHT; magnet links work without a tracker. |
| **IPv6** | On (default) / Off | Listen/connect over IPv6. |
| **µTP** | On (default) / Off | BEP 29 micro transport protocol. |
| **Max concurrent BT downloads** | `5` (default) | Active BT task limit. |
| **Max peers per torrent** | `128` (default) | Per-torrent connection limit. |
| **Global download / upload limit** | `0` (unlimited) by default | BT-specific rate limits. |
| **Upload limit / share ratio** | Upload limit `0`; ratio `0` (unlimited) | Stop seeding after the ratio; optional pause-on-upload-limit. |
| **Lightweight BT mode** | On / Off (default) | Unload the BT engine while idle and start it on demand, reducing idle memory. |
| **Public tracker list** | Built-in list URL | Source of trackers injected into new magnets/torrents. |
| **Anti-leech** | On / Off (default) | Defaults: 300s grace, 0.1 give-back ratio, 3600s ban, 4 upload slots; choose throttle-only or ban. |
| **IP blocklist** | On / Off (default) | Loads an eMule `.dat` or plaintext CIDR list. |

---

## 6. Aria2 RPC

limedl exposes an Aria2-compatible JSON-RPC 2.0 endpoint over both HTTP POST and WebSocket at `/jsonrpc`. It is **disabled by default** for security.

- **Enable Aria2 RPC**: off by default; opt in from Settings.
- **Listen address / port**: `127.0.0.1:6800` by default. A non-loopback listener requires authentication.
- **Authentication**:
  - **Single shared secret**: requests carry `token:<secret>`;
  - **Per-client tokens**: each client gets its own token; only an Argon2id hash and a short plaintext prefix are stored locally.
- **CORS**: `http://localhost` and `http://127.0.0.1` are allowed by default; add origins or allow all (`*`). A token is still required.
- **Shutdown behavior**: the desktop client ignores `aria2.shutdown`; `limedl-server` maps it to process exit.

---

## 7. Storage & Buffer Pool

### Media detection and overrides

- Windows identifies SSD/HDD from storage-device properties. Network shares, mapped drives, and network mounts on Linux/macOS are treated as "network" and scheduled like SSDs.
- `\\wsl$\<distro>` resolves the backing `ext4.vhdx` and schedules by the host volume's real media.
- **Directory overrides (`disk_type_overrides`)**: set SSD/HDD per directory under **Settings → IO Lab → Directory media override**. Matching is a normalized prefix (most specific wins) and takes effect immediately, rebuilding the I/O queues.

### Buffer pool parameters

| Setting | Default | Notes |
| :--- | :--- | :--- |
| HDD buffer limit | `1024 MiB` | Shared write-buffer ceiling for HDD downloads. |
| HDD parallel buffer slots | `4` | Concurrent ordered HDD write buffers. |
| HDD double buffering | On | Off switches HDD to a smaller write-combining buffer. |
| SSD write-combining buffer | `0` (auto, by chunk size) | Per-task cap, up to 4096 MiB. |
| Game Mode buffer | `128 MiB` | Buffer size while Game/Performance mode is active. |
| Game Mode parallel slots | `1` | Reduced buffer parallelism in Game Mode. |

Game Mode and Overclock Mode are runtime toggles and are not persisted: the former lowers disk I/O interference, the latter relaxes concurrency limits.

---

## 8. Logging

| Setting | Default | Notes |
| :--- | :--- | :--- |
| Enable logging | On | When off, only console output remains. |
| Log level | `info` | trace / debug / info / warn / error. |
| Log file path | Empty (default file under the data directory) | An absolute path may be supplied. |
| Retention count / days | Empty (no count/age cleanup) | Old logs are removed at startup. |

Logs live in the data directory (see the installation guide). Attach the relevant lines when filing an issue.

---

## 9. Labs

- **CDN acceleration**: supports **Cloudflare** (default), **Fastly**, and a **custom provider**.
  - A custom provider takes a test URL plus IPv4/IPv6 CIDR lists;
  - A speed test consumes 20–50 MB of traffic; apply the best edge IP afterwards;
  - Acceleration only applies to domains matching the selected provider's ranges and falls back to normal DNS on failure.
- **URL rewrite rules**: match by host/prefix/regex/wildcard and rewrite slow sources to mirrors, with optional headers and replacement templates.
- **Integrity verification**: the task context menu offers "Verify integrity"; see [Tasks & Concurrency](/en/guides/tasks/).
