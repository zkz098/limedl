---
type: integration
title: Aria2 JSON-RPC Compatibility Server
description: The aria2-compatible HTTP and WebSocket server of limedl — method routing, GID derivation and caching, three auth modes with Argon2 token storage, aria2 option translation, notification ownership, and the graceful hot-reload port handoff.
tags: [aria2, rpc, integration, authentication, json-rpc, websocket]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T01:52:48.489Z
sources:
  - id: openwiki-source-8ec1f0436491ce5daa75720b
    resource: repo://crates/limedl-core/src/aria2_rpc/context.rs
  - id: openwiki-source-a126aed2e5b28c6cc1b7781c
    resource: repo://crates/limedl-core/src/aria2_rpc/dispatch.rs
  - id: openwiki-source-4d67c2a24e4b561bd4166816
    resource: repo://crates/limedl-core/src/aria2_rpc/download.rs
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
generated: { by: "pi", at: "2026-10-04T01:52:48.489Z" }
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

Evidence: `repo://crates/limedl-core/src/aria2_rpc/server.rs#L42-L155`.

### Hot-reload port handoff

Saving settings restarts the RPC server. The predecessor drops its listener on a
different task, so the replacement can observe `AddrInUse`. `bind_with_retry`
retries `AddrInUse` for a 5 s window at 25 ms intervals and returns any other
error immediately; a port genuinely held by another process still fails after
the window. This is why a settings save does not silently lose the RPC endpoint.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/server.rs#L1-L40`.

## Authentication

`AuthConfig::from_settings` produces one of three states:

- `Disabled` — `single` mode with an empty `secret`; every request passes.
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

`dispatch_method` runs `check_token(ctx, &params)` **before** `strip_token`, then
routes via `dispatch_authorized`. The order matters and the location matters:

- With per-handler checks the `strip_token`/`check_token` order was easy to
  invert, which made a configured secret reject every legitimate request; and
  bulk methods such as `pauseAll` silently skipped authentication entirely.
- `check_token` only accepts the `token:`-prefixed first parameter, matching
  aria2's wire format. New handlers are protected automatically.

`system.multicall` calls `dispatch_authorized` for nested calls, because the outer
request was already authenticated; it strips a repeated token element but does
not re-check.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/dispatch.rs#L9-L40`,
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
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L275-L325`,
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L370-L405`.

## Method routing

`dispatch_authorized` maps method names to handlers. The list includes
`addUri`/`addTorrent`, `pause`/`unpause`/`pauseAll`/`unpauseAll`,
`remove`/`removeDownloadResult`/`purgeDownloadResult`, `tellStatus`/`tellActive`/
`tellWaiting`/`tellStopped`, `getGlobalStat`, `getOption`/`changeOption`,
`getGlobalOption`/`changeGlobalOption`, `getFiles`, `getUris`, `getPeers`,
`getSessionInfo`, `saveSession`, `shutdown`, `multicall`, plus
`system.listMethods`/`system.listNotifications`. Unknown methods return the
method-not-found error code.

`handle_list_methods` advertises exactly the implemented set (about 30 entries);
`addMetalink`, `changePosition`, `changeUri`, `getServers` and `forceShutdown` are
intentionally absent.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/dispatch.rs#L42-L65`,
`repo://crates/limedl-core/src/aria2_rpc/query.rs#L378-L413`.

### multicall response shape

Each nested result is wrapped in a **single-element array** — `[value]` on
success, `[{"code","message"}]` on failure. AriaNg/Motrix index into element 0,
so a two-element `[null, value]` wrapper would be read as a failure.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/system.rs#L51-L88`.

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

Evidence: `repo://crates/limedl-core/src/aria2_rpc/download.rs#L9-L19`,
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L21-L124`.

## Runtime option changes

`handle_change_option` deliberately supports a **restricted key set** and errors
with the key name for anything else, instead of silently ignoring it:

- `pause` (both protocols) — pause when truthy, resume otherwise.
- `select-file` (BT only) — 1-based comma-separated aria2 indices parsed by
  `parse_select_file`, converted to the engine's 0-based indices.
- `max-download-limit` / `max-upload-limit` (BT only) — the status is read first
  so the other direction is preserved, then `bt_set_speed_limit` is called.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/options.rs#L84-L115`,
`repo://crates/limedl-core/src/aria2_rpc/options.rs#L300-L385`.

## Terminal-result visibility

`max_in_memory_downloads` only evicts tasks from memory. `summary_for_raw_id`
falls back to `db.get_download_header` for a task the HTTP manager no longer
holds, so `tellStatus`/`tellStopped`/`getGlobalStat` remain correct until
`purgeDownloadResult` or `removeDownloadResult` deletes the row. `tellStopped`
also uses `list_download_headers` and `count_terminal_downloads`.

The `keys` filter is supported on the query methods with aria2's positional
indices (1 for `tellStatus`, 0 for `tellActive`, 2 for `tellWaiting`/`tellStopped`).

Evidence: `repo://crates/limedl-core/src/aria2_rpc/query.rs#L39-L57`,
`repo://crates/limedl-core/src/aria2_rpc/query.rs#L59-L135`,
`repo://crates/limedl-core/src/aria2_rpc/query.rs#L131-L210`.

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

Evidence: `repo://crates/limedl-core/src/aria2_rpc/download.rs#L110-L122`,
`repo://crates/limedl-core/src/aria2_rpc/download.rs#L205-L272`,
`repo://crates/limedl-core/src/bt_backend/alerts.rs`.

## The protocol-specific accessor rule

`RpcContext::http()` and `RpcContext::bt()` are the **only** sanctioned
`get_typed` downcasts in the aria2 layer. The RPC surface is inherently
protocol-specific (`getOption` reads the HTTP manifest, `tellStatus.files` asks
the BT engine), so those downcasts are centralized; new handlers must use these
accessors rather than calling `get_typed` directly.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/context.rs#L46-L68`.

<!-- openwiki: broken internal link [/openwiki/architecture/protocol-routing-and-dispatcher.md] link "/openwiki/architecture/protocol-routing-and-dispatcher.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Related pages: [Protocol Routing and the Dispatcher Facade](/openwiki/architecture/protocol-routing-and-dispatcher.md),
<!-- openwiki: broken internal link [/openwiki/systems/settings-and-configuration.md] link "/openwiki/systems/settings-and-configuration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Settings and Configuration](/openwiki/systems/settings-and-configuration.md),
<!-- openwiki: broken internal link [/openwiki/workflows/bit-torrent-backend.md] link "/openwiki/workflows/bit-torrent-backend.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[BitTorrent Backend](/openwiki/workflows/bit-torrent-backend.md).
