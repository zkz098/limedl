---
id: workflows/http-download-lifecycle
title: HTTP Download Lifecycle
status: verified
generated_at: 2026-11-13T06:42:18Z
commit: c28b25a7355ded0aa81f07dfee7c66c53537eaf3
evidence:
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 395
  - 462
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: 94af1699d0caab982e477e8395a83b8a5869d7d4
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 99
  - 150
  blob: 06fa226e016db7406aab96ecc589b08052421836
  snippet_sha1: e95f9e8ab3e64f09b122c34e51baf3346b95ade6
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 564
  - 569
  blob: 06fa226e016db7406aab96ecc589b08052421836
  snippet_sha1: 36bfeae98ec8324d605aca144b5dc1b720c6d466
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 162
  - 186
  blob: 06fa226e016db7406aab96ecc589b08052421836
  snippet_sha1: f3b7ac1847c7158355eb0b44f57818d13264fee8
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 130
  - 150
  blob: 06fa226e016db7406aab96ecc589b08052421836
  snippet_sha1: 37a8d1921fde0eff8a736144fe84fadccf73c6e6
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 29
  - 56
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: ae7c0ece89332bacb7f3fdf092af2ffd5093c0f8
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 60
  - 118
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: 4c1a880e729fe1f793ae78ca6bfbff95ea3a3b54
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 171
  - 215
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: d3a10d0ccd1b56a3874748ac492e1e3aa0254684
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 217
  - 238
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: 80d53c1d626f24289cead767fbf1f5ce0ed3810a
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 240
  - 251
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: eb87adedf6fee2f0945df0e991c552e7ffeb6bfd
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 331
  - 413
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: 80d933fa81734d72dc42c1c362dba7019c98bbe2
- path: crates/limedl-types/src/manifest.rs
  lines:
  - 138
  - 199
  blob: 6b6398443471f60ea175d08c3f3feb508d636146
  snippet_sha1: 198f76a4aa0d0f77daaee178f3d4b3ee2bcfa356
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 253
  - 311
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: 067ff9873306bb46dadb129cedfad6319407e8f7
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 97
  - 213
  blob: 4080a38ecd8d1c475cc747f753e65c58658ad9ce
  snippet_sha1: 510340b421d181218f60292fcc7623b32ced01e8
