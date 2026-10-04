---
type: system
title: Networking, HTTP Clients and Rate Control
description: limedl's shared outbound HTTP path — the HttpClientFactory every client must use, proxy and User-Agent semantics, the HTTP/3 situation, the global token-bucket rate limiter, retry/backoff policy, and URL rewriting for mirrors.
tags: [networking, http, proxy, rate-limiter, retry, url-rewrite]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T01:52:48.489Z
sources:
  - id: openwiki-source-4905fab56ecf9fa5e1ebbf3f
    resource: repo://.cargo/config.toml
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-f4ce02d0eec9d2510e1b00e5
    resource: repo://crates/limedl-core/src/http_client_factory/mod.rs
  - id: openwiki-source-32ac4e63b57c822bae04f7a5
    resource: repo://crates/limedl-core/src/http_client_factory/tests.rs
  - id: openwiki-source-385628f66a6c216078934666
    resource: repo://crates/limedl-core/src/http_executor/mod.rs
  - id: openwiki-source-c1216bb52f086b1114796ff0
    resource: repo://crates/limedl-core/src/rate_limiter/mod.rs
  - id: openwiki-source-61df807712674b34c14141ea
    resource: repo://crates/limedl-core/src/retry.rs
  - id: openwiki-source-5dec6002b80585dbbafe39ac
    resource: repo://crates/limedl-core/src/types/common.rs
  - id: openwiki-source-7ef10e5bb7f9bf65c86b6285
    resource: repo://crates/limedl-core/src/types/settings.rs
  - id: openwiki-source-191c52d830a19ec45ce7e929
    resource: repo://crates/limedl-core/src/url_rewrite/mod.rs
generated: { by: "pi", at: "2026-10-04T01:52:48.489Z" }
---

# Networking, HTTP Clients and Rate Control

Every outbound HTTP request in limedl is built through one module,
`http_client_factory`, so proxy and User-Agent behaviour is uniform. On top of
that sit the global rate limiter, the retry/backoff policy and the URL-rewrite
mirror selection.

## HttpClientFactory

`build_http_client(settings)` is the convenience constructor; `configure_client_builder`
returns a `ClientBuilder` so callers can append their own configuration
(DNS override for CDN acceleration, a different UA or timeout for the updater).
Shared configuration applied by the factory:

| Setting | Value |
| --- | --- |
| Redirect policy | `Policy::limited(10)` |
| TCP | `tcp_nodelay(true)`, keepalive 60 s |
| Read timeout | 15 s |
| Connect timeout | 30 s |
| Pool | 20 idle per host, 120 s idle timeout |

Proxy behaviour follows `settings.proxy.mode`:

- `Disabled` → `builder.no_proxy()` (turns off the automatic system/env proxy);
- `System` → nothing added, leaving reqwest's system detection active;
- `Manual` → `Proxy::all(manual_url)`.

**A bare `reqwest::Client::builder()` is forbidden** anywhere in the engine,
because it silently bypasses `settings.proxy`. This is why the bootstrap
Dispatcher client, the BT `.torrent` client and the updater all go through this
module.

Evidence: `repo://crates/limedl-core/src/http_client_factory/mod.rs#L28-L66`,
`repo://crates/limedl-core/src/types/common.rs#L84-L90`.

### The system-proxy feature guard

`ProxyMode::System` only reads the Windows registry / macOS system configuration
because reqwest's default feature set includes `system-proxy`
(`hyper-util/client-proxy-system`). If someone re-enables
`default-features = false` without listing `system-proxy`, the feature disappears
and system proxies are silently ignored — the symptom is "system proxy doesn't
work". A regression test parses the workspace manifest and fails unless
`system-proxy` is enabled, either by leaving defaults on or by listing it
explicitly. hyper-util only reads static `ProxyServer`/`ProxyOverride`; it does
not parse PAC/WPAD.

Evidence: `repo://crates/limedl-core/src/http_client_factory/tests.rs#L282-L326`.

### User-Agent freshness

`normalize_user_agent` trims, substitutes the built-in default when empty, and
rejects values longer than 512 bytes or containing invalid header characters. The
built-in default is a browser-shaped Chrome/154 UA; it is a deliberate
"disguise" that some mirrors (e.g. Tsinghua TUNA) police by version — an
outdated or not-yet-released Chrome version is rejected as a fake browser, while
tool UAs pass. Updating it means editing `default_http_user_agent()`, the
`tab_download.slint` hint and the three `.po` catalogs. Existing `settings.json`
files keep the old value (no automatic migration).

Evidence: `repo://crates/limedl-core/src/http_client_factory/mod.rs#L11-L25`,
`repo://crates/limedl-core/src/types/settings.rs#L102-L113`.

### HTTP/3 status

The root manifest enables reqwest's `http3` feature (pulling `h3`/`h3-quinn`/
`quinn`), and `.cargo/config.toml` adds the required `--cfg reqwest_unstable` to
every target. reqwest dispatches h3 per **request** HTTP version, and no call site
sets `Version::HTTP_3`, so the client factory currently uses h1/h2. reqwest has
no Alt-Svc auto-upgrade and no h3→h1/h2 fallback, and the h3 connector does not
pass through the proxy connector — so enabling it in production requires handling
the "configured proxy must not leak over h3" case first.

