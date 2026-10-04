---
type: workflow
title: HTTP Download Lifecycle
description: End-to-end orchestration of an HTTP download — start validation and slot acquisition, remote probing, chunk planning, single-stream vs chunked execution with retries and mirror failover, 429 downgrade, progress throttling, and checksum-verified finalization.
tags: [http, download, lifecycle, chunked, retry, checksum]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T01:52:48.489Z
sources:
  - id: openwiki-source-6e8cafe6d766137aab11d68d
    resource: repo://crates/limedl-core/src/download/shared.rs
  - id: openwiki-source-7e3442210263e22ce18a7bd5
    resource: repo://crates/limedl-core/src/http_executor/chunked.rs
  - id: openwiki-source-453d8ef1f0ed3f8f7b5ec966
    resource: repo://crates/limedl-core/src/http_executor/finalize.rs
  - id: openwiki-source-385628f66a6c216078934666
    resource: repo://crates/limedl-core/src/http_executor/mod.rs
  - id: openwiki-source-3b69c7e6871977d0c7442885
    resource: repo://crates/limedl-core/src/http_executor/run.rs
  - id: openwiki-source-056980230994a6266009d28f
    resource: repo://crates/limedl-core/src/http_executor/single.rs
  - id: openwiki-source-be8d7d8e3afa06fad7add3ab
    resource: repo://crates/limedl-core/src/http_executor/worker.rs
  - id: openwiki-source-0c4cd5f8953750852a1f4d6d
    resource: repo://crates/limedl-core/src/manager.rs
  - id: openwiki-source-9fa813eab6e27ac3f5fdbf1d
    resource: repo://crates/limedl-core/src/manifest.rs
  - id: openwiki-source-41f2f85d035ed1edf0c09b2e
    resource: repo://crates/limedl-core/src/persistence.rs
  - id: openwiki-source-098d28438aacd15b419786dc
    resource: repo://crates/limedl-core/src/task_lifecycle/mod.rs
generated: { by: "pi", at: "2026-10-04T01:52:48.489Z" }
---

# HTTP Download Lifecycle

An HTTP download flows from a frontend request through `DownloadManager` and the
`HttpExecutor` actor. The page follows the five phases: frontend → probe/plan →
execute → schedule → finalize.

## Phase 1 — Start validation and slot acquisition

`DownloadManager::start(request)`:

- parses the URL and requires an `http`/`https` scheme;
- acquires an HTTP concurrency slot (`try_acquire_http`) — the returned
  `DownloadSlotGuard` is held by the spawned task and releases the slot on drop;
- resolves the User-Agent from the request or settings;
- validates the destination directory: non-empty, absolute, and free of `..`
  traversal that escapes the parent;
- creates the destination directory and sanitizes the file name (rejecting an
  empty result).

Evidence: `repo://crates/limedl-core/src/manager.rs#L386-L455`.

`TaskLifecycle::spawn_download` persists the manifest and spawns a tokio task that
holds the slot guard and drives the run. It builds the URL list from
`mirror_urls` (or the single URL) starting at `current_mirror_index`, and with
mirrors present it uses **one retry per URL** and fails over only on network
errors (`is_connect`/`is_timeout`/`is_body`); non-network errors fail the task
immediately. For each attempt it resolves the client (CDN-aware), records
`cdn_accelerated`/`cdn_node_ip`, and calls `run_download`.

Evidence: `repo://crates/limedl-core/src/task_lifecycle/mod.rs#L85-L190`.

## Phase 2 — Probe and plan

`HttpExecutor::probe` issues a `HEAD`, falling back to `GET` with
`Range: bytes=0-0` when the HEAD is not successful. It returns `RemoteMetadata`:
final URL, file name, total bytes, range support, ETag and Last-Modified.

A `403 Forbidden` is disambiguated by sniffing a bounded body prefix:

- If it looks like a WAF / mirror anti-abuse page, the probe fails **terminally**
  with an actionable error and does **not** try candidate Referers — probing an
  already-suspicious client only adds more suspicious requests.
