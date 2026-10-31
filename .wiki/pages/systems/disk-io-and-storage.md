---
id: systems/disk-io-and-storage
title: Disk I/O, Buffer Pool and Storage Detection
status: verified
generated_at: 2026-10-31T14:57:33Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence:
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 56
  - 65
  blob: 223db46be6c2ffe5e0d20d9871c138f919921913
  snippet_sha1: 74b8b47a7dbdd7b5a4855af18eb1985d6222e2d7
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 92
  - 104
  blob: 223db46be6c2ffe5e0d20d9871c138f919921913
  snippet_sha1: 164ddb547f4a2f0f7d869ccfdf6609925188e1c2
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 31
  - 44
  blob: 223db46be6c2ffe5e0d20d9871c138f919921913
  snippet_sha1: c19151a417a1bd712123376f631a90c9886f894d
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 106
  - 123
  blob: 223db46be6c2ffe5e0d20d9871c138f919921913
  snippet_sha1: 68ca935b01ca020e0938de83666629a2a06ce235
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 216
  - 220
  blob: 223db46be6c2ffe5e0d20d9871c138f919921913
  snippet_sha1: a801bb4006ef14c6c402b00571cb9eb267284b04
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 979
  - 1011
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: ef982cb782e2f770c2306e91f4a8060cb7eb6252
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 135
  - 151
  blob: 223db46be6c2ffe5e0d20d9871c138f919921913
  snippet_sha1: ab902e66d2726850583c36d3f0a28fcb7c9db5e7
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 163
  - 196
  blob: 223db46be6c2ffe5e0d20d9871c138f919921913
  snippet_sha1: 2402dcd64a5beb39a7285113fedf67df811d8d03
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 97
  - 118
  blob: 10a37a526acd26abd4f96c9ea74aaac126fb9ad2
  snippet_sha1: 3c279ffe69380688f765adedd60df71aacfd92e9
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 43
  - 91
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: d5ddb9a16d34d415aa0a9b82352735d91f29e0ac
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 398
  - 434
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: d8c2b5fbcef662e5e97c0f584aebe426609fab84
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 18
  - 30
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: f6bd1afc0858c6c5b4d3d5602320358bb7baf7e3
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 294
  - 396
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: 19597c0ac79350c9436bbfdb00c6c7b221a63dc9
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 32
  - 41
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: dfb2260c2504a42965b14fa4119a8935649ed67e
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 609
  - 649
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: 66042a765d2c0d412a3b9d02c83636a2436b0d3b
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 436
  - 473
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: 8fde4c6e49ccbb9d64c582eb31eeb242336cfcd5
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 796
  - 810
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: c8ad7a8789324f6a119c405638e14fc2d1d136c7
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 889
  - 940
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: 5de671f50e31423e78881d9017be12392c1ea219
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 820
  - 825
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: 0fd3ad7f2d091e251221962712df39a6eae7f754
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 874
  - 891
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: edfcef3d7da258636edebce5c36127841e03ed84
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 812
  - 825
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: 28e57c02a5a4ed090ceb858bf4d2c8d302a613c0
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 916
  - 926
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: 8c4df6c7f010344941e0ba5f64b47ecb656caeb1
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 160
  - 186
  blob: 10a37a526acd26abd4f96c9ea74aaac126fb9ad2
  snippet_sha1: 71f24e37897ec05afc7fe0895fdde116284a16f6
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 97
  - 144
  blob: bae71a514c0f0bd275dbb0d373f42cc94611629f
  snippet_sha1: 28c39418475ecdd0528f466543b5e4930f75ae4d
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 544
  - 597
  blob: bae71a514c0f0bd275dbb0d373f42cc94611629f
  snippet_sha1: be5225b5e3e9290cac21ccfe0619027f083ee0b0
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 905
  - 935
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: d9b3d8cc5140fa88cfe8b701c6c4f1b70cf79e72
- path: Cargo.toml
  lines:
  - 113
  - 119
  blob: 76657a09d63846e6e4df9e5ff601c82e5fe5f04e
  snippet_sha1: b04f01ed6c05a24e104a06ee7aeb841bda92ffa3
