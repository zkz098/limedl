---
type: workflow
title: Metalink Parsing and Mirror Selection
description: limedl's Metalink subsystem — Metalink 4.0/3.0 and Metalink/HTTP parsing, best-checksum and priority selection, the scored MirrorPool with leasing and cooldown, latency probing, and how aria2.addMetalink and addUri mirror_urls feed the HTTP download failover loop.
tags: [metalink, mirrors, failover, parsing, aria2, download]
sources:
  - id: openwiki-source-4d67c2a24e4b561bd4166816
    resource: repo://crates/limedl-core/src/aria2_rpc/download.rs
  - id: openwiki-source-c8ca1e3187dacc725ff333e9
    resource: repo://crates/limedl-core/src/aria2_rpc/e2e_tests.rs
  - id: openwiki-source-d256d453001d5dddbed5926e
    resource: repo://crates/limedl-core/src/database/schema.rs
  - id: openwiki-source-4b83dfd7538606d5871938ef
    resource: repo://crates/limedl-core/src/dispatcher.rs
  - id: openwiki-source-9fa813eab6e27ac3f5fdbf1d
    resource: repo://crates/limedl-core/src/manifest.rs
  - id: openwiki-source-027128edc8d4f44ef9628acf
    resource: repo://crates/limedl-core/src/metalink/http_link.rs
  - id: openwiki-source-abe7ffde604a66915cdc6d18
    resource: repo://crates/limedl-core/src/metalink/mod.rs
  - id: openwiki-source-40f68d06f46cb8735118d90d
    resource: repo://crates/limedl-core/src/metalink/pool.rs
  - id: openwiki-source-9ebbf922b423430217bcc21b
    resource: repo://crates/limedl-core/src/metalink/prober.rs
  - id: openwiki-source-f77703f849e27c9a6588180d
    resource: repo://crates/limedl-core/src/metalink/scorer.rs
  - id: openwiki-source-60ceefd6554ff8fc9f67e129
    resource: repo://crates/limedl-core/src/metalink/tests.rs
  - id: openwiki-source-9d2faab5a599482a9d6c59c8
    resource: repo://crates/limedl-core/src/metalink/types.rs
  - id: openwiki-source-d8cd2c1d62d641336187bee3
    resource: repo://crates/limedl-core/src/metalink/xml_parser.rs
  - id: openwiki-source-098d28438aacd15b419786dc
    resource: repo://crates/limedl-core/src/task_lifecycle/mod.rs
generated: { by: "pi", at: "2026-10-07T04:23:45.747Z" }
verified:
  - by: openwiki/0.7.1
    at: 2026-10-07T04:23:45.747Z
---

# Metalink Parsing and Mirror Selection

`crates/limedl-core/src/metalink/` implements the Metalink family of formats so a
single download can be served from several mirrors and verified against a
declared hash: Metalink 4.0 (RFC 5854), Metalink 3.0, and the Metalink/HTTP
response-header extensions (RFC 6249 with RFC 3230/5843 digests). The module is a
pure library plus a set of tested runtime components; the parts the HTTP engine
actually consumes are the parsed document and the `mirror_urls` list on a
`StartDownloadRequest`.

Evidence: `repo://crates/limedl-core/src/metalink/mod.rs#L1-L21`,
`repo://crates/limedl-core/src/lib.rs#L23-L23`.

## XML parsing (`parse_metalink_xml`)

`parse_metalink_xml(xml)` streams the document with `quick-xml` and builds a
`MetalinkDocument`. It is a small state machine: the public function owns the
reader, enforces the size limit and delegates each `Start`/`Text`/`End` event to
`MetalinkBuilder` (`on_start`/`on_text`/`on_end`), with per-element attributes
held in `MetalinkAttrs`. Rules that matter:

- Payloads above 10 MiB are rejected up front with `InvalidRequest`, so a hostile
  or accidental large body cannot exhaust memory.
- A document with no `<file>` entries is an `InvalidResponse`, not an empty
  success.
- The `<file name>` is reduced to its basename and sanitized with
  `sanitize_filename`; an empty result becomes `download`. This prevents a
  Metalink from writing outside the chosen directory.
- Mirror priority is normalized so that **1 is highest** (RFC 5854 semantics):
  an explicit `priority` attribute wins, otherwise `preference` is inverted
  (`101 - min(preference, 100)`), otherwise the default is 100.
- `<url>` entries whose `type` is `bittorrent` (or any non-HTTP type that does not
  start with `http://`/`https://`) go to `metaurls`; HTTP(S) URLs become
  `MirrorResource`s. `<metaurl>` elements become `MetalinkMetaUrl`s.
- Whole-file `<hash>` elements become `ChecksumEntry`s; a `<pieces>` block keeps
  its `length`/`type` and its per-piece hashes as `PieceVerification`.
