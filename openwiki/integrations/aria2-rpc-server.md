---
type: integration
title: Aria2 JSON-RPC Compatibility Server
description: The aria2-compatible HTTP and WebSocket server of limedl — method routing, GID derivation and caching, three auth modes with Argon2 token storage, type-driven request parsing and aria2 option translation, the fuller tellStatus/getServers/changeUri/changePosition surface, notification ownership, and the graceful hot-reload port handoff.
tags: [aria2, rpc, integration, authentication, json-rpc, websocket]
sources:
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
  - id: openwiki-source-093388d09b520118fa26ce32
    resource: repo://crates/limedl-core/src/bt_backend/alerts.rs
  - id: openwiki-source-0c4cd5f8953750852a1f4d6d
    resource: repo://crates/limedl-core/src/manager.rs
  - id: openwiki-source-7ef10e5bb7f9bf65c86b6285
    resource: repo://crates/limedl-core/src/types/settings.rs
generated: { by: "pi", at: "2026-10-04T10:20:09.270Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T10:20:09.270Z
---

# Aria2 JSON-RPC Compatibility Server

`crates/limedl-core/src/aria2_rpc/` implements an aria2-compatible JSON-RPC 2.0
server over HTTP and WebSocket so clients such as AriaNg and Motrix can drive
limedl. It is behind the optional `aria2-rpc` feature (enabled by the desktop
crate and available for server builds). Internal downloads are mapped to aria2
GIDs.

The server is the second frontend over the same `Dispatcher` used by the Slint
desktop, so both stay in sync through the `EventBus`.

## Server assembly and lifecycle

`Aria2RpcServer::new(registry, &settings, event_bus)` builds an `RpcContext` with
the registry, an `AuthConfig` derived from settings, a GID cache and a fresh
session id, and binds `127.0.0.1:{settings.port}`. `serve(shutdown, cors)`
assembles an axum `Router` with `POST /jsonrpc` (HTTP JSON-RPC) and
`GET /jsonrpc` (WebSocket upgrade), applies a CORS layer, and runs
`axum::serve(...).with_graceful_shutdown(...)` driven by a `watch` receiver.

CORS defaults to `http://localhost` / `http://127.0.0.1`; configured origins that
all fail to parse fall back to localhost with a warning.

**The service is disabled by default.** `Aria2RpcSettings::default()` has
`enabled = false`, because with an empty `secret` the endpoint answers
anonymously and loopback-only binding still lets any local process — and any page
the default CORS policy admits — drive downloads and read destination paths.
Enabling it is an explicit Settings action; an existing `settings.json` with
`"enabled": true` keeps working. When the server does come up with
`AuthConfig::Disabled`, `serve` logs a warning naming the address and pointing at
the secret/per-client options. That check reads `AuthConfig::is_enabled` *before*
the context is moved into the router, which is why the method is `pub(crate)`.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/server.rs#L71-L150`,
`repo://crates/limedl-core/src/aria2_rpc/context.rs#L39-L41`,
`repo://crates/limedl-core/src/types/settings.rs#L380-L400`.

### Hot-reload port handoff

Saving settings restarts the RPC server. The predecessor drops its listener on a
different task, so the replacement can observe `AddrInUse`. `bind_with_retry`
retries `AddrInUse` for a 5 s window at 25 ms intervals and returns any other
error immediately; a port genuinely held by another process still fails after
the window. This is why a settings save does not silently lose the RPC endpoint.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/server.rs#L1-L40`.

## Authentication

`AuthConfig::from_settings` produces one of three states:

- `Disabled` — `single` mode with an empty `secret`; every request passes. It is
  reachable in practice only through the legacy `"enabled": true` +
  `"secret": null` combination, since a fresh install starts disabled.
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
`repo://crates/limedl-core/src/aria2_rpc/context.rs#L70-L110`.

## GIDs: stable identity across restarts

`internal_id_to_gid(raw_id)` is `xxh3_64(raw_id)` formatted as 16 lowercase hex
digits. The underlying task id is a UUID for HTTP and the info hash for BT, both
of which are stable across restarts, so a GID is stable too. `resolve_gid` checks
the cache first, then scans all backends' `list()` and populates the cache on a
hit. The cache is an optimization, not the source of truth.

Lifecycle correctness: `addUri` and `resolve_gid`'s scan insert entries, and
`aria2.remove`, `aria2.removeDownloadResult` and `purgeDownloadResult` evict
them — otherwise the cache would grow without bound in a long session.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/protocol.rs#L74-L105`,
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
still deliberately absent (Metalink is out of scope); the HTTP GET/JSONP/Batch
transports are also not served.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/dispatch.rs#L44-L86`,
`repo://crates/limedl-core/src/aria2_rpc/query.rs#L568-L612`.

### multicall response shape

Each nested result is wrapped in a **single-element array** — `[value]` on
success, `[{"code","message"}]` on failure. AriaNg/Motrix index into element 0,
so a two-element `[null, value]` wrapper would be read as a failure. Nested
calls are authenticated individually (see centralized validation), and the
Tier 1 test `interop_ariang_multicall_shape` sends them with only per-call
tokens, as aria2 clients do.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/system.rs#L51-L90`.

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
is returned and cached instead of starting a second task.

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
`repo://crates/limedl-core/src/aria2_rpc/options.rs#L89-L142`.

## Runtime option changes

`handle_change_option` deliberately supports a **restricted key set** and errors
with the key name for anything else, instead of silently ignoring it:

- `pause` (both protocols) — pause when truthy, resume otherwise.
- `select-file` (BT only) — 1-based comma-separated aria2 indices parsed by
  `parse_select_file`, converted to the engine's 0-based indices.
- `max-download-limit` / `max-upload-limit` (BT only) — the status is read first
  so the other direction is preserved, then `bt_set_speed_limit` is called.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/options.rs#L130-L160`,
`repo://crates/limedl-core/src/aria2_rpc/options.rs#L351-L426`.

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
  acknowledges and warns the UI instead of exiting: limedl runs as a managed
  subsystem and the application UI owns exit.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/query.rs#L427-L490`,
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L462-L606`,
`repo://crates/limedl-core/src/manager.rs#L845-L935`.

## Known, intentional deviations

These are documented choices, not bugs, and are tracked alongside the Tier 1/Tier
2 plan in `docs/aria2-interop-testing.md`:

- `aria2.addMetalink` is not implemented, and HTTP GET/JSONP/Batch transports are
  not served (POST + WebSocket only).
- `changePosition` collapses onto the priority model (above).
- BitTorrent `tellStatus.dir`/`getOption.dir` report the BT backend's default
  output dir, not a per-task `dir`, because the BT backend does not track one;
  `addTorrent`'s `out`/`select-file` are parsed but not applied at start (use
  `aria2.changeOption` after metadata).
- `getOption` returns several fixed placeholder values and `getGlobalOption` only
  a subset; `getVersion` still reports a stale version/feature list.
- GID prefix matching is not supported (exact GID only); HTTPS RPC, HTTP Basic
  auth and `--rpc-listen-all` are not served.

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

Evidence: `repo://crates/limedl-core/src/aria2_rpc/context.rs#L46-L68`.

Related pages: [Protocol Routing and the Dispatcher Facade](../architecture/protocol-routing-and-dispatcher.md),
[Settings and Configuration](../systems/settings-and-configuration.md),
[BitTorrent Backend](../workflows/bit-torrent-backend.md).
