---
type: integration
title: Aria2 JSON-RPC Compatibility Server
description: The aria2-compatible HTTP and WebSocket server of limedl — method routing, GID derivation and caching, three auth modes with Argon2 token storage, the configurable bind address with a fail-closed auth gate, CORS wildcard mode, type-driven request parsing and aria2 option translation, the tellStatus/getServers/changeUri/changePosition surface, notification ownership, exit_on_shutdown and the graceful hot-reload port handoff.
tags: [aria2, rpc, integration, authentication, json-rpc, websocket]
sources:
  - id: openwiki-source-06de9eea8068258882d65c0b
    resource: repo://.github/workflows/aria2-oracle.yml
  - id: openwiki-source-574430b2d80ebfee1870b543
    resource: repo://crates/limedl-core/src/aria2_rpc/bind.rs
  - id: openwiki-source-8ec1f0436491ce5daa75720b
    resource: repo://crates/limedl-core/src/aria2_rpc/context.rs
  - id: openwiki-source-a126aed2e5b28c6cc1b7781c
    resource: repo://crates/limedl-core/src/aria2_rpc/dispatch.rs
  - id: openwiki-source-4d67c2a24e4b561bd4166816
    resource: repo://crates/limedl-core/src/aria2_rpc/download.rs
  - id: openwiki-source-3659606b404344d4dd4d1487
    resource: repo://crates/limedl-core/src/aria2_rpc/interop_tests.rs
  - id: openwiki-source-9529b707cb48393fd5c5dcfb
    resource: repo://crates/limedl-core/src/aria2_rpc/options.rs
  - id: openwiki-source-cb3b278da9fc4917fdb881e9
    resource: repo://crates/limedl-core/src/aria2_rpc/oracle_tests.rs
  - id: openwiki-source-b02cae0469c12fb7309d95fb
    resource: repo://crates/limedl-core/src/aria2_rpc/protocol.rs
  - id: openwiki-source-0a4bf1d1495b125d077c4e3c
    resource: repo://crates/limedl-core/src/aria2_rpc/query.rs
  - id: openwiki-source-18347dd81d612fba86839f34
    resource: repo://crates/limedl-core/src/aria2_rpc/server.rs
  - id: openwiki-source-7d47346a6f4a8d73d2b59c64
    resource: repo://crates/limedl-core/src/aria2_rpc/system.rs
  - id: openwiki-source-f8a658e29a23b144a8733e19
    resource: repo://crates/limedl-core/src/aria2_rpc/token.rs
  - id: openwiki-source-1ad782a385f3efd488e8d368
    resource: repo://crates/limedl-core/src/aria2_rpc/transport.rs
  - id: openwiki-source-093388d09b520118fa26ce32
    resource: repo://crates/limedl-core/src/bt_backend/alerts.rs
  - id: openwiki-source-0c4cd5f8953750852a1f4d6d
    resource: repo://crates/limedl-core/src/manager.rs
  - id: openwiki-source-7ef10e5bb7f9bf65c86b6285
    resource: repo://crates/limedl-core/src/types/settings.rs
  - id: openwiki-source-3fe9812b75a7522e89f74344
    resource: repo://docs/aria2-interop-testing.md
