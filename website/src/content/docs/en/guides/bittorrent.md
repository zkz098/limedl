---
title: BitTorrent Tuning
description: Deep dive into the embedded Irontide BitTorrent engine, supported BEP protocol standards, magnet resolution, and NAT traversal
---

# BitTorrent Protocol & Tuning

limedl deeply integrates a modern, pure Rust BitTorrent engine (**Irontide**), offering exhaustive compliance with P2P protocol specifications while ensuring maximum throughput, minimal RAM overhead, and reliable NAT traversal.

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
| **UPnP / NAT-PMP** | Auto Port Forwarding | Automatically requests external inbound port mapping on compatible home routers, transforming NAT clients into fully connectable peers. |
| **MSE / PE** | Stream Encryption | Negotiates stream encryption to thwart ISP deep packet inspection (DPI) and traffic throttling. |

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
- limedl automatically registers port mapping (default port `6881`). Being connectable from the public internet drastically expands your available peer pool.

### 2. Maintain a High-Quality Public Tracker List
While DHT works autonomously, providing healthy public trackers accelerates the discovery of seeds for rare or obscure torrents.

In **Settings -> BitTorrent -> Public Trackers**, paste curated tracker URLs (one per line). limedl automatically injects these trackers into every new torrent and magnet task.

### 3. Upload Throttling & Seeding Ratios
On asymmetric consumer broadband connections (such as fiber with asymmetric upload), maxing out your upload bandwidth will choke TCP ACK packets, causing downstream download speeds to plummet and increasing web browsing latency.

- **Upload Speed Limit**: In **Settings -> Speed Limits**, cap the global upload limit at **70% ~ 80%** of your line's maximum physical upload rate.
- **Seeding Ratio**: Default is `1.0`. Once uploaded bytes equal downloaded bytes, seeding concludes automatically, striking a balance between community sharing and resource conservation.

---

## Anti-Leech & Corrupted Block Safeguards

limedl incorporates defensive algorithms against malicious or defective peers:
- **Client Anomaly Detection**: Detects spoofed client IDs and peers that intentionally discard upload requests.
- **Circuit-Breaking on Bad Hashes**: If a peer repeatedly delivers data chunks that fail hash verification, the engine drops the connection and temporarily blacklists the peer to conserve downstream bandwidth.
