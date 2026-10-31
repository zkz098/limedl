---
id: workflows/http-download-lifecycle
title: HTTP Download Lifecycle
status: verified
generated_at: 2026-10-31T14:58:09Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence:
- path: crates/limedl-core/src/manager.rs
  lines:
  - 395
  - 462
  blob: fa2c301dce0e94dfff5d64faea9fa56e053c3b70
  snippet_sha1: 68391a3433e9edbc4163bf84c6e5c47e7029f8d2
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 99
  - 150
  blob: e2b0f898b2e1fea578b56a582f8dd6d4165d682a
  snippet_sha1: 35de1cc207ef69c0cba79b669ac9541c1799d761
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 564
  - 569
  blob: e2b0f898b2e1fea578b56a582f8dd6d4165d682a
  snippet_sha1: e07aa19b60a1f612cc6b129e0ddf53c04ac08957
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 162
  - 186
  blob: e2b0f898b2e1fea578b56a582f8dd6d4165d682a
  snippet_sha1: f33635ef872705197c7fa05f712d9f18fb5ab8ec
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 130
  - 150
  blob: e2b0f898b2e1fea578b56a582f8dd6d4165d682a
  snippet_sha1: 8c1f216c45009ff56fbaaeb510197611a18e2644
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 29
  - 56
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: add989d04bb39d81e1c3f54d22a8c058a8d838ca
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 60
  - 118
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: 4c1a880e729fe1f793ae78ca6bfbff95ea3a3b54
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 158
  - 215
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: 985a93b8f55e461c3fadd068aa8dd6808430f816
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 217
  - 238
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: c06325612ec9d77416bb7871e61f8e6fd559a643
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 240
  - 251
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: c6e7259c3ee47239af82c28a60e06090c7533c4c
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 331
  - 413
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: d80c7473bebd26f60c54916e8c860c590dcb44fb
- path: crates/limedl-types/src/manifest.rs
  lines:
  - 138
  - 199
  blob: 54ad684831aca2c5fa70d6650426e2a12e96fd1e
  snippet_sha1: 198f76a4aa0d0f77daaee178f3d4b3ee2bcfa356
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 253
  - 311
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: 839220c78a88cfdc8987d6f8758a4a745be468b4
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 97
  - 213
  blob: bae71a514c0f0bd275dbb0d373f42cc94611629f
  snippet_sha1: f5eec2949d480252580e3d20e2f981cd1b7114d3
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 54
  - 175
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: be705b97679b03eb4747b311a7d75d5ea33c67b6
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 46
  - 47
  blob: 10a37a526acd26abd4f96c9ea74aaac126fb9ad2
  snippet_sha1: fdb9af25d877f0d906554fc0955c9b4c32c2dcee
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 177
  - 190
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: d57cdfd1f9bbeb8bb61fea0c687689d8a3048fb8
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 939
  - 951
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: 05c5052c3c936a4990ba744403cb861354a5b77f
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 326
  - 438
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: cd2472313e8146fbaed43564dea00ca971af4f54
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 274
  - 307
  blob: 10a37a526acd26abd4f96c9ea74aaac126fb9ad2
  snippet_sha1: d3dd7c0d7482b3625823f77606bf808a4b2bfe94
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 488
  - 542
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: 25011e18c014b6e38c364c95e529ce082c7c8529
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 677
  - 698
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: 16e970282f3ebc98305aa9bdd2256ba24e6f0b81
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
  blob: bae71a514c0f0bd275dbb0d373f42cc94611629f
  snippet_sha1: 82c5ebbf255b180aa6f61a8b33a6009689e0ea98
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 580
  - 589
  blob: bae71a514c0f0bd275dbb0d373f42cc94611629f
  snippet_sha1: 7391ff4762fa4cc59fffb3520183c0f7fc16e0ee
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 199
  - 208
  blob: bae71a514c0f0bd275dbb0d373f42cc94611629f
  snippet_sha1: 1942e961049788ba2e5500215b78cc458890b8b4
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 38
  - 93
  blob: b39f476924d7685e34bb874806ba1366b92cd35b
  snippet_sha1: 4366420ead6835edabd4147e5f87375e1cd563ab
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 141
  - 219
  blob: b39f476924d7685e34bb874806ba1366b92cd35b
  snippet_sha1: 5f2fcefd68e8d9ef3ab4a6f039145bf2ba80ad36
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 29
  - 35
  blob: b39f476924d7685e34bb874806ba1366b92cd35b
  snippet_sha1: 386a4528311fa8ec28afd91a5ab00b27039c91ea
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 156
  - 160
  blob: b39f476924d7685e34bb874806ba1366b92cd35b
  snippet_sha1: a02165d043433a90490cb2f7ae69a05cca200312
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 372
  - 406
  blob: b39f476924d7685e34bb874806ba1366b92cd35b
  snippet_sha1: 829a95fe8a77caaca2985c162e0e868199648fd4
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 684
  - 694
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: bc304387cf97d99dc14da003a7d36d9f45d11d5f
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
  blob: 10a37a526acd26abd4f96c9ea74aaac126fb9ad2
  snippet_sha1: 338251018a825a68488b5321d4309209b38870f3
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 283
  - 311
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: fae5e958ef86017e4bbbd20df139379c31f61ae9
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 236
  - 272
  blob: 10a37a526acd26abd4f96c9ea74aaac126fb9ad2
  snippet_sha1: 33402a83fd562a917327a7aafd63a1072cb10c99
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 155
  - 220
  blob: 1425cef6f504dffe584d6b9fd22fbdcb396efdab
  snippet_sha1: 03fcc6b4c81015f661ef72079c00b759220c963a
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 84
  - 90
  blob: 10a37a526acd26abd4f96c9ea74aaac126fb9ad2
  snippet_sha1: 0fe05db9366e1579f02d87c9c970da06ed961ec4
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 875
  - 890
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: 64b885eb8626bf8409c25510c0ea4cc2b52809a9
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
  blob: 54ad684831aca2c5fa70d6650426e2a12e96fd1e
  snippet_sha1: 0c10c040314faae744ea92d84a5c0471b50d406e
