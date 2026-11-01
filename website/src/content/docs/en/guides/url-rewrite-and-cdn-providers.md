---
title: URL Rewrite & Custom CDN
description: Configure limedl URL rewrite rules (Host/prefix/regex/wildcard, PrefixProxy/Template) and Cloudflare, Fastly, or custom CDN providers
---

# URL Rewrite & Custom CDN

limedl offers two ways to take a faster route:

- **URL rewriting**: turn the original link into mirror candidates before dispatch;
- **CDN probing**: choose a faster edge IP inside Cloudflare, Fastly, or a custom CDN network.

They work independently or together. This page documents the actual configuration fields and matching behavior.

---

## URL rewriting

Where: **Settings → Labs → URL Rewrite**.

### Rule structure

| Field | Meaning |
| :--- | :--- |
| **Enabled** | Disabled rules never match |
| **Match type** | `Host`, `Prefix`, `Regex`, `Wildcard` |
| **Pattern** | The string interpreted by the match type |
| **Replacement mode** | `Prefix proxy (PrefixProxy)` or `Template` |
| **Targets** | Each target has a URL template, enabled flag, and order |
| **URL encoding** | Percent-encode the original URL before inserting it |
| **Fallback to original** | On by default; appends the original URL after all targets |
| **Order** | Lower first; **the first matching rule wins** and later rules are not evaluated |

### Match types

| Type | Behavior | Example |
| :--- | :--- | :--- |
| `Host` | Compares the host; `*.example.com` matches subdomains | `github.com` |
| `Prefix` | `url.startsWith(pattern)` | `https://github.com/zkz098/` |
| `Regex` | Rust `regex` `is_match` | `^https://github\.com/([^/]+)/([^/]+)/releases/download/(.+)$` |
| `Wildcard` | `*` any sequence, `?` one character | `https://*.githubusercontent.com/*` |

### Replacement modes

**Prefix proxy**: use the target template as a prefix, followed by the original URL (optionally percent-encoded).

```text
Target:      https://mirror.example.com/
Original:    https://github.com/zkz098/limedl/releases/download/v0.6.1/app.zip
Result:      https://mirror.example.com/https://github.com/zkz098/limedl/releases/download/v0.6.1/app.zip
```

**Template**: regex rules can reference capture groups with `$1`, `$2`; non-regex rules can use the `{url}` (encoded) and `{raw_url}` (original) placeholders.

```text
Match type:  Regex
Pattern:     ^https://github\.com/([^/]+)/([^/]+)/releases/download/(.+)$
Target:      https://mirror.example.com/https://github.com/$1/$2/releases/download/$3
```

### Behavior details

- Targets generate candidate URLs in order, de-duplicated and tried in sequence; `Fallback to original` appends the original URL at the end.
- Rules are sorted by `order`, and **only the first matching rule is used**; lower the `order` of an exception to make it win.
- Rewriting happens before dispatch. File name and checksum metadata are derived from the original task and do not change when the source changes.
- URL encoding suits mirrors that expect the full URL as a path parameter; leave it off otherwise to avoid double encoding.
- **Rules have no per-rule header fields**: pass `Cookie`, `Referer`, `User-Agent`, and similar headers through task options or the browser extension's `header` array.

### Troubleshooting

- Rule not applied: check the enabled flag, whether an earlier rule with a lower `order` matched first, and whether the match type is correct.
- Mirror returns 403: likely User-Agent anti-abuse or hotlink protection; see [Troubleshooting](/en/guides/troubleshooting/).
- Mirror content differs: keep checksums enabled, or switch to Metalink hash verification.

---

## CDN providers

Where: **Settings → Labs → CDN Network Optimization**.

| Provider | Description |
| :--- | :--- |
| **Cloudflare** (default) | Built-in official Anycast ranges and a test URL |
| **Fastly** | Fastly static ranges and a test URL |
| **Custom** | Your own test URL plus IPv4/IPv6 CIDRs, one per line |

### Probe flow

1. Load the provider's IP ranges;
2. Screen candidates with concurrent TCP/TLS handshakes;
3. Measure real throughput over small chunks;
4. Install the best IP into the transport resolver.

Connections still send the original TLS SNI and `Host` header, so certificate validation remains valid.

### Limits and caveats

- A probe consumes roughly **20–50 MB** of traffic.
- It only applies to domains inside the selected provider's ranges; other domains use normal DNS.
- If the best candidate is **not faster** than the default route, limedl keeps default routing.
- Switching providers requires a fresh probe; the old node is not migrated.
- If the default DNS resolves to a private IP (TUN/VPN takeover), the probe aborts with a conflict warning.
- The applied node is persisted and used by later downloads until you re-probe or clear it.

---

## Recommended combination

1. Record a baseline with `curl` for the same URL;
2. Target served by Cloudflare/Fastly: enable CDN probing and apply the best node;
3. Reliable mirror available: add a URL rewrite rule and keep checksums enabled;
4. Multiple mirrors: let Metalink handle aggregation and per-piece verification;
5. Still slow: check the proxy, IPv6, ISP egress, and mirror 403s.

---

## Related documentation

- [CDN Acceleration](/en/guides/cdn/): state machine and DNS rewriting.
- [Metalink & Mirror Selection](/en/advanced/metalink-and-mirrors/): multi-source aggregation and verification.
- [Cross-Border & Mirrors](/en/use-cases/cross-border-mirrors/): the complete workflow.
- [Configuration](/en/getting-started/configuration/): Labs and network settings.