- path: crates/limedl-engine/src/http_executor/chunk_steal_sync.rs
  lines:
  - 12
  - 116
  blob: 737a940b372ad66b1fbcf9d2397cc75a20005c75
  snippet_sha1: 0fb862be3dd71f1bb5979338f407890c8f545915
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 46
  - 47
  blob: 440aa6e7e0e75317a748ed5618b9c2869db2bb86
  snippet_sha1: 0e3d9e3ad29e1cfc39aa2a541724903f58104fc8
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 177
  - 190
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: c1b51553be886916025efc5d12ba857cf2eff260
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 939
  - 951
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: 07fb85349fc1bbb019e618b242f2586dd6134126
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 326
  - 438
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: e444d5d5601ffa163b19ad6a99f73024d9e41fe6
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 274
  - 307
  blob: 440aa6e7e0e75317a748ed5618b9c2869db2bb86
  snippet_sha1: 2f8ee01c59e92b67a27fc01b772fd7fa1e6bf227
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 488
  - 542
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: d2e04a1c4479ab586429ef4138f5b88efdfd2dd9
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 677
  - 698
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: 50eb3b5c57a24b871549fd52d3250e384b162f47
- path: crates/limedl-engine/src/download/shared.rs
  lines:
  - 67
  - 69
  blob: 4fd41e05b1cb50e99f9aad5e048f9a31ab2fba7f
  snippet_sha1: cb28756d78f9173307b8fa1c3a5964156fa60579
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 549
  - 579
  blob: 4080a38ecd8d1c475cc747f753e65c58658ad9ce
  snippet_sha1: 91ef45efa3fa2cd0130749914aa2bd8cf33d5199
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 580
  - 589
  blob: 4080a38ecd8d1c475cc747f753e65c58658ad9ce
  snippet_sha1: 5cf8ca8d7d6bd6def78cb3edc8a5e34f2b70e01a
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 199
  - 208
  blob: 4080a38ecd8d1c475cc747f753e65c58658ad9ce
  snippet_sha1: 7c22cfead2820e1da53a3571ca3fe3ac0d0f788d
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 38
  - 41
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: 9697cb211af8cd53616e2ab7de1d62773e972fa2
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 45
  - 93
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: 9c6f999d89679f7693a7a1c3923b2458667cce90
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 141
  - 219
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: 1ede538ccbd0c7d1f59bebf955a7ea52c6a46df8
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 29
  - 35
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: 386a4528311fa8ec28afd91a5ab00b27039c91ea
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 156
  - 160
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: a02165d043433a90490cb2f7ae69a05cca200312
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 372
  - 406
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: 2d0b85e2a3c9157d16d701602aeb59ad9833c8fd
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 684
  - 694
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: aba676a05915e5d9bbd13b0565f513118d217886
- path: crates/limedl-engine/src/download/shared.rs
  lines:
  - 16
  - 22
  blob: 4fd41e05b1cb50e99f9aad5e048f9a31ab2fba7f
  snippet_sha1: 2667e546d111bef59ff5700b9db173370b656a7c
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 309
  - 343
  blob: 440aa6e7e0e75317a748ed5618b9c2869db2bb86
  snippet_sha1: 03533620cbb807e2fced5e9ed5c4b298909441d6
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 283
  - 311
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: dd0376bef6ca03355d558cdee0831d3315c921af
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 236
  - 272
  blob: 440aa6e7e0e75317a748ed5618b9c2869db2bb86
  snippet_sha1: 48c41fca55b0567f38e3e7bf502a18d2040cfc4c
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 155
  - 220
  blob: 4adcb7ff251dfe41230bee5d56018e575c67bbe9
  snippet_sha1: b25d94ff5544df90b0d774fdf49fbeac4f8e7d29
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 84
  - 90
  blob: 440aa6e7e0e75317a748ed5618b9c2869db2bb86
  snippet_sha1: 31fd23c9f1e2c5ebf2f57fd817a906bf6b443c80
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 875
  - 890
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: 5aebd4983a75e1dc117b01906dc5f93175c3901c
- path: crates/limedl-store/src/database/chunk_repo.rs
  lines:
  - 27
  - 46
  blob: 0bd58c6ca0db07244f40d8a66471f0c56c5ae265
  snippet_sha1: 4262eae4f5d28c5348dc6493aebcac40c5eff851
- path: crates/limedl-types/src/manifest.rs
  lines:
  - 73
  - 124
  blob: 6b6398443471f60ea175d08c3f3feb508d636146
  snippet_sha1: 0c10c040314faae744ea92d84a5c0471b50d406e
