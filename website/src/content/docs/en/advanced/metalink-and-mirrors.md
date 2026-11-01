---
title: Metalink & Mirror Selection
description: How limedl parses Metalink 4.0/3.0/HTTP headers, scores mirrors, leases connections, and verifies cross-mirror pieces
---

# Metalink & Mirror Selection

When a file has several sources, limedl can distribute connections according to a Metalink description, isolate bad mirrors, and verify the result against declared hashes. This page covers the supported formats, selection rules, and known limits.

---

## Supported inputs

| Source | How to use it |
| :--- | :--- |
| `.metalink` / `.meta4` file | Drag it into the window, or submit it as a local task |
| Metalink 4.0 XML (RFC 5854) | Upload base64 XML through `aria2.addMetalink` |
| Metalink 3.0 | Same as above |
| HTTP `Link` headers (RFC 6249) | Recognized automatically for `rel="duplicate"`, `rel="describedby"`, and `Digest` |
| Plain mirror lists | A task carrying `mirror_urls` also creates a mirror pool |

:::note[Parser limits]
- Metalink XML larger than **10 MiB** is rejected.
- A document with no `<file>` entries is an error, not an empty success.
- File names are reduced to a basename and sanitized so a Metalink cannot write outside the target directory.
- Only **SHA-512 / SHA-256 / BLAKE3** are recognized checksum algorithms; anything else is treated as no checksum.
:::

---

## How mirrors are ranked

The engine keeps a composite score for each mirror:

| Factor | Effect |
| :--- | :--- |
| `<url priority>` | RFC 5854 semantics: **1 is highest**, larger numbers come later |
| Protocol | HTTPS preferred |
| Geography | Same or nearby region scores higher when a preferred location is configured |
| Consecutive errors | Each failure lowers the score and triggers exponential backoff |
| RTT | The pool's scoring function accepts RTT, but the current download path builds the pool from a URL list **without an active RTT probe**; ranking mainly uses priority, protocol, region, and error history |

So “multiple mirrors” does not mean “automatic speed test”. It is a priority pool with health tracking. For real throughput measurement, use [CDN probing](/en/guides/cdn/).

---

## Leases, cooldown, and failure isolation

- **Dynamic leases**: each worker leases a mirror from `MirrorPool`; the lease holds a concurrency slot until it is dropped.
- **Per-mirror cap**: mirrors at `max-connections` are skipped; when every mirror is full, the pool falls back to the mirror with the fewest active connections.
- **Failure backoff**: consecutive failures back off for `2^errors` seconds (capped at 60 s); cooling mirrors are skipped.
- **Corrupt-piece isolation**: a mirror that returns a failing piece gets five extra errors and is isolated for about 300 seconds.
- **RAII leases**: early returns and errors release the slot automatically.

---

## Verification and failure behavior

- Whole-file hashes prefer **SHA-512 > SHA-256 > BLAKE3**.
- A Metalink `<pieces>` block enables per-piece verification: a corrupt piece is reset immediately and the task fails; manual resume/retry re-downloads that piece.
- A failed whole-file checksum marks the task failed and preserves the temp file as `*.part.corrupt`; limedl **does not** infer and re-download only the bad blocks.
- The context-menu “Verify file integrity” action can recompute hashes for completed files or `.part` / `.part.corrupt` files.

---

## Recommendations

1. **Prefer official Metalink documents** over hand-built lists of unknown mirrors.
2. When a piece/whole-file hash is declared, an HTML error page from a mirror fails verification, and a bad piece also isolates that mirror. **Without a declared hash, limedl cannot identify a 200-status error page** and will not guess whether the content is correct.
3. If you need to know which mirror is fastest, use CDN probing or URL rewriting first, then let Metalink aggregate the remaining sources.
4. Through Aria2 RPC, `select-file` is a **1-based** comma-separated file index for multi-file Metalinks.

---

## Known limits

- Live RTT probing is not wired into the download path today; `probe_mirrors` is a library component that the scheduler does not call.
- Mirror geography comes from Metalink metadata; it is never guessed when absent.
- Hashes guarantee mirror consistency; if the Metalink declares no hash, limedl cannot detect a tampered mirror.

---

## Related documentation

- [Tasks & Concurrency](/en/guides/tasks/): AIMD, chunking, tail sprint, and integrity checks.
- [URL Rewrite & Custom CDN](/en/guides/url-rewrite-and-cdn-providers/): pick the fast source first.
- [Aria2 RPC](/en/ecosystem/aria2-rpc/): `addMetalink` parameters and response.
- [Benchmarks](/en/advanced/benchmarks/): buffer pool and HTTP end-to-end measurements.
