---
title: System Architecture
description: Deep dive into limedl single-track pure Rust design, protocol routing, EventBus broadcasting, and cryptographic self-update pipeline
---

# System Architecture

limedl is engineered around a **pure Rust 2024 single-track** philosophy. Over its architectural evolution, the project deliberately discarded hybrid shells (Electron / WebView) in favor of a lean, native, and memory-safe design that has zero external runtime overhead.

---

## Workspace Layout

limedl is a multi-crate Rust workspace. The engine is split by responsibility, and frontends go through the facade:

```
limedl/
├── crates/limedl-types/    # Domain types, settings, errors, events, manifest, DownloadBackend
├── crates/limedl-disk/     # file_ops, buffer_pool, io_scheduler
├── crates/limedl-store/    # SQLite task/chunk/manifest store, Alt-Svc cache
├── crates/limedl-http/     # HTTP client factory, identity encoding, rate limiter
├── crates/limedl-engine/   # HTTP executor, scheduler, CDN, Metalink, task lifecycle
├── crates/limedl-bt/       # librqbit BT backend (the only crate naming the engine)
├── crates/limedl-rpc/      # Aria2 JSON-RPC + WebSocket server (above the engine)
├── crates/limedl-testkit/  # Test-only HTTP harness
├── crates/limedl-core/     # Facade: Dispatcher, BackendRegistry, DownloadManager, bootstrap
├── crates/limedl-native/   # Slint desktop client (FemtoVG renderer)
├── crates/limedl-server/   # Headless daemon (NAS / routers / Docker)
└── xtask/                  # Build/release tooling: keys, dual-signing, gates
```

- `limedl-core` is a compatibility facade: it keeps `Dispatcher`, `DownloadManager`, `bootstrap`, and historical `pub use` paths, while HTTP execution, scheduling, CDN, and Metalink live in `limedl-engine` and BitTorrent lives in `limedl-bt`.
- `limedl-rpc` sits above the engine/facade; `limedl-core` does not depend on it. Each frontend decides whether to start RPC.
- Each frontend links the engine into its own process and calls it through Rust functions, with no cross-process serialization.

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
                │ DownloadManager  │  │  RqbitBtBackend  │
                │ (limedl-core)    │  │ (limedl-bt)      │
                └────────┬─────────┘  └────────┬─────────┘
                         │                     │
                         └──────────┬──────────┘
                                    │
                                    ▼
                ┌────────────────────────────────────────┐
                │ Shared infrastructure:                 │
                │ • limedl-disk  BufferPool / FileOps    │
                │ • limedl-store SQLite / Alt-Svc        │
                │ • limedl-http  Client / token bucket   │
                │ • limedl-types Settings / events       │
                └────────────────────────────────────────┘
```

### 1. Protocol Routing via BackendRegistry
limedl standardizes all operations under the `DownloadBackend` trait:
- Every task receives a globally unique `TaskId`.
- TaskIds are prefixed semantically: `http:<uuid>` routes to `DownloadManager`, while `bt:<infohash>` routes to `RqbitBtBackend`.
- The `Dispatcher` facade exposes uniform lifecycle methods (`start`, `pause`, `resume`, `cancel`, `remove`), hiding protocol-specific mechanics from the UI.

### 2. Three Sibling Actors
To prevent memory cycles or deadlock hazards in complex asynchronous flows, `DownloadManager` delegates execution to three sibling actors:
- **HttpExecutor**: a zero-sized type handling metadata probing, range requests, and byte-stream reads.
- **Scheduler**: an autonomous background task that runs every 2 seconds to evaluate AIMD throughput and rebalance worker threads.
- **TaskLifecycle**: coordinates state transitions, file cleanup, progress accounting, and database persistence.

Scheduler and TaskLifecycle own runtime state; they are not zero-sized types.

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

## Post-Quantum Hybrid-Signing Update Architecture (`sigilup`: Ed25519 + ML-DSA-65)

To protect against classical and quantum computing MITM threats and supply-chain tampering, limedl employs **`sigilup`**, an end-to-end **Ed25519 + Post-Quantum Cryptography (ML-DSA-65 / NIST FIPS 204)** DSSEv1 hybrid-signing pipeline:

```
1. GitHub Actions generates artifacts for Windows, macOS, and Linux
2. sigilup sign writes a .hsig DSSEv1 hybrid-signature envelope for each artifact
3. sigilup manifest builds latest.hsig.json (with BLAKE3 + SHA-512 digests and security_epoch)
                     │
                     ▼ Verified HTTPS Fetch
4. Client fetches latest.hsig.json
5. Verifies both Ed25519 and ML-DSA-65 signatures and security_epoch against sigilup-pins.json before parsing
6. Downloads matching platform binary
7. Validates BLAKE3 + SHA-512 digests in a single streaming pass
8. Invokes sigilup::replace to swap the executable in-place and relaunch
```

- **Anti-Rollback & Streaming Digests**: `latest.hsig.json` enforces `security_epoch` against rollback attacks and verifies BLAKE3 + SHA-512 digests in a single streaming pass.
- **Fail-Closed Release Gates (`sigilup guard`)**: The release pipeline verifies all signatures and the manifest against `sigilup-pins.json`, failing the release if anything drifts.
- **Channel Specific Handlers**:
  - Windows Setup: Downloads `setup.exe` and spawns `/P /R` silent updater.
  - Windows Portable: Atomically swaps `limedl-native.exe` in-place.
  - macOS: Swaps the binary inside `limedl.app/Contents/MacOS/` and re-applies ad-hoc signing.
  - Linux: Informs users of package-managed updates or updates portable archives in-place.