- Otherwise it is treated as anti-hotlink protection: `infer_candidate_referers`
  generates candidates, each is tried, and the first success is stored in the
  manifest's `extra_headers` so all chunk workers inherit it.

Evidence: `repo://crates/limedl-core/src/http_executor/run.rs#L8-L118`.

`run_download` then:

- checks free disk space for the remaining bytes;
- resolves `chunk_size` from the settings strategy and decides `supports_parallel`
  via `supports_parallelism`;
- resolves thread settings and writes the probe results into the manifest
  (`apply_probe_to_manifest`), which returns a `RunPlan`;
- auto-detects a SHA-256 from server/mirror checksum files when enabled;
- refreshes the AIMD state when the desired threads/profile changed, notifies the
  scheduler to rebalance, and prepares a fresh temp file / resets progress when
  required.

`apply_probe_to_manifest` resets the chunk plan when validators changed, total
bytes changed, range support changed, or a parallel plan had no chunks. A special
case: when the server no longer supports ranges but the manifest has partial chunk
progress, it clears progress and forces a single-stream restart.

Evidence: `repo://crates/limedl-core/src/http_executor/run.rs#L122-L205`,
`repo://crates/limedl-core/src/http_executor/run.rs#L262-L345`,
`repo://crates/limedl-core/src/manifest.rs#L98-L175`.

## Phase 3 — Execution

`run_download` dispatches to `download_chunked` when parallel is supported, else
`download_single`, and post-processes the outcome (`Finished` → finalize and emit
a summary; `Paused`/`Canceled` → bookkeeping and cleanup).

Evidence: `repo://crates/limedl-core/src/http_executor/run.rs#L198-L250`.

### Chunked (parallel)

`download_chunked` opens the temp file and builds the write buffer, then loops:

- on cancellation: shut workers down, flush the buffer, return Canceled;
- every 30 s: re-check disk space and fail the task when the remaining bytes no
  longer fit;
- when all chunks are complete: shut workers down, flush, return Finished;
- compute the target worker count (bounded by half the chunk count) and grow the
  worker pool;
- run the optional **Tail Sprint** (retry stalled tail chunks with fresh
  connections, then split the last unclaimed chunk);
- join one worker and fold its outcome into a `SupervisorStep`.

Workers claim chunks by an interleaved stripe (`chunk.index % worker_count`) with
an any-unclaimed fallback, and steal work by splitting the active chunk with the
largest remaining range when it is at least 2 MiB. Releasing a claim only clears
it if the claim still belongs to that worker.

Evidence: `repo://crates/limedl-core/src/http_executor/chunked.rs#L7-L130`,
`repo://crates/limedl-core/src/http_executor/worker.rs#L28-L135`.

### 429 single-thread downgrade

`fetch_segment` maps a `429 Too Many Requests` to `DowngradeSingleThread` when
more than one thread is allocated. The supervisor then shuts the workers down,
flushes the buffer, fixes the task to `ThreadMode::Fixed` with one requested,
desired and allocated thread, sets the note "单线程（429 限流降级）", persists,
notifies the scheduler, waits 1500 ms, and continues with one worker. Progress is
never lost.

`RestartSingle` (a range/validator failure) is different: it clears the buffer,
prepares a fresh temp file, resets progress, and restarts as a single stream.

Evidence: `repo://crates/limedl-core/src/http_executor/worker.rs#L254-L315`,
`repo://crates/limedl-core/src/http_executor/chunked.rs#L428-L486`.

### Single stream

`download_single` opens the temp file and buffer, then loops: wait/stop checks,
build an `If-Range` validator, resume from `contiguous_prefix_end(manifest)`,
issue the request through `request_with_retry`, and stream the body into
`write_chunk_bytes` while accounting the rate limiter (`BatchLimiter`) and
throttling persist/progress. It guards the received content length and flushes
the rate limiter and buffer on every exit path.

Evidence: `repo://crates/limedl-core/src/http_executor/single.rs#L9-L125`.

## Phase 4 — Scheduling and progress

The scheduler is documented separately; from the executor's point of view the
relevant interactions are `dm.scheduler.rebalance_allocations`, the
`rebalance_notify` wakeup, and the per-worker `allocated_thread_count` used to
decide the 429 downgrade.

