---
title: Cross-Border & Mirror Acceleration
description: Combine CDN edge probing, URL rewriting, Metalink mirrors, and proxies to speed up GitHub Releases, HuggingFace, and Linux distribution downloads
---

# Cross-Border & Mirror Acceleration

## Who this is for

- You frequently download from GitHub Releases, HuggingFace, or Linux distribution mirrors;
- The link opens in a browser, but the download crawls or keeps dropping;
- You do not want an opaque third-party “accelerator” and prefer to control mirrors and verification yourself.

This scenario combines four capabilities: **CDN probing**, **URL rewriting**, **Metalink mirrors**, and **proxies**. They address different layers and can be used separately or together.

---

## Step 1: Find the bottleneck

1. Measure the same URL with `curl` to establish a baseline;
2. Decide whether the delay is **first-byte**, **throughput throughout**, or **periodic stalls**;
3. Identify which CDN serves the host (look at `server`, `cf-ray`, or `x-served-by` response headers).

If `curl` is also slow, the problem is usually the route, DNS, or the mirror — not limedl's concurrency scheduler.

---

## Capability 1: CDN edge probing

Use when the resource is served from Cloudflare, Fastly, or a custom CDN network you specify.

How it works:

1. Load the provider's IP CIDR list;
2. Screen candidates with concurrent TCP/TLS handshakes;
3. Measure real throughput over small chunks;
4. Install the best IP into the transport resolver while still sending the original TLS SNI and `Host` header.

Where: **Settings → Labs → CDN Network Optimization**.

Caveats:

- A probe consumes roughly **20–50 MB** of traffic;
- It only applies to domains inside the selected provider's CIDRs; everything else uses normal DNS;
- If the best candidate is not faster than the default route, limedl keeps default routing instead of applying the probe result;
- Switching providers requires a fresh probe; the old node is not migrated;
- If the default DNS resolves to a private IP (for example a TUN/VPN takeover), the probe aborts with a conflict warning.

See [CDN Acceleration](/en/guides/cdn/).

---

## Capability 2: URL rewriting

Use when the resource is outside the selected CDN but has a reliable public mirror.

Add a rule under **Settings → Labs → URL Rewrite**. The classic GitHub Releases rule:

```regex
^https://github\.com/([^/]+)/([^/]+)/releases/download/(.+)$
```

Replacement:

```text
https://mirror.example.com/https://github.com/$1/$2/releases/download/$3
```

- Rules support host, prefix, regex, and wildcard matching;
- You can configure prefix-proxy/template replacement, URL encoding, and fallback (rules have no header fields);
- Rewriting happens before dispatch, so the original URL, file name, and checksum metadata should be preserved.

**Verify that the mirror content matches the origin.** Mirrors may cache stale versions, return HTML error pages, or inject content; prefer tasks with checksums, or run an integrity check after download.

---

## Capability 3: Metalink mirrors

Use when the resource publishes multiple mirrors (`.metalink` / `.meta4`, or a Metalink response header).

limedl supports RFC 5854 / RFC 6249 and will:

- Score mirrors by priority and RTT;
- Lease the best mirror with a connection cap;
- Isolate and down-rank mirrors that return corrupted pieces;
- Verify Blake3 / SHA-256 / SHA-512 checksums declared by the Metalink, per piece or overall.

Drag a `.metalink` / `.meta4` file into the window, or submit it through Aria2 RPC `addMetalink`.

See [Tasks & Metalink](/en/guides/tasks/).

---

## Capability 4: Proxies

If the bottleneck is the local network egress rather than CDN routing:

| Proxy mode | Behavior |
| :--- | :--- |
| Direct | No proxy |
| System | Reads the OS static proxy settings (Windows Internet Options / macOS Network settings); **PAC/WPAD is not supported** |
| Manual | A full URL such as `socks5://127.0.0.1:1080` |

The proxy also applies to update checks; their User-Agent is fixed to `limedl-native/<version>`.

---

## Recommended combination

1. **Measure a baseline** with `curl`;
2. **Cloudflare/Fastly target**: enable CDN probing and apply the fastest node;
3. **Outside those CIDRs but a reliable mirror exists**: add a URL rewrite rule and keep checksums enabled;
4. **Metalink available**: prefer it, so mirror selection and verification are automatic;
5. **Still slow**: check the proxy, IPv6, ISP egress, and mirror 403s (User-Agent anti-abuse).

---

## Checklist

- [ ] Recorded a `curl` baseline for the same URL.
- [ ] CDN probing only applies to domains inside the selected provider's CIDRs.
- [ ] URL rewrite mirrors match the origin, and integrity checking is enabled.
- [ ] Metalink hashes use a supported algorithm (Blake3 / SHA-256 / SHA-512).
- [ ] Proxy mode and address are correct; system proxy does not support PAC.
- [ ] For mirror 403s, adjust the User-Agent before switching mirrors.

---

## Related documentation

- [CDN Acceleration](/en/guides/cdn/): state machine, providers, and DNS rewriting.
- [Tasks & Concurrency](/en/guides/tasks/): AIMD, Metalink, and integrity checks.
- [Configuration](/en/getting-started/configuration/): network, proxy, and Labs settings.
- [Troubleshooting](/en/guides/troubleshooting/): 403s, IPv4/IPv6, and logs.
