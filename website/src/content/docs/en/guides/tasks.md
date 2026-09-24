---
title: Tasks & Concurrency Management
description: Understanding chunked parallel downloading and AIMD adaptive concurrency in limedl
---

# Tasks & Concurrency Management

limedl replaces rigid fixed-thread concurrency (like static 16/32 threads) with an intelligent **AIMD (Additive Increase Multiplicative Decrease)** adaptive concurrency controller.

## Adding Downloads

You can add download tasks in several convenient ways:
1. **Clipboard Detection**: Copy any HTTP/HTTPS URL or magnet link, and limedl will automatically detect it when focused.
2. **New Task Dialog**: Click the `+ New` button in the top toolbar.
3. **Drag and Drop**: Drag any `.torrent` file directly into the application window.
4. **Browser Integration**: With the Aria2 browser extension installed, clicking downloads directly intercepts and hands them to limedl.

## Adaptive Concurrency (AIMD)

- **Slow Start**: Begins with 2–4 workers while probing server capabilities (`Accept-Ranges`, response latency).
- **Additive Increase**: Gradually ramps up worker connections as long as chunks complete smoothly without latency spikes.
- **Multiplicative Decrease**: Instantly dials down concurrency if the remote server signals rate limiting (HTTP 429/503) or packet drops, avoiding aggressive IP bans.
- **Mirror Failover**: If multiple mirror URLs are available, failed chunks automatically retry on the fastest fallback mirror.
