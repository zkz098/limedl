---
id: workflows/metalink-and-mirror-selection
title: Metalink Parsing and Mirror Selection
status: verified
generated_at: 2026-10-31T15:04:49Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence:
- path: crates/limedl-engine/src/metalink/mod.rs
  lines:
  - 1
  - 21
  blob: cf6cc5fa5c501540ebe0221dc16f58b8c57b600c
  snippet_sha1: fa4b00874797f06cab99bad480242a5ad5182036
- path: crates/limedl-engine/src/metalink/xml_parser.rs
  lines:
  - 12
  - 42
  blob: 3f53184e7d1b2ce4569008d119ee74e2527829df
  snippet_sha1: f4822fe95c8c40c6e6910b7951e8e29118132620
- path: crates/limedl-engine/src/metalink/xml_parser.rs
  lines:
  - 13
  - 17
  blob: 3f53184e7d1b2ce4569008d119ee74e2527829df
  snippet_sha1: eb9c7d1cd0907cf8433e616ea3c0802155b7c8ce
- path: crates/limedl-engine/src/metalink/xml_parser.rs
  lines:
  - 291
  - 303
  blob: 3f53184e7d1b2ce4569008d119ee74e2527829df
  snippet_sha1: 1309795a3d9b1736ce8aca181bb523aa5101d6c0
- path: crates/limedl-engine/src/metalink/xml_parser.rs
  lines:
  - 105
  - 120
  blob: 3f53184e7d1b2ce4569008d119ee74e2527829df
  snippet_sha1: 5e64307fe53245ac626c86b207c2324eb515c8bc
- path: crates/limedl-engine/src/metalink/xml_parser.rs
  lines:
  - 221
  - 265
  blob: 3f53184e7d1b2ce4569008d119ee74e2527829df
  snippet_sha1: 29d5ae8be7c74d7cd32876bb9e3398d1073df593
- path: crates/limedl-engine/src/metalink/xml_parser.rs
  lines:
  - 122
  - 137
  blob: 3f53184e7d1b2ce4569008d119ee74e2527829df
  snippet_sha1: ccf69aadb95d25b556d1e0924ab0d95e999a7c61
- path: crates/limedl-engine/src/metalink/xml_parser.rs
  lines:
  - 202
  - 219
  blob: 3f53184e7d1b2ce4569008d119ee74e2527829df
  snippet_sha1: 66cc33ef74fe2cdca4e5e7be314e88e475f26408
- path: crates/limedl-engine/src/metalink/xml_parser.rs
  lines:
  - 166
  - 199
  blob: 3f53184e7d1b2ce4569008d119ee74e2527829df
  snippet_sha1: 27297dc33604572124e912bdfc5015ee6929a8b1
- path: crates/limedl-engine/src/metalink/xml_parser.rs
  lines:
  - 306
  - 314
  blob: 3f53184e7d1b2ce4569008d119ee74e2527829df
  snippet_sha1: 40c1b4da92ab2a0df57e0044ee16c041d89acfb7
- path: crates/limedl-engine/src/metalink/http_link.rs
  lines:
  - 20
  - 115
  blob: ae9f146edec42aa4df573348414dc11c4bfa31bb
  snippet_sha1: d80e8cfbd618cc0b623ca57a83a7d64628d8dfb4
- path: crates/limedl-engine/src/metalink/http_link.rs
  lines:
  - 117
  - 136
  blob: ae9f146edec42aa4df573348414dc11c4bfa31bb
  snippet_sha1: 189a2f3755d0c5a27f421c8c4a1c8bfe23541b9b
- path: crates/limedl-engine/src/metalink/types.rs
  lines:
  - 5
  - 64
  blob: 1642915c3707abf4e0de6ed6d845eedb9e1e1953
  snippet_sha1: d597eaa31ae67e361e3957d06404cdd5b7b0a3e3
- path: crates/limedl-engine/src/metalink/types.rs
  lines:
  - 84
  - 95
  blob: 1642915c3707abf4e0de6ed6d845eedb9e1e1953
  snippet_sha1: cfc38b367a3a91b639499fae1cc01a533d17b215
