---
title: Tasks & Concurrency
description: Deep dive into limedl task lifecycle, HTTP chunked parallel downloads, and the AIMD adaptive concurrency algorithm
---

# Tasks & Concurrency

Rather than blindly opening fixed thread counts (e.g., arbitrarily forcing 16 or 32 connections), limedl incorporates an **AIMD (Additive Increase Multiplicative Decrease) adaptive concurrency engine** inspired by modern network congestion control theories.

---

## Multiple Ways to Create Tasks

During daily desktop usage, you can initiate downloads via multiple seamless entry points:

1. **Smart Clipboard Monitoring**:
   Copying an HTTP/HTTPS URL or a `magnet:?xt=urn:btih:` link prompts limedl automatically when you switch to its window, parsing the target name and link type on the fly.
2. **Toolbar & Keyboard Shortcuts**:
   Click the **`+ New Task`** button on the top toolbar or press `Ctrl + N` (`Cmd + N` on macOS) to open the task creation dialog.
3. **Drag & Drop Torrents & Metalinks**:
   Drag any `.torrent`, `.metalink`, or `.meta4` descriptor file directly into the limedl window to inspect files or mirror pools.
4. **Browser Extension Interception**:
   Clicking download links in Chrome, Edge, or Firefox forwards the request directly to limedl via its Aria2 RPC integration.
5. **Command-Line Invocations**:
   Launch `limedl-native <URL|PATH>` from your shell; limedl's single-instance IPC forwards the payload to the running desktop client in milliseconds.
6. **Checksum Verification**:
   Optionally paste expected Blake3, SHA-256, or SHA-512 hashes in the new task dialog (the algorithm is automatically detected by hash length). Downloaded files are verified upon completion.

---

## The HTTP Chunked Engine Pipeline

Each HTTP download task undergoes a structured five-phase pipeline inside limedl:

```
┌─────────────────┐      HEAD / Range:0-0       ┌───────────────────┐
│ 1. Metadata Probe│ ──────────────────────────► │ Extract ETag/Size │
└────────┬────────┘                             └─────────┬─────────┘
         │                                                │
         ▼                                                ▼
┌─────────────────┐      Slicing by Size        ┌───────────────────┐
│ 2. Chunk Plan   │ ──────────────────────────► │ Save to SQLite DB │
└────────┬────────┘                             └─────────┬─────────┘
         │                                                │
         ▼                                                ▼
┌─────────────────┐      JoinSet Worker Pool    ┌───────────────────┐
│ 3. Parallel Pull│ ◄─────────────────────────► │ AIMD Adaptive Loop│
└────────┬────────┘                             └─────────┬─────────┘
         │                                                │
         ▼                                                ▼
┌─────────────────┐      BufferPool Flushing    ┌───────────────────┐
│ 4. Disk Buffer  │ ──────────────────────────► │ Write Combining   │
└────────┬────────┘                             └─────────┬─────────┘
         │                                                │
         ▼                                                ▼
┌─────────────────┐      Blake3/SHA-256 Digest  ┌───────────────────┐
│ 5. Verification │ ──────────────────────────► │ Atomic Rename     │
└─────────────────┘                             └───────────────────┘
```

### Phase 1: Metadata Probing
When a task is submitted, the engine issues a lightweight `HEAD` request (falling back to `GET Range: bytes=0-0` if HEAD is rejected by the server):
- Verifies resume support (`Accept-Ranges: bytes`);
- Reads the total file size (`Content-Length`);
- Captures entity tags (`ETag` and `Last-Modified`) to validate payload consistency across reconnections;
- Sanitizes destination filenames extracted from the `Content-Disposition` header or URL path.

### Phase 2: Dynamic Chunk Planning
If byte-range requests are supported and the resource exceeds the chunking threshold, limedl partitions the payload into discrete chunks:
- **Small Files**: Downloaded via a single stream with full resume capability.
- **Large Files**: Sliced into dynamic chunk sizes (typically between 2MB and 8MB) and tracked via a `ChunkManifest` stored in SQLite. State survives crashes or unexpected system reboots.

### Phase 3: AIMD Dynamic Concurrency
Fixed thread allocation creates dual problems: too many connections trigger IP bans or anti-scraping blocks, while too few underutilize high-bandwidth fiber connections.

