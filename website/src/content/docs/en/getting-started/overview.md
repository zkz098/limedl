---
title: Overview
description: Learn about limedl design principles, architecture, and core advantages
---

# About limedl

**limedl** is a fast, ultra-lightweight, multi-protocol download manager built for high performance and responsiveness. Powered by a pure **Rust** core engine with a native **Slint** desktop interface.

It features **HTTP/HTTPS chunked parallel downloading with adaptive concurrency (AIMD)**, full-featured **BitTorrent (magnet links / .torrent)** support, built-in **CDN acceleration via smart IP probing**, and 100% compatibility with the **Aria2 JSON-RPC 2.0** protocol.

## Core Design Philosophy

- **Say Goodbye to Bloatware**: No 300MB+ Electron memory overhead. limedl uses Slint native rendering, launching in milliseconds with an idle footprint of just ~20MB.
- **Adaptive Concurrency (AIMD)**: Instead of flooding the connection with fixed thread counts, limedl dynamically adjusts concurrency using TCP-inspired AIMD algorithms to maximize throughput while avoiding packet loss.
- **Disk-Friendly Architecture**: Smart detection of storage devices (SSD vs. HDD). SSDs benefit from write-combining to reduce wear, while HDDs use double-buffering and file pre-allocation to eliminate seek thrashing.
- **Ecosystem Compatibility**: Works out of the box with existing Aria2 extensions, browser download interceptors, and user scripts.

## Architecture

```
limedl Workspace
├── crates/limedl-core/    # Pure Rust download engine (headless, no UI dependencies)
│   ├── DownloadManager    # HTTP/HTTPS chunked adaptive transfers
│   ├── IrontideBtBackend  # BitTorrent engine (DHT, PEX, UPnP)
│   ├── CdnAccelerator     # Cloudflare IP probing & optimization
│   ├── BufferPool         # SSD write-combining & HDD double-buffering
│   └── Aria2RpcServer     # Aria2 JSON-RPC 2.0 compatible server
└── crates/limedl-native/  # Native Slint desktop UI (Windows / macOS / Linux)
```
