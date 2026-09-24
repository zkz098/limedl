---
title: Disk Buffer Pool Architecture
description: Learn how limedl protects storage hardware with SSD write-combining and HDD double-buffering
---

# Disk Buffer Pool Architecture

High-speed downloads over gigabit connections frequently overwhelm local storage with random I/O thrashing. limedl implements a specialized **Buffer Pool** that adapts to your storage drive type.

## SSD: Write Combining

- **Minimizing Write Amplification**: SSD flash memory operates in larger erase blocks; tiny random writes accelerate drive wear.
- **Batched Flushes**: Writes are aggregated into optimal memory chunks (1MB–4MB) before being committed sequentially, maximizing flash durability and throughput.

## HDD: Double Buffering

- **Eliminating Head Thrashing**: Mechanical hard drives suffer severe latency when multiple threads write non-sequential chunks concurrently.
- **Ordered Sequential Writes**: A producer-consumer double buffer queues downloads in RAM and writes to spinning disks in ascending offset order, preventing drive bottlenecks.

![Disk Buffer Pool Settings Screenshot](../../../../assets/settings-buffer-pool.png)