- path: crates/limedl-native/src/crash.rs
  lines:
  - 10
  - 22
  blob: 9d98e28fb51d9f08d518412204d05fc334056a30
  snippet_sha1: ecfba4b18de5ec147c9855b7bef21e66a35f6977
- path: crates/limedl-disk/src/buffer_pool/worker.rs
  lines:
  - 1
  - 52
  blob: 4fc817c5a2a8688dbce1302c862f672f65d6b82f
  snippet_sha1: c532374ab3622e160b279ca6379304889a788499
- path: crates/limedl-disk/src/io_scheduler/mod.rs
  lines:
  - 103
  - 116
  blob: 70cace1980790599843de01ed3b3f5083906804c
  snippet_sha1: 4cdba12d4fdd2635f49dc6359c9a7c6b1074129f
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 153
  - 180
  blob: 4a1d4e3cf3cd778927ac17aa7a35df143f3cbbba
  snippet_sha1: 86494cea4557f13a63d64a49024ddab7f55d1a4b
- path: crates/limedl-disk/src/io_scheduler/queue.rs
  lines:
  - 52
  - 126
  blob: 6ed1df946fca93a365ce5d3e62d262e7c96c1d97
  snippet_sha1: f36f5fa17deb505ebe9a04e5ee2c923331ffea4b
- path: crates/limedl-disk/src/io_scheduler/queue.rs
  lines:
  - 191
  - 215
  blob: 6ed1df946fca93a365ce5d3e62d262e7c96c1d97
  snippet_sha1: 04b086603f9d06e3bfcaeba00b1e3ebb89a83694
- path: crates/limedl-disk/src/io_scheduler/queue.rs
  lines:
  - 128
  - 189
  blob: 6ed1df946fca93a365ce5d3e62d262e7c96c1d97
  snippet_sha1: 69340e650db12dbe5ad3fb5a2f332532848bf421
- path: crates/limedl-disk/src/file_ops/coalesce.rs
  lines:
  - 17
  - 66
  blob: b691760922701f0b36cd9bd6866d57bdc790f1bd
  snippet_sha1: 41d14337d8b38b8eac26d28742a19ba17a1463bc
- path: crates/limedl-disk/src/file_ops/mod.rs
  lines:
  - 25
  - 45
  blob: fdc0466005070507b08a4f0c537b148733132a57
  snippet_sha1: dbf2203b267f21fa2fe1467863e4c655375a4734
- path: crates/limedl-disk/src/file_ops/mod.rs
  lines:
  - 679
  - 735
  blob: fdc0466005070507b08a4f0c537b148733132a57
  snippet_sha1: 029ecf86172d93430d661e397b1e826b9c1dc93a
- path: crates/limedl-disk/src/file_ops/mod.rs
  lines:
  - 465
  - 532
  blob: fdc0466005070507b08a4f0c537b148733132a57
  snippet_sha1: 1f3ad48f1223d27edd60e045f620a4a5a61fd738
- path: crates/limedl-disk/src/file_ops/mod.rs
  lines:
  - 614
  - 626
  blob: fdc0466005070507b08a4f0c537b148733132a57
  snippet_sha1: 82f28b13281cbc48c08f2aa02c09d03f3bc4d817
- path: crates/limedl-disk/src/file_ops/mod.rs
  lines:
  - 628
  - 677
  blob: fdc0466005070507b08a4f0c537b148733132a57
  snippet_sha1: ea0452cfa72e2f4c9a1872e1115ca8606ade233d
- path: crates/limedl-disk/src/file_ops/mod.rs
  lines:
  - 47
  - 110
  blob: fdc0466005070507b08a4f0c537b148733132a57
  snippet_sha1: 62d490bef80e16d6ea695f3f2ffc40060255e778
