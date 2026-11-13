---
id: systems/disk-io-and-storage
title: Disk I/O, Buffer Pool and Storage Detection
status: verified
generated_at: 2026-11-13T10:44:49Z
commit: ef4bed786d602c1598991bc6d8ce0eb0856786ce
evidence:
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 64
  - 97
  blob: 7e4c43e81f1b57d5d21ccc5ba8da049732aaa74b
  snippet_sha1: 31dfa50420c40855f144b4ad0d150e4bff27413c
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 116
  - 156
  blob: 7e4c43e81f1b57d5d21ccc5ba8da049732aaa74b
  snippet_sha1: 00bc81373c041338ae1f4ae5b6a38f8440b9cd78
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 40
  - 53
  blob: 7e4c43e81f1b57d5d21ccc5ba8da049732aaa74b
  snippet_sha1: c19151a417a1bd712123376f631a90c9886f894d
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 158
  - 175
  blob: 7e4c43e81f1b57d5d21ccc5ba8da049732aaa74b
  snippet_sha1: 68ca935b01ca020e0938de83666629a2a06ce235
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 259
  - 263
  blob: 7e4c43e81f1b57d5d21ccc5ba8da049732aaa74b
  snippet_sha1: a801bb4006ef14c6c402b00571cb9eb267284b04
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 789
  - 820
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: 27aa3e7ad99adcdffe42d4745ec97ecc06a8fd6a
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 187
  - 195
  blob: 7e4c43e81f1b57d5d21ccc5ba8da049732aaa74b
  snippet_sha1: 865e31131ee4479817b9aebaa3fba985ab1ae1dc
- path: crates/limedl-disk/src/buffer_pool/mod.rs
  lines:
  - 207
  - 239
  blob: 7e4c43e81f1b57d5d21ccc5ba8da049732aaa74b
  snippet_sha1: 33e6a92d9d08156fa3afa3eb81a207fa60e54c25
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 97
  - 118
  blob: 440aa6e7e0e75317a748ed5618b9c2869db2bb86
  snippet_sha1: 35454b7fbce100af637faf5df031d3d3e321445b
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 43
  - 91
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: 28333f7e9ab00dd53e4b5c160b8ae17f3b49bf4f
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 398
  - 434
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: f33309db00ffc1742d7815ceea2b5b61c5513ed8
- path: crates/limedl-disk/src/buffer_pool/pingpong_sync.rs
  lines:
  - 17
  - 46
  blob: 3400c6257b1ae63850b8ad6557118c125395d5b7
  snippet_sha1: 9e608be685863792767f395b40e1fe72ab61e566
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 294
  - 348
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: 8530c0d251ca48c6e387b1666c673d21142af09e
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 352
  - 386
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: 756208024da14444f76ea7b9b95422a3668faf99
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 393
  - 438
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: 1e2000cdb67104ab0319156ee3543f6f55818200
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 20
  - 29
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: dfb2260c2504a42965b14fa4119a8935649ed67e
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 597
  - 637
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: 77e7f7b5c5c16b2c6f0b29e4051384929bc8c10c
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 424
  - 461
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: 8954708a955cf5aaabe6fb7b898c6ac5da75f38e
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 796
  - 810
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: 5ca6365046f1513ea83520a70a75a741710c4bb9
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 889
  - 940
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 820
  - 825
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: b3127725f678ca5b1038b1df45a06f2ff4e1f544
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 874
  - 891
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: f2284d2bff1c1f3703617582ef6888cbceabe9ea
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 812
  - 825
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: a4340e20c547e2f9884a766d89d9d579f1c843a9
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 916
  - 926
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 160
  - 186
  blob: 440aa6e7e0e75317a748ed5618b9c2869db2bb86
  snippet_sha1: 77728954058cad9f43d9b0ed59a555afec553481
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 97
  - 144
  blob: 4080a38ecd8d1c475cc747f753e65c58658ad9ce
  snippet_sha1: 4cdeea792ec17a190c1e9c2982ca9d81aff0bb82
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 544
  - 597
  blob: 4080a38ecd8d1c475cc747f753e65c58658ad9ce
  snippet_sha1: 83f10b9cff020ce33dae75fd8afed77b4bd72e6c
