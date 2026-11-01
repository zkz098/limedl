---
title: FAQ
description: limedl vs Aria2/Motrix, telemetry, SmartScreen, RPC security, data backup, platform limits, and upgrade paths
---

# Frequently Asked Questions

## How is limedl different from Aria2, Motrix, or Thunder?

limedl is not an Aria2 GUI shell or an Electron wrapper. It implements HTTP chunked downloads, AIMD adaptive concurrency, BitTorrent (librqbit), Metalink mirror scheduling, and a disk buffer pool in Rust, with a native Slint desktop UI. It is **compatible with** the Aria2 JSON-RPC 2.0 API, so existing frontends such as AriaNg, Aria2 Explorer, and Camtd can drive it — but the engine is a separate implementation.

Unlike commercial downloaders, limedl has no ads, paid speed tiers, or telemetry, and is licensed under GPL-3.0.

## Can it replace my existing Aria2 setup?

For most everyday workflows, yes. Enable the built-in Aria2 RPC service, point your frontend at `http://127.0.0.1:6800/jsonrpc`, and supply the token. The server routes the 37 methods commonly used by AriaNg and similar frontends, including `addUri`, `addTorrent`, `tellStatus`, `getGlobalStat`, queue operations, and `system.multicall`.

It is not a one-to-one clone of every Aria2 option: unrouted methods return method-not-found, and some Aria2-specific settings have no equivalent switch. Read the compatibility notes in [Aria2 RPC](/en/ecosystem/aria2-rpc/) before migrating.

## Does limedl collect telemetry or require an account?

No. The client contains no analytics, tracking, or telemetry reporting, and needs no account. The only network request unrelated to downloads is the update check against `latest-native.json` and its signatures on GitHub Releases; it follows the proxy configured in settings.

## Why does Windows show a SmartScreen or antivirus warning?

limedl signs releases with Minisign + ML-DSA-65 but does **not** use a commercial Windows Authenticode certificate, so SmartScreen may still say “unknown publisher”. That is not evidence of tampering, nor proof that any download is safe.

Compare the SHA-256 digest shown on the GitHub Release page and verify the `.sig` / `.pqc.sig` files as described in [Security & Verification](/en/security/). If that is not acceptable, build from source.

## macOS says “cannot verify the developer” or “the file is damaged”

macOS builds are ad-hoc signed and not notarized, so Gatekeeper blocks the first launch. Either:

- Control-click `limedl.app` in Finder, choose **Open**, then confirm **Open**;
- or run `xattr -cr /Applications/limedl.app` to remove the quarantine attribute.

See the [installation guide](/en/getting-started/installation/).

## Is there an Intel Mac, Windows ARM, or Linux ARM desktop build?

No. Desktop builds exist only for Windows x86_64, macOS Apple Silicon, and Linux x86_64 — no Intel Mac, native Windows ARM64, or Linux ARM64 desktop build.

For headless downloads on ARM hardware, use the aarch64 `limedl-server` build. See [Platform Support](/en/getting-started/platform-support/).

## Where are settings, task history, and torrent state stored?

Default data directories:

| Platform | Path |
| :--- | :--- |
| Windows | `%LOCALAPPDATA%\limedl\` |
| macOS | `~/Library/Application Support/limedl/` |
| Linux | `~/.local/share/limedl/` |

Set `LIMEDL_DATA_DIR` to override it. Quit the app before backing up, then copy the whole directory; copying only `settings.json` loses the task database and torrent state.

## How do I upgrade, roll back, or move to another machine?

- **Upgrade**: Setup and portable builds update in place; MSIX goes through Microsoft Store; for AppImage and `.deb`, download the new package or let the package manager handle it.
- **Roll back**: install an older release from GitHub Releases, but a newer version may already have migrated settings or the database. Back up the entire data directory first.
- **Migrate**: quit the app, copy the data directory, and update absolute paths such as `defaultDownloadDir` for the new machine; downloaded files must be copied separately.

## Is exposing the Aria2 RPC to LAN or the internet safe?

The defaults — RPC off and bound to `127.0.0.1:6800` — are the safest. `limedl-server` **refuses to start** when binding a non-loopback address without a token.

Do not expose port 6800 directly to the internet. Keep the loopback listener and reverse-proxy `/jsonrpc` through Caddy/Nginx with TLS; use per-client tokens and restrict CORS origins. See [headless server deployment](/en/ecosystem/server-daemon/) and [Aria2 RPC](/en/ecosystem/aria2-rpc/).

## A mirror returns 403 Forbidden. What should I do?

Some university and open-source mirrors validate the `User-Agent` as an anti-abuse measure. Set the default UA to a current stable Chrome UA, or to a legal `limedl/<version>` identifier. If your ISP's IPv4 egress is rate-limited, try IPv6 or another mirror.

More steps are in [Troubleshooting](/en/guides/troubleshooting/).

## BitTorrent speed is zero or no peers connect

First check whether the torrent still has seeders, then watch the DHT node count; it should climb to 100+ within a minute or two. If it stays at zero, allow limedl's UDP traffic through the firewall and enable UPnP port mapping on the router. For obscure torrents, add a public tracker list.

See [BitTorrent tuning](/en/guides/bittorrent/) and [Troubleshooting](/en/guides/troubleshooting/).

## Is HTTP/3 supported?

Yes. HTTP/3 Alt-Svc probing is enabled by default: hosts advertising Alt-Svc are tried over QUIC, and failures fall back to TCP. The switch is `scheduler.http3AltSvcEnabled` in `settings.json` (not in the settings dialog); make sure UDP 443 works locally.

## Can it download from cloud drives directly?

limedl does not implement cloud-drive account protocols or link parsing. You can dispatch tasks through Aria2 browser extensions or userscripts that forward headers such as `Cookie` and `Referer`; success depends on the script and the service. Follow the service's terms of use.

## Is limedl open source?

Yes, under GPL-3.0. Source, issues, and release archives live on [GitHub](https://github.com/zkz098/limedl). You can also [build from source](/en/advanced/development/).

---

## Still stuck?

- [Troubleshooting](/en/guides/troubleshooting/): 403s, BitTorrent, Gatekeeper, Linux dialogs and tray, log locations.
- [Security & Verification](/en/security/): signatures, public keys, and the update trust chain.
- [GitHub Issues](https://github.com/zkz098/limedl/issues): include your OS, limedl version, task type, and redacted logs.