- path: crates/limedl-disk/src/file_ops/mod.rs
  lines:
  - 112
  - 138
  blob: fdc0466005070507b08a4f0c537b148733132a57
  snippet_sha1: 21be9569da3ed93b83d6202a6aab2f09f167a890
- path: crates/limedl-disk/src/file_ops/disk_detect.rs
  lines:
  - 525
  - 607
  blob: 9cfbccf20cd7de01cd926f258942845fc5e5bfaf
  snippet_sha1: 1adad08c39383a076030c8d895f107f14d53d348
- path: crates/limedl-disk/src/file_ops/disk_detect.rs
  lines:
  - 649
  - 776
  blob: 9cfbccf20cd7de01cd926f258942845fc5e5bfaf
  snippet_sha1: 8d7c7f1d4a444c0fd26ec0c3827472dba79a89b6
- path: crates/limedl-disk/src/file_ops/disk_detect.rs
  lines:
  - 979
  - 1037
  blob: 9cfbccf20cd7de01cd926f258942845fc5e5bfaf
  snippet_sha1: 41af1f68656e8d575122b38974c984dab8c5bdf5
- path: crates/limedl-disk/src/file_ops/disk_detect.rs
  lines:
  - 1211
  - 1236
  blob: 9cfbccf20cd7de01cd926f258942845fc5e5bfaf
  snippet_sha1: eb553471902a02211b587edc7127475d34e43ca9
- path: crates/limedl-disk/src/file_ops/disk_detect.rs
  lines:
  - 1311
  - 1330
  blob: 9cfbccf20cd7de01cd926f258942845fc5e5bfaf
  snippet_sha1: 6694e6c1c472fe6b47b14e65813decc955ff293e
- path: crates/limedl-disk/src/file_ops/disk_detect.rs
  lines:
  - 1365
  - 1383
  blob: 9cfbccf20cd7de01cd926f258942845fc5e5bfaf
  snippet_sha1: 03e4479bcc6d27b888f0820857a68f09e908cdeb
- path: crates/limedl-disk/src/file_ops/disk_detect.rs
  lines:
  - 327
  - 370
  blob: 9cfbccf20cd7de01cd926f258942845fc5e5bfaf
  snippet_sha1: ed3ed22ce124210f49cb14d889dfbe1464d749f1
- path: crates/limedl-disk/src/file_ops/disk_detect.rs
  lines:
  - 419
  - 435
  blob: 9cfbccf20cd7de01cd926f258942845fc5e5bfaf
  snippet_sha1: 7c7f3c5edb025a8b1b800f029116281ff8617e45
- path: crates/limedl-disk/src/file_ops/disk_detect.rs
  lines:
  - 844
  - 852
  blob: 9cfbccf20cd7de01cd926f258942845fc5e5bfaf
  snippet_sha1: 2cf9577c6b58b381618059d35017e907eaeef4cf
- path: crates/limedl-disk/src/services/disk_io.rs
  lines:
  - 1
  - 48
  blob: 20bead2ff6ef084b22dcdb5883df7ec2dcdd5b84
  snippet_sha1: a5c89e10dd5709696d45dcbd657dbf667404286a
- path: crates/limedl-disk/src/services/disk_io.rs
  lines:
  - 77
  - 104
  blob: 20bead2ff6ef084b22dcdb5883df7ec2dcdd5b84
  snippet_sha1: 7660de8f58cf76cdb992a99200acd61a1d34a9fc
- path: crates/limedl-disk/src/file_ops/media.rs
  lines:
  - 168
  - 228
  blob: a205c41ea980f58cffe9e9c1a6ea5b82ff4c2ff6
  snippet_sha1: 829470e4f0a824523e5c1122021943ab25cd8a64
- path: crates/limedl-disk/src/file_ops/media.rs
  lines:
  - 23
  - 81
  blob: a205c41ea980f58cffe9e9c1a6ea5b82ff4c2ff6
  snippet_sha1: 20073776519ffd29323c51e3e6299e6a3f9093b0