- path: crates/limedl-disk/src/buffer_pool/download_buffer.rs
  lines:
  - 905
  - 935
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
- path: Cargo.toml
  lines:
  - 113
  - 119
  blob: 4432791995601330a33bc1b6d774c6fa31036526
  snippet_sha1: 35744ee7c901b9cab8b0e3971c7bca00fce35bdc
  note: drift-projected L113-L119@d6e99b28 -> L113-L119 (similarity 1.00)
- path: crates/limedl-core/src/crash.rs
  lines:
  - 18
  - 39
  blob: 339b0d9c68a04e07cf182c5ded9c4f2d1d816b2d
  snippet_sha1: 622955c9ffe7dfbc6df8c12aa318e3374cab9df8
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
  blob: f1711aeb3badc5423223acaf6e2dd1b0ecfc7751
  snippet_sha1: 3221236b051f6b3179c0144a9eb3496180d3a7f2
- path: crates/limedl-disk/src/io_scheduler/queue.rs
  lines:
  - 52
  - 126
  blob: 10fb9db675f2715c434078bb0ba6efb5067e1e41
  snippet_sha1: f36f5fa17deb505ebe9a04e5ee2c923331ffea4b
- path: crates/limedl-disk/src/io_scheduler/queue.rs
  lines:
  - 191
  - 215
  blob: 10fb9db675f2715c434078bb0ba6efb5067e1e41
  snippet_sha1: 04b086603f9d06e3bfcaeba00b1e3ebb89a83694
- path: crates/limedl-disk/src/io_scheduler/queue.rs
  lines:
  - 128
  - 189
  blob: 10fb9db675f2715c434078bb0ba6efb5067e1e41
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
  blob: ef3b86ce99edd9017d1c9b71ecdd56268d8e902a
  snippet_sha1: 5c650075683e5ecd8474680dc865c0f7e8a1ef9c
- path: crates/limedl-disk/src/services/disk_io.rs
  lines:
  - 77
  - 104
  blob: ef3b86ce99edd9017d1c9b71ecdd56268d8e902a
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
  blob: ef3b86ce99edd9017d1c9b71ecdd56268d8e902a
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
- path: crates/limedl-engine/src/context.rs
  lines:
  - 68
  - 76
  blob: 26aa8d3d8e5c1669ccede7ef4f1c3f9670669554
  snippet_sha1: ee0f0be0fc21fe293e1ebcac0031beeee80a5fda
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 368
  - 372
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: 41cc728e582e044008200ee067c28e61f9e85635
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 63
  - 66
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: b0bb6267290bdece85a83c2e23297eb2155bf658
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 451
  - 453
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: d5f95790b312725a08242b8b228e6d09a38a9fbc
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 120
  - 175
  blob: acc4494d3d2689ddbda4f66c37515366282fa5d3
  snippet_sha1: 83b8758fdbba2ab3170cbd5296ba3790268308f6
- path: crates/limedl-native/src/update/mod.rs
  lines:
  - 578
  - 646
  blob: b81bed4e871719ca834a78d5fa058d62c97e8ea8
  snippet_sha1: 62ac0a2e311e8abec66e638618cf5476badafd48
- path: crates/limedl-native/src/update/mod.rs
  lines:
  - 664
  - 673
  blob: b81bed4e871719ca834a78d5fa058d62c97e8ea8
  snippet_sha1: 54ebd78ef86737dc1184afa9a6ae88ee291cb4a4
- path: crates/limedl-native/src/update/mod.rs
  lines:
  - 633
  - 638
  blob: b81bed4e871719ca834a78d5fa058d62c97e8ea8
  snippet_sha1: 3f6752d48759f0228d2341ff6d63cbba552f0189
- path: crates/limedl-native/src/update/tests.rs
  lines:
  - 175
  - 186
  blob: c5bcfcf00bfa2de653775ba70c8cf7bdda7a104b
  snippet_sha1: 9bd34f7c0e8e2e65218dce5237f433aca79c4ab7
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
parameters in `BufferPoolConfig` behind a short-lived `Mutex`, a
`tokio::sync::Semaphore` of slots, a usage counter and an active count.
`half_size()` — the capacity of one buffer half — is
`effective_limit / effective_max_parallel / 2`, floored at 64 KiB so even tiny
configurations stay functional.
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L64-L97]]
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L116-L156]]