- path: crates/limedl-engine/src/metalink/scorer.rs
  lines:
  - 21
  - 66
  blob: 4b7367d657475e07f79e8dd74013f177bc32ea9a
  snippet_sha1: 1de14d56d5ffabb4a80c75ba827f98d3f2cb55f8
- path: crates/limedl-engine/src/metalink/scorer.rs
  lines:
  - 1
  - 19
  blob: 4b7367d657475e07f79e8dd74013f177bc32ea9a
  snippet_sha1: 95334fbc7d4ad6116f58d6bfc88b8aebb88635a4
- path: crates/limedl-engine/src/metalink/scorer.rs
  lines:
  - 68
  - 93
  blob: 4b7367d657475e07f79e8dd74013f177bc32ea9a
  snippet_sha1: 0ec7f48b368e3b4b25e1d1d08119849d364bd62e
- path: crates/limedl-engine/src/metalink/pool.rs
  lines:
  - 10
  - 53
  blob: 80364f39178e2561c83ec1172e109f78d3158135
  snippet_sha1: cf9b1b17559a4bf69dc580308e36b96dd1f41492
- path: crates/limedl-engine/src/metalink/pool.rs
  lines:
  - 55
  - 123
  blob: 80364f39178e2561c83ec1172e109f78d3158135
  snippet_sha1: c720f08a6484ea84fe122c0212f584db8729f84c
- path: crates/limedl-engine/src/metalink/pool.rs
  lines:
  - 142
  - 191
  blob: 80364f39178e2561c83ec1172e109f78d3158135
  snippet_sha1: 478cafaf0943437e70d3f99a3d595aafc0f55d6b
- path: crates/limedl-engine/src/metalink/pool.rs
  lines:
  - 280
  - 306
  blob: 80364f39178e2561c83ec1172e109f78d3158135
  snippet_sha1: 2c571003f11830d3597ff3013e687e587f3e830e
- path: crates/limedl-engine/src/metalink/pool.rs
  lines:
  - 193
  - 248
  blob: 80364f39178e2561c83ec1172e109f78d3158135
  snippet_sha1: 93f8485e3cb824a211c714fd0f1d6d4a8ab3488b
- path: crates/limedl-engine/src/metalink/pool.rs
  lines:
  - 125
  - 140
  blob: 80364f39178e2561c83ec1172e109f78d3158135
  snippet_sha1: c0b57d737dd7771bd78cce1dac895324354a301f
- path: crates/limedl-engine/src/metalink/pool.rs
  lines:
  - 257
  - 277
  blob: 80364f39178e2561c83ec1172e109f78d3158135
  snippet_sha1: ba3ec471e69a16b00b51427716388c9d226644c7
- path: crates/limedl-engine/src/metalink/prober.rs
  lines:
  - 20
  - 105
  blob: fded440612be9f5eda282c8a90862ea5d276a73a
  snippet_sha1: 5430a633b88319b2588d93bcce85f6c9044c0b79
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 230
  - 341
  blob: 6d4ba265077514cf12750f572dff878f6ba8f2fd
  snippet_sha1: dff372a8eb7519f06da1f625ae8c06c9737d4805
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 318
  - 323
  blob: 6d4ba265077514cf12750f572dff878f6ba8f2fd
  snippet_sha1: b2adefe15241dccbe20cb51e9bc7dcbb7fbcd174
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 46
  - 93
  blob: 6d4ba265077514cf12750f572dff878f6ba8f2fd
  snippet_sha1: d1115f6a284efd3414c5dbe78d4cbd5e40065f20
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 148
  - 170
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 35e6a0a39292b4455eb4bbb0663b8d28833fbde5
- path: crates/limedl-core/src/manager.rs
  lines:
  - 836
  - 850
  blob: fa2c301dce0e94dfff5d64faea9fa56e053c3b70
  snippet_sha1: 33c274162455553f89eecad78ebd2da34a149030
- path: crates/limedl-core/src/manager.rs
  lines:
  - 744
  - 750
  blob: fa2c301dce0e94dfff5d64faea9fa56e053c3b70
  snippet_sha1: ec08540616b9a036a4ad759107dae6c081b5122d
