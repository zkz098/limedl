---
title: Disk Buffer Pool
description: "Deep dive into limedl media-aware disk buffering: SSD write combining and HDD double buffering for physical storage protection"
---

# Disk Buffer Pool

On high-speed connections (1 Gbps to 10 Gbps), download managers receive hundreds of thousands of network packets per second. Naive download managers issue small system write calls on incoming chunks, pushing disk activity to 100%, causing UI freezing, and degrading SSD flash memory cells.

limedl incorporates a **Media-Aware Adaptive Buffer Pool** that tailors its I/O scheduling to the underlying storage hardware.

---

## The Physical Media Dilemma

```
┌─────────────────────────────────────────────────────────────┐
│ Solid-State Drives (SSD / NVMe)                             │
│ • Zero mechanical seek latency; fast random access          │
│ • Hardware constraint: Large erase blocks (4MB to 16MB)     │
│ • Bottleneck: Small random writes cause extreme Write       │
│   Amplification Factor (WAF), accelerating cell degradation │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ Hard Disk Drives (HDD)                                      │
│ • Sustained sequential read/write speeds up to 150-250 MB/s │
│ • Hardware constraint: Mechanical seek latency (5ms-15ms)   │
│ • Bottleneck: Concurrent multi-threaded writes cause head   │
│   thrashing, queue congestion, and complete system lockups  │
└─────────────────────────────────────────────────────────────┘
```

To address both architectures, limedl switches dynamically between two dedicated I/O engines:

---

## Intelligent Storage Detection

Before creating files on disk, limedl queries the physical storage subsystem:

- **Windows**:
  The engine opens a volume handle via `CreateFileW` and issues a `DeviceIoControl` call with `IOCTL_STORAGE_QUERY_PROPERTY` to check `STORAGE_DEVICE_SEEK_PENALTY_PROPERTY`:
  - If a seek penalty is reported, the drive is treated as a mechanical **HDD**.
  - If no seek penalty is reported, the drive is treated as an **SSD**.
- **Network Locations (`DiskType::Network`)**:
  UNC shares, mapped network drives (`DRIVE_REMOTE`) and network or host-brokered filesystems on Linux/macOS (NFS, CIFS, 9p, virtiofs, …) have no local block device to probe — they used to be silently reported as SSDs. They are now marked as **network share, media unknown**, scheduled like an SSD (the transport, not the platter, is the bottleneck — the far side reorders writes anyway) and listed in **Settings → IO Lab**, which is exactly the set of locations that need a manual override.
- **WSL distributions (`\\wsl$\<distro>` / `\\wsl.localhost\<distro>`)**:
  Not network storage. The 9p/virtiofs server behind that UNC transport serves a **local** `ext4.vhdx`, so limedl looks the distro up by name under `HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss`, takes its `BasePath`, and runs the ordinary local probe on `<BasePath>\ext4.vhdx` (falling back to `BasePath` itself for `wsl --import --vhd` layouts and WSL1 distros). The answer is therefore the **real media of the host volume**. A drive mapped to `\\wsl$\Ubuntu` is resolved back to that UNC target first; only a distro that cannot be resolved falls back to "network".
- **macOS / Linux**:
  Local disks are probed through `/sys/block/<dev>/queue/rotational` (Linux) or the IOKit `IOMedia` `Rotational` property (macOS); network mounts are classified by filesystem type, and everything else keeps SSD mode.
- **Manual Overrides (`disk_type_overrides`)**:
  Under **Settings → IO Lab → Directory Media Overrides**, users can pin a directory (an SMB share, a WSL path, a virtual disk) to SSD or HDD mode. Keys are directories and matching is a **component-boundary prefix match** over normalized paths (longest key wins): `D:\Downloads` covers everything inside it without catching `D:\Downloads-old`, and on Windows case and trailing separators do not affect the match. Every row shows the media auto-detection currently reports for that path.

![Buffer Pool Settings Screenshot](../../../../assets/settings-buffer-pool.png)

---

## SSD Mode: Write Combining

For flash storage, limedl stages incoming chunks in an in-memory red-black tree (`BTreeMap<u64, Bytes>`):

1. **In-Memory Ordering**: Incoming chunks are staged in memory sorted by file offset.
2. **Block Alignment & Aggregation**: When staged chunks reach optimal alignment boundaries (e.g. 1MB to 4MB) or become contiguous, the engine merges them into a single bulk buffer.
3. **Asynchronous Batch Flush**: The aggregated buffer is handed to a dedicated I/O worker thread for an aligned sequential write.

**Benefits**:
- Minimizes random write operations, driving down the **Write Amplification Factor (WAF)**.
- Prevents SLC cache exhaustion, sustaining peak NVMe write speeds during multi-gigabit transfers.

---

## HDD Mode: Double Buffering (Ping-Pong Buffer)

To protect mechanical disk heads from thrashing, limedl uses a classic producer-consumer double-buffering architecture:

```
Concurrent Network Workers
         │
         ▼ Stream incoming data
┌────────────────────────────────┐
│      Active Half-Buffer        │  ◄── Limit reached OR 2s timer triggers
└────────────────┬───────────────┘
                 │
                 ▼ Atomic Ping-Pong Pointer Swap
┌────────────────────────────────┐
│      Flushing Half-Buffer      │
└────────────────┬───────────────┘
                 │
                 ▼ Strictly sorted by physical file offset
┌────────────────────────────────┐
│ Dedicated Single IoWorker      │
└────────────────┬───────────────┘
                 │
                 ▼ Smooth sequential write to magnetic platter
          [Zero head thrashing; quiet, smooth disk operation]
```

1. **Dual Symmetric Half-Buffers**: The pool maintains two symmetrical buffers. Network workers write exclusively into the Active buffer.
2. **Ping-Pong Pointer Swap**: When the active half-buffer reaches its calculated capacity threshold (minimum 64 KiB) or a 2-second timer elapses, the buffers swap roles atomically.
3. **Sequential Ordered Flushing**: The Flushing buffer is processed by a dedicated single-threaded `IoWorker` that writes data **in strictly ascending physical file offset order**.

**Benefits**:
- Magnetic heads move smoothly in one direction across cylinders, eliminating the grinding noise caused by multi-thread seek thrashing.
- Sustains 180MB/s+ sequential writes on mechanical drives without triggering 100% disk utilization lockups.

---

## File Pre-Allocation & Atomic Finalization

Low-level filesystem operations are equally crucial for performance and reliability:

### 1. High-Speed Space Pre-Allocation
Before receiving data, limedl verifies that free space exceeds the target file size plus a 10% safety margin, then pre-allocates contiguous clusters:
- **Linux**: Calls `fallocate()`.
- **Windows**: Calls `SetFileValidData` or `SetEndOfFile`.

Pre-allocation eliminates filesystem fragmentation and guarantees unfragmented contiguous sectors.

### 2. Atomic Finalization
- All downloads write to a temporary file (`.tmp`).
- Only when all chunks are verified against their hash digest does the engine atomically rename (`rename`) the file to its destination path.
- For cross-volume transfers, limedl uses a 256KB streaming buffer (compared to the standard 8KB runtime buffer) to stream the completed file safely.
