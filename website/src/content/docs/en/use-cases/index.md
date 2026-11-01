---
title: Use Cases
description: Pick limedl by workflow — headless NAS downloads, cross-border mirror acceleration, or browser handoff
---

# Use Cases

One download engine, three very different workflows. Start with the scenario closest to yours, then follow that page.

| Scenario | Best for | Capabilities used | Start here |
| :--- | :--- | :--- | :--- |
| **NAS / soft-router downloads** | Synology, QNAP, TrueNAS, OpenWrt, Linux servers | `limedl-server`, Aria2 RPC, Docker, systemd, AriaNg | [Headless downloads](/en/use-cases/nas-offline-download/) |
| **Cross-border & mirror acceleration** | Frequent downloads from GitHub Releases, HuggingFace, Linux distributions | CDN probing, URL rewriting, Metalink mirrors, proxy | [Cross-border mirrors](/en/use-cases/cross-border-mirrors/) |
| **One-click browser handoff** | Clicking download links in a browser all day | Aria2 RPC + Aria2 Explorer/Camtd, header forwarding, interception rules | [Browser handoff](/en/use-cases/browser-handoff/) |

---

## Choosing

- **No desktop environment**: use the NAS/soft-router scenario with `limedl-server`.
- **Desktop app, but the source is overseas or the mirror is slow**: use the cross-border scenario and enable CDN probing or URL rewriting.
- **Mostly clicking downloads on web pages**: use browser handoff and let the extension push tasks to the local limedl.

The scenarios are not exclusive: a NAS running `limedl-server` can also serve browser extensions over the LAN, as long as you handle tokens and TLS.

---

## Common prerequisites

- Complete [installation](/en/getting-started/installation/) or [server deployment](/en/ecosystem/server-daemon/), and confirm the platform matches [Platform Support](/en/getting-started/platform-support/).
- For RPC, the default is loopback-only and disabled. Set a token before exposing it to the LAN, and use a TLS reverse proxy for internet access.
- Release authenticity is covered in [Security & Verification](/en/security/); server binaries are not part of the desktop self-update signing chain.