- path: crates/limedl-disk/src/file_ops/media.rs
  lines:
  - 93
  - 166
  blob: a205c41ea980f58cffe9e9c1a6ea5b82ff4c2ff6
  snippet_sha1: 27e90ab7680255186a4a93123dec704208948e14
- path: crates/limedl-disk/src/services/disk_io.rs
  lines:
  - 65
  - 85
  blob: 20bead2ff6ef084b22dcdb5883df7ec2dcdd5b84
  snippet_sha1: 1475c13e9bee8f7d3b3a6a5f44ab8abadd89bdd7
- path: crates/limedl-disk/src/io_scheduler/topology.rs
  lines:
  - 91
  - 113
  blob: 66064a7cfddb499a3447365eec086f266c051e04
  snippet_sha1: dc4372518a5a389f0df2640c61869c83ce302eb0
- path: crates/limedl-disk/src/io_scheduler/topology.rs
  lines:
  - 73
  - 89
  blob: 66064a7cfddb499a3447365eec086f266c051e04
  snippet_sha1: ab51ea5390ff9b7580c4395bc352f4954cfced3d
- path: crates/limedl-disk/src/io_scheduler/mod.rs
  lines:
  - 53
  - 80
  blob: 70cace1980790599843de01ed3b3f5083906804c
  snippet_sha1: 0ea6f053f526d10e693a4dc8b4e2b47186457a2f
- path: crates/limedl-core/src/context.rs
  lines:
  - 68
  - 76
  blob: 26aa8d3d8e5c1669ccede7ef4f1c3f9670669554
  snippet_sha1: ee0f0be0fc21fe293e1ebcac0031beeee80a5fda
- path: crates/limedl-core/src/manager.rs
  lines:
  - 368
  - 372
  blob: fa2c301dce0e94dfff5d64faea9fa56e053c3b70
  snippet_sha1: 84e99ce806e3c98621ac4cad57421f42bfffe658
type: system
---
# Disk I/O, Buffer Pool and Storage Detection

Downloads are write-heavy and their storage can be a spinning disk, flash, or a
network share. limedl adapts its write strategy to the media it detects, then
falls back to user overrides where detection cannot know the answer. Since the
crate split this machinery lives in `limedl-disk` (`buffer_pool`, `file_ops`,
`io_scheduler`, `services`), with the write-buffer *policy* — which mode a task
gets, and what the durable-progress observer does — on the engine side in
`limedl-engine::http_executor`.

## BufferPool: slots and memory

`BufferPool` is a global pool for HDD buffering. It stores limit and parallelism
parameters in atomics, a `tokio::sync::Semaphore` of slots, a usage counter and an
active count. `half_size()` — the capacity of one buffer half — is
`effective_limit / effective_max_parallel / 2`, floored at 64 KiB so even tiny
configurations stay functional.
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L56-L65]]
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L92-L104]]

`SlotGuard` is the RAII permit: `acquire_slot()` awaits an owned permit,
increments `active_count` and returns the guard; dropping it returns the permit to
the semaphore. The matching `active_count` decrement is not in the guard's `Drop`
— it happens in `release_slot()`, which `DownloadBuffer::Drop` calls, because the
buffer (not the guard) is what owns the slot for the download's lifetime.
`degradation_count()` always returns 0 by design: the double-buffer
backpressures instead of degrading to direct I/O.
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L31-L44]]
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L106-L123]]
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L216-L220]]
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L979-L1011]]

Game mode swaps `effective_limit()` and `effective_max_parallel()` to their
reduced values, and explicitly does **not** revoke permits already held — only new
`half_size()` calculations see the lower limit. `update_limits` grows the
semaphore when the effective maximum increases and deliberately never shrinks it:
existing permits are safe to keep and return naturally on drop.
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L135-L151]]
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L163-L196]]