limedl's **AIMD State Machine** handles concurrency dynamically:
- **Additive Increase**:
  Every 2 seconds, the scheduler evaluates chunk throughput and connection latency. If transmission is stable and error-free, worker connections ramp up incrementally (+1, +2...) to saturate downstream bandwidth.
- **Multiplicative Decrease**:
  Upon encountering HTTP 429 (Too Many Requests), HTTP 503, connection timeouts, or socket drops, the engine immediately **halves the active thread allocation (×0.5)** and enters a brief cooldown period to prevent blacklisting.
- **Mirror Failover**:
  When backup mirrors are configured, chunks stalling on one origin are automatically reassigned to alternate endpoints.

---

## Metalink Multi-Source Mirror Engine (RFC 5854 / RFC 6249)

When submitting `.metalink` or `.meta4` payloads via drag-and-drop or RPC, limedl engages its integrated **Metalink Smart Scheduler**:

- **Full Specification Coverage**: Parses Metalink 4.0 XML (RFC 5854), Metalink 3.0, and HTTP Header Metalink links (RFC 6249 / RFC 3230);
- **Multi-Factor Mirror Scoring**:
  Calculates dynamic mirror weights across several dimensions:
  - **Explicit Priority** and protocol security (HTTPS prioritized over HTTP);
  - **Geographical Proximity** based on region tags;
  - **Lightweight Latency Probing**: Fires millisecond HEAD/GET handshake probes across mirrors concurrently;
  - **Failure Penalties**: Penalizes endpoints that return 5xx errors or connection timeouts.
- **Dynamic Leasing (`MirrorPool` & `MirrorLease`)**:
  Uses an RAII `MirrorLease` to balance chunk requests across the fastest nodes while respecting per-server concurrency limits (`max-connection-per-server`), saturating bandwidth without overloading origins;
- **End-to-End Cryptographic Validation**:
  Extracts Blake3, SHA-256, and SHA-512 hashes from Metalink manifests and validates data upon completion, ensuring cross-mirror integrity.

---

## Scheduling Modes & Thread Budgets

Choose between two scheduling strategies under **Settings -> Downloads & Scheduler**:

### Automatic Mode (Recommended)
- **Core Principle**: Allocates thread budgets based on **Remaining Bytes** and task priorities.
- **Benefits**: Large files automatically receive larger worker pools to reduce overall waiting time, dynamically reallocating bandwidth as tasks complete.

### Traditional Mode
- **Core Principle**: Strict FIFO (First-In, First-Out) queuing.
- **Logic**: Enforces a strict ceiling of `max_parallel_tasks`. Subsequent tasks wait until preceding downloads finalize.

---

## Task Priority Hierarchy

Assign priorities to downloads via the context menu or list view:

| Priority | Weight Multiplier | Best For |
| :--- | :--- | :--- |
| **High** | 2.0x thread allocation preference | Urgent files, drivers, or system images needing immediate bandwidth. |
| **Normal** | 1.0x baseline weight | Default priority; competes equally for available workers. |
| **Low** | 0.5x thread allocation constraint | Background downloads that yield bandwidth to higher priority tasks. |

---

## High-Precision Speed Tracking (SpeedTracker)

limedl tracks transfer rates using a sliding-window time sampler (`SpeedTracker`):
- Eliminates visual jitter in the user interface, providing smooth, accurate real-time speeds.
- Computes weighted ETA (Estimated Time of Arrival) predictions that remain stable and reliable.

---

## Checksum Validation & Chunk-Level Self-Healing

Supported cryptographic and non-cryptographic hashing algorithms:
- **Blake3**: Cryptographic hash with extraordinary multi-core hashing throughput.
- **SHA-256**: Universal industry-standard digest.
- **SHA-512**: Stronger standard digest for sources that need collision and forgery resistance.

### Chunk-Level Self-Healing
With conventional downloaders, a checksum failure at the end of a 20GB download necessitates redownloading the entire 20GB file.

In limedl:
1. The engine tracks intermediate checksums for each downloaded chunk.
2. If the final digest fails, the validator pinpoints the **exact corrupted chunk**.
3. The engine **marks only the defective chunk for re-fetching**, repairing the entire multi-gigabyte file in seconds!