- path: crates/limedl-types/src/manifest.rs
  lines:
  - 55
  - 60
  blob: 54ad684831aca2c5fa70d6650426e2a12e96fd1e
  snippet_sha1: ebfad4fc9b83e315550429afb38f44ae2ae81e1c
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 99
  - 150
  blob: e2b0f898b2e1fea578b56a582f8dd6d4165d682a
  snippet_sha1: 35de1cc207ef69c0cba79b669ac9541c1799d761
- path: crates/limedl-store/src/database/schema.rs
  lines:
  - 137
  - 154
  blob: ea561ba2f671bb2bd40c2a609e106f177aa5e9b4
  snippet_sha1: c3e08afb5e9701077a64685fea83eb829b803ff2
- path: crates/limedl-core/src/manager.rs
  lines:
  - 852
  - 879
  blob: fa2c301dce0e94dfff5d64faea9fa56e053c3b70
  snippet_sha1: d0c7b47318584a70bee2c1bbab9a1404b593b0df
- path: crates/limedl-engine/src/http_executor/chunked.rs
  lines:
  - 59
  - 75
  blob: bae71a514c0f0bd275dbb0d373f42cc94611629f
  snippet_sha1: 682fe64c7c42672de8d5a21eeb5f3e906c04507e
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 440
  - 455
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: 23f011ce266dced020ae9b476c0dbc29f00c5150
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 240
  - 253
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: c79258140c505821bb0c0617719365cd2dc9ff9b
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 544
  - 560
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: c8a5140db7a37fbdb6439e0b3cefde500b448c69
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 813
  - 825
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: 21878c24d98fa90c7f91de7c9d59b4c91bb5f6ce
- path: crates/limedl-engine/src/metalink/http_link.rs
  lines:
  - 19
  - 21
  blob: ae9f146edec42aa4df573348414dc11c4bfa31bb
  snippet_sha1: 7c3327823f9344afd92b82af5da757b7c4c55b20
- path: crates/limedl-engine/src/metalink/tests.rs
  lines:
  - 7
  - 40
  blob: 66bc40c311e6537226247116c0e8d80e198ae246
  snippet_sha1: c6da9b31ac665d98bcf1ff839ed7fbd393f45221
- path: crates/limedl-rpc/src/e2e_tests.rs
  lines:
  - 2434
  - 2478
  blob: 68213e81202fc594e5cfd0ba1e6755dadeb8592a
  snippet_sha1: be2605fd1bbecceb8f7ae009115e6dea85ba2b7e
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 546
  - 560
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: 0bc6dd9945e1673f09d40ef432d079f48fcff223
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 158
  - 166
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: 15ddfc98dd3d92fb4cda67523420e2d98c7b54c5
type: workflow
---
# Metalink Parsing and Mirror Selection

`crates/limedl-engine/src/metalink/` implements the Metalink family of formats so
a single download can be served from several mirrors and verified against a
declared hash: Metalink 4.0 (RFC 5854), Metalink 3.0, and the Metalink/HTTP
response-header extensions (RFC 6249 with RFC 3230/5843 digests). The module is a
pure library plus a set of tested runtime components; the parts the RPC layer
and the HTTP engine actually consume are the parsed document, the
`mirror_urls` list on a `StartDownloadRequest`, and — since the executor picked
up scored leasing — a runtime `MirrorPool`
[[cite:crates/limedl-engine/src/metalink/mod.rs:L1-L21]].

## XML parsing (`parse_metalink_xml`)

`parse_metalink_xml(xml)` streams the document with `quick-xml` and builds a
`MetalinkDocument`. It is a small state machine: the public function owns the
reader, enforces the size limit and delegates each `Start`/`Text`/`End` event to
`MetalinkBuilder` (`on_start`/`on_text`/`on_end`), with per-element attributes
held in `MetalinkAttrs`. Rules that matter
[[cite:crates/limedl-engine/src/metalink/xml_parser.rs:L12-L42]]:

- Payloads above 10 MiB are rejected up front with `InvalidRequest`, so a hostile
  or accidental large body cannot exhaust memory
  [[cite:crates/limedl-engine/src/metalink/xml_parser.rs:L13-L17]].
