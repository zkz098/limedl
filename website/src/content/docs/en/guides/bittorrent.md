---
title: BitTorrent Engine & Tuning
description: Understanding the Irontide BitTorrent backend and network traversal
---

# BitTorrent Engine & Tuning

limedl embeds a modern, pure-Rust BitTorrent engine (**Irontide**) with comprehensive P2P protocol capabilities.

## Protocol Support

- **Distributed Hash Table (DHT)**: Full Mainline DHT support for trackerless magnet downloads and peer discovery.
- **Peer Exchange (PEX)**: Dynamically expands the peer swarm via peer-to-peer announcements.
- **Local Service Discovery (LSD)**: Discovers peers on your local network (LAN) for ultra-fast direct transfers.
- **UPnP / NAT-PMP Port Mapping**: Automatically configures router port forwarding to establish inbound connections.

## Tuning Best Practices

1. **Router UPnP**: Ensure UPnP is enabled on your router so limedl can map inbound listening ports automatically.
2. **Public Trackers**: Adding active public tracker lists can speed up swarm discovery for rarer torrents.
3. **Upload Rate Limits**: Set an upload limit in **Settings -> BitTorrent** that matches your ISP's upstream bandwidth.