In practice game mode only affects the HDD pool: the pool is consulted only when
the resolved media is `Hdd` and HDD buffering is enabled; every other task builds
its own local buffer whose size comes from chunk size or the SSD write-combine
setting, not from the pool.
[[cite:crates/limedl-engine/src/http_executor/mod.rs:L97-L118]]

## DownloadBuffer: HDD double-buffering and SSD write-combining

`DownloadBuffer` has two modes, `BufferMode::Double` (pool-backed HDD ping-pong)
and `BufferMode::LocalPingPong` (the SSD/Network write-combining buffer, same
ping-pong logic without global pool or slot management). The flip core is
`buffer_chunk_pingpong_impl`, shared by both with a `PingPongCfg`; the only
differences the config carries are whether a global pool participates and which
`SyncMode` the background flush uses.
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L43-L91]]
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L398-L434]]

The flip sequence is a load-bearing invariant:

1. load `active_is_a` and select the active half;
2. if the half has room, insert under the half's lock and re-check that the half
   did not switch while waiting;
3. the flipper takes a `flip_token`, awaits any previous background flush, folds
   leftovers into the new half, spawns the background flush, **stores the new
   flush handle**, then atomically flips `active_is_a`, and finally inserts the
   chunk into the new active half.

Storing the handle *before* flipping is what prevents a concurrent waiter from
observing "flipped but no handle". The `flip_token` serializes flips and is
released by `FlipTokenGuard` even on panic. If a background flush failed, the
error flag is checked before every iteration and at each await point, so the call
fails immediately rather than downloading data destined for a failed write.
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L18-L30]]
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L294-L396]]

### The flush observer: the buffer is the durability oracle

`DownloadBuffer` carries a `FlushObserver` —
`Arc<dyn Fn(&[(u64, u64)]) + Send + Sync>` receiving the `(offset, len)` ranges a
write just put on the file. The buffer is the **only** component that knows when
buffered data left memory, so the download engine's durable-progress counters are
advanced from here and nowhere else (see
[[wiki:systems/persistence-and-recovery]]).
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L32-L41]]

The rules the plumbing follows:

- Every successful path reports: the background flush on the device queue, its
  `spawn_blocking` fallback, `flush_one_half` (HDD), `flush_one_half_local`
  (SSD/Network) and `write_oversized_direct` (a chunk larger than a half).
  [[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L609-L649]]
  [[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L436-L473]]
  [[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L796-L810]]
  [[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L889-L940]]
- **Nothing reports on failure.** A failed write must not advance durable
  progress; the worker's direct-write fallback reports those bytes instead, once
  `write_all_at` has returned.
  [[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L820-L825]]
  [[cite:crates/limedl-engine/src/http_executor/worker.rs:L874-L891]]
- `entry_ranges` snapshots the ranges before the entries are moved into the write
  path, and `notify_flushed` is the single call site, so a new flush path cannot
  silently skip the report.
  [[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L812-L825]]
- `sync_data` failing after a successful write still counts as durable (only the
  power-loss guarantee is missing), so the observer is notified either way.
  [[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L916-L926]]

The observer takes the task's core lock, and `parking_lot` mutexes are not
reentrant: **an observer must never run while the caller holds that guard.** The
callers that can flush (`finish_buffer_flush`, and the pause/cancel/demote/downgrade
paths in `chunked.rs` and `single.rs`) therefore release the guard *before*
awaiting the flush — `finish_buffer_flush` scopes the `flushing` flag write in its
own block and only then awaits `flush_all`. The incidental benefit is that a long
flush no longer blocks progress updates.
[[cite:crates/limedl-engine/src/http_executor/mod.rs:L160-L186]]
[[cite:crates/limedl-engine/src/http_executor/chunked.rs:L97-L144]]
[[cite:crates/limedl-engine/src/http_executor/chunked.rs:L544-L597]]

