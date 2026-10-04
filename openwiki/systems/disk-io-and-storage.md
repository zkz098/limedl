---
type: system
title: Disk I/O, Buffer Pool and Storage Detection
description: limedl's write path to disk — HDD double-buffering vs SSD write-combining, the IoWorker and slot lifecycle, preallocation and cross-device finalization, disk-space vs single-file-limit errors, per-platform media detection, and the directory override mechanism.
tags: [disk-io, buffer-pool, storage, filesystem, hdd, ssd]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T01:52:48.489Z
sources:
  - id: openwiki-source-f81695e53a2587711a61d13e
    resource: repo://crates/limedl-core/src/buffer_pool/download_buffer.rs
  - id: openwiki-source-6fac1289a4e6263e72a21817
    resource: repo://crates/limedl-core/src/buffer_pool/mod.rs
  - id: openwiki-source-042bbddb734c34bb6db90965
    resource: repo://crates/limedl-core/src/buffer_pool/worker.rs
  - id: openwiki-source-ba0579da85cf30927f31bfd3
    resource: repo://crates/limedl-core/src/file_ops/disk_detect.rs
  - id: openwiki-source-448f00bcf6e08ed002c79cdf
    resource: repo://crates/limedl-core/src/file_ops/media.rs
  - id: openwiki-source-3c484547210ce754a4755d21
    resource: repo://crates/limedl-core/src/file_ops/mod.rs
  - id: openwiki-source-164e9cc25784db5725389085
    resource: repo://crates/limedl-core/src/io_scheduler/mod.rs
  - id: openwiki-source-e63d7fe6266613d063307ac3
    resource: repo://crates/limedl-core/src/io_scheduler/queue.rs
  - id: openwiki-source-5482090b6666ff61df781e90
    resource: repo://crates/limedl-core/src/io_scheduler/topology.rs
generated: { by: "pi", at: "2026-10-04T01:52:48.489Z" }
---

# Disk I/O, Buffer Pool and Storage Detection

Downloads are write-heavy and their storage can be a spinning disk, flash, or a
network share. limedl adapts its write strategy to the media it detects, then
falls back to user overrides where detection cannot know the answer.

## BufferPool: slots and memory

`BufferPool` is a global pool for HDD buffering. It stores limit and parallelism
parameters in atomics, a `tokio::sync::Semaphore` of slots, a usage counter and an
active count. `half_size()` — the capacity of one buffer half — is
`effective_limit / effective_max_parallel / 2`, floored at 64 KiB so even tiny
configurations stay functional.

`SlotGuard` is the RAII permit: `acquire_slot()` awaits a permit and returns it;
dropping the guard returns it and decrements `active_count`. `release_slot()` is
called from `DownloadBuffer::Drop`. `degradation_count()` always returns 0 by
design: the double-buffer backpressures instead of degrading to direct I/O.

Evidence: `repo://crates/limedl-core/src/buffer_pool/mod.rs#L30-L51`,
`repo://crates/limedl-core/src/buffer_pool/mod.rs#L93-L125`,
`repo://crates/limedl-core/src/buffer_pool/mod.rs#L217-L223`.

Game mode swaps `effective_limit()` and `effective_max_parallel()` to their
reduced values. It does **not** revoke permits already held; only new
acquisitions see the lower limit. `update_limits` grows the semaphore when the
effective maximum increases and deliberately never shrinks it — existing permits
are safe to keep and return naturally. In practice game mode only affects the HDD
pool; SSD downloads keep their own buffering.

Evidence: `repo://crates/limedl-core/src/buffer_pool/mod.rs#L137-L199`.

## DownloadBuffer: HDD double-buffering and SSD write-combining

`DownloadBuffer` has three modes: a pool-backed HDD ping-pong, a local ping-pong
(SSD), and a plain write-combining buffer. The ping-pong core is
`buffer_chunk_pingpong_impl`, shared by HDD and SSD with a `PingPongCfg`; the
only difference is whether a global pool participates.

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
released by a guard even on panic. If a background flush failed, an error flag is
checked before every iteration and the call fails immediately rather than
downloading data destined for a failed write.