- A document with no `<file>` entries is an `InvalidResponse`, not an empty
  success [[cite:crates/limedl-engine/src/metalink/xml_parser.rs:L291-L303]].
- The `<file name>` is reduced to its basename and sanitized with
  `sanitize_filename`; an empty result becomes `download`. This prevents a
  Metalink from writing outside the chosen directory
  [[cite:crates/limedl-engine/src/metalink/xml_parser.rs:L105-L120]].
- Mirror priority is normalized so that **1 is highest** (RFC 5854 semantics):
  an explicit `priority` attribute wins, otherwise `preference` is inverted
  (`101 - min(preference, 100)`), otherwise the default is 100.
- `<url>` entries whose `type` is `bittorrent` go to `metaurls`; HTTP(S) URLs
  become `MirrorResource`s; `<metaurl>` elements become `MetalinkMetaUrl`s
  [[cite:crates/limedl-engine/src/metalink/xml_parser.rs:L221-L265]].
- Whole-file `<hash>` elements become `ChecksumEntry`s; a `<pieces>` block keeps
  its `length`/`type` and its per-piece hashes as `PieceVerification`
  [[cite:crates/limedl-engine/src/metalink/xml_parser.rs:L122-L137]],
  [[cite:crates/limedl-engine/src/metalink/xml_parser.rs:L202-L219]].
- `identity`, `version`, `language`, `os`, `origin` and `published` are preserved
  for selection/display [[cite:crates/limedl-engine/src/metalink/xml_parser.rs:L166-L199]].

Only SHA-256, SHA-512 and BLAKE3 are recognized (`parse_algo`); anything else
parses to `ChecksumMode::None` and is ignored by selection
[[cite:crates/limedl-engine/src/metalink/xml_parser.rs:L306-L314]].

## Metalink/HTTP headers (`parse_metalink_headers`)

A server can advertise the same information without a document
[[cite:crates/limedl-engine/src/metalink/http_link.rs:L20-L115]]:

- `Link: <url>; rel="duplicate"; pri=1; geo=de` adds a mirror; `pref` clamps the
  priority to 10 or better.
- `Link: <...>; rel="describedby"; type="application/metalink4+xml"` (or a
  `.meta4`/`.metalink` URL) records the external document URL in
  `metalink_document_url`.
- `Digest: SHA-256=<base64>` / `SHA-512=<base64>` becomes a `ChecksumEntry` with
  the digest converted to lowercase hex
  [[cite:crates/limedl-engine/src/metalink/http_link.rs:L117-L136]].

## Types and selection helpers

`MetalinkDocument { files, origin, published }` wraps `MetalinkFile`, which
carries `hashes`, `pieces`, `resources`, `metaurls` and the metadata fields.
Two helpers do the per-file selection
[[cite:crates/limedl-engine/src/metalink/types.rs:L5-L64]]:

- `MetalinkFile::best_checksum()` picks the strongest supported whole-file hash
  in the order **SHA-512 > SHA-256 > BLAKE3**, returning `(ChecksumMode, hash)`.
- `MetalinkFile::sorted_mirror_urls()` sorts the resources by ascending priority
  (lowest number = most preferred) and returns the URLs.

`MirrorResource` is the shared unit used by the parser, the pool and the prober:
`url`, normalized `priority`, optional ISO-3166 `location` and optional
`max_connections` [[cite:crates/limedl-engine/src/metalink/types.rs:L84-L95]].

## Mirror scoring (`calculate_mirror_score`)

`calculate_mirror_score(resource, rtt_ms, consecutive_errors, config)` produces a
higher-is-better composite [[cite:crates/limedl-engine/src/metalink/scorer.rs:L21-L66]]:

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
without an exact country match [[cite:crates/limedl-engine/src/metalink/scorer.rs:L1-L19]],
[[cite:crates/limedl-engine/src/metalink/scorer.rs:L68-L93]].

## MirrorPool: leasing, health and cooldown