The `catch_unwind` around the `spawn_blocking` fallback is a development and
`test-utils` safety net only: the release profile builds with `panic = "abort"`,
where it cannot catch anything and the process dies instead — the only evidence
then being the native client's panic hook, which writes
`<state_dir>/logs/crash.log`.
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L905-L935]]
[[cite:Cargo.toml:L113-L119]]
[[cite:crates/limedl-native/src/crash.rs:L10-L22]]

### Where the bytes actually go

Every production flush is submitted to the per-device queues: `IoWorker` is now a
thin `Arc<dyn BatchWriteSink>` handle (the seam is declared in `file_ops` so the
buffering layer never has to name `io_scheduler`), and in production that sink is
`DiskDeviceManager`, so there is exactly one place where a batch reaches the disk
and exactly one place that can fail. `spawn_blocking` remains only for the
test/bench-only constructors that carry no sink.
[[cite:crates/limedl-disk/src/buffer_pool/worker.rs:L1-L52]]
[[cite:crates/limedl-disk/src/io_scheduler/mod.rs:L103-L116]]
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L153-L180]]

`DeviceQueue::new` fixes its writer-thread count at construction: 1 serialized
channel for HDD (to eliminate seek storms) and 4 parallel channels for
`Ssd`/`Network`, because a network transport already reorders writes on the far
side; each channel is an unbounded `mpsc` thread. Batches are routed by file
identity so writes to one file stay serialised.
[[cite:crates/limedl-disk/src/io_scheduler/queue.rs:L52-L126]]
[[cite:crates/limedl-disk/src/io_scheduler/queue.rs:L191-L215]]

The queue thread writes with `write_coalesced_entries`, which merges adjacent
entries into one vectored (`pwritev`) or single-syscall write, then applies the
`SyncMode` policy: no sync for SSD, the 16 MiB / 3 s adaptive threshold for the
HDD background path, and an immediate `sync_data` when the buffer is flushed on
pause, cancel, finish or `flush_all`.
[[cite:crates/limedl-disk/src/io_scheduler/queue.rs:L128-L189]]
[[cite:crates/limedl-disk/src/file_ops/coalesce.rs:L17-L66]]

## FileOps

### Open and preallocate

`open_download_file` creates parent directories, opens (or reuses) the file
without truncating, then reserves the full size; `reset_download_file` truncates
to zero and re-reserves. `preallocate_file`:

- on Windows first tries `SetFileValidData` (instantaneous, needs
  `SeManageVolumePrivilege`, acquired once per process), then enables sparse mode,
  then calls `set_len`;
- on Linux calls `fallocate`, falling back to `set_len` when it is unsupported
  (`EPERM`, `EINVAL`, `ENOSYS`, `ENOTSUP`/`EOPNOTSUPP` and the glibc `524`
  spelling); other Unix platforms go straight to `set_len`.
[[cite:crates/limedl-disk/src/file_ops/mod.rs:L25-L45]]
[[cite:crates/limedl-disk/src/file_ops/mod.rs:L679-L735]]
[[cite:crates/limedl-disk/src/file_ops/mod.rs:L465-L532]]

### Disk space vs single-file limit

`check_disk_space` rejects when available space is below `required * 1.1` (a 10 %
buffer). That check cannot catch a per-file size limit, because a volume with
terabytes free and a 4 GiB per-file cap (FAT32) passes it and only fails when the
file is extended.
[[cite:crates/limedl-disk/src/file_ops/mod.rs:L614-L626]]

`reservation_error` is where that is translated, after `reserve_download_space`
re-reads the volume's free space:

- `ErrorKind::FileTooLarge` (EFBIG / `ERROR_FILE_TOO_LARGE`) → `FileTooLarge`;
- `StorageFull` when the post-failure free space is still ≥ the file size → also
  `FileTooLarge` (some volumes answer "disk full" for an oversized reservation);
- `StorageFull` otherwise → `InsufficientDiskSpace`, or the raw I/O error when no
  directory was available to measure;
- anything else keeps its raw I/O error.

