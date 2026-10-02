---
title: Overview
description: Learn about limedl design principles, architecture, core advantages, and benchmark comparisons
---

# About limedl

**limedl** is a next-generation multi-protocol download manager engineered for raw speed, minimal resource consumption, and an ultra-fluid desktop experience. It pairs a high-performance **Rust** engine with a modern native GUI powered by **Slint**.

The project was created to address longstanding pain points across download tooling: commercial downloaders bloated with advertisements, aggressive telemetry, and paywalled speeds; as well as popular open-source alternatives that rely on Electron—frequently consuming 300MB+ of idle RAM and causing disk I/O freezes under multi-gigabit connections.

:::tip[In a Nutshell]
limedl combines an **uncompromising pure Rust engine** with a **sub-100ms cold startup native Slint GUI**, delivering HTTP AIMD adaptive concurrency, complete BitTorrent protocol support, proprietary Cloudflare CDN edge probing, and 100% Aria2 RPC ecosystem compatibility.
:::

---

## Core Design Philosophy

### 1. Say Goodbye to Bloatware, Return to Native
limedl completely rejects Chromium/Electron multi-process overhead. Built with Slint and rendered natively via hardware-accelerated FemtoVG:
- **Instant Launch**: Cold startup completes in **under 100 milliseconds** with immediate responsiveness.
- **Minimal Footprint**: Idle memory in system tray sits comfortably at **~35 to 50 MB**—roughly 1/6th of typical Electron downloaders.
- **Single-Process Model**: The engine and user interface reside within the same binary process, communicating via internal asynchronous channels (EventBus) with zero network serialization overhead.

### 2. Congestion-Aware Adaptive Concurrency (AIMD)
Traditional downloaders often blindly assign fixed thread counts (e.g., 32 or 64 threads), frequently triggering HTTP 429 / 503 rate-limits or severe router packet drops on congested networks.

limedl borrows battle-tested congestion control principles from TCP, implementing **AIMD (Additive Increase Multiplicative Decrease)** dynamic scheduling:
- **Smooth Ramp-Up**: Probes connection latency and response time, gradually ramping up workers using additive increments.
- **Intelligent Backoff**: Upon sensing rate limits (HTTP 429 / 503) or dropped chunks, concurrency cuts back multiplicatively to safeguard the connection.
- **Mirror Failover**: Transparently falls back to secondary mirror endpoints if the primary origin stalls or times out.

### 3. Hardware-Aware Disk Protection
At multi-gigabit speeds, naive multi-threaded file writes overwhelm storage subsystems and induce excessive flash memory wear:
- **Solid-State Drives (SSD)**: Aligned with flash erase-block architecture, limedl employs **Write Combining**, aggregating small chunks in memory into optimal aligned blocks before issuing batch asynchronous flushes—dramatically reducing Write Amplification Factor (WAF).
- **Hard Disk Drives (HDD)**: Employs a **Double-Buffering Pool** with strict physical file offset sorting. Network worker threads feed the active half-buffer while a dedicated background I/O thread flushes sequentially to disk, completely eliminating seek thrashing and OS freeze.

### 4. Open & Seamless Aria2 Ecosystem Compatibility
There is no need to abandon existing browser extensions or scripts. limedl embeds a compliant **Aria2 JSON-RPC 2.0** server:
- Full support for Chrome, Edge, and Firefox extensions (e.g., Aria2 Explorer, Camtd).
- Compatible with cloud-drive export tools and Tampermonkey user scripts.
- Interoperates with web management dashboards such as AriaNg.

---

## Architecture Topology

limedl is organized as a single-track Rust workspace, cleanly decoupling the headless core engine from native desktop presentation:

