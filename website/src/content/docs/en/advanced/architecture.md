---
title: System Architecture
description: Deep dive into limedl single-track pure Rust design, protocol routing, EventBus broadcasting, and cryptographic self-update pipeline
---

# System Architecture

limedl is engineered around a **pure Rust 2024 single-track** philosophy. Over its architectural evolution, the project deliberately discarded hybrid shells (Electron / WebView) in favor of a lean, native, and memory-safe design that has zero external runtime overhead.

---

## Workspace Layout

The repository is structured as a tightly focused Rust workspace:

```
limedl/
├── crates/limedl-core/     # Headless pure Rust multi-protocol engine (no UI dependencies)
├── crates/limedl-native/   # Slint-based native desktop client (Skia hardware backend)
└── xtask/                  # Repository maintenance tooling (Minisign signing & release gates)
```

### 1. `limedl-core` (The Download Engine)
- **Responsibilities**: Task orchestration, AIMD concurrency scheduling, HTTP range probing and chunking, BitTorrent swarms (irontide), Cloudflare CDN probing, hardware-aware buffer pools, SQLite persistence, and the Aria2 JSON-RPC 2.0 server.
- **Characteristics**: Built purely on Tokio's asynchronous runtime with zero UI dependencies.

### 2. `limedl-native` (Native Slint Client)
- **Responsibilities**: Native declarative UI built with **Slint** on top of Skia rendering. Handles window geometry persistence, single-instance mutexes, system tray menus, power locks (preventing sleep during active downloads), desktop notifications, and cryptographic in-app updates.
- **In-Memory IPC**: The client and core engine reside in the **same OS process**. All interactions occur via direct Rust function invocations, eliminating cross-process network serialization.

### 3. `xtask` (Security & Release Guard)
- **Responsibilities**: Generates Minisign Ed25519 signing keys, generates update manifests, and enforces that CI builds match embedded public keys before any release tag is published.

---

## Subsystems & Protocol Routing

```
               User Interactivity / Deep Links / RPC Commands
                                     │
                                     ▼
                      ┌────────────────────────────┐
                      │    Facade: Dispatcher      │
                      └──────────────┬─────────────┘
                                     │
                                     ▼
                      ┌────────────────────────────┐
                      │ BackendRegistry Router     │
                      └──────┬──────────────┬──────┘
                             │              │
       [TaskId with "http:"] │              │ [TaskId with "bt:"]
                             ▼              ▼
                ┌──────────────────┐  ┌──────────────────┐
                │ DownloadManager  │  │IrontideBtBackend │
                │ (HTTP/HTTPS AIMD)│  │ (BitTorrent P2P) │
                └────────┬─────────┘  └────────┬─────────┘
                         │                     │
                         └──────────┬──────────┘
                                    │
                                    ▼
                ┌────────────────────────────────────────┐
                │ Shared Infrastructure:                 │
                │ • BufferPool (SSD Combine / HDD Ping)  │
                │ • Database (SQLite WAL Atomic Store)   │
                │ • RateLimiter (Token Bucket Limiter)   │
                │ • SettingsService (Single Source Truth)│
                │ • EventBus (Tokio Broadcast Channel)   │
                └────────────────────────────────────────┘
```

### 1. Protocol Routing via BackendRegistry
limedl standardizes all operations under the `DownloadBackend` trait:
- Every task receives a globally unique `TaskId`.
- TaskIds are prefixed semantically: `http:<uuid>` routes to `DownloadManager`, while `bt:<infohash>` routes to `IrontideBtBackend`.
- The `Dispatcher` facade exposes uniform CRUD methods (`start`, `pause`, `resume`, `cancel`, `remove`), hiding protocol-specific mechanics from the UI.

### 2. Zero-Sized Type (ZST) Actors
To prevent memory cycles or deadlock hazards in complex asynchronous flows, `DownloadManager` delegates work to three ZST actors:
- **HttpExecutor**: Functional methods for metadata probing, range request generation, and byte stream reads.
- **Scheduler**: An autonomous background task executing every 2 seconds to evaluate AIMD throughput states and dynamically balance worker threads.
- **TaskLifecycle**: Coordinates file cleanup, progress calculation, and database transitions.

---

## Reactive EventBus Architecture

limedl replaces polling with a global `tokio::sync::broadcast` channel:

```
                      Engine State Transitions
                                 │
                                 ▼
        ┌─────────────────────────────────────────────┐
        │        EventBus::publish(DownloadEvent)     │
        └──────────────────────┬──────────────────────┘
                               │
            ┌──────────────────┴──────────────────┐
            ▼                                     ▼
┌───────────────────────────────┐ ┌───────────────────────────────┐
│ limedl-native Desktop Loop    │ │ Aria2RpcServer Relay          │
│ • Exhaustive match on events  │ │ • Filters GID modifications   │
│ • Computes view model diffs   │ │ • Dispatches WebSocket frames │
│ • slint::invoke_from_event_loop│ │   (e.g. onDownloadStart)      │
│ • Triggers microsecond repaint│ └───────────────────────────────┘
└───────────────────────────────┘
```

- **Compile-Time Exhaustive Handling**: The desktop event loop matches every `DownloadEvent` variant without catch-all wildcards (`_ => {}`), ensuring any newly added engine events must be explicitly handled by the UI.
- **Lock-Free UI Performance**: The UI thread processes small, pre-computed model deltas. Even when downloading at multi-gigabit speeds, the native GUI maintains a fluid 60+ FPS refresh rate.

---

## Minisign Cryptographic Self-Update Pipeline

To guard against supply-chain attacks and CDN tampering, limedl employs an end-to-end cryptographic verification pipeline:

```
1. GitHub Actions generates artifacts for Windows, macOS, and Linux
2. Minisign signs every artifact using a protected CI private key (.sig)
3. Manifest latest-native.json is assembled and published
                     │
                     ▼ Verified HTTPS Fetch
4. Client fetches latest-native.json
5. Verifies manifest signature against hardcoded PUBKEY_B64
6. Downloads matching platform binary
7. Validates SHA-256 hash and Minisign signature of the binary
8. Invokes self_replace to swap the executable in-place and relaunch
```

- **Hardcoded Root of Trust**: The desktop client embeds `PUBKEY_B64`. Unsigned or tampered update packages are rejected instantly.
- **Channel Specific Handlers**:
  - Windows Setup: Downloads `setup.exe` and spawns `/P /R` silent updater.
  - Windows Portable: Atomically swaps `limedl-native.exe` in-place.
  - macOS: Swaps the binary inside `limedl.app/Contents/MacOS/` and re-applies ad-hoc signing.
  - Linux: Informs users of package-managed updates or updates portable archives in-place.
