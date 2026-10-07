---
type: workflow
title: CDN Acceleration
description: How limedl probes a CDN's anycast IPs and rewrites DNS for a faster download path — the CdnAccelerator state machine, the provider abstraction, the DNS-rewritten client, CdnService event monitoring, and how DownloadManager consumes it.
tags: [cdn, acceleration, cloudflare, speed-test, dns]
sources:
  - id: openwiki-source-fd95ce7448707ad4ddaa3b43
    resource: repo://crates/limedl-core/src/bootstrap.rs
  - id: openwiki-source-a3ec1897f754d856cefa8de6
    resource: repo://crates/limedl-core/src/cdn/accelerator.rs
  - id: openwiki-source-6c6d0fa7529bc4ad3e5b30ef
    resource: repo://crates/limedl-core/src/cdn/provider.rs
  - id: openwiki-source-aa2a3d280434102afb7af803
    resource: repo://crates/limedl-core/src/cdn/resolver.rs
  - id: openwiki-source-dbf568038e30f39fb0272461
    resource: repo://crates/limedl-core/src/cdn/service.rs
  - id: openwiki-source-4b83dfd7538606d5871938ef
    resource: repo://crates/limedl-core/src/dispatcher.rs
  - id: openwiki-source-0c4cd5f8953750852a1f4d6d
    resource: repo://crates/limedl-core/src/manager.rs
  - id: openwiki-source-e4fbb1ee7bbda6133fb042fc
    resource: repo://crates/limedl-native/src/handlers/labs/cdn.rs
generated: { by: "pi", at: "2026-10-07T03:53:23.435Z" }
verified:
  - by: openwiki/0.7.1
    at: 2026-10-07T03:53:23.435Z
---

# CDN Acceleration

CDN acceleration picks the fastest reachable edge IP for a CDN-fronted host and
builds a `reqwest::Client` whose DNS resolves that host to the chosen IP while
keeping the original hostname as the TLS SNI. It is opt-in through
`settings.cdn_acceleration.enabled`.

Note: the provider layer is no longer Cloudflare-only — there is a `CdnProvider`
abstraction with `Cloudflare`, `Fastly` and `Custom` kinds (older documentation
claiming a hardcoded Cloudflare is stale).

## CdnAccelerator

`CdnAccelerator` holds:

- `state: RwLock<AccelState>` — `Idle`, `Testing`, `Ready`, `Error(String)`;
- `accelerated_client: RwLock<Option<reqwest::Client>>`;
- an `AtomicU8` phase indicator (0 = FetchingRanges, 1 = Screening,
  2 = MeasuringThroughput, sentinel `PHASE_NONE` = no active phase) and two
  `AtomicU64` progress counters, so the speed-test callback can store progress
  without spawning a task or taking an async lock;
- the provider, IP cache, candidate results, default-node result and a
  `CancellationToken`.

Evidence: `repo://crates/limedl-core/src/cdn/accelerator.rs#L20-L90`.

### start_test

`start_test(settings)` is idempotent (a no-op while `Testing`), builds the
provider from settings, clears prior results, creates a cancellation token and
spawns the work:

1. **FetchingRanges** — `provider.fetch_ip_ranges(...)`; cancellation or an empty
   IP list moves the state to `Idle`/`Error`.
2. **Screening → MeasuringThroughput** — `run_speed_test` over all candidate IPs
   and `measure_default_node` run concurrently via `tokio::join!`; a progress
   callback stores the phase/progress atomics.
3. The best candidate is chosen by throughput (falling back to latency when no
   throughput succeeded). If the best candidate is **not faster** than the
   default route, the state becomes `Error` and default routing is kept. If the
   default node's DNS resolved to a private IP, the test aborts with a warning
   that a TUN proxy/VPN may be interfering.
4. On success `apply_ip` is called and the state becomes `Ready`.

Evidence: `repo://crates/limedl-core/src/cdn/accelerator.rs#L110-L310`.

### apply_ip and init_from_settings

`apply_ip(ip, speed_mbps, settings)` builds the accelerated client for the
provider's test-URL hostname, stores the client, IP and speed, and moves to
`Ready`. `init_from_settings` restores the provider and, when acceleration is
enabled, re-applies the persisted `active_ip`/`active_speed_mbps` so downloads are
accelerated immediately without re-running the test. An unparseable persisted IP
is logged and ignored. `clear()` resets state, client, IP, candidates and phase.