Evidence: `repo://crates/limedl-core/src/buffer_pool/download_buffer.rs#L245-L360`.

All flush requests go to a dedicated `IoWorker` thread pool (an unbounded
`mpsc` channel per worker), with `spawn_blocking` only as a fallback. HDD gets a
single serialized writer; SSD/Network get parallel channels, because a network
transport already reorders writes on the far side. `write_coalesced_entries`
merges adjacent writes before issuing them.

Evidence: `repo://crates/limedl-core/src/buffer_pool/worker.rs#L38-L140`.

## FileOps

### Open and preallocate

`open_download_file` creates parent directories, opens (or reuses) the file
without truncating, then reserves the full size. `reset_download_file` truncates
to zero and re-reserves. `preallocate_file`:

- on Windows first tries `SetFileValidData` (instantaneous, needs the volume
  privilege), then enables sparse mode, then calls `file.allocate`;
- falls back to `set_len` when `allocate` is unsupported (`EPERM`, `EINVAL`,
  `ENOSYS`, `ENOTSUP`/`EOPNOTSUPP` and the glibc `524` spelling).

Evidence: `repo://crates/limedl-core/src/file_ops/mod.rs#L16-L36`,
`repo://crates/limedl-core/src/file_ops/mod.rs#L621-L655`.

### Disk space vs single-file limit

`check_disk_space` rejects when available space is below `required * 1.1` (a 10 %
buffer). That check cannot catch a per-file size limit, because a volume with
terabytes free and a 4 GiB per-file cap (FAT32) passes it and only fails when the
file is extended.

`reservation_error` is where that is translated:

- `ErrorKind::FileTooLarge` (EFBIG / `ERROR_FILE_TOO_LARGE`) → `FileTooLarge`;
- `StorageFull` when the post-failure free space is still ≥ the file size → also
  `FileTooLarge` (some volumes answer "disk full" for an oversized reservation);
- `StorageFull` otherwise → `InsufficientDiskSpace`;
- anything else keeps its raw I/O error.

The rule is **do not guess a filesystem from an unrelated failure** — the old
"FAT32 can't hold 4 GiB" hint guessed from file size and sprayed noise on
NTFS/ext4/APFS. `preallocate_file` returns a raw `io::Result` precisely because
only the caller knows the destination directory (and thus the free space) needed
to make this decision; `reset_download_file`, which has only a `File`, decides by
`ErrorKind` alone.

Evidence: `repo://crates/limedl-core/src/file_ops/mod.rs#L553-L618`.

### Finalization

`finalize_temp_file(temp, dest)`:

1. If `dest` exists and has identical content, it is treated as an idempotent
   retry: clean staging leftovers and remove the temp file. Otherwise it errors.
2. Primary path: atomic `rename` (same filesystem, O(1)).
3. On `ErrorKind::CrossesDevices`: copy into a unique staging file **in the
   destination's directory** using a 1 MiB buffer, flush and `sync_all`, then
   `rename` staging → destination (same-volume atomic). On copy failure the
   staging file is removed and the source/temp file is preserved. A conflicting
   destination that matches is again accepted idempotently.

The invariant is that finalization never discards the only copy of the data
because one publish attempt failed.

Evidence: `repo://crates/limedl-core/src/file_ops/mod.rs#L38-L130`.

## Media detection

`detect_disk_type(path)` is implemented per platform:

- **Windows**: `CreateFileW(\\.\C:)` + `IOCTL_STORAGE_QUERY_PROPERTY` /
  `STORAGE_DEVICE_SEEK_PENALTY_PROPERTY`. UNC shares, device namespaces and
  verbatim UNC paths are reported `Network`; mapped drives are classified by what
  the letter points at (`DRIVE_REMOTE`).
- **Linux**: `/sys/block/<dev>/queue/rotational`, plus fstype classification for
  network/host-brokered mounts.