- path: crates/limedl-engine/src/persistence.rs
  lines:
  - 32
  - 130
  blob: dac6df18b1fbb4ac265f3454ec8096905dbe3057
  snippet_sha1: 4157be261610ab7a86064de05df1d0a92c5edde4
- path: crates/limedl-core/src/manager.rs
  lines:
  - 1196
  - 1212
  blob: fa2c301dce0e94dfff5d64faea9fa56e053c3b70
  snippet_sha1: b758a9b65e29b2eedf72a1441c2b2b6288e20d57
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 184
  - 195
  blob: 1425cef6f504dffe584d6b9fd22fbdcb396efdab
  snippet_sha1: 4170daa8311f64a7f9475ecc176a75cf7372c71f
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 170
  - 181
  blob: 1425cef6f504dffe584d6b9fd22fbdcb396efdab
  snippet_sha1: bd23b9b4c031600a8d38c56684f1f8d3334714d8
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 192
  - 195
  blob: 1425cef6f504dffe584d6b9fd22fbdcb396efdab
  snippet_sha1: a6c0bebec5970eaf3ad0debfbf9c9ce37ee5adcf
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 18
  - 228
  blob: 98508e0c71004d01fdccd3e13ccca55eeb40160b
  snippet_sha1: ffafbf1cb09433f19abae892a780406ea04b2168
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 119
  - 155
  blob: 98508e0c71004d01fdccd3e13ccca55eeb40160b
  snippet_sha1: 62a4189e47a1f36834e6305a56b4a985cb98c1ac
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 177
  - 227
  blob: 98508e0c71004d01fdccd3e13ccca55eeb40160b
  snippet_sha1: 697f3b70ee57f43d06561f58a155e9a226f37743
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 231
  - 256
  blob: 98508e0c71004d01fdccd3e13ccca55eeb40160b
  snippet_sha1: 9fdb37de037c0988fe0d0b3063d4161bd1324f16
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 74
  - 343
  blob: 10a37a526acd26abd4f96c9ea74aaac126fb9ad2
  snippet_sha1: 6b81dbf48cd134a68dc7c6a532491d1ffaa8da0e
- path: crates/limedl-core/src/manager.rs
  lines:
  - 1115
  - 1190
  blob: fa2c301dce0e94dfff5d64faea9fa56e053c3b70
  snippet_sha1: a2f31dcaca911a69114b1fe9767d89f897c35cf9
- path: crates/limedl-core/src/manager.rs
  lines:
  - 1150
  - 1158
  blob: fa2c301dce0e94dfff5d64faea9fa56e053c3b70
  snippet_sha1: 089c7d1df38b5668183db58c597bc3a25f607c35
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
[[cite:crates/limedl-core/src/manager.rs:L395-L462]]:

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
[[cite:crates/limedl-engine/src/http_executor/run.rs:L158-L215]]. Checksum
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
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L54-L175]]. The default
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
[[cite:crates/limedl-engine/src/http_executor/single.rs:L38-L93]],
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
[[cite:crates/limedl-core/src/manager.rs:L1196-L1212]] — all substitute
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
[[cite:crates/limedl-engine/src/http_executor/finalize.rs:L18-L228]]:

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
[[cite:crates/limedl-core/src/manager.rs:L1115-L1190]]. Chunk rows are loaded
lazily, only for non-terminal downloads
[[cite:crates/limedl-core/src/manager.rs:L1150-L1158]]. See
[[wiki:systems/persistence-and-recovery]].

Related pages: [[wiki:workflows/scheduler-and-concurrency]],
[[wiki:systems/persistence-and-recovery]],
[[wiki:systems/networking-and-rate-control]],
[[wiki:systems/disk-io-and-storage]].