- path: crates/limedl-engine/src/persistence.rs
  lines:
  - 32
  - 130
  blob: dac6df18b1fbb4ac265f3454ec8096905dbe3057
  snippet_sha1: 4157be261610ab7a86064de05df1d0a92c5edde4
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 1196
  - 1212
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: a973407a0e96b8aea4a8240f327f351d143c34c9
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 184
  - 195
  blob: 4adcb7ff251dfe41230bee5d56018e575c67bbe9
  snippet_sha1: 12bf033d1af5a0203ffc6b763adf4f555cfb0af1
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 170
  - 181
  blob: 4adcb7ff251dfe41230bee5d56018e575c67bbe9
  snippet_sha1: 6d4ce9364aeabb0d8c5b540a328536eee573bcaa
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 192
  - 195
  blob: 4adcb7ff251dfe41230bee5d56018e575c67bbe9
  snippet_sha1: 5a1eaeda4a425d14d346625de0fea3aa4b500440
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 17
  - 21
  blob: acc4494d3d2689ddbda4f66c37515366282fa5d3
  snippet_sha1: 0905ae1cf2077c5d0eb301d67812ecf56b8c9fab
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 24
  - 228
  blob: acc4494d3d2689ddbda4f66c37515366282fa5d3
  snippet_sha1: 1790465180a09dff22199fa2d4ce950ebc9da192
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 119
  - 155
  blob: acc4494d3d2689ddbda4f66c37515366282fa5d3
  snippet_sha1: fe1cb46fdef6c987e857c230e2b4d513bfef9fb4
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 177
  - 227
  blob: acc4494d3d2689ddbda4f66c37515366282fa5d3
  snippet_sha1: 2ab129bb2cb6456544f78a6d2d7bc588a8afd9c0
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 231
  - 256
  blob: acc4494d3d2689ddbda4f66c37515366282fa5d3
  snippet_sha1: f5695ca0e3c692765722167d709886b4b4be7a62
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 74
  - 343
  blob: 440aa6e7e0e75317a748ed5618b9c2869db2bb86
  snippet_sha1: 95b6e60a4c94c9f6407fbbd6a7a588536680ef50
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 1115
  - 1190
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: 7cf68365ce46f5a603186d8dcd9129254f0220bf
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 1150
  - 1158
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: 2fa80cae4d897e960d859cb0aa92ca6634f6acda
type: workflow
---
# HTTP Download Lifecycle

An HTTP download flows from a frontend request through `DownloadManager` and the
`HttpExecutor` actor. Since the engine split, the manager facade lives in
`limedl-core` while the executor, lifecycle, persistence and manifest planning
live in `limedl-engine`, with chunk/manifest data types in `limedl-types` and
the SQLite chunk repository in `limedl-store`. The page follows the five phases:
frontend → probe/plan → execute → schedule → finalize.

## Phase 1 — Start validation

`DownloadManager::start` in the `limedl-core` facade
[[cite:crates/limedl-engine/src/manager.rs:L395-L462]]:

- parses the URL and requires an `http`/`https` scheme;
- resolves the User-Agent from the request or settings;
- validates the destination directory: non-empty, absolute, and free of `..`
  traversal that escapes the parent;
- creates the destination directory and sanitizes the file name (rejecting an
  empty result).

`TaskLifecycle::spawn_download` persists the manifest and spawns a tokio task
that drives the run. It builds the URL list from `mirror_urls` (or the single
URL) starting at `current_mirror_index`, and with mirrors present it uses
**one retry per URL**
[[cite:crates/limedl-engine/src/task_lifecycle/mod.rs:L99-L150]]; failover to the
next mirror happens only on network errors — `is_network_error` classifies
`is_connect`/`is_timeout`/`is_body`/`is_decode`
[[cite:crates/limedl-engine/src/task_lifecycle/mod.rs:L564-L569]] — while any
other error fails the task immediately
[[cite:crates/limedl-engine/src/task_lifecycle/mod.rs:L162-L186]]. For each
attempt it resolves the client (CDN-aware), records `cdn_accelerated` /
`cdn_node_ip`, and calls `run_download`
[[cite:crates/limedl-engine/src/task_lifecycle/mod.rs:L130-L150]].

## Phase 2 — Probe and plan

`HttpExecutor::probe` issues a `HEAD`, falling back to `GET` with
`Range: bytes=0-0` when the HEAD is not successful. It returns `RemoteMetadata`:
final URL, file name, total bytes, range support, ETag and Last-Modified. The
probe (and the anti-hotlink candidates) force `Accept-Encoding: identity`, so
the reported length and range support describe the identity representation even
when the client has response decompression enabled
[[cite:crates/limedl-engine/src/http_executor/run.rs:L29-L56]].

A `403 Forbidden` is disambiguated by sniffing a bounded body prefix
[[cite:crates/limedl-engine/src/http_executor/run.rs:L60-L118]]:

- If it looks like a WAF / mirror anti-abuse page, the probe fails **terminally**
  with an actionable error and does **not** try candidate Referers — probing an
  already-suspicious client only adds more suspicious requests.
- Otherwise it is treated as anti-hotlink protection: `infer_candidate_referers`
  generates candidates, each is tried, and the first success is stored in the
  manifest's `extra_headers` so all chunk workers inherit it.

`run_download` then checks free disk space for the remaining bytes, resolves
`chunk_size` from the settings strategy (Metalink piece length and expert-mode
custom size take precedence) and decides `supports_parallel` via
`supports_parallelism`; it resolves thread settings and writes the probe results
into the manifest (`apply_probe_to_manifest`), which returns a `RunPlan`
[[cite:crates/limedl-engine/src/http_executor/run.rs:L171-L215]]. Checksum
auto-detection runs when enabled: response headers (RFC 3230 / RFC 9530 / S3 /
GCS) have priority, otherwise external `.sha256` / `SHA256SUMS` candidates are
probed
[[cite:crates/limedl-engine/src/http_executor/run.rs:L217-L238]]. Finally the
AIMD state is refreshed when the desired threads/profile changed, the scheduler
is notified to rebalance, and a fresh temp file / progress reset is prepared
when the plan requires it
[[cite:crates/limedl-engine/src/http_executor/run.rs:L240-L251]].

`apply_probe_to_manifest` resets the chunk plan when validators changed, total
bytes changed, range support changed, or a parallel plan had no chunks. A
special case: when the server no longer supports ranges but the manifest has
partial chunk progress, it clears progress and forces a single-stream restart
[[cite:crates/limedl-engine/src/http_executor/run.rs:L331-L413]]. The chunk
plan itself (`plan_chunks`, `resolve_chunk_size`) is computed in the
`limedl-types` crate [[cite:crates/limedl-types/src/manifest.rs:L138-L199]].

## Phase 3 — Execution

`run_download` dispatches to `download_chunked` when parallel is supported, else
`download_single`, and `post_process_run` folds the outcome (`Finished` →
finalize and emit a summary; `Paused`/`Canceled` → bookkeeping and cleanup)
[[cite:crates/limedl-engine/src/http_executor/run.rs:L253-L311]].

### Chunked (parallel)

The `download_chunked` supervisor loop opens the temp file, builds the write
buffer and mirrors pool, then loops: on cancellation it shuts workers down,
flushes the buffer and returns Canceled; it re-checks disk space every 30 s via
`check_disk_space_periodically`; when all chunks are complete it shuts workers
down, flushes and returns Finished; it computes the target worker count (the
scheduler's current allocation, capped at the chunk count), honors a full
demotion to zero by releasing claims and flushing, runs the optional **Tail
Sprint** (fresh-connection retry for stalled tail chunks, then splitting the
last unclaimed chunk), grows the worker pool and joins one worker, folding its
outcome into a `SupervisorStep`
[[cite:crates/limedl-engine/src/http_executor/chunked.rs:L97-L213]].

Workers claim chunks by an interleaved stripe (`chunk.index % worker_count`)
with an any-unclaimed fallback, and steal work by splitting the active chunk
with the largest remaining range at or above the minimum split size
[[cite:crates/limedl-engine/src/http_executor/chunk_steal_sync.rs:L12-L116]]. The default
work-steal floor is 2 MiB (`WORK_STEAL_MIN_SPLIT_SIZE`)
[[cite:crates/limedl-engine/src/http_executor/mod.rs:L46-L47]]. Releasing a
claim only clears it if the claim still belongs to that worker
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L177-L190]].
`mark_complete` only advances the *received* counters; the durable counter for
those bytes catches up when the write buffer reports the flush that contains
them (see [Durable vs received progress](#durable-vs-received-progress))
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L939-L951]].