`MirrorPool` is a `Clone` handle over `Arc<Mutex<MirrorPoolInner>>` holding one
`MirrorCandidate` per resource with live metrics (`active_connections`, `score`,
`consecutive_errors`, `backoff_until`, `total_bytes_downloaded`,
`chunks_completed`, `rtt_ms`)
[[cite:crates/limedl-engine/src/metalink/pool.rs:L10-L53]].

- `MirrorPool::new` seeds each candidate's score from the initial priority and
  config; `from_urls(primary, mirrors, …)` builds a pool from a plain URL list,
  giving the primary priority 1 and the rest ascending priorities
  [[cite:crates/limedl-engine/src/metalink/pool.rs:L55-L123]].
- `lease_best_mirror()` skips mirrors that are cooling down or at
  `max_connections`, picks the highest score, and — when every mirror is at
  capacity — falls back to the one with the fewest active connections (still
  skipping cooling-down mirrors). It increments the lease count and returns a
  `MirrorLease` [[cite:crates/limedl-engine/src/metalink/pool.rs:L142-L191]].
- `MirrorLease` is RAII: dropping it decrements the mirror's active connections,
  so a worker cannot leak a slot on an early return
  [[cite:crates/limedl-engine/src/metalink/pool.rs:L280-L306]].
- `report_success(url, bytes)` resets errors/cooldown and adds bytes and one
  completed chunk; `report_failure(url)` increments errors and applies
  exponential backoff (`2^min(errors, 6)` seconds, capped at 60 s);
  `report_corrupted_piece(url)` adds five errors and isolates the mirror for
  300 s [[cite:crates/limedl-engine/src/metalink/pool.rs:L193-L248]].
- `update_rtt(url, rtt_ms)` folds a probe result into the score
  [[cite:crates/limedl-engine/src/metalink/pool.rs:L125-L140]].
- `snapshots()` returns serializable `MirrorStatusSnapshot`s (including
  `is_cooling_down`) for UI/RPC display
  [[cite:crates/limedl-engine/src/metalink/pool.rs:L257-L277]].

## Latency probing (`probe_mirrors`)

`probe_mirrors(client, mirrors, concurrency_limit, probe_timeout)` runs a bounded
`FuturesUnordered` pool. Each probe issues `GET` with `Range: bytes=0-0` and the
per-probe timeout, then reports reachability, RTT and whether the server
answered `206 Partial Content` (or advertised `Accept-Ranges: bytes`). This is
the input for `MirrorPool::update_rtt` and for deciding whether a mirror can
serve parallel chunks [[cite:crates/limedl-engine/src/metalink/prober.rs:L20-L105]].

## Entrypoints into the download engine

### `aria2.addMetalink`

`handle_add_metalink` accepts a base64-encoded Metalink XML document. It decodes
the payload, parses it with `parse_metalink_xml`, honours the 1-based
`select-file` option, and starts one HTTP task per selected file
[[cite:crates/limedl-rpc/src/download.rs:L230-L341]]:

- the best-priority URL (`sorted_mirror_urls()[0]`) is the primary and the full
  sorted list becomes `mirror_urls` (only when more than one exists);
- `best_checksum()` supplies `checksum` + `expected_checksum`;
- `dir`, `out`, `pause` and `position` follow the `addUri` rules;
- when the file declares a `<pieces>` block, the parsed `PieceVerification` is
  handed to the manager via `set_piece_verification` for progressive piece
  checking [[cite:crates/limedl-rpc/src/download.rs:L318-L323]];
- the return value is the array of created GIDs.

### `aria2.addUri` and the Dispatcher

`handle_add_uri` treats the `uris` array as an ordered candidate list: element 0
is the primary URL and, for HTTP tasks with more than one entry, the whole list
is stored as `mirror_urls` [[cite:crates/limedl-rpc/src/download.rs:L46-L93]].
Independently, `Dispatcher::start` fills `mirror_urls` from the URL-rewrite
rules when the caller supplied none, using `resolve_mirror_urls` (settings
`url_rewrite.enabled` + `rewrite_url`)
[[cite:crates/limedl-core/src/dispatcher.rs:L148-L170]]. `DownloadManager` also
exposes `mirror_urls_for`, which re-resolves the candidate list at resume time
so a settings change takes effect
[[cite:crates/limedl-core/src/manager.rs:L836-L850]],
[[cite:crates/limedl-core/src/manager.rs:L744-L750]].