`SlotGuard` is the RAII permit: `acquire_slot()` awaits an owned permit,
increments `active_count` and returns the guard; dropping it returns the permit to
the semaphore. The matching `active_count` decrement is not in the guard's `Drop`
— it happens in `release_slot()`, which `DownloadBuffer::Drop` calls, because the
buffer (not the guard) is what owns the slot for the download's lifetime.
`degradation_count()` always returns 0 by design: the double-buffer
backpressures instead of degrading to direct I/O.
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L40-L53]]
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L158-L175]]
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L259-L263]]
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L789-L820]]

Game mode swaps `effective_limit()` and `effective_max_parallel()` to their
reduced values, and explicitly does **not** revoke permits already held — only new
`half_size()` calculations see the lower limit. `update_limits` grows the
semaphore when the effective maximum increases and deliberately never shrinks it:
existing permits are safe to keep and return naturally on drop.
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L187-L195]]
[[cite:crates/limedl-disk/src/buffer_pool/mod.rs:L207-L239]]

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
[[cite:crates/limedl-disk/src/buffer_pool/pingpong_sync.rs:L17-L46]]
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L294-L348]]
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L352-L386]]
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L393-L438]]

### The flush observer: the buffer is the durability oracle

`DownloadBuffer` carries a `FlushObserver` —
`Arc<dyn Fn(&[(u64, u64)]) + Send + Sync>` receiving the `(offset, len)` ranges a
write just put on the file. The buffer is the **only** component that knows when
buffered data left memory, so the download engine's durable-progress counters are
advanced from here and nowhere else (see
[[wiki:systems/persistence-and-recovery]]).
[[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L20-L29]]

The rules the plumbing follows:

- Every successful path reports: the background flush on the device queue, its
  `spawn_blocking` fallback, `flush_one_half` (HDD), `flush_one_half_local`
  (SSD/Network) and `write_oversized_direct` (a chunk larger than a half).
  [[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L597-L637]]
  [[cite:crates/limedl-disk/src/buffer_pool/download_buffer.rs:L424-L461]]
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
[[cite:crates/limedl-core/src/crash.rs:L18-L39]]

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
[[cite:crates/limedl-engine/src/context.rs:L68-L76]]
[[cite:crates/limedl-engine/src/manager.rs:L368-L372]]

## Async paths never call `std::fs`

Async functions and async blocks must use `tokio::fs` (or `spawn_blocking` for a
genuinely synchronous chunk) rather than `std::fs`; SonarQube reports the latter
as `rust:S7493`. The engine's async paths follow that shape — bootstrap creates
the BT state/output directories through `tokio::fs`, the manager creates the
destination, and finalization renames a corrupt temp file through `tokio::fs`.
[[cite:crates/limedl-core/src/bootstrap.rs:L63-L66]]
[[cite:crates/limedl-engine/src/manager.rs:L451-L453]]
[[cite:crates/limedl-engine/src/http_executor/finalize.rs:L120-L175]]

The exception is genuinely synchronous flows: the self-update executable
extraction and `clean_update_work_dir` are synchronous and would otherwise drag a
runtime into code that does not have one, so they keep `std::fs`.
[[cite:crates/limedl-native/src/update/mod.rs:L578-L646]]
[[cite:crates/limedl-native/src/update/mod.rs:L664-L673]]

On Unix the extracted updater executable is written with mode `0o700`, not
`0o755`: it lives in the user's own work directory and is executed only by that
user, so group/other bits would be unnecessary surface on a file that replaces
the running process.
[[cite:crates/limedl-native/src/update/mod.rs:L633-L638]]
[[cite:crates/limedl-native/src/update/tests.rs:L175-L186]]

Related pages: [[wiki:workflows/http-download-lifecycle]],
[[wiki:systems/settings-and-configuration]],
[[wiki:systems/persistence-and-recovery]].