generated: { by: "pi", at: "2026-10-04T14:09:40.431Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T14:09:40.431Z
---

# Aria2 JSON-RPC Compatibility Server

`crates/limedl-core/src/aria2_rpc/` implements an aria2-compatible JSON-RPC 2.0
server over HTTP and WebSocket so clients such as AriaNg and Motrix can drive
limedl. It is behind the optional `aria2-rpc` feature (enabled by the desktop
crate and by the headless `limedl-server` daemon). Internal downloads are mapped
to aria2 GIDs.

The server drives the same `Dispatcher` as the Slint desktop, so the desktop UI
and an RPC client stay in sync through the `EventBus`. For the daemon the RPC
endpoint is the only frontend; see
[Headless Server Daemon](headless-server-daemon.md).

## Server assembly and lifecycle

`Aria2RpcServer::new(registry, &settings, event_bus)` builds an `RpcContext` with
the registry, an `AuthConfig` derived from settings, a GID cache, a fresh session
id, the `exit_on_shutdown` flag and an `Arc<Notify>` shutdown handle. It resolves
the bind address as `format_bind_addr(settings.listen_address, settings.port)` —
`listen_address` defaults to `127.0.0.1`, and `bind.rs` brackets an IPv6 literal
so `::1` becomes `[::1]:6800` instead of the malformed `::1:6800` — and stores the
CORS configuration for `serve`.

`serve(shutdown)` first enforces the security gate below, then assembles an axum
`Router` with `POST /jsonrpc` (HTTP JSON-RPC) and `GET /jsonrpc` (WebSocket
upgrade), applies the CORS layer built from the settings captured in `new`, and
runs `axum::serve(...).with_graceful_shutdown(...)` driven by the `watch`
receiver. (CORS used to be passed to `serve` separately from the settings given
to `new`; it now lives on the server so the two cannot disagree.)

Evidence: `repo://crates/limedl-core/src/aria2_rpc/server.rs#L113-L199`,
`repo://crates/limedl-core/src/aria2_rpc/bind.rs#L20-L40`.

### Bind address and the fail-closed authentication gate

`Aria2RpcSettings::listen_address` makes the listener configurable, which is what
lets the daemon serve a LAN. A **non-loopback** address is accepted only together
with authentication: `public_bind_rejection` returns a reason when the host is not
loopback and `AuthConfig::is_enabled` is false, and `serve` refuses to start with
that reason. `is_loopback_bind_address` accepts `127.0.0.1`, `::1` and
`localhost`; anything that does not parse as an IP (including a hostname it
cannot recognize) counts as public, so the gate fails closed rather than open.

The refusal is not cosmetic. The aria2 `dir` option is not confined to a download
root — it only has to be an absolute path without `..` — so an anonymous
network-reachable endpoint would let anyone who can reach the port write files
anywhere the process can.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/server.rs#L51-L59`,
`repo://crates/limedl-core/src/aria2_rpc/bind.rs#L20-L40`.

### CORS

`build_cors_layer(allowed, allow_any)`:

- `allow_any_origin = true` emits `Access-Control-Allow-Origin: *` and
  deliberately omits `Access-Control-Allow-Credentials` (browsers reject `*`
  paired with credentials). This is the mode the daemon uses when AriaNg is
  served from a different origin.
- Otherwise the parsed `cors_allowed_origins` are echoed; with none configured
  the default is `http://localhost` / `http://127.0.0.1`; configured origins that
  all fail to parse fall back to localhost with a warning.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/server.rs#L66-L100`.

### exit_on_shutdown and the shutdown notify

`aria2.shutdown` / `aria2.forceShutdown` always publishes a `Warning` event and
returns the acknowledgement string, but it only calls
`RpcContext::shutdown_notify.notify_one()` when `exit_on_shutdown` is set. The
desktop leaves that off (it is a managed subsystem and the UI owns exit); the
daemon sets it and awaits `Aria2RpcServer::shutdown_notify()` before calling
`registry.shutdown_all()`. `notify_one` rather than `notify_waiters` is
deliberate: it leaves a permit if the client's shutdown arrives before the daemon
starts awaiting, so the signal is not lost.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/system.rs#L38-L56`,
`repo://crates/limedl-core/src/aria2_rpc/server.rs#L155-L157`,
`repo://crates/limedl-core/src/aria2_rpc/context.rs#L44-L57`.

**The service is disabled by default.** `Aria2RpcSettings::default()` has
`enabled = false`, because with an empty `secret` the endpoint answers
anonymously and loopback-only binding still lets any local process — and any page
the default CORS policy admits — drive downloads and read destination paths.
Enabling it is an explicit Settings action; an existing `settings.json` with
`"enabled": true` keeps working. The headless daemon overrides `enabled` to
`true` (the RPC endpoint is its interface) and consequently must satisfy the
authentication gate whenever it binds non-loopback. When the server does come up
with `AuthConfig::Disabled` on loopback, `serve` logs a warning naming the address
and pointing at the secret/per-client options. That check reads
`AuthConfig::is_enabled` *before* the context is moved into the router, which is
why the method is `pub(crate)`.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/server.rs#L113-L180`,
`repo://crates/limedl-core/src/aria2_rpc/context.rs#L39-L41`,
`repo://crates/limedl-core/src/types/settings.rs#L404-L422`.

### Hot-reload port handoff

Saving settings restarts the RPC server. The predecessor drops its listener on a
different task, so the replacement can observe `AddrInUse`. `bind_with_retry`
retries `AddrInUse` for a 5 s window at 25 ms intervals and returns any other
error immediately; a port genuinely held by another process still fails after
the window. This is why a settings save does not silently lose the RPC endpoint.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/server.rs#L14-L48`.

## Authentication

`AuthConfig::from_settings` produces one of three states:

- `Disabled` — `single` mode with an empty `secret`; every request passes. It is
  reachable in practice only through the legacy `"enabled": true` +
  `"secret": null` combination, since a fresh install starts disabled, and a
  non-loopback bind refuses to start in this state.
- `Shared { secret }` — the legacy single shared secret, compared with
  `subtle::ConstantTimeEq`.
- `PerClient(PerClientAuth)` — each configured client has its own Argon2id
  `token_hash`.

An **empty client list in `per_client` mode fails closed** (rejects everything),
because opting into per-client mode and silently degrading to anonymous access
would be the opposite of the intent.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/context.rs#L8-L34`.

### Token storage and verification

- `generate_token()` produces 32 random bytes URL-safe base64 (no padding).
- `hash_token()` returns an Argon2id PHC string; only that may be persisted.
  `settings.json` never stores plaintext.
- `verify_token()` returns `false` for a malformed hash rather than erroring, so
  a hand-edited settings file cannot take the server down.
- Successful verifications are cached in-memory by `BLAKE3(token)` so the ~20-40 ms
  Argon2 cost is paid once per token. The digest is never persisted and never
  accepted on its own — it is only inserted after a full Argon2 success, so a
  settings-file leak cannot be replayed against a running server.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/token.rs#L1-L20`,
`repo://crates/limedl-core/src/aria2_rpc/token.rs#L54-L110`.

### Centralized validation

`dispatch_method` enforces the token centrally, then routes via
`dispatch_authorized`. Centralizing it is deliberate: with per-handler checks the
`strip_token`/`check_token` order was easy to invert (making a configured secret
reject every legitimate request) and bulk methods such as `pauseAll` silently
skipped authentication. `check_token` only accepts the `token:`-prefixed first
parameter, matching aria2's wire format, so new handlers are protected
automatically.

Three methods are exempt from the **outer** check, mirroring aria2:

- `system.listMethods` and `system.listNotifications` are anonymous — they only
  reveal names, and AriaNg calls them without a token even when a secret is set.
- `aria2.multicall`/`system.multicall` carry no outer token in aria2. Each
  nested call is authenticated on its own: `handle_multicall` routes every
  nested call back through `dispatch_method`, so each nested handler checks and
  strips its own `token:` element. An unauthenticated nested call fails with its
  own Unauthorized entry instead of executing, so skipping the outer check is
  not a hole.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/dispatch.rs#L14-L42`,
`repo://crates/limedl-core/src/aria2_rpc/system.rs#L51-L90`,
`repo://crates/limedl-core/src/aria2_rpc/context.rs#L89-L131`.

## GIDs: stable identity across restarts

`internal_id_to_gid(raw_id)` is `xxh3_64(raw_id)` formatted as 16 lowercase hex
digits. The underlying task id is a UUID for HTTP and the info hash for BT, both
of which are stable across restarts, so a GID is stable too. `resolve_gid` checks
the cache first, then scans all backends' `list()` and populates the cache on a
hit. The cache is an optimization, not the source of truth. `resolve_gid` also
accepts an **abbreviated GID**: it prefers an exact match and otherwise resolves a
prefix that identifies exactly one task. A prefix shared by two tasks is refused
rather than guessed, and only full GIDs are cached because a prefix mapping could
go stale when a new task appears.

Lifecycle correctness: `addUri` and `resolve_gid`'s scan insert entries, and
`aria2.remove`, `aria2.removeDownloadResult` and `purgeDownloadResult` evict
them — otherwise the cache would grow without bound in a long session.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/protocol.rs#L94-L150`,
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L112-L132`,
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L298-L345`,
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L393-L460`.

## Method routing

`dispatch_authorized` maps method names to handlers: `addUri`/`addTorrent`,
`pause`/`forcePause`/`pauseAll`/`forcePauseAll`, `unpause`/`unpauseAll`,
`remove`/`forceRemove`/`removeDownloadResult`/`purgeDownloadResult`,
`tellStatus`/`tellActive`/`tellWaiting`/`tellStopped`, `getGlobalStat`,
`getOption`/`changeOption`, `getGlobalOption`/`changeGlobalOption`, `getFiles`,
`getUris`, `getServers`, `getPeers`, `changeUri`, `changePosition`,
`getSessionInfo`, `saveSession`, `shutdown`/`forceShutdown`, `multicall`, plus
`system.listMethods`/`system.listNotifications`. Unknown methods return the
method-not-found error code.

`handle_list_methods` advertises exactly the routed set — 36 entries. The Tier 1
test `interop_list_methods_matches_the_routed_surface` asserts that equality and
that every advertised method is reachable, so a handler added without a listing
(or vice versa) fails the suite. `aria2.addMetalink` is the one aria2 method
still deliberately absent (Metalink is out of scope). A top-level JSON-RPC batch
is served; the HTTP GET/JSONP transports are not.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/dispatch.rs#L44-L86`,
`repo://crates/limedl-core/src/aria2_rpc/query.rs#L568-L612`.

### multicall response shape

Each nested result is wrapped in a **single-element array** — `[value]` on
success, `[{"code","message"}]` on failure. AriaNg/Motrix index into element 0,
so a two-element `[null, value]` wrapper would be read as a failure. Nested
calls are authenticated individually (see centralized validation), and the
Tier 1 test `interop_ariang_multicall_shape` sends them with only per-call
tokens, as aria2 clients do.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/system.rs#L58-L90`.

## URI classification and addUri options

`classify_aria2_uri` routes **only `magnet:` to BT**; an HTTP(S) URL — including a
`.torrent` URL — stays HTTP, because aria2 downloads a `.torrent` URL as a plain
file and uses `addTorrent` for the parsed form. The deeper
`StartDownloadRequest::classify_kind` heuristic (which maps a `.torrent`
extension to BT) is deliberately not used here.

`handle_add_uri` treats the `uris` array as an ordered candidate list: element 0
is the primary URL and the rest become `mirror_urls` for HTTP. It translates the
aria2 request options into a `StartDownloadRequest`:

| aria2 option | Effect |
| --- | --- |
| `header`, `referer`, `http-user`/`http-passwd` | request headers (Basic auth is synthesized) |
| `checksum` (`TYPE=DIGEST`) | maps to `ChecksumMode` + expected digest (sha-256/sha-512/blake3) |
| `user-agent` | overrides the UA |
| `split` | thread count |
| `max-tries` | retry limit |
| `out` | file name |
| `dir` | destination directory, defaulting to the configured download dir |
| `pause` | start the task paused (also accepts the string `"true"`) |

A non-terminal HTTP download with the same URL is deduplicated: its existing GID
is returned and cached instead of starting a second task. `dir` must be absolute
and without `..` components, but it is not confined to a download root — which is
why the bind gate above insists on authentication for a network-reachable
listener.

Trailing arguments are read **by JSON type**, not position (`split_aria2_tail`
in `options.rs`). aria2's signatures are `addUri(uris[, options[, position]])`
and `addTorrent(torrent[, uris[, options[, position]]])`; AriaNg sends
`addTorrent` as `[torrent, [], options]`. Reading `params[1]` as the options
object therefore mistook AriaNg's empty `uris` array for the options and
silently dropped `dir`/`out`/`pause`/`select-file` — a bug limedl's own
`[torrent, options]` tests could not see. The type scan accepts both shapes, and
`addUri`/`addTorrent` honour `position` only when it is `0` (front of the queue →
High priority), the same priority approximation `changePosition` uses.
`addTorrent` applies `dir`/`pause` to the request; the BT backend does not yet
honour `out`/`select-file` at start (see deviations).

Evidence: `repo://crates/limedl-core/src/aria2_rpc/download.rs#L13-L19`,
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L21-L135`,
`repo://crates/limedl-core/src/aria2_rpc/options.rs#L89-L142`,
`repo://crates/limedl-core/src/manager.rs#L402-L444`.

## Runtime option changes

`handle_change_option` deliberately supports a **restricted key set** and errors
with the key name for anything else, instead of silently ignoring it:

- `pause` (both protocols) — pause when truthy, resume otherwise.
- `select-file` (BT only) — 1-based comma-separated aria2 indices parsed by
  `parse_select_file`, converted to the engine's 0-based indices.
- `max-download-limit` / `max-upload-limit` (BT only) — the status is read first
  so the other direction is preserved, then `bt_set_speed_limit` is called.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/options.rs#L342-L420`,
`repo://crates/limedl-core/src/aria2_rpc/options.rs#L859-L935`.

## Status fields (`tellStatus` and the list methods)

`summary_to_aria2_status` (`protocol.rs`) produces the always-present aria2 keys:
`gid`, `status`, `totalLength`, `completedLength`, `uploadLength`,
`downloadSpeed`, `uploadSpeed`, `connections`, `dir` and `files`, plus — for BT —
top-level `infoHash`, `numSeeders` and `seeder`. `errorCode`/`errorMessage`
appear only for terminal states: code `"0"` when there is no error, `"1"`
otherwise, with the human-readable reason in `errorMessage` (limedl does not
persist aria2's specific exit-status codes). `files[].path` is the absolute save
path, as aria2 reports it, and `seeder` describes the local endpoint (true once
the whole payload is present), not connected peers.

`enrich_status` adds the fields that need a manifest or backend lookup, on
`tellStatus` and `tellActive` only (the active set is small; `tellWaiting` and
`tellStopped` stay cheap):

- HTTP: `bitfield`, `numPieces` and `pieceLength` from the live manifest chunk map
  (`http_pieces`), packed MSB-first by `bitfield_from_bits`.
- BT: the `bittorrent` object (`mode` single/multi, `info.name`, `announceList`
  from the trackers) plus `bitfield`/`numPieces` from the piece set. A fresh
  magnet whose metadata has not arrived reports `files: []` and no `bittorrent`
  object, mirroring aria2.

`getGlobalStat.uploadSpeed` is the real sum of per-task upload speeds rather than
a constant.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/protocol.rs#L146-L245`,
`repo://crates/limedl-core/src/aria2_rpc/query.rs#L47-L152`.

## Server, URI and queue-position methods

- `aria2.getServers` (`handle_get_servers`) returns one file-index entry with a
  server struct per URI candidate for HTTP (`uri`/`currentUri`/`downloadSpeed`,
  the live speed on the in-use candidate and `"0"` on the rest), or the tracker
  list for BT.
- `aria2.changeUri` (`handle_change_uri` plus
  `DownloadManager::change_task_uris`) removes one occurrence per `delUris`
  entry, inserts `addUris` at `position`, refuses to remove the last URI, and
  returns aria2's `[deleted, added]`. `fileIndex` must be `1` for limedl's
  single-file HTTP downloads; BT is rejected.
- `aria2.changePosition` (`handle_change_position`) maps the requested index onto
  limedl's three priority levels — front to High, back to Low, otherwise Normal
  for `POS_SET`, one level up/down for `POS_CUR` — and returns the **actual**
  resulting index recomputed from `waiting_order`. `tellWaiting` uses the same
  order (priority, then creation time), so the reply matches the queue. Arbitrary
  positions among more than three same-priority tasks cannot be represented; this
  is an intentional, documented limitation.
- `aria2.forceShutdown` routes to the same handler as `aria2.shutdown`, which
  acknowledges and warns. It signals the daemon to stop when
  `exit_on_shutdown` is set, but the desktop leaves that off because it runs as a
  managed subsystem and the application UI owns exit.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/query.rs#L427-L490`,
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L462-L606`,
`repo://crates/limedl-core/src/aria2_rpc/system.rs#L38-L56`,
`repo://crates/limedl-core/src/manager.rs#L845-L935`.

## AriaNg option coverage

`getGlobalOption` answers the complete key set AriaNg's Settings pages read
(`aria2GlobalAvailableOptions` in its `aria2Options.js`, plus the quick-settings
speed limits), and `getOption` does the same for the task dialog
(`aria2TaskAvailableOptions`). Values come from limedl's real settings where the
engine has an equivalent (`dir`, `max-concurrent-downloads`, `split`,
`max-tries`, `user-agent`, `max-overall-*-limit`, the proxy and the mappable
`bt-*` / `enable-*` / `rpc-*` keys) and are documented aria2 defaults otherwise;
every value is a string, as aria2 serialises them. `changeGlobalOption` applies
the subset it can map (`dir`, `max-concurrent-downloads`, `split`,
`max-connection-per-server`, `max-tries`, `user-agent`, the global speed limits,
`enable-dht`, `enable-peer-exchange`, `bt-max-peers`, `seed-ratio`, `bt-tracker`,
`listen-port`) and ignores unrecognised keys, as aria2 clients expect.

The key sets are pinned by Tier 1 (`interop_get_global_option_covers_ariang_keys`,
`interop_get_option_covers_ariang_task_keys`) and by the Tier 2 oracle.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/options.rs#L9-L182`,
`repo://crates/limedl-core/src/aria2_rpc/options.rs#L491-L731`,
`repo://crates/limedl-core/src/aria2_rpc/options.rs#L935-L1245`.

## Transports and error codes

The endpoint serves `POST /jsonrpc` and `GET /jsonrpc` (WebSocket upgrade). A
POST body may be a single request object or a **JSON-RPC 2.0 batch** (a top-level
array): each element is dispatched and answered, notifications (elements without
an `id`) execute without a response entry, an all-notification batch returns no
body, and an empty array is a single Invalid Request. The WebSocket transport
accepts the same shapes.

Error objects follow aria2 rather than the JSON-RPC-specific codes: unknown
method, missing/invalid params, unknown GID, unauthorized and internal failures
all use `code: 1`, so the method-not-found case is recognized by its message.
Only wire-level parse and version failures keep `-32700`/`-32600`, because they
never reach a method. `ERR_METHOD_NOT_FOUND`/`ERR_INVALID_PARAMS`/`ERR_INTERNAL`
are kept as named constants but all equal 1.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/transport.rs#L9-L172`,
`repo://crates/limedl-core/src/aria2_rpc/protocol.rs#L39-L55`,
`repo://crates/limedl-core/src/tests/aria2_ws_e2e_tests.rs`.

## Known, intentional deviations

These are documented choices, not bugs, and are tracked alongside the Tier 1/Tier
2 plan in `docs/aria2-interop-testing.md`:

- `aria2.addMetalink` is not implemented (Metalink is out of scope), and HTTP
  GET/JSONP transports are not served — POST (including a JSON-RPC batch) and
  WebSocket are. HTTPS RPC and HTTP Basic auth are likewise not served, so a LAN
  deployment needs a reverse proxy for TLS; a configurable bind address (aria2's
  `--rpc-listen-all`) *is* supported, but a non-loopback bind requires
  authentication.
- `changePosition` collapses onto the priority model (above).
- BitTorrent `tellStatus.dir`/`getOption.dir` report the BT backend's default
  output dir, not a per-task `dir`, because the BT backend does not track one;
  `addTorrent`'s `out`/`select-file` are parsed but not applied at start (use
  `aria2.changeOption` after metadata).
- `getOption`/`getGlobalOption` answer AriaNg's full key sets, but several values
  are documented aria2 defaults because the engine has no equivalent. `getVersion`
  is truthful: `Async DNS`, `BitTorrent`, `GZip`, `Brotli`, `Zstd`, `HTTPS`,
  `Message Digest`, and it deliberately omits aria2's
  `Metalink`/`SFTP`/`XML-RPC`/`Firefox3 Cookie`.
- Error codes match aria2 (`1` for every domain failure); GID prefix matching is
  supported (a unique prefix resolves, an ambiguous one is refused).

## Tier 2 oracle (live `aria2c`)

`aria2_rpc/oracle_tests.rs` starts a real `aria2c --enable-rpc` beside limedl's
server and reports how the same requests differ. It is opted in by
`ARIA2_ORACLE_BIN`: unset, each test returns early so the normal core gate needs
no aria2; set but unusable, it panics. `.github/workflows/aria2-oracle.yml` sets
it on a nightly `schedule` and `workflow_dispatch` (never on PRs, Linux only).

Differences are **allowlisted, not open-ended**: the oracle asserts that limedl
answers every AriaNg option key, the always-present `tellStatus` keys, and the
documented `listMethods`/`listNotifications` delta, and that both servers answer
a JSON-RPC batch with an array; a run fails on anything outside those allowlists
(and when the oracle cannot start or a server stops answering). The remaining
aria2-only differences are printed as allowlisted notes by
`--success-output=final`.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/oracle_tests.rs#L1-L60`,
`repo://docs/aria2-interop-testing.md#L77-L156`,
`repo://.github/workflows/aria2-oracle.yml#L1-L45`.

## Terminal-result visibility

`max_in_memory_downloads` only evicts tasks from memory. `summary_for_raw_id`
falls back to `db.get_download_header` for a task the HTTP manager no longer
holds, so `tellStatus`/`tellStopped`/`getGlobalStat` remain correct until
`purgeDownloadResult` or `removeDownloadResult` deletes the row. `tellStopped`
also uses `list_download_headers` and `count_terminal_downloads`.

The `keys` filter is supported on the query methods with aria2's positional
indices (1 for `tellStatus`, 0 for `tellActive`, 2 for `tellWaiting`/`tellStopped`).

Evidence: `repo://crates/limedl-core/src/aria2_rpc/query.rs#L154-L210`,
`repo://crates/limedl-core/src/aria2_rpc/query.rs#L190-L266`,
`repo://crates/limedl-core/src/aria2_rpc/query.rs#L304-L370`.

## Notification ownership

The most important correctness rule is that each lifecycle transition has exactly
one emitter:

- **HTTP** start/pause/resume/stop notifications are broadcast by the RPC handler
  (`broadcast_event`), because there is no HTTP alert bridge; complete/error events
  come from the executor/lifecycle layer.
- **BT** start/pause/resume/complete/error notifications all come only from the BT
  alert bridge (`bt_backend/alerts.rs`). The RPC handler therefore fires an
  aria2 notification only when `kind == TaskKind::Http`; doing it for BT would
  duplicate every notification (a historical bug in `addTorrent`/`pause`).

Evidence: `repo://crates/limedl-core/src/aria2_rpc/download.rs#L100-L135`,
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L218-L295`,
`repo://crates/limedl-core/src/bt_backend/alerts.rs`.

## The protocol-specific accessor rule

`RpcContext::http()` and `RpcContext::bt()` are the **only** sanctioned
`get_typed` downcasts in the aria2 layer. The RPC surface is inherently
protocol-specific (`getOption` reads the HTTP manifest, `tellStatus.files` asks
the BT engine), so those downcasts are centralized; new handlers must use these
accessors rather than calling `get_typed` directly.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/context.rs#L62-L75`.

Related pages: [Protocol Routing and the Dispatcher Facade](../architecture/protocol-routing-and-dispatcher.md),
[Headless Server Daemon](headless-server-daemon.md),
[Settings and Configuration](../systems/settings-and-configuration.md),
[BitTorrent Backend](../workflows/bit-torrent-backend.md).