## Engine failover and persistence

The manifest carries `url` (primary), `mirror_urls` (candidate list),
`mirror_url` (the currently used candidate) and `current_mirror_index`
[[cite:crates/limedl-types/src/manifest.rs:L55-L60]].
`spawn_download` walks `mirror_urls` from `current_mirror_index` and, when more
than one candidate exists, uses **one retry per URL** (`actual_retries = 1`) and
fails over to the next candidate only on a network-class error
(`is_network_error`); any other error fails the task immediately. Before each
attempt it records the index and mirror URL on both manifest and snapshot, and
resolves the (possibly CDN-aware) client for that URL
[[cite:crates/limedl-engine/src/task_lifecycle/mod.rs:L99-L150]].

The mirror state is persisted in the `downloads` table by migration v3
(`mirror_url`, `mirror_urls` JSON text, `current_mirror_index`), so a restart
resumes on the same candidate
[[cite:crates/limedl-store/src/database/schema.rs:L137-L154]].
`aria2.changeUri` rewrites the candidate list and keeps the primary URL, mirror
list, current index and snapshot consistent
[[cite:crates/limedl-core/src/manager.rs:L852-L879]].

## What is wired vs. what is a library surface

The chunked executor now leases from a live `MirrorPool`: when the manifest has
more than one candidate, `download_chunked` builds a pool via
`MirrorPool::from_urls` (with the expert `max_connections_per_host` budget), and
each segment fetch takes the best mirror's lease, overrides the request URL with
it, and reports success/failure back through the lease
[[cite:crates/limedl-engine/src/http_executor/chunked.rs:L59-L75]],
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L440-L455]],
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L240-L253]].
Metalink `<pieces>` verification is likewise wired end to end: the chunk worker
verifies the piece hash before completing a chunk and resets a mismatched chunk
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L544-L560]],
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L813-L825]].

Still library-only: `probe_mirrors` and `MirrorPool::update_rtt` have no
production caller (the pool's RTT term stays at its seeded value),
`report_corrupted_piece` is not yet called from the piece-verification failure
path, and `parse_metalink_headers` is not consumed by the probe — so Metalink/HTTP
response headers do not currently feed the manifest. Treat those as the
remaining seam
[[cite:crates/limedl-engine/src/metalink/pool.rs:L125-L140]],
[[cite:crates/limedl-engine/src/metalink/http_link.rs:L19-L21]].

## Tests

`metalink/tests.rs` covers Metalink 4.0 and 3.0 parsing, Metalink/HTTP header
parsing, mirror scoring, and pool leasing/concurrency limits
[[cite:crates/limedl-engine/src/metalink/tests.rs:L7-L40]]. On the RPC side,
`e2e_add_metalink_creates_downloads_and_reports_status` proves `aria2.addMetalink`
creates downloads that report status
[[cite:crates/limedl-rpc/src/e2e_tests.rs:L2434-L2478]].

## Extension seams

- A new Metalink element/algorithm is a `parse_algo`/`types` change plus a parser
  arm; unknown algorithms already degrade to `ChecksumMode::None`
  [[cite:crates/limedl-engine/src/metalink/xml_parser.rs:L306-L314]].
- Health-aware cooling of poisoned mirrors means calling
  `report_corrupted_piece` from the piece-mismatch path (the pool already
  carries the isolation semantics)
  [[cite:crates/limedl-engine/src/http_executor/worker.rs:L546-L560]].
- Consuming Metalink/HTTP headers means calling `parse_metalink_headers` on the
  probe response and merging its mirrors/digest into the manifest before
  planning [[cite:crates/limedl-engine/src/http_executor/run.rs:L158-L166]].

Related pages: [[wiki:workflows/http-download-lifecycle]],
[[wiki:integrations/aria2-rpc-server]],
[[wiki:testing/testing-strategy]],
[[wiki:systems/networking-and-rate-control]].