```
limedl Workspace
├── crates/limedl-core/       # Headless Rust engine (zero UI dependencies)
│   ├── manager.rs            # DownloadManager orchestration & task lifecycle
│   ├── scheduler.rs          # AIMD thread balancing & allocation engine
│   ├── http_executor.rs      # HTTP Range probing & concurrent chunk workers
│   ├── bt_backend/           # BitTorrent subsystem (pinned irontide 1.7.0)
│   ├── cdn/                  # Cloudflare edge screening & DNS rewrite
│   ├── buffer_pool.rs        # SSD write-combining & HDD double buffering
│   ├── file_ops/             # Disk space pre-allocation & atomic finalization
│   ├── checksum/             # Blake3 / SHA-256 / SHA-512 validation
│   ├── rate_limiter/         # Token bucket global speed limiter & schedule
│   └── aria2_rpc.rs          # Aria2 JSON-RPC 2.0 compatible server
│
├── crates/limedl-native/     # Native desktop client (Slint + FemtoVG rendering)
│   ├── main.rs               # Application bootstrapping & event loop
│   ├── bridge/               # Rust model to Slint UI property mapper
│   ├── handlers/             # UI callback dispatchers
│   ├── update.rs             # Minisign cryptographic in-app self-update
│   └── ui/                   # Slint component definitions & theme tokens
│
└── xtask/                    # Repository tooling: Minisign sign/guard gates
```

### Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    Presentation Layer (Slint Native UI)     │
│       [Cards/Table View]  [New Task]  [Settings]  [CDN Labs]│
└───────────────────────┬───────────────────▲─────────────────┘
                        │ UI Callbacks      │ Model Update
                        ▼                   │ (UI Repaint)
┌─────────────────────────────────────────────────────────────┐
│                limedl-native Bridge / Handlers              │
└───────────────────────┬───────────────────▲─────────────────┘
                        │ API Invocations   │ EventBus Sub
                        ▼                   │ (Broadcast)
┌─────────────────────────────────────────────────────────────┐
│                 limedl-core Dispatcher                      │
├─────────────────────────────────────────────────────────────┤
│   BackendRegistry (Protocol Router)                         │
│      ├─ [http: prefix] ──► DownloadManager (AIMD Chunking)  │
│      └─ [bt: prefix]   ──► IrontideBtBackend (DHT & Swarms) │
├─────────────────────────────────────────────────────────────┤
│   Core Subsystems:                                          │
│   • BufferPool (I/O Buffer)      • Database (SQLite WAL)    │
│   • CdnAccelerator (IP Probing)  • RateLimiter (Tokens)     │
│   • Aria2RpcServer (Port 6800)   • FileOps (Pre-allocation) │
└─────────────────────────────────────────────────────────────┘
```

---

## Comparison Matrix

| Metric / Dimension | **limedl** | Commercial (e.g. Xunlei) | Electron (e.g. Motrix) | Classic CLI (Aria2) |
| :--- | :--- | :--- | :--- | :--- |
| **Core Engine** | **Pure Rust 2024** | Proprietary C++ | Node.js + Aria2 bundle | Pure C++ |
| **User Interface** | **Slint (Native FemtoVG)** | Webview wrapper (ads/popups) | Electron (Chromium) | Headless CLI |
| **Cold Startup Time** | **< 100 ms** | 3 ~ 6 s (auth & banners) | 2 ~ 4 s (blank window) | Instantaneous |
| **Idle Memory Footprint** | **~35 to 50 MB** | 180 ~ 350 MB+ | 250 ~ 450 MB+ | < 20 MB |
| **Concurrency Strategy** | **AIMD Dynamic Adaptive** | Fixed threads / paywalled | Fixed thread count | Fixed thread count |
| **Storage Protection** | **SSD Combining + HDD Ping-Pong** | Generic unbuffered | Basic stream write | Standard buffering |
| **CDN Edge Acceleration** | **Integrated Cloudflare Probing** | None (Private P2P only) | None | None |
| **BitTorrent Engine** | **Irontide (DHT/PEX/LSD)** | Proprietary P2P | Aria2 BT implementation | Native Aria2 BT |
| **Aria2 RPC Ecosystem** | **100% Native Compliant** | Unsupported | Self as GUI wrapper | Protocol definer |
| **License & Cleanliness** | **GPL-3.0 (Zero Ads/Telemetry)** | Proprietary closed-source | MIT Open Source | GPL-2.0 Open Source |

---

## What to Read Next

- [Installation Guide](/en/getting-started/installation/): Detailed steps to acquire prebuilt packages for Windows, macOS, and Linux.
- [Configuration Reference](/en/getting-started/configuration/): Comprehensive breakdown of all settings and parameters.
- [Tasks & Concurrency](/en/guides/tasks/): Learn how AIMD chunking and scheduling work under the hood.
- [Browser Extensions](/en/ecosystem/browser/): Connect Chrome, Edge, and Firefox for seamless one-click link interception.