Each worker also carries a `RequestBudget` of
`MAX_SEGMENT_FETCHES_PER_CHUNK = 24` segment requests, charged once per
request, so a server that keeps ending the body early fails the download
instead of driving the worker's re-fetch loop forever
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L326-L438]]. The budget
mechanism and its livelock rationale are documented on the shared type in
`limedl-engine` [[cite:crates/limedl-engine/src/http_executor/mod.rs:L274-L307]].

A body that breaks *mid-segment* is not fatal either. `consume_segment` returns
`SegmentBody::StreamInterrupted(err)` instead of propagating the error; the
worker records a retry penalty, notes the interruption, backs off with jitter
and re-requests the rest of the segment from its current offset, which becomes
a `Range` request; the chunk's `Retrying` marker is flipped back to
`Downloading` when the next response arrives. The consecutive-error streak is
reset on forward progress, and only bodies that keep failing without progress
past `max_retries` are propagated as a failure
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L488-L542]].

### 429 single-thread downgrade

`fetch_segment` maps a `429 Too Many Requests` to `DowngradeSingleThread` when
more than one thread is allocated
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L677-L698]]. The
supervisor then shuts the workers down, flushes the buffer, fixes the task to
`ThreadMode::Fixed` with one requested, desired and allocated thread, sets the
note `RATE_LIMIT_DEGRADED` — "单线程（429 限流降级）"
[[cite:crates/limedl-engine/src/download/shared.rs:L67-L69]] — persists,
notifies the scheduler, waits 1500 ms, and continues with one worker. Progress
is never lost [[cite:crates/limedl-engine/src/http_executor/chunked.rs:L549-L579]].

`RestartSingle` (a plain `200 OK` where a range was expected) is different: it
shuts the workers down, clears the buffer so the Drop canary does not misfire,
prepares a fresh temp file, resets progress, and restarts as a single stream
[[cite:crates/limedl-engine/src/http_executor/chunked.rs:L580-L589]],
[[cite:crates/limedl-engine/src/http_executor/chunked.rs:L199-L208]].

### Single stream

`download_single` opens the temp file and buffer, then loops through
`SingleRun::attempt`: wait/stop checks, read the `If-Range` validator, resume
from `contiguous_prefix_end(manifest)` — the durable contiguous prefix — issue
the request through `request_with_retry` (with an opportunistic HTTP/3 attempt
from the Alt-Svc cache), stream the body into `write_chunk_bytes` while
accounting the rate limiter (`BatchLimiter`) and throttling persist/progress,
and guard the received content length
[[cite:crates/limedl-engine/src/http_executor/single.rs:L38-L41]]
[[cite:crates/limedl-engine/src/http_executor/single.rs:L45-L93]],
[[cite:crates/limedl-engine/src/http_executor/single.rs:L141-L219]].

The loop is bounded: `RequestBudget::new(MAX_SINGLE_STREAM_REQUESTS = 64)` is
charged once per response. A server that always ends the body early would
therefore spin here forever (the offset would never move, and each request gets
a fresh `max_retries`), and the budget turns that into a reported failure
instead [[cite:crates/limedl-engine/src/http_executor/single.rs:L29-L35]],
[[cite:crates/limedl-engine/src/http_executor/single.rs:L156-L160]]. The
chunked worker has the same mechanism per chunk
(`MAX_SEGMENT_FETCHES_PER_CHUNK = 24`) — see
[[wiki:systems/networking-and-rate-control]].

A body that breaks mid-transfer is recovered in place rather than failing the
task. When the stream yields an error, `handle_stream_error` logs the offset it
reached, records a retry penalty, sleeps a jittered backoff and re-issues the
request — which resumes from the **durable** offset through the same
`If-Range`/`Range` path, so bytes already written are never re-fetched. The
`consecutive_stream_errors` streak restarts whenever the offset advanced, so a
long download that keeps making progress can absorb many interruptions; only a
body that keeps breaking without progress past `max_retries` propagates the
error and fails the task. Cancelling during the backoff returns the normal
cancel outcome
[[cite:crates/limedl-engine/src/http_executor/single.rs:L372-L406]].

