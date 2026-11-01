---
title: Benchmarks
description: What limedl's in-repo benchmarks measure, how to reproduce them, a result template, and common misreadings
---

# Benchmarks

limedl does not publish a cross-machine leaderboard. Download speed depends on the route, mirror, disk, and server-side throttling; startup time and memory vary by platform and build. A single number without its environment is easy to misread.

This page lists the benchmarks that actually exist in the repository, what each measures, how to reproduce it, and how to record comparable results.

---

## Measurement principles

- Compare only results from the **same machine, network, disk, and build**. Cross-machine comparisons are meaningless.
- Repeat network throughput at least 3 times and report the median; record ISP, target host, file size, and time of day.
- Always point disk benchmarks at the intended disk. Without `BENCH_DISK`, the benchmark writes to the system temp directory, which may be tmpfs/RAM and will look artificially fast.
- Benchmark **release builds**. The release profile uses `opt-level = "s"`, Thin LTO, `panic = "abort"`, and stripping; debug numbers do not represent shipped binaries.
- Startup and memory figures on the home page are architectural targets/historical observations, not a controlled benchmark. If you need to cite them, measure with the method below.

---

## Built-in benchmarks

All benchmarks use [divan](https://github.com/nvzqz/divan); sources live under `crates/*/benches/`.

| Benchmark | Command | What it measures | Real I/O |
| :--- | :--- | :--- | :--- |
| AIMD scheduler | `cargo bench -p limedl-engine --bench aimd` | Throughput sampling, thread increase/decrease, convergence, initial thread count; pure computation | No |
| Token bucket | `cargo bench -p limedl-http --features test-utils --bench rate_limiter` | Unlimited/limited consumption, `set_rate`, blocking consumption, multiple consumers | No |
| Disk buffer pool | `BENCH_DISK=/path/to/disk/dir cargo bench -p limedl-disk --features test-utils --bench buffer_pool` | 100 MiB in 1 MiB chunks: HDD double buffer, SSD local ping-pong buffer, direct write, 4-stream reverse writes | **Yes** |
| Local HTTP end-to-end | `cargo bench -p limedl-core --features test-utils --bench download_e2e` | A 1 MB file from the local TestServer: no delay, 10 ms, 50 ms first-byte delay, 10 MB/s, 5 MB/s caps | Loopback network |

### What each benchmark does not cover

- **AIMD / rate limiter** measure CPU cost only, not network or disk. They are regression tools, not download-speed claims.
- **Buffer pool** writes directly to `BENCH_DISK` and compares buffering strategies. It measures the buffer layer, not end-to-end download performance.
- **Download end-to-end** uses a local `TestServer` and a shared `reqwest` client under controlled delay/bandwidth. It exercises the HTTP client, not the full engine scheduler, and never touches the public internet.

---

## Reproducing

### 1. Prepare

```bash
# One-time: fetch the MiSans font (not committed to git)
cargo xtask fetch-font

# Release build; the first compile takes a while
cargo build --release -p limedl-native

# Record the exact revision
git describe --tags --always
```

Record CPU, memory, OS, filesystem, and network route. For disk tests, point `BENCH_DISK` at an empty directory on the target disk.

### 2. Run the built-in benchmarks

```bash
cargo bench -p limedl-engine --bench aimd
cargo bench -p limedl-http --features test-utils --bench rate_limiter
BENCH_DISK=/mnt/data/limedl-bench cargo bench -p limedl-disk --features test-utils --bench buffer_pool
cargo bench -p limedl-core --features test-utils --bench download_e2e
```

> On Windows: `$env:BENCH_DISK = "D:\bench"`.

### 3. Measure desktop startup and idle memory

For startup, use an OS-level tool (Windows Performance Recorder / ETW, macOS Instruments, Linux `perf`) or a manual stopwatch from process creation to an interactive main window. limedl enforces a single instance: fully quit the previous process before each run, or the second launch only focuses the existing window.

For idle memory, leave the window idle with no active tasks and wait 60 seconds before reading.

```powershell
# Windows
Get-Process limedl-native | Select-Object WorkingSet64, PrivateMemorySize64
```

```bash
# Linux / macOS
ps -o rss=,vsz= -p "$(pgrep -x limedl-native)"
```

### 4. Measure real network throughput

Pick a stable large file and run limedl and `curl` against the same URL in the same time window:

```bash
curl -L -o /dev/null -w 'avg %{speed_download} B/s\n' "https://example.com/large-file.iso"
```

Record file size, elapsed time, average and peak speed, whether CDN probing or URL rewriting applied, and whether a speed schedule was active. If CDN probing is on, also record the provider and selected edge IP.

---

## Result template

Copy this table into an issue or PR so comparisons use the same fields:

| Field | Value |
| :--- | :--- |
| limedl version / commit | |
| OS / kernel | |
| CPU / memory | |
| Disk model and filesystem | |
| Network / ISP / time window | |
| AIMD benchmark median | |
| Rate limiter median | |
| Buffer pool throughput (HDD / SSD / direct) | |
| Cold start to interactive window | |
| Idle RSS / private memory | |
| Real HTTP average (limedl / curl) | |

---

## Common misreadings

- **Treating loopback as internet speed**: `download_e2e` has no real RTT, loss, or CDN routing; it measures client overhead.
- **Running the buffer benchmark on tmpfs**: without real seeks and write amplification, the HDD double-buffer advantage disappears.
- **Running once**: network jitter makes single runs noisy; take at least 3 and use the median.
- **Mixing debug and release**: debug builds add scheduler and write overhead and cannot be compared with shipped binaries.
- **Assuming more threads is better**: AIMD searches for the concurrency the path can sustain; fixed high concurrency triggers 429/503 on weak links or anti-abuse servers.

---

## Related documentation

- [Tasks & Concurrency](/en/guides/tasks/): how AIMD, chunking, and tail sprint work.
- [Disk Buffer Pool](/en/advanced/buffer-pool/): HDD double buffering and SSD write combining.
- [CDN Acceleration](/en/guides/cdn/): edge IP screening and throughput probing.
- [Build & Contributing](/en/advanced/development/): release builds and running the test suite.