Evidence: `repo://Cargo.toml#L16-L28`, `repo://.cargo/config.toml#L1-L45`.

## Global rate limiter

`RateLimiter` is a thread-safe token bucket shared across the whole app. The
rate is an `AtomicU64` for a lock-free unlimited fast path, and the bucket state
(`rate`, `capacity`, `tokens`, `last_refill`) sits behind a `parking_lot::Mutex`
held only for brief arithmetic — never across an await or a blocking call.

- Rate `0` means unlimited (`consume` returns immediately).
- Bucket capacity is `max(2 * rate, 1)`.
- `set_rate` refills tokens at the old rate before switching, preserving budget
  coherently.
- `try_consume` refills by elapsed time, and on a deficit it keeps the existing
  tokens and returns a sleep duration instead of zeroing them — zeroing caused an
  infinite oscillation when a single request was larger than what one refill
  window could accumulate.
- `consume` is async (`tokio::time::sleep`); `consume_blocking` is the
  `spawn_blocking` variant.

Evidence: `repo://crates/limedl-core/src/rate_limiter/mod.rs#L12-L96`,
`repo://crates/limedl-core/src/rate_limiter/mod.rs#L128-L160`.

### Batching in the download path

Chunk workers do not consume the limiter per received chunk. `BatchLimiter`
accumulates bytes and chunk counts and consumes once when either 256 KiB or 8
chunks is reached, and `flush()` consumes any leftover bytes before the worker
exits. The AIMD sampling window is independent of this batching.

Evidence: `repo://crates/limedl-core/src/http_executor/mod.rs#L216-L252`.

## Retry and backoff

`request_with_retry(factory, token, max_retries, managed)` drives a request
attempt loop under a cancellation token:

- Every attempt is `select!`ed against cancellation, so a cancel returns
  `Interrupted` promptly.
- A `403 Forbidden` gets a bounded body sniff (`ANTI_ABUSE_SNIFF_LIMIT`); if it
  looks like a WAF/mirror anti-abuse page the request fails terminally with an
  actionable error rather than retrying or probing Referers.
- `classify_download_response` separates `Use`, `Retryable` and `Invalid`.
  Retryable responses increment the attempt, record an AIMD penalty and back off;
  terminal (`Invalid`) responses fail immediately. Only 429/5xx retry.
- Backoff is `250 ms * 2^min(attempt, 4)`, capped at 4 s: 500 ms, 1 s, 2 s, 4 s,
  4 s, …
- A `429 Too Many Requests` on a multi-threaded download aborts retries
  (`rate_limit_aborts`) so the chunked path can downgrade to single-thread instead
  of hammering the host.
- `register_retry_penalty` sets the task to `Retrying`, records the error and
  marks an AIMD penalty for connection backpressure.

Evidence: `repo://crates/limedl-core/src/retry.rs#L25-L180`.

## URL rewriting and mirror selection

`url_rewrite` rewrites a download URL to configured mirrors/proxies (ghproxy,
hf-mirror, …). `matches_rule` supports four match types:

- `Host` — exact host, `*.suffix` wildcard suffix, or `*`/`?` wildcard;
- `Prefix` — string prefix;
- `Regex` — regex match (invalid regex never matches);
- `Wildcard` — `*`/`?` glob.

`rewrite_url` returns `[original]` when rewriting is disabled or no rule matches.
When a rule matches, it generates candidate URLs from that rule's enabled targets
in priority order, de-duplicated, and appends the original URL at the end when
`fallback_to_original` is set. Two replacement modes exist:

- `PrefixProxy` — `base/<url>` (optionally percent-encoding the URL with the RFC
  3986 unreserved set);
- `Template` — regex `replace_all` with capture groups, or `{url}` / `{raw_url}`
  placeholder substitution.

The Dispatcher uses this in `start` (filling `mirror_urls` when the caller
supplied none) and in `resolve_mirror_urls`.

Evidence: `repo://crates/limedl-core/src/url_rewrite/mod.rs#L41-L90`,
`repo://crates/limedl-core/src/url_rewrite/mod.rs#L100-L200`,
`repo://crates/limedl-core/src/dispatcher.rs#L148-L170`.

<!-- openwiki: broken internal link [/openwiki/workflows/http-download-lifecycle.md] link "/openwiki/workflows/http-download-lifecycle.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Related pages: [HTTP Download Lifecycle](/openwiki/workflows/http-download-lifecycle.md),
<!-- openwiki: broken internal link [/openwiki/workflows/cdn-acceleration.md] link "/openwiki/workflows/cdn-acceleration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[CDN Acceleration](/openwiki/workflows/cdn-acceleration.md),
<!-- openwiki: broken internal link [/openwiki/workflows/scheduler-and-concurrency.md] link "/openwiki/workflows/scheduler-and-concurrency.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Scheduler, AIMD and Concurrency Control](/openwiki/workflows/scheduler-and-concurrency.md),
<!-- openwiki: broken internal link [/openwiki/desktop/self-update-and-distribution.md] link "/openwiki/desktop/self-update-and-distribution.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Self-Update and Distribution Channels](/openwiki/desktop/self-update-and-distribution.md).
