---
title: Expert Settings & HTTP/3
description: limedl expert-mode scheduler, AIMD, timeout, retry, and queue parameters, plus HTTP/3 Alt-Svc gating, fallback, and circuit breaking
---

# Expert Settings & HTTP/3

Most users never need to change these values. Expert mode is for people who know where the bottleneck is and will validate changes with reproducible benchmarks.

---

## Expert mode

Where: **Settings → Downloads & Scheduler → Expert Mode**.

When off, the client uses fixed policies; when on, the following values are read from `scheduler.expert` in `settings.json` and override the automatic strategy.

### Scheduling and concurrency

| Setting | Default | Notes |
| :--- | :--- | :--- |
| Scheduler tick | `2000 ms` | Lower values increase CPU use |
| Max connections per host | `16` | Per-site concurrency; too high triggers 429/503 |
| Global max workers | `16` | Shared HTTP chunk workers across all tasks |
| Max threads per task | `8` | Upper bound for one task |
| Min threads per task | `0` | 0 means no floor; the task can queue under budget pressure |
| AIMD profile | Conservative / Balanced (default) / Aggressive | Controls ramp-up and backoff aggressiveness |
| Chunk strategy | Adaptive (default) / Fixed | Fixed uses the custom chunk size |
| Queue ordering | Priority first (default) / Shortest first / FIFO | Sort key in automatic mode |

### AIMD details

| Setting | Default |
| :--- | :--- |
| Decrease / increase / severe-drop thresholds | `0.16` / `0.04` / `0.50` |
| Cooldown | `3 s` |
| Sample count / probe stable cycles | `1` / `2` |
| Decrease factor / increase ratio | `0.50` / `0.25` |
| Oscillation threshold / hysteresis | `3` / `4` |

AIMD looks for the concurrency the path can sustain, not the maximum thread count. Fixed high concurrency usually trips rate limits faster on weak links, shared egress, or anti-abuse servers.

### Chunking, tail, and retries

| Setting | Default | Notes |
| :--- | :--- | :--- |
| Custom chunk size | `0` (auto) | MiB; too small adds request overhead, too large extends the tail |
| Tail Sprint stall window / min split | `8 s` / `1 MiB` | Splits the last large chunks for retry |
| Work-steal min split | `2 MiB` | Minimum granularity for idle workers to steal tail work |
| Connect / read timeout | `30 s` / `30 s` | Ranges 3–120 s and 5–300 s |
| Pool idle per host / TCP keepalive | `20` / `60 s` | Idle connections and keepalive |
| Retry backoff base / cap / jitter | `250 ms` / `4000 ms` / `25%` | Exponential backoff |

### When to change anything

- **Frequent 429/503**: lower max connections per host and max threads per task before touching AIMD.
- **Visible long tail**: keep Tail Sprint on; tune the minimum split or stall window.
- **Many small files**: fixed small chunks add overhead; automatic is usually better.
- **High-latency links**: do not just raise timeouts; confirm whether loss, proxy, or the mirror is the cause.
- **High CPU**: inspect the scheduler tick, thread counts, and BT upload; do not lower the tick to “speed up”.

:::caution[Validate every change]
Pair any expert change with [Benchmarks](/en/advanced/benchmarks/) or a same-URL `curl` baseline, repeated at least 3 times. One run that “feels faster” is not evidence.
:::

---

## HTTP/3 and Alt-Svc

HTTP/3 is enabled by default but is **not exposed in the settings dialog**. The switch lives in `settings.json`:

```json
{
  "scheduler": {
    "http3AltSvcEnabled": true
  }
}
```

### When HTTP/3 is attempted

All of these must hold:

1. The switch is on;
2. The **proxy mode is Direct** (QUIC must not leak through a proxy);
3. The origin is HTTPS;
4. A valid, unexpired `Alt-Svc` record exists for the origin;
5. The origin is not inside an HTTP/3 circuit-breaker backoff window.

`Alt-Svc` records come from response headers, keep only `h3` protocols (preferring standard `h3`), and are cached in SQLite across tasks and sessions.

### Attempt and fallback

- Each request tries HTTP/3 **once**, with a **5-second** timeout;
- On failure or timeout it falls back to the normal TCP retry path;
- Failures trip a per-origin circuit breaker with roughly `5 min → 10 min → 30 min → 24 h` exponential backoff;
- A successful HTTP/3 request clears the breaker.

### When to turn it off

- The host or firewall blocks UDP 443;
- A proxy/VPN environment requires all traffic over TCP;
- The network applies QoS to QUIC and first packets time out;
- You are isolating a connection problem and want to remove HTTP/3 from the equation.

Turning it off still leaves HTTP/1.1 or HTTP/2 (depending on the server and connection negotiation); downloads continue normally.

---

## Related documentation

- [Configuration](/en/getting-started/configuration/): full settings and defaults.
- [Tasks & Concurrency](/en/guides/tasks/): how AIMD and chunk scheduling work.
- [Benchmarks](/en/advanced/benchmarks/): how to validate parameter changes.
- [Cross-Border & Mirrors](/en/use-cases/cross-border-mirrors/): network troubleshooting order.