Progress uses a shared `ProgressThrottle`: the manifest is persisted every
`PERSIST_INTERVAL` (300 ms) and a `Progress` event is emitted at most every
500 ms. Terminal states emit immediately. `BatchLimiter` consumes the global rate
limiter in 256 KiB / 8 chunk batches and flushes leftovers before exit.

Evidence: `repo://crates/limedl-core/src/http_executor/mod.rs#L217-L280`,
`repo://crates/limedl-core/src/download/shared.rs#L15-L20`.

## Phase 5 — Finalization

`finalize_download`:

1. bails out if already canceled, then sets the task to `Verifying` (connection
   and allocated threads zeroed) and persists.
2. Computes the checksum unless one is already present, then compares it against
   `expected_checksum` (case-insensitive). On mismatch it **renames the temp file
   to `{id}.part.corrupt`** so the evidence survives cleanup, marks the task
   `Failed`, persists and returns. The current finalizer does not attempt a
   targeted re-download of affected chunks.
3. On success, creates the destination parent, calls `finalize_temp_file`
   (atomic rename, cross-device copy fallback), sets `Completed`, marks every
   chunk complete and releases claims, persists, and publishes
   `aria2.onDownloadComplete`.

Evidence: `repo://crates/limedl-core/src/http_executor/finalize.rs#L7-L161`.

## Shared helpers that must not be duplicated

`http_executor/mod.rs` holds the helpers used by both the single and chunked
paths. Refactoring a path must reuse these rather than re-implement them:

| Helper | Responsibility |
| --- | --- |
| `build_write_buffer` | HDD shared double-buffer vs SSD/Network local ping-pong (half size clamped 64 KiB–8 MiB) |
| `flush_write_buffer` | flush on exit paths; failures are logged, never propagated |
| `wait_or_stop` | wait for active state and translate pause/cancel into `RunOutcome` |
| `finish_buffer_flush` | flush after completion while toggling the UI `flushing` flag |
| `check_disk_space_periodically` | 30 s space check; insufficient space → `Failed` + `Warning("disk full")` |
| `BatchLimiter` | 256 KiB / 8 chunk rate-limiter batching |
| `ProgressThrottle` | `PERSIST_INTERVAL` persistence + 500 ms progress events |

Evidence: `repo://crates/limedl-core/src/http_executor/mod.rs#L71-L280`.

## Crash recovery

On restart, `load_downloads_from_db` reconstructs tasks and clears every chunk's
`claimed_by` (a fresh manager owns no workers, so persisted claims are stale).
Tasks left `Downloading` stay `Downloading` with connection/allocated counts
zeroed so the scheduler re-allocates, and resume continues from the persisted
chunk state. See
<!-- openwiki: broken internal link [/openwiki/systems/persistence-and-recovery.md] link "/openwiki/systems/persistence-and-recovery.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[SQLite Persistence and Crash Recovery](/openwiki/systems/persistence-and-recovery.md).

Evidence: `repo://crates/limedl-core/src/persistence.rs#L30-L95`.

<!-- openwiki: broken internal link [/openwiki/workflows/scheduler-and-concurrency.md] link "/openwiki/workflows/scheduler-and-concurrency.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Related pages: [Scheduler, AIMD and Concurrency Control](/openwiki/workflows/scheduler-and-concurrency.md),
<!-- openwiki: broken internal link [/openwiki/systems/disk-io-and-storage.md] link "/openwiki/systems/disk-io-and-storage.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Disk I/O, Buffer Pool and Storage Detection](/openwiki/systems/disk-io-and-storage.md),
<!-- openwiki: broken internal link [/openwiki/systems/networking-and-rate-control.md] link "/openwiki/systems/networking-and-rate-control.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Networking, HTTP Clients and Rate Control](/openwiki/systems/networking-and-rate-control.md),
<!-- openwiki: broken internal link [/openwiki/systems/persistence-and-recovery.md] link "/openwiki/systems/persistence-and-recovery.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[SQLite Persistence and Crash Recovery](/openwiki/systems/persistence-and-recovery.md).