- **macOS**: IOKit `IOMedia` `Rotational`.
- **Unknown targets**: the fallback module returns `Ssd` (unknown ≠ remote).

**WSL** (`\\wsl$\<distro>` / `\\wsl.localhost\<distro>`) is deliberately *not*
network storage: it serves a local `ext4.vhdx`, so the code resolves the distro's
`BasePath` from the registry and detects the host volume that stores the VHDX.
A WSL distro is also listed in the settings UI so an override can reach it.

Evidence: `repo://crates/limedl-core/src/file_ops/disk_detect.rs#L465-L505`,
`repo://crates/limedl-core/src/file_ops/disk_detect.rs#L1115-L1125`.

`is_network_filesystem` is a *closed* set of remote and host-brokered transports
(NFS, CIFS/SMB, AFP, WebDAV, GlusterFS, 9p, virtiofs, drvfs, guest additions).
An unknown fstype — a brand-new local filesystem, or `fuse` such as ntfs-3g —
keeps the historical `Ssd` answer rather than being called remote.

Evidence: `repo://crates/limedl-core/src/file_ops/media.rs#L178-L205`.

## Directory overrides

`disk_type_overrides` maps user-entered **directories** to a forced `DiskType`.
Normalization (`normalize_media_path`) lowercases and converts separators on
Windows, strips the `\\?\` / `\\?\UNC\` prefixes, and strips trailing separators
while preserving a root. Override keys must be absolute (`\\server\share` counts
on Windows).

Matching is a **component-boundary prefix match, longest key first**:
`D:\downloads` covers `D:\downloads\a.bin` but not `D:\downloads-old`, and a
nested rule wins over its parent. `MediaOverrides` snapshots a pre-normalized,
longest-first list so device topology does not re-normalize the map per lookup;
`lookup_media_override` rebuilds it for the per-start buffer decision.

Overrides take effect in exactly two places: the buffer-mode decision at download
start (`DiskIoService::resolve_disk_type`) and device-queue media resolution.

Evidence: `repo://crates/limedl-core/src/file_ops/media.rs#L39-L61`,
`repo://crates/limedl-core/src/file_ops/media.rs#L95-L175`.

## Device queues and why overrides drop them

`DeviceQueue::new` fixes its writer-thread count at construction: 1 serialized
channel for HDD (to eliminate seek storms) and 4 parallel channels for
`Ssd`/`Network`. `DeviceTopology::resolve_device` makes overrides outrank
detection and caches per path.

Because the channel count is fixed, `DeviceTopology::set_overrides` clears its
resolution cache, and `DiskDeviceManager::set_overrides` additionally **drops the
entire queue map**. In-flight writers keep the `Arc` they already hold and finish
normally; the next lookup builds a queue matching the new policy. Without this,
a saved override needed an app restart before the write-thread count changed.
`SystemContext::with_components` seeds the topology with persisted overrides at
startup for the same reason.

Evidence: `repo://crates/limedl-core/src/io_scheduler/queue.rs#L52-L65`,
`repo://crates/limedl-core/src/io_scheduler/mod.rs#L53-L80`,
`repo://crates/limedl-core/src/io_scheduler/topology.rs#L78-L108`.

<!-- openwiki: broken internal link [/openwiki/workflows/http-download-lifecycle.md] link "/openwiki/workflows/http-download-lifecycle.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Related pages: [HTTP Download Lifecycle](/openwiki/workflows/http-download-lifecycle.md),
<!-- openwiki: broken internal link [/openwiki/systems/settings-and-configuration.md] link "/openwiki/systems/settings-and-configuration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Settings and Configuration](/openwiki/systems/settings-and-configuration.md),
<!-- openwiki: broken internal link [/openwiki/systems/persistence-and-recovery.md] link "/openwiki/systems/persistence-and-recovery.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[SQLite Persistence and Crash Recovery](/openwiki/systems/persistence-and-recovery.md).