- `identity`, `version`, `language`, `os`, `origin` and `published` are preserved
  for selection/display.

Only SHA-256, SHA-512 and BLAKE3 are recognized (`parse_algo`); anything else
parses to `ChecksumMode::None` and is ignored by selection.

Evidence: `repo://crates/limedl-core/src/metalink/xml_parser.rs#L12-L39`,
`repo://crates/limedl-core/src/metalink/xml_parser.rs#L93-L290`,
`repo://crates/limedl-core/src/metalink/xml_parser.rs#L306-L314`.

## Metalink/HTTP headers (`parse_metalink_headers`)

A server can advertise the same information without a document:

- `Link: <url>; rel="duplicate"; pri=1; geo=de` adds a mirror; `pref` clamps the
  priority to 10 or better.
- `Link: <...>; rel="describedby"; type="application/metalink4+xml"` (or a
  `.meta4`/`.metalink` URL) records the external document URL in
  `metalink_document_url`.
- `Digest: SHA-256=<base64>` / `SHA-512=<base64>` becomes a `ChecksumEntry` with
  the digest converted to lowercase hex.

Evidence: `repo://crates/limedl-core/src/metalink/http_link.rs#L1-L134`.

## Types and selection helpers

`MetalinkDocument { files, origin, published }` wraps `MetalinkFile`, which
carries `hashes`, `pieces`, `resources`, `metaurls` and the metadata fields.
Two helpers do the per-file selection:

- `MetalinkFile::best_checksum()` picks the strongest supported whole-file hash
  in the order **SHA-512 > SHA-256 > BLAKE3**, returning `(ChecksumMode, hash)`.
- `MetalinkFile::sorted_mirror_urls()` sorts the resources by ascending priority
  (lowest number = most preferred) and returns the URLs.

`MirrorResource` is the shared unit used by the parser, the pool and the prober:
`url`, normalized `priority`, optional ISO-3166 `location` and optional
`max_connections`.

Evidence: `repo://crates/limedl-core/src/metalink/types.rs#L5-L66`.

## Mirror scoring (`calculate_mirror_score`)

`calculate_mirror_score(resource, rtt_ms, consecutive_errors, config)` produces a
higher-is-better composite:

| Factor | Contribution |
| --- | --- |
| Server priority | `100 / max(priority, 1)` |
| HTTPS when preferred | `+15` |
| Geographic match with `preferred_location` | `+50`; close region `+25` |
| Measured RTT | `80` (≤50 ms) down to `0` (>800 ms) |
| Consecutive errors | `-40` each |

`MirrorScoringConfig` holds `preferred_location` (lowercase country code) and
`preferred_protocol` (default `https`). `are_regions_close` groups East Asia,
Europe and North America so a same-region mirror beats a distant one even
without an exact country match.

Evidence: `repo://crates/limedl-core/src/metalink/scorer.rs#L1-L90`.

## MirrorPool: leasing, health and cooldown

`MirrorPool` is a `Clone` handle over `Arc<Mutex<MirrorPoolInner>>` holding one
`MirrorCandidate` per resource with live metrics (`active_connections`, `score`,
`consecutive_errors`, `backoff_until`, `total_bytes_downloaded`,
`chunks_completed`, `rtt_ms`).

- `MirrorPool::new` seeds each candidate's score from the initial priority and
  config; `from_urls(primary, mirrors, …)` builds a pool from a plain URL list,
  giving the primary priority 1 and the rest ascending priorities.
- `lease_best_mirror()` skips mirrors that are cooling down or at
  `max_connections`, picks the highest score, and — when every mirror is at
  capacity — falls back to the one with the fewest active connections (still
  skipping cooling-down mirrors). It increments the lease count and returns a
  `MirrorLease`.
- `MirrorLease` is RAII: dropping it decrements the mirror's active connections,
  so a worker cannot leak a slot on an early return.
- `report_success(url, bytes)` resets errors/cooldown and adds bytes and one
  completed chunk; `report_failure(url)` increments errors and applies
  exponential backoff (`2^min(errors, 6)` seconds, capped at 60 s);
  `report_corrupted_piece(url)` adds five errors and isolates the mirror for
  300 s.
- `update_rtt(url, rtt_ms)` folds a probe result into the score.
- `snapshots()` returns serializable `MirrorStatusSnapshot`s (including
  `is_cooling_down`) for UI/RPC display.

Evidence: `repo://crates/limedl-core/src/metalink/pool.rs#L1-L305`.

## Latency probing (`probe_mirrors`)

`probe_mirrors(client, mirrors, concurrency_limit, probe_timeout)` runs a bounded
`FuturesUnordered` pool. Each probe issues `GET` with `Range: bytes=0-0` and the
per-probe timeout, then reports reachability, RTT and whether the server
answered `206 Partial Content` (or advertised `Accept-Ranges: bytes`). This is
the input for `MirrorPool::update_rtt` and for deciding whether a mirror can
serve parallel chunks.

