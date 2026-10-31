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
7. **Paste an aria2c Command**:
   The new-task dialog and clipboard monitor understand common `aria2c` flags (`-x/--split`, `--header`, `--referer`, `--checksum`, `-d/--dir`, `-o/--out`, and more), turning an existing command line into a task.

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
  Upon encountering HTTP 429 (Too Many Requests), HTTP 503, connection timeouts, or socket drops, the engine applies the configured **multiplicative decrease** (×0.5 for Balanced/Aggressive, ×0.7 for Conservative) and enters a cooldown period to prevent blacklisting.
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
  - **Failure Penalties**: Penalizes endpoints that return 5xx errors or connection timeouts.

  > The current scoring uses these static properties plus error history. `MirrorPool` carries an RTT field, but the download path does not perform active RTT probing yet.
- **Dynamic Leasing (`MirrorPool` & `MirrorLease`)**:
  Uses an RAII `MirrorLease` to balance chunk requests across the fastest nodes while respecting per-server concurrency limits (`max-connection-per-server`), saturating bandwidth without overloading origins;
- **End-to-End Cryptographic Validation**:
  Extracts Blake3, SHA-256, and SHA-512 hashes from Metalink manifests and validates data upon completion, ensuring cross-mirror integrity.

---

## Scheduling Modes & Thread Budgets

Choose between two scheduling strategies under **Settings -> Downloads & Scheduler**:

### Automatic Mode (Recommended)
- **Core Principle**: Orders tasks by **priority and total file size** (static keys that do not change with progress). A running task keeps its thread allocation (sticky); higher-priority tasks preempt immediately.
- **Benefits**: Large files get larger worker pools to reduce overall waiting time. When the budget cannot cover every task, the longest-waiting task is rotated in on a fixed cadence (~30 s), which avoids starving the tail without thrashing the active set.

### Traditional Mode
- **Core Principle**: Priority first, then creation time (FIFO within the same priority).
- **Logic**: Enforces a ceiling of `max_parallel_tasks`; higher-priority tasks may overtake lower-priority ones.

---

## Task Priority Hierarchy

Assign priorities to downloads via the context menu or list view:

| Priority | Scheduling behavior | Best For |
| :--- | :--- | :--- |
| **High** | Ordered first; may preempt lower-priority tasks when the budget is tight | Urgent files, drivers, or system images. |
| **Normal** | Default order; ties broken by size and creation time | Everyday downloads. |
| **Low** | Ordered last and is the first preemption victim | Background downloads. |

Priority does not allocate threads by a fixed multiplier: it only drives ordering and preemption, while the actual thread count comes from the scheduler mode and AIMD.

---

## High-Precision Speed Tracking (SpeedTracker)

limedl tracks transfer rates using a sliding-window time sampler (`SpeedTracker`):
- Eliminates visual jitter in the user interface, providing smooth, accurate real-time speeds.
- Computes weighted ETA (Estimated Time of Arrival) predictions that remain stable and reliable.

---

## Checksums & Integrity Verification

limedl supports Blake3, SHA-256, and SHA-512. Checksums can come from:

- **Manual input** when creating a task (the algorithm is inferred from the hash length);
- **HTTP response headers** (RFC 3230/9530, S3, GCS);
- **Companion files** such as `.sha256` and `SHA256SUMS` (auto-detection can be disabled in Settings);
- **Metalink piece hashes**, verified per chunk during a chunked download; a corrupt piece resets that chunk and fails the task (resume/retry re-downloads it);
- **Whole-file verification** after the download, using the configured or task-provided algorithm.

### What happens when whole-file verification fails

limedl does **not** infer and re-download only the "bad chunks". Current behavior:

1. The task is marked **Failed**, and the error reports the expected and computed values;
2. The temporary file is renamed to `*.part.corrupt` and preserved for diagnosis; it is not deleted automatically;
3. The task context menu offers **Verify integrity**, which recomputes the hash for a completed file (or an existing `.part` / `.part.corrupt` file) with progress reporting.

Metalink piece verification is independent: a corrupted piece is detected during the download, its chunk is reset, and the task fails; resuming/retrying re-downloads that chunk instead of waiting for whole-file verification.