The rule is **do not guess a filesystem from an unrelated failure** — the old
"FAT32 can't hold 4 GiB" hint guessed from file size and sprayed noise on
NTFS/ext4/APFS. `preallocate_file` returns a raw `io::Result` precisely because
only the caller knows the destination directory (and thus the free space) needed
to make this decision; `reset_download_file`, which has only a `File`, passes
`None` and therefore decides by `ErrorKind` alone.
[[cite:crates/limedl-disk/src/file_ops/mod.rs:L628-L677]]

### Finalization

`finalize_temp_file(temp, dest)`:

1. If `dest` exists and has identical content, it is treated as an idempotent
   retry: clean staging leftovers and remove the temp file. Otherwise it errors.
2. Primary path: atomic `rename` (same filesystem, metadata-only, O(1)).
3. On `ErrorKind::CrossesDevices`: copy into a unique staging file **in the
   destination's directory** using a 1 MiB buffer, flush and `sync_all`, then
   `rename` staging → destination (same-volume atomic). On copy failure the
   staging file is removed and the source/temp file is preserved. A conflicting
   destination that matches is again accepted idempotently.

The invariant is that finalization never discards the only copy of the data
because one publish attempt failed.
[[cite:crates/limedl-disk/src/file_ops/mod.rs:L47-L110]]
[[cite:crates/limedl-disk/src/file_ops/mod.rs:L112-L138]]

## Media detection

`detect_disk_type(path)` is implemented per platform:

- **Windows**: `CreateFileW(\\.\C:)` + `IOCTL_STORAGE_QUERY_PROPERTY`, probing the
  seek-penalty property, then bus type (NVMe/SCM/SD/MMC ⇒ SSD), then TRIM, on the
  volume and on the physical drive behind it. UNC shares, device namespaces and
  verbatim UNC paths are reported `Network`; mapped drives (`DRIVE_REMOTE`) are
  classified by what the letter points at.
  [[cite:crates/limedl-disk/src/file_ops/disk_detect.rs:L525-L607]]
  [[cite:crates/limedl-disk/src/file_ops/disk_detect.rs:L649-L776]]
- **Linux**: `/sys/block/<dev>/queue/rotational` via `/sys/dev/block/<major>:<minor>`,
  with fstype classification of network/host-brokered mounts from
  `/proc/self/mountinfo` first.
  [[cite:crates/limedl-disk/src/file_ops/disk_detect.rs:L979-L1037]]
- **macOS**: `statfs` for the BSD device, then IOKit `IOMedia` and the
  `Rotational` property of the owning `IOBlockStorageDriver`.
  [[cite:crates/limedl-disk/src/file_ops/disk_detect.rs:L1211-L1236]]
  [[cite:crates/limedl-disk/src/file_ops/disk_detect.rs:L1311-L1330]]
- **Unknown targets**: the fallback module returns `Ssd` (unknown ≠ remote).
  [[cite:crates/limedl-disk/src/file_ops/disk_detect.rs:L1365-L1383]]

**WSL** (`\\wsl$\<distro>` / `\\wsl.localhost\<distro>`) is deliberately *not*
network storage: the 9p/virtiofs server in the utility VM serves a local
`ext4.vhdx`, so the code resolves the distro's `BasePath` from the
`HKCU\…\Lxss` registry key and detects the host volume that stores the VHDX
(falling back to `Network` only when no registry entry exists). Result is cached
per distro. `detect_all_disk_types` also lists each registered distro under
`\\wsl$\<distro>` so the settings UI can offer it for an override.
[[cite:crates/limedl-disk/src/file_ops/disk_detect.rs:L327-L370]]
[[cite:crates/limedl-disk/src/file_ops/disk_detect.rs:L419-L435]]
[[cite:crates/limedl-disk/src/file_ops/disk_detect.rs:L844-L852]]