Evidence: `repo://crates/limedl-core/src/metalink/prober.rs#L1-L105`.

## Entrypoints into the download engine

### `aria2.addMetalink`

`handle_add_metalink` accepts a base64-encoded Metalink XML document. It decodes
the payload, parses it with `parse_metalink_xml`, honours the 1-based
`select-file` option, and starts one HTTP task per selected file:

- the best-priority URL is the primary and the remaining `sorted_mirror_urls()`
  become `mirror_urls` (only when more than one exists);
- `best_checksum()` supplies `checksum` + `expected_checksum`;
- `dir`, `out`, `pause` and `position` follow the `addUri` rules;
- the return value is the array of created GIDs.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/download.rs#L219-L316`.

### `aria2.addUri` and the Dispatcher

`handle_add_uri` treats the `uris` array as an ordered candidate list: element 0
is the primary URL and the rest are stored as `mirror_urls`. Independently,
`Dispatcher::start` fills `mirror_urls` from the URL-rewrite rules when the
caller supplied none, using `resolve_mirror_urls` (settings
`url_rewrite.enabled` + `rewrite_url`). `DownloadManager` also exposes
`mirror_urls_for`, which re-resolves the candidate list at resume time so a
settings change takes effect.

Evidence: `repo://crates/limedl-core/src/aria2_rpc/download.rs#L21-L135`,
`repo://crates/limedl-core/src/dispatcher.rs#L148-L170`,
`repo://crates/limedl-core/src/manager.rs#L831-L845`.

## Engine failover and persistence

The manifest carries `url` (primary), `mirror_urls` (candidate list),
`mirror_url` (the currently used candidate) and `current_mirror_index`.
`spawn_download` walks `mirror_urls` from `current_mirror_index` and, when more
than one candidate exists, uses **one retry per URL** (`actual_retries = 1`) and
fails over to the next candidate only on a network-class error
(`is_network_error`); any other error fails the task immediately. Before each
attempt it records the index and mirror URL on both manifest and snapshot, and
resolves the (possibly CDN-aware) client for that URL.

The mirror state is persisted in the `downloads` table by migration v3
(`mirror_url`, `mirror_urls` JSON text, `current_mirror_index`), so a restart
resumes on the same candidate. `aria2.changeUri` rewrites the candidate list and
keeps the primary URL, mirror list, current index and snapshot consistent.

Evidence: `repo://crates/limedl-core/src/task_lifecycle/mod.rs#L85-L200`,
`repo://crates/limedl-core/src/manifest.rs#L54-L62`,
`repo://crates/limedl-core/src/database/schema.rs#L112-L140`,
`repo://crates/limedl-core/src/manager.rs#L845-L935`.

## What is wired vs. what is a library surface

`MirrorPool`, `probe_mirrors` and `parse_metalink_headers` are fully implemented
and unit-tested, but the HTTP executor currently selects and fails over using the
simple ordered `mirror_urls` list above; it does not yet lease from a `MirrorPool`
or consume Metalink/HTTP response headers. Treat those components as the
prepared seam for scored, health-aware mirror selection rather than as the live
data path.

Evidence: `repo://crates/limedl-core/src/metalink/mod.rs#L1-L21`,
`repo://crates/limedl-core/src/task_lifecycle/mod.rs#L85-L200`.

## Tests

`metalink/tests.rs` covers Metalink 4.0 and 3.0 parsing, Metalink/HTTP header
parsing, mirror scoring, and pool leasing/concurrency limits. On the RPC side,
`e2e_add_metalink_creates_downloads_and_reports_status` proves `aria2.addMetalink`
creates downloads that report status.

Evidence: `repo://crates/limedl-core/src/metalink/tests.rs#L1-L222`,
`repo://crates/limedl-core/src/aria2_rpc/e2e_tests.rs#L2404-L2447`.

## Extension seams

- A new Metalink element/algorithm is a `parse_algo`/`types` change plus a parser
  arm; unknown algorithms already degrade to `ChecksumMode::None`.
- Wiring scored mirror selection into the executor means leasing from
  `MirrorPool` (or building one from the manifest's `mirror_urls`) and calling
  `report_success`/`report_failure`/`report_corrupted_piece` from the worker
  paths; the pool already carries the cooldown and capacity semantics.
- Consuming Metalink/HTTP headers means calling `parse_metalink_headers` on the
  probe response and merging its mirrors/digest into the manifest before
  planning.

Related pages: [HTTP Download Lifecycle](http-download-lifecycle.md),
[Aria2 JSON-RPC Compatibility Server](../integrations/aria2-rpc-server.md),
[Testing Strategy](../testing/testing-strategy.md),
[Networking, HTTP Clients and Rate Control](../systems/networking-and-rate-control.md).