Evidence: `repo://crates/limedl-core/src/cdn/accelerator.rs#L340-L410`.

## Provider abstraction

`CdnProvider` supplies the provider kind/name, a default throughput-test URL,
fallback IPv4/IPv6 CIDRs, a `fetch_ip_ranges` implementation and an `matches_ip`
membership test. `CdnProviderKind` is `cloudflare`, `fastly` or `custom`;
`create_provider_from_settings` maps `settings.cdn_acceleration.provider` and, for
`custom`, parses `custom_cidrs` (comma/newline/semicolon/space separated, IPv6 by
the presence of `:`) and `custom_test_url`.

Evidence: `repo://crates/limedl-core/src/cdn/provider.rs#L27-L120`,
`repo://crates/limedl-core/src/cdn/accelerator.rs#L478-L515`.

## The DNS-rewritten client

`build_accelerated_client(domain, ip, settings)` builds a client through
`configure_client_builder` (so proxy/UA/timeouts still apply) and adds
`resolve_to_addrs(domain, &[SocketAddr::new(ip, 0)])`. The override sends `domain`
as the TLS SNI hostname while connecting to `ip`, which is what makes it work for
TLS-fronted CDNs. `is_private_ip` is used to detect TUN/VPN interception, and
`is_cloudflare_domain(url, cache)` checks membership using the live cache or the
static fallback with a short DNS cache.

Evidence: `repo://crates/limedl-core/src/cdn/resolver.rs#L45-L70`,
`repo://crates/limedl-core/src/cdn/resolver.rs#L110-L190`.

## CdnService and event monitoring

`CdnService` wraps the accelerator and is the facade the frontends use
(`start_test`, `cancel_test`, `apply_ip`, `clear`, `init_from_settings`, `status`,
`phase`, `phase_progress`, `candidates`, `default_node`, `get_client`,
`ip_cache`). `from_accelerator` lets the same accelerator be shared with
`DownloadManager`.

`monitor_test(event_bus)` is the shared polling loop used by the desktop and NAS
handlers: it sleeps 500 ms and, while `Testing`, publishes `CdnProgress` with the
phase string (`fetchingRanges`/`screening`/`measuringThroughput`) and progress;
on `Ready`/`Error` it publishes `CdnComplete` and returns a `CdnTestOutcome`
(state, active IP, speed, candidates, default node) for the caller to persist.

Evidence: `repo://crates/limedl-core/src/cdn/service.rs#L18-L210`.

## How DownloadManager consumes it

`bootstrap` creates the `CdnService`, injects its accelerator into the manager via
`set_cdn_accelerator`, and calls `init_from_settings`. At download time
`resolve_client`:

1. returns the standard client when CDN acceleration is disabled;
2. reads the accelerator's IP cache (or `None`) and calls `is_cloudflare_domain`;
   a non-matching domain gets the standard client;
3. resolves the active IP from the in-memory accelerator, falling back to the
   persisted `active_ip` setting;
4. returns a cached accelerated client for `(host, ip)` or builds one, evicting
   the whole cache when it exceeds `MAX_CDN_CLIENT_CACHE_SIZE`;
5. falls back to the standard client if building the accelerated one fails.

Evidence: `repo://crates/limedl-core/src/manager.rs#L255-L330`,
`repo://crates/limedl-core/src/bootstrap.rs#L90-L92`.

## Desktop wiring

The desktop's Labs CDN handler drives this: it starts a test, spawns
`monitor_test`, updates the Labs form from `CdnProgress`/`CdnComplete` through the
EventBus subscriber, applies a candidate via `apply_ip`, and persists the chosen
`active_ip`/`active_speed_mbps` through `save_settings_with`. Disabling CDN
acceleration in settings triggers `CdnService::clear()` in the Dispatcher save
fan-out.

Evidence: `repo://crates/limedl-native/src/handlers/labs/cdn.rs#L111-L180`,
`repo://crates/limedl-native/src/handlers/labs/cdn.rs#L340-L360`,
`repo://crates/limedl-core/src/dispatcher.rs#L308-L313`.

Related pages: [Networking, HTTP Clients and Rate Control](../systems/networking-and-rate-control.md),
[Native Desktop UI (Slint)](../desktop/native-ui-architecture.md),
[HTTP Download Lifecycle](http-download-lifecycle.md).
