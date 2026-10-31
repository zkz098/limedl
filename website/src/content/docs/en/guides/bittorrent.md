---
title: BitTorrent Tuning
description: Deep dive into the embedded librqbit BitTorrent engine, supported BEP protocol standards, magnet resolution, and NAT traversal
---

# BitTorrent Protocol & Tuning

limedl deeply integrates a modern, pure Rust BitTorrent engine (**librqbit**), offering exhaustive compliance with P2P protocol specifications while ensuring maximum throughput, minimal RAM overhead, and reliable NAT traversal.

---

## Supported BEP Protocols

limedl strictly follows the official BitTorrent Enhancement Proposals (BEP):

| Standard | Specification | Key Capabilities |
| :--- | :--- | :--- |
| **BEP 03** | Core BitTorrent Protocol | Core peer-to-peer chunk transfers, Choke/Unchoke handshake state machine, and Bitfield management. |
| **BEP 05** | Mainline DHT | Kademlia-based decentralized routing table. Resolves peers across the global swarm even when all central trackers are down. |
| **BEP 09** | Metadata Transfer (`ut_metadata`) | Enables instant Magnet link (`magnet:?xt=urn:btih:...`) resolution. Downloads the torrent dictionary directly from peers before fetching file payloads. |
| **BEP 10** | Extension Protocol | Dynamic negotiation of advanced peer capabilities (such as metadata exchange and PEX). |
| **BEP 11** | Peer Exchange (PEX) | Directly exchanges lists of known active peers among participants in a swarm, significantly accelerating discovery. |
| **BEP 14** | Local Peer Discovery (LSD) | Multicast discovery across the local subnet. Peers connected to the same switch or router transfer data at full gigabit/10GbE local speeds. |
| **BEP 15** | UDP Tracker Protocol | Compact, low-overhead UDP communication with trackers to prevent socket exhaustion. |
| **UPnP** | Auto Port Forwarding | Requests external inbound port mappings on UPnP-capable routers; enable it in Settings (off by default). |

---

## Fast Magnet Link Resolution Pipeline

When you paste a magnet link into limedl, the background engine progresses through four streamlined stages:

```
┌────────────────┐      Extract InfoHash    ┌────────────────┐
│ 1. Magnet Parse│ ───────────────────────► │ 40-char Hex Hash│
└───────┬────────┘                          └───────┬────────┘
        │                                           │
        ▼                                           ▼
┌────────────────┐      Kademlia Lookup     ┌────────────────┐
│ 2. DHT Query   │ ───────────────────────► │ Candidate Peers │
└───────┬────────┘                          └───────┬────────┘
        │                                           │
        ▼                                           ▼
┌────────────────┐      ut_metadata Stream  ┌────────────────┐
│ 3. Fetch Info  │ ───────────────────────► │ Parse Torrent   │
└───────┬────────┘                          └───────┬────────┘
        │                                           │
        ▼                                           ▼
┌────────────────┐      Selective Files     ┌────────────────┐
│ 4. File Dialog │ ───────────────────────► │ Begin Download │
└────────────────┘                          └────────────────┘
```

1. **Instant Parsing**: Extracts the InfoHash and optional announce trackers (`tr`) from the URI.
2. **DHT Swarm Discovery**: Sends `get_peers` queries to active Kademlia nodes in the local routing table.
3. **Metadata Retrieval**: Negotiates `ut_metadata` messages with the first available peers, downloading the metadata dictionary in parallel chunks.
4. **Selective File Picking**: Once metadata is parsed, a dialog presents the complete file tree. Unchecked files consume zero disk space.

---

## Network Tuning & Optimization Guide

Network connectability is the single biggest determinant of BitTorrent performance.

### 1. Enable Router UPnP
- Access your router's admin panel (typically `192.168.1.1` or `192.168.0.1`).
- Under "Advanced" or "NAT Forwarding", ensure **UPnP** is toggled **On**.
- With UPnP enabled in Settings, limedl requests port mappings at startup. Leave the listen port empty for an OS-assigned port, or pin a value such as 6881. Inbound connectivity from the public internet drastically expands the peer pool.

### 2. Maintain a High-Quality Public Tracker List
While DHT works autonomously, providing healthy public trackers accelerates the discovery of seeds for rare or obscure torrents.

In **Settings -> BitTorrent -> Public tracker list URL**, provide a subscription URL serving a plain-text tracker list (one announce URL per line). limedl fetches the list and injects those trackers into every new torrent and magnet task; a maintained list URL is built in by default.

### 3. Upload Throttling & Seeding Ratios
On asymmetric consumer broadband connections (such as fiber with asymmetric upload), maxing out your upload bandwidth will choke TCP ACK packets, causing downstream download speeds to plummet and increasing web browsing latency.

- **Upload Speed Limit**: In **Settings -> Speed Limits**, cap the global upload limit at **70% ~ 80%** of your line's maximum physical upload rate.
- **Seeding Ratio**: Defaults to `0` (unlimited). Set `1.0` to stop seeding once uploaded bytes equal the downloaded amount; 1.0–2.0 is a typical range.

### 4. Enable Lightweight BT Mode for Idle Efficiency (`bt.lightweightMode`)
For users who primarily download HTTP/HTTPS resources, keeping the DHT routing table and listen sockets active consumes background memory.
Enable **Lightweight BT Mode** under **Settings -> BitTorrent**:
- The librqbit engine will not pre-warm on application launch;
- Once all torrents are paused or completed with no active seeding, the engine automatically unloads its session from memory;
- Torrent records remain visible and resumable via the SQLite `bt_tasks` cache;
- Submitting a new magnet or resuming a task immediately awakens the engine within milliseconds.

---

## Anti-Leech Safeguards

limedl includes defensive handling for abusive peers:
- **Leech Detection**: Uses a grace period, a give-back ratio, and upload/download rates to flag peers that only download, or that hold upload slots without reciprocating.
- **Throttle or Ban**: Configure it to reduce the peer's upload slots or ban it temporarily (auto-unban after 3600 seconds by default). Data integrity itself is protected by BT piece hashes.
