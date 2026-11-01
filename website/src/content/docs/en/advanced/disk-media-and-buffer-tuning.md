---
title: Disk Media Detection & Buffer Tuning
description: How limedl detects SSD/HDD/network storage (including WSL, VMs, and shares), directory overrides, and how to tune HDD double buffering and SSD write combining
---

# Disk Media Detection & Buffer Tuning

Download speed is not only a network problem: concurrent writes that land as random seeks on a mechanical disk can freeze the whole machine. limedl detects the target directory's media first, then chooses a write strategy; when detection is wrong, you can override it.

---

## How media is detected

| Platform | Detection |
| :--- | :--- |
| **Windows** | Storage device properties: seek penalty, bus type (NVMe/SCM/SD/MMC ⇒ SSD), TRIM; checked on both the volume and the physical drive behind it |
| **Linux** | Classifies network/host-brokered filesystems from `/proc/self/mountinfo` first, then reads `/sys/block/<dev>/queue/rotational` |
| **macOS** | `statfs` for the BSD device, then the IOKit `IOMedia` / `Rotational` property |

Results:

- **Network**: UNC shares, remote mapped drive letters, and NFS, SMB/CIFS, AFP, WebDAV, GlusterFS, Ceph, Lustre, 9p, virtiofs, and similar remote or host-brokered filesystems.
- **SSD**: NVMe, SCM, SD/MMC, TRIM support, or an unrecognized local filesystem.
- **HDD**: rotational media with a meaningful seek penalty.

:::note[Unknown is not remote]
An unrecognized filesystem falls back to the **SSD** strategy rather than being treated as network storage. This avoids misclassifying a new local filesystem as high-latency storage; use a directory override if the guess is wrong.
:::

### WSL

`\\wsl$\<distro>` and `\\wsl.localhost\<distro>` are **not** treated as network storage: the WSL 9p/virtiofs server serves a local `ext4.vhdx`. limedl resolves the distro's `BasePath` from the `HKCU\…\Lxss` registry key and detects the host volume that stores the VHDX; it falls back to Network only when the registry entry is missing. The result is cached per distro.

### VMs and network shares

- Virtual disks may be detected as SSD or unknown; use an override when the strategy does not match reality.
- SMB/NFS download directories are scheduled with the **SSD/Network write-combining** path, not the HDD double buffer, to avoid random-write amplification on network filesystems.

---

## Directory media overrides

Where: **Settings → IO Labs → Directory Media Override**.

- Keys must be **absolute paths**; Windows accepts `\\server\share`.
- Paths are normalized: trimmed, lowercased on Windows, separators unified, `\\?\` prefixes and trailing separators removed.
- Matching is a **component-boundary prefix match, longest key first**:
  - `D:\downloads` covers `D:\downloads\a.bin` but not `D:\downloads-old`;
  - a nested rule beats its parent.
- Overrides apply at the next download start and rebuild the device queue; **in-flight writers** finish on the old queue.

---

## Buffer strategy

| Media | Strategy | Defaults |
| :--- | :--- | :--- |
| **HDD** | Global pool + double buffer; a single background writer flushes sequentially | Pool `1024 MiB`, `4` parallel slots, double buffering on |
| **SSD / Network** | Local ping-pong buffer + write combining | Combine limit `0` (auto, per chunk), max `4096 MiB` |
| **Game mode** | Reduces disk I/O interference | Buffer `128 MiB`, `1` slot |

- **Game Mode / Overclock Mode** are runtime switches and are not written to `settings.json`.
- Game Mode only affects the HDD pool; SSD/Network keep their local limits.
- The HDD background path syncs adaptively by threshold, while the SSD path avoids forced sync to reduce flash write amplification.

---

## Tuning advice

### Mechanical disks

- Keep **double buffering on**; with many concurrent downloads, raise the pool limit/slots instead of unbounded task concurrency.
- If memory is tight, lower the pool limit; an oversized buffer moves pressure to RAM and swap.
- Multiple HDD tasks writing to the same disk compete for sequential order; limit parallel tasks with the scheduler when needed.

### SSDs

- Leaving the combine limit at `0` (auto) is usually best; a large manual value only increases memory use.
- Enable Game Mode while gaming or streaming to reduce background flush interference.
- Watch free space and TRIM; a nearly full drive loses write performance quickly.

### Network shares / WSL / VMs

- Network shares run the SSD strategy; do not expect HDD double-buffer sequential-write gains.
- If a WSL download directory is detected as Network, check the registry mapping or add a directory override.
- If a VM disk is misclassified, force it to SSD/HDD with an override.

---

## Validation and benchmarks

- Use [Benchmarks](/en/advanced/benchmarks/) with `BENCH_DISK` pointing at the real target disk to compare HDD double buffering, SSD local buffering, and direct writes.
- Never run disk benchmarks on tmpfs/RAM, where seek and write-amplification differences disappear.
- After changing overrides or buffer settings, measure the same file at the same time of day and take the median.

---

## Related documentation

- [Disk Buffer Pool](/en/advanced/buffer-pool/): implementation of double buffering and write combining.
- [Configuration](/en/getting-started/configuration/): buffer and Labs settings.
- [Benchmarks](/en/advanced/benchmarks/): reproducible disk benchmark commands.
- [Platform Support](/en/getting-started/platform-support/): Linux glibc/CPU and network-share limits.
