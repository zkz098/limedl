---
title: 性能基准
description: limedl 仓库内基准测试的测量口径、复现命令、结果记录模板与常见误读
---

# 性能基准

limedl 不在网站上发布跨机器的“跑分排行榜”。下载速度取决于线路、镜像、磁盘与服务端限流，启动时间和内存又随平台与构建方式变化；脱离环境的单个数字很容易被误读。

本页说明仓库里实际存在哪些基准、每项测什么、如何复现，以及如何记录一组可比较的结果。

---

## 先明确测量口径

- **只比较同机、同网、同盘、同版本**的结果。跨机器比较没有意义。
- 网络吞吐至少重复 3 次，报告中位数；同时记录 ISP、目标站点、文件大小与时段。
- 磁盘基准必须显式指定目标盘。`BENCH_DISK` 未设置时会落到系统临时目录，可能位于 tmpfs/RAM，数字会虚高。
- 基准使用 **release 构建**。仓库的 release profile 启用了 `opt-level = "s"`、Thin LTO、`panic = "abort"` 与 strip，调试构建的数字不能代表发布版。
- 首页列出的启动时间与内存区间属于架构目标/历史观测，不是受控基准；需要引用时请用下面的方法自行测量。

---

## 仓库内置的基准

所有基准基于 [divan](https://github.com/nvzqz/divan)，源码在 `crates/*/benches/`。

| 基准 | 命令 | 测量内容 | 是否涉及真实 I/O |
| :--- | :--- | :--- | :--- |
| AIMD 调度 | `cargo bench -p limedl-engine --bench aimd` | 吞吐采样、线程增减、收敛过程、初始线程数；纯计算 | 否 |
| 令牌桶限速 | `cargo bench -p limedl-http --features test-utils --bench rate_limiter` | 不限速/限速消费、`set_rate`、阻塞消费、多消费者 | 否 |
| 磁盘缓冲池 | `BENCH_DISK=/path/to/disk/dir cargo bench -p limedl-disk --features test-utils --bench buffer_pool` | 100 MiB 数据、1 MiB 分块：HDD 双缓冲、SSD 本地乒乓缓冲、直接写入、4 路反向随机写 | **是** |
| 本地 HTTP 端到端 | `cargo bench -p limedl-core --features test-utils --bench download_e2e` | 本地 TestServer 上的 1 MB 文件：无延迟、10 ms、50 ms 首字节延迟、10 MB/s、5 MB/s 限速 | 回环网络 |

### 每项基准的边界

- **AIMD / 限速基准**只测调度与令牌桶的 CPU 开销，不包含网络与磁盘。它们适合做回归（例如某次改动让采样变慢），不适合对外宣称“下载速度”。
- **缓冲池基准**直接把数据写到 `BENCH_DISK` 指向的目录，比较不同缓冲策略的写入吞吐。它测的是缓冲层，不等于真实下载的端到端速度。
- **下载端到端基准**使用本地 `TestServer` 与共享 `reqwest` 客户端，测量的是 HTTP 客户端在受控延迟/带宽下的表现，不是完整引擎的调度路径，也不涉及公网。

---

## 复现步骤

### 1. 准备环境

```bash
# 首次构建前拉取 MiSans 字体（字体不入库）
cargo xtask fetch-font

# 使用 release 构建；首次编译时间较长
cargo build --release -p limedl-native

# 确认版本
git describe --tags --always
```

记录 CPU、内存、操作系统、文件系统与网络线路。涉及磁盘时，把 `BENCH_DISK` 指向待测盘上的空目录。

### 2. 运行内置基准

```bash
cargo bench -p limedl-engine --bench aimd
cargo bench -p limedl-http --features test-utils --bench rate_limiter
BENCH_DISK=/mnt/data/limedl-bench cargo bench -p limedl-disk --features test-utils --bench buffer_pool
cargo bench -p limedl-core --features test-utils --bench download_e2e
```

> Windows 下设置环境变量：`$env:BENCH_DISK = "D:\bench"`。

### 3. 测量桌面端启动与空闲内存

启动时间建议用系统级工具（Windows Performance Recorder / ETW、macOS Instruments、Linux `perf` 或手动计时），从进程创建记到主窗口可交互。注意 limedl 有单实例保护：每轮测量前必须完全退出上一个实例，否则第二次启动只会唤醒已有窗口。

空闲内存：启动后保持窗口不动、无活动任务，等待 60 秒再读数。

```powershell
# Windows
Get-Process limedl-native | Select-Object WorkingSet64, PrivateMemorySize64
```

```bash
# Linux / macOS
ps -o rss=,vsz= -p "$(pgrep -x limedl-native)"
```

### 4. 测量真实网络吞吐

选一个稳定的大文件，用同一 URL、同一时段分别跑 limedl 与 `curl` 作为对照：

```bash
curl -L -o /dev/null -w 'avg %{speed_download} B/s\n' "https://example.com/large-file.iso"
```

记录：文件大小、耗时、平均速度、峰值速度、是否触发 CDN 探针/URL 重写、是否开启限速计划。若使用 CDN 探针，额外记录所选 Provider 与边缘 IP。

---

## 结果记录模板

复制下表到 Issue 或 PR 中，保证每次对比的字段一致：

| 字段 | 值 |
| :--- | :--- |
| limedl 版本 / commit | |
| 操作系统 / 内核 | |
| CPU / 内存 | |
| 磁盘型号与文件系统 | |
| 网络线路 / ISP / 时段 | |
| AIMD 基准中位数 | |
| 限速基准中位数 | |
| 缓冲池吞吐（HDD / SSD / 直接写） | |
| 冷启动到窗口可交互 | |
| 空闲 RSS / 私有内存 | |
| 真实 HTTP 平均吞吐（limedl / curl） | |

---

## 常见误读

- **把本地回环测试当成公网速度**：`download_e2e` 没有真实 RTT、丢包与 CDN 路由，数字只反映客户端开销。
- **在 tmpfs 上测缓冲池**：没有真实磁盘寻道与写放大，HDD 双缓冲的优势不会出现。
- **只跑一次**：网络抖动会让单次结果偏差很大；至少 3 次取中位数。
- **混用 debug 与 release**：debug 构建的调度与写入开销明显更高，不能与发布版比较。
- **把线程数当成越高越好**：AIMD 的目标是找到链路可承受的并发，固定高并发在弱网或反滥用服务端会触发 429/503。

---

## 相关文档

- [任务管理与并发](/guides/tasks/)：AIMD、分块与尾部冲刺的工作方式。
- [磁盘缓冲池机制](/advanced/buffer-pool/)：HDD 双缓冲与 SSD 写合并。
- [CDN 探针加速](/guides/cdn/)：边缘 IP 筛选与吞吐测速。
- [源码编译与开发](/advanced/development/)：构建 release 版本与运行测试。