## Phase 4 — Scheduling and progress

The scheduler is documented separately; from the executor's point of view the
relevant interactions are `rebalance_allocations` plus the
`rebalance_notify` wakeup after the plan is applied
[[cite:crates/limedl-engine/src/http_executor/run.rs:L240-L251]], and the
per-task `allocated_thread_count` used to decide the 429 downgrade
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L684-L694]].

Progress uses a shared `ProgressThrottle`: the manifest is persisted every
`PERSIST_INTERVAL` (300 ms)
[[cite:crates/limedl-engine/src/download/shared.rs:L16-L22]] and a `Progress`
event is emitted at most every 500 ms
[[cite:crates/limedl-engine/src/http_executor/mod.rs:L309-L343]]. Terminal
transitions call `emit_progress` / `emit_single_summary` directly, outside the
throttle [[cite:crates/limedl-engine/src/http_executor/run.rs:L283-L311]].
`BatchLimiter` consumes the global rate limiter in 256 KiB / 8 chunk batches
and flushes leftovers before exit
[[cite:crates/limedl-engine/src/http_executor/mod.rs:L236-L272]].

`Progress` events and the persisted row deliberately read *different* counters:
the event carries the received progress (what the user sees), while the row
stores the durable progress (what is on the file). See below.

## Durable vs received progress

Every chunk carries two counters, and keeping them apart is what makes a crash
recoverable [[cite:crates/limedl-engine/src/download/managed.rs:L155-L220]]:

- `ChunkManifest::downloaded` / `::completed` are the **received** counters,
  advanced by `record_progress_on_managed` as bytes arrive. The scheduler, the
  claim/unclaim logic, `all_chunks_completed`, the tail-sprint state and the UI
  snapshot all follow these — they describe what the transfer has achieved.
- `ChunkManifest::durable_downloaded` and `DownloadCore::durable_bytes` are the
  **durable** counters, advanced by `record_durable_bytes` and nothing else. The
  write buffer calls it with the `(offset, len)` ranges a flush put on the file
  [[cite:crates/limedl-engine/src/http_executor/mod.rs:L84-L90]], and the
  direct-write paths call it right after `write_all_at` returns
  [[cite:crates/limedl-engine/src/http_executor/worker.rs:L875-L890]].

What the database stores follows from that split: `chunk_to_params` in
`limedl-store` writes `downloaded = chunk.durable_bytes()` and
`completed = durable >= chunk.byte_len()`, and `chunk.durable_bytes()` — defined
in `limedl-types` — clamps to the received count, so a stale counter can only
under-report (which costs a re-download, never a hole)
[[cite:crates/limedl-store/src/database/chunk_repo.rs:L27-L46]],
[[cite:crates/limedl-types/src/manifest.rs:L73-L124]]. The three writers —
`persist_manifest_snapshot`, `persist_manifest_snapshots_batch`
[[cite:crates/limedl-engine/src/persistence.rs:L32-L130]] and
`DownloadManager::persist`
[[cite:crates/limedl-engine/src/manager.rs:L1196-L1212]] — all substitute
`core.durable_bytes` for the task-level counter.

Why it matters: before this, the 300 ms cycle stored the *received* counters,
which run ahead of the file by whatever the buffer still held. A hard kill then
left the database claiming a chunk was complete while its bytes were only in
memory, and the next resume skipped over the resulting hole
[[cite:crates/limedl-engine/src/download/managed.rs:L184-L195]].

Two consequences worth remembering when touching this area:

- `record_progress_on_managed` must not set `chunk.dirty`. The dirty flag is what
  schedules a row write, and a row may only record durable bytes; the observer
  sets it instead [[cite:crates/limedl-engine/src/download/managed.rs:L170-L181]].
