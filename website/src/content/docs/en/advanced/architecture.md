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
├── crates/limedl-native/   # Slint-based native desktop client (FemtoVG hardware backend)
├── crates/limedl-server/   # Headless daemon (Aria2 JSON-RPC server for NAS / routers / Docker)
└── xtask/                  # Repository maintenance tooling (PQC dual-signing & release gates)
```

### 1. `limedl-core` (The Download Engine)
- **Responsibilities**: Task orchestration, AIMD concurrency scheduling, HTTP range probing and chunking, BitTorrent swarms (irontide), Metalink 4.0/3.0 mirror selection, Cloudflare CDN probing, hardware-aware buffer pools, SQLite persistence, and the Aria2 JSON-RPC 2.0 server.
- **Characteristics**: Built purely on Tokio's asynchronous runtime with zero UI dependencies.

### 2. `limedl-native` (Native Slint Client)
- **Responsibilities**: Native declarative UI built with **Slint** on top of FemtoVG rendering. Handles window geometry persistence, single-instance mutexes, system tray menus, power locks (preventing sleep during active downloads), desktop notifications, and cryptographic in-app updates.
- **In-Memory IPC**: The client and core engine reside in the **same OS process**. All interactions occur via direct Rust function invocations, eliminating cross-process network serialization.

### 3. `limedl-server` (Headless Server Daemon)
- **Responsibilities**: Lightweight daemon for NAS appliances, soft routers, Linux servers, and Docker containers. Shares the same underlying download engine and exposes compliant Aria2 JSON-RPC 2.0 with per-client token authentication.

### 4. `xtask` (Security & Release Guard)
- **Responsibilities**: Generates Minisign Ed25519 and ML-DSA-65 keypairs, signs release artifacts, and verifies that CI builds strictly match client-embedded public keys prior to publishing.

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

## Post-Quantum Hybrid Dual-Signing Update Architecture (Minisign + ML-DSA-65)

To protect against classical and quantum computing MITM threats and supply-chain tampering, limedl employs an end-to-end **Ed25519 (Minisign) + Post-Quantum Cryptography (ML-DSA-65 / NIST FIPS 204)** hybrid dual-signing pipeline:

```
1. GitHub Actions generates artifacts for Windows, macOS, and Linux
2. xtask generates dual signatures: .sig (Minisign) and .pqc.sig (ML-DSA-65)
3. Manifest latest-native.json is assembled and dual-signed
                     │
                     ▼ Verified HTTPS Fetch
4. Client fetches latest-native.json and both signature files
5. Verifies both Ed25519 and ML-DSA-65 signatures against embedded public keys before JSON parsing
6. Downloads matching platform binary
7. Validates SHA-256 hash and independent dual signatures of the downloaded binary
8. Invokes self_replace to swap the executable in-place and relaunch
```

- **Domain Separation Contexts**: ML-DSA-65 signatures enforce strict domain separation (`limedl-manifest` for manifests, `limedl-artifact` for binary payloads), preventing cross-context signature replay attacks.
- **Fail-Closed Release Gates (`cargo xtask guard`)**: The release pipeline derives public keys from CI secrets and strictly compares them against embedded client constants (`PUBKEY_B64`, `PQC_PUBKEY_B64`), failing the release if anything drifts.
- **Channel Specific Handlers**:
  - Windows Setup: Downloads `setup.exe` and spawns `/P /R` silent updater.
  - Windows Portable: Atomically swaps `limedl-native.exe` in-place.
  - macOS: Swaps the binary inside `limedl.app/Contents/MacOS/` and re-applies ad-hoc signing.
  - Linux: Informs users of package-managed updates or updates portable archives in-place.