`DiskIoService::resolve_disk_type` is the single entry point used at download
start: it checks the settings overrides first, then (on Unix only) an in-process
cache keyed by the `st_dev` device number from `std::os::unix::fs::MetadataExt`,
and finally falls back to `detect_disk_type`. The cache lets several destination
paths on one volume share a single probe; Windows has no equivalent because its
platform probe is already cheap and is cached per drive letter inside the
detector. The cache field and its initializers are `#[cfg(unix)]`, and so are the
`foldhash::HashMap` / `parking_lot::Mutex` imports it uses — importing them
unconditionally made Windows builds fail with `unused_imports` under
`CARGO_BUILD_WARNINGS=deny`.
[[cite:crates/limedl-disk/src/services/disk_io.rs:L1-L48]]
[[cite:crates/limedl-disk/src/services/disk_io.rs:L77-L104]]

`is_network_filesystem` is a *closed* set of remote and host-brokered transports
(NFS, CIFS/SMB, AFP, WebDAV, GlusterFS, Ceph, Lustre, 9p, virtiofs, drvfs, VM
guest additions, and the named remote FUSE backends). An unknown fstype — a
brand-new local filesystem, or bare `fuse` such as ntfs-3g — keeps the historical
`Ssd` answer rather than being called remote.
[[cite:crates/limedl-disk/src/file_ops/media.rs:L168-L228]]

## Directory overrides

`disk_type_overrides` maps user-entered **directories** to a forced `DiskType`.
Normalization (`normalize_media_path`) trims, and on Windows lowercases, converts
separators and strips the `\\?\` / `\\?\UNC\` prefixes; every platform strips
trailing separators while preserving a root (an empty key would match everything).
Override keys must be absolute (`\\server\share` counts on Windows, even without a
trailing separator) — `is_usable_override_key` rejects the rest so the settings
editor never persists a key that can match nothing.
[[cite:crates/limedl-disk/src/file_ops/media.rs:L23-L81]]

Matching is a **component-boundary prefix match, longest key first**:
`D:\downloads` covers `D:\downloads\a.bin` but not `D:\downloads-old`, and a
nested rule wins over its parent. `MediaOverrides` snapshots a pre-normalized,
longest-first list so device topology does not re-normalize the map per lookup;
`lookup_media_override` rebuilds it for the per-start buffer decision, which is
deliberate because that map has a handful of entries at most.
[[cite:crates/limedl-disk/src/file_ops/media.rs:L93-L166]]

Overrides take effect in exactly two places: the buffer-mode decision at download
start (`DiskIoService::resolve_disk_type`) and device-queue media resolution.
[[cite:crates/limedl-disk/src/services/disk_io.rs:L65-L85]]

## Device queues and why overrides drop them

`DeviceQueue::new` fixes its writer-thread count at construction (see above), so a
queue built under the previous media policy can never adopt a new one.
`DeviceTopology::resolve_device` makes overrides outrank detection and caches the
resolved `(DeviceId, DiskType)` per path for the process lifetime.
[[cite:crates/limedl-disk/src/io_scheduler/topology.rs:L91-L113]]

Because of that caching, `DeviceTopology::set_overrides` clears its resolution
cache (and reports whether anything actually changed), and
`DiskDeviceManager::set_overrides` additionally **drops the entire queue map**.
In-flight writers keep the `Arc` they already hold and finish normally; the next
lookup builds a queue matching the new policy. Without this, a saved override
needed an app restart before the write-thread count changed.
`SystemContext::with_components` seeds the topology with persisted overrides at
startup for the same reason, and `DownloadManager` re-pushes them on every
settings save.
[[cite:crates/limedl-disk/src/io_scheduler/topology.rs:L73-L89]]
[[cite:crates/limedl-disk/src/io_scheduler/mod.rs:L53-L80]]
[[cite:crates/limedl-core/src/context.rs:L68-L76]]
[[cite:crates/limedl-core/src/manager.rs:L368-L372]]

Related pages: [[wiki:workflows/http-download-lifecycle]],
[[wiki:systems/settings-and-configuration]],
[[wiki:systems/persistence-and-recovery]].