- The durability boundary is `pwrite`, not `fsync`: a successful flush counts,
  which covers a process crash/`SIGKILL`/OOM kill but not power loss (where the
  page cache and the WAL commits are lost together anyway)
  [[cite:crates/limedl-engine/src/download/managed.rs:L192-L195]].

## Phase 5 — Finalization

`finalize_download`
[[cite:crates/limedl-engine/src/http_executor/finalize.rs:L17-L21]]
[[cite:crates/limedl-engine/src/http_executor/finalize.rs:L24-L228]]:

1. bails out if already canceled, then sets the task to `Verifying` (connection
   and allocated threads zeroed) and persists.
2. Computes the checksum unless one is already present (reusing the on-the-fly
   hash), then compares it against `expected_checksum` case-insensitively. On
   mismatch it **renames the temp file to `{id}.part.corrupt`** so the evidence
   survives cleanup, marks the task `Failed`, persists and returns
   [[cite:crates/limedl-engine/src/http_executor/finalize.rs:L119-L155]]. The
   current finalizer does not attempt a targeted re-download of affected chunks.
3. On success, calls `ensure_core_fully_durable(&core)` — every chunk must have
   reached the file, otherwise the download fails with the missing byte count
   instead of publishing a truncated file — then `finalize_temp_file` (atomic
   rename), sets `Completed`, marks every chunk complete and releases claims,
   persists, and publishes `aria2.onDownloadComplete`
   [[cite:crates/limedl-engine/src/http_executor/finalize.rs:L177-L227]].

The durability guard takes the guard the caller already holds on purpose:
`finalize_download` is inside the `managed.lock_core()` block that also toggles
the state, and calling `lock_core()` again would deadlock (`parking_lot` mutexes
are not reentrant). See
[test-regression-notes.md](../../docs/test-regression-notes.md) for how that
showed up [[cite:crates/limedl-engine/src/http_executor/finalize.rs:L231-L256]].

## Shared helpers that must not be duplicated

`limedl-engine`'s `http_executor/mod.rs` holds the helpers used by both the
single and chunked paths. Refactoring a path must reuse these rather than
re-implement them
[[cite:crates/limedl-engine/src/http_executor/mod.rs:L74-L343]]:

| Helper | Responsibility |
| --- | --- |
| `build_write_buffer` | HDD shared double-buffer vs SSD/Network local ping-pong (half size clamped 64 KiB–8 MiB) |
| `flush_write_buffer` | flush on exit paths; failures are logged, never propagated |
| `wait_or_stop` | wait for active state and translate pause/cancel into `RunOutcome` |
| `finish_buffer_flush` | flush after completion while toggling the UI `flushing` flag |
| `check_disk_space_periodically` | 30 s space check; insufficient space → `Failed` + `Warning("disk full")` |
| `BatchLimiter` | 256 KiB / 8 chunk rate-limiter batching |
| `RequestBudget` | per-unit-of-work cap on separate requests |
| `ProgressThrottle` | `PERSIST_INTERVAL` persistence + 500 ms progress events |

## Crash recovery

On restart, `load_downloads_from_db` reconstructs tasks and clears every chunk's
`claimed_by` (a fresh manager owns no workers, so persisted claims are stale).
Tasks left `Downloading` stay `Downloading` with connection/allocated counts
zeroed so the scheduler re-allocates; other non-terminal states become `Paused`;
and resume continues from the persisted chunk state — which is exactly the
durable state, because that is all the database ever held
[[cite:crates/limedl-engine/src/manager.rs:L1115-L1190]]. Chunk rows are loaded
lazily, only for non-terminal downloads
[[cite:crates/limedl-engine/src/manager.rs:L1150-L1158]]. See
[[wiki:systems/persistence-and-recovery]].

Related pages: [[wiki:workflows/scheduler-and-concurrency]],
[[wiki:systems/persistence-and-recovery]],
[[wiki:systems/networking-and-rate-control]],
[[wiki:systems/disk-io-and-storage]].
