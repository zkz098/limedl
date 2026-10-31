---
id: systems/networking-and-rate-control
title: Networking, HTTP Clients and Rate Control
status: verified
generated_at: 2026-10-31T15:02:55Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence:
- path: crates/limedl-http/src/lib.rs
  lines:
  - 1
  - 20
  blob: 1cd9e3db0dece56a2a8b205c4d1d644c7f4bc019
  snippet_sha1: 194d0b8f0cfb3a0b2cbc8db91ca480af43b35055
- path: crates/limedl-http/src/http_client_factory/mod.rs
  lines:
  - 11
  - 53
  blob: c30dfdcb24738eb6b9950c498db893f16e88dddb
  snippet_sha1: 91e798148d849844d138260e383fb6d83bd212e4
- path: crates/limedl-engine/src/cdn/resolver.rs
  lines:
  - 45
  - 66
  blob: 93a13517b1cf7b6dfa754465391ead3238d99f3e
  snippet_sha1: b46a0f674b7685e617493d3da8e82486f21d6d96
- path: crates/limedl-native/src/update/mod.rs
  lines:
  - 370
  - 384
  blob: 594a999f50c27ef1c5555accc6b7e73755260561
  snippet_sha1: b29f3377ed0392ef401e9d2026a04c9bb60c5df7
- path: crates/limedl-http/src/http_client_factory/mod.rs
  lines:
  - 25
  - 41
  blob: c30dfdcb24738eb6b9950c498db893f16e88dddb
  snippet_sha1: a5c40c837d7b5eefaa2e8de8a2b94fb00d74be43
- path: crates/limedl-http/src/http_client_factory/mod.rs
  lines:
  - 55
  - 67
  blob: c30dfdcb24738eb6b9950c498db893f16e88dddb
  snippet_sha1: f69886bdf324c951a7cafb4db064765df92a5fb6
- path: crates/limedl-types/src/types/common.rs
  lines:
  - 86
  - 91
  blob: 22d9bc8410a3945fcd353adba848b94dd013180b
  snippet_sha1: d36d9446e59538c6e39f7bce5c6ad856815e36a7
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 101
  - 114
  blob: e7734589a56cfbec112230cb3856dab3b3ef1e19
  snippet_sha1: 33b587fbe2abd6e6f29efdb1c9278c6d1f75a29f
- path: crates/limedl-bt/src/session.rs
  lines:
  - 99
  - 105
  blob: fb6e096e6f7267bcabcfb722dfc2bdfee3390258
  snippet_sha1: bbefe5ceb16c644d05a698af35ebcf39193c26f3
- path: crates/limedl-bt/src/session.rs
  lines:
  - 190
  - 196
  blob: fb6e096e6f7267bcabcfb722dfc2bdfee3390258
  snippet_sha1: 4b7073ba2e64a59eef359120cff9f82ae7eebc03
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 379
  - 389
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: afb5940f1c28d51d9e503028a90442fa75f2c990
- path: crates/limedl-http/src/http_client_factory/tests.rs
  lines:
  - 221
  - 273
  blob: dfdc8ec91882d6a8569bb1fd6903a6c2caea4f19
  snippet_sha1: 201a68785960f31f6ef0a70e3a57afc195efc3b4
- path: Cargo.toml
  lines:
  - 25
  - 47
  blob: 76657a09d63846e6e4df9e5ff601c82e5fe5f04e
  snippet_sha1: df0d8f6e8963a752a9a9027579de5c1cfe2b51a7
- path: crates/limedl-types/src/settings/mod.rs
  lines:
  - 21
  - 40
  blob: e8fe36a862b028e422dacb74d56b0e135d9c84bf
  snippet_sha1: 385f281248222ceccb53854767f13da59796ea77
- path: crates/limedl-types/src/types/settings.rs
  lines:
  - 340
  - 366
  blob: d36d3f34aad95b687828c40c0c27eaae53fccddb
  snippet_sha1: 20c830ea441d27b1bd3f76d76689ec8968e083e3
- path: crates/limedl-native/ui/components/settings/tab_download.slint
  lines:
  - 100
  - 106
  blob: e89247504e1a556c92c6a796cbbb2c0d1cbee0a5
  snippet_sha1: c77df719ac83391597b50df525cb131f6e16300a
- path: Cargo.toml
  lines:
  - 31
  - 38
  blob: 76657a09d63846e6e4df9e5ff601c82e5fe5f04e
  snippet_sha1: 5dc63d006b85597f7462eff567d0c7d81825abab
- path: .cargo/config.toml
  lines:
  - 1
  - 42
  blob: 2f22eee2bc7e1af060093888b6066427fba7847e
  snippet_sha1: 891836d4c885c961ad8bf062d69a4641073f4194
- path: crates/limedl-store/src/alt_svc/mod.rs
  lines:
  - 1
  - 22
  blob: b2d96af5369f4cac07846117e7f79990e48177d4
  snippet_sha1: a0395e038a9ad8d063e00669c185595cced3cd57
- path: crates/limedl-store/src/alt_svc/mod.rs
  lines:
  - 209
  - 282
  blob: b2d96af5369f4cac07846117e7f79990e48177d4
  snippet_sha1: 7440ad5cadfe718c7495bee0ac80c6ff051e30e6
- path: crates/limedl-store/src/database/schema.rs
  lines:
  - 237
  - 254
  blob: ea561ba2f671bb2bd40c2a609e106f177aa5e9b4
  snippet_sha1: 7a67e023945836ae9a6bdf4402741973560d48a1
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 131
  - 132
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: 4fcc05854e6411f71e7aa883b1cce725cd25e63e
- path: crates/limedl-store/src/alt_svc/mod.rs
  lines:
  - 264
  - 282
  blob: b2d96af5369f4cac07846117e7f79990e48177d4
  snippet_sha1: 4f2b0d9d14815ee761c828361f3b42eda1d382b3
- path: crates/limedl-types/src/types/settings.rs
  lines:
  - 312
  - 336
  blob: d36d3f34aad95b687828c40c0c27eaae53fccddb
  snippet_sha1: 4df64b50e018e19a8c64211928ee2fc7ab966a51
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 445
  - 508
  blob: b39f476924d7685e34bb874806ba1366b92cd35b
  snippet_sha1: 63bd7383e1472a12d2947085459a76c5a74d1100
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 597
  - 643
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: 055ce1cff3c29092e99199bae2f4beb4ac695802
- path: crates/limedl-store/src/alt_svc/mod.rs
  lines:
  - 41
  - 64
  blob: b2d96af5369f4cac07846117e7f79990e48177d4
  snippet_sha1: d53138ecca24f98d8611d4ed7258768777c58aaa
- path: crates/limedl-core/src/manager.rs
  lines:
  - 153
  - 158
  blob: fa2c301dce0e94dfff5d64faea9fa56e053c3b70
  snippet_sha1: 6ebd9c52495128bc65da8c2387f50d38727bc8c5
- path: Cargo.toml
  lines:
  - 39
  - 46
  blob: 76657a09d63846e6e4df9e5ff601c82e5fe5f04e
  snippet_sha1: 3ce98a1a66badad25b6d761c17720a393f689b74
- path: crates/limedl-http/src/http/mod.rs
  lines:
  - 205
  - 241
  blob: 9dc32195af6d3982fad1d090f95559e3839465e9
  snippet_sha1: c9b42382d689c944d3e8c5958f4d3bb0be48d3cb
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 36
  - 56
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: 6df93404c9bd08fa3d2bf4b99574d4f61339ff0e
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 82
  - 104
  blob: 925c25f26f26d36e70df7e163eadada24b72c26f
  snippet_sha1: 417e6dafd0bf4b98aa78d81d0f3f052e3e8a21e4
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 563
  - 591
  blob: b39f476924d7685e34bb874806ba1366b92cd35b
  snippet_sha1: 8d7a1c0a1393a573506a987b9ff87f79fa5b6eee
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 617
  - 620
  blob: b39f476924d7685e34bb874806ba1366b92cd35b
  snippet_sha1: 7855c09446891cc539b8adfc058ea02ca27a798d
- path: crates/limedl-core/src/tests/http_executor_tests/compression.rs
  lines:
  - 1
  - 20
  blob: d61bc7661c9c417d6ca58acc00b0ae0874214a0b
  snippet_sha1: 81d99262ccc07777f3745021f08d62df1a3d5380
- path: crates/limedl-http/src/rate_limiter/mod.rs
  lines:
  - 11
  - 28
  blob: 4b702a075f5942b03c36742b19c5694643c28069
  snippet_sha1: 20b81413ac4dee63385f3f5bb8d415df03b72b54
- path: crates/limedl-http/src/rate_limiter/mod.rs
  lines:
  - 70
  - 84
  blob: 4b702a075f5942b03c36742b19c5694643c28069
  snippet_sha1: 900ee707334cb5f8d92266216d154ae7c3a6c87e
- path: crates/limedl-http/src/rate_limiter/mod.rs
  lines:
  - 44
  - 68
  blob: 4b702a075f5942b03c36742b19c5694643c28069
  snippet_sha1: 48fdffe2aca8ae106d65044d91532ddf63c9964c
- path: crates/limedl-http/src/rate_limiter/mod.rs
  lines:
  - 127
  - 155
  blob: 4b702a075f5942b03c36742b19c5694643c28069
  snippet_sha1: 91b86e0460447ed1f27ef29c8807caf33a62cc93
- path: crates/limedl-http/src/rate_limiter/mod.rs
  lines:
  - 86
  - 104
  blob: 4b702a075f5942b03c36742b19c5694643c28069
  snippet_sha1: 3bcf4c12e7d6e0c4ec4effee79afc4fcf16472c0
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 236
  - 272
  blob: 10a37a526acd26abd4f96c9ea74aaac126fb9ad2
  snippet_sha1: 33402a83fd562a917327a7aafd63a1072cb10c99
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 377
  - 381
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: 11a6456b74b3f676aa55bfdc4841cc9f2a3aa54c
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 737
  - 741
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: fa50881129100d37b605a6ba3dd15e8f5119f292
- path: crates/limedl-engine/src/aimd/mod.rs
  lines:
  - 39
  - 56
  blob: 8e194d080f3e28b29f50aff0f4908200a49d1290
  snippet_sha1: 27d214e9409ce414fed835398ffd0af130146525
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 503
  - 506
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 3ccf9fda733ec0376f4938bb7a140f9e6a786e6d
- path: crates/limedl-engine/src/retry/mod.rs
  lines:
  - 25
  - 66
  blob: 502dbddc7f6c6a8849c762f2548591307114e226
  snippet_sha1: d590e54658e6483041b3c252735f34623568fa73
- path: crates/limedl-engine/src/retry/mod.rs
  lines:
  - 68
  - 143
  blob: 502dbddc7f6c6a8849c762f2548591307114e226
  snippet_sha1: 6aec5fd5779f4b98684d64fda365daa8501ffcb3
- path: crates/limedl-http/src/http/mod.rs
  lines:
  - 18
  - 34
  blob: 9dc32195af6d3982fad1d090f95559e3839465e9
  snippet_sha1: eef0351bf02d1379b524e318a534b86006328725
- path: crates/limedl-engine/src/retry/mod.rs
  lines:
  - 153
  - 171
  blob: 502dbddc7f6c6a8849c762f2548591307114e226
  snippet_sha1: 6dd3bebcd2f0b9a872049875c3d7b4ecacb4855f
- path: crates/limedl-engine/src/retry/mod.rs
  lines:
  - 173
  - 217
  blob: 502dbddc7f6c6a8849c762f2548591307114e226
  snippet_sha1: 4bcca825c8d4090791a9d9b580326c9df5517e1f
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 563
  - 569
  blob: e2b0f898b2e1fea578b56a582f8dd6d4165d682a
  snippet_sha1: f714324fc172e0fb1cbd0d61891906eb57ca8316
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 162
  - 171
  blob: e2b0f898b2e1fea578b56a582f8dd6d4165d682a
  snippet_sha1: edb1d0b7d6afd7ee00f81d667609e11ecbafdd1f
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 274
  - 307
  blob: 10a37a526acd26abd4f96c9ea74aaac126fb9ad2
  snippet_sha1: d3dd7c0d7482b3625823f77606bf808a4b2bfe94
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 325
  - 326
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: 6607c9d0e83b5d47a4b97fad39028632699686a3
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 435
  - 438
  blob: 786277ec199e1f2e7ef4d29ce69ef18aed0e95ed
  snippet_sha1: dba0f1f916963817e545d0168c59fee9d42fd9b1
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 29
  - 35
  blob: b39f476924d7685e34bb874806ba1366b92cd35b
  snippet_sha1: 386a4528311fa8ec28afd91a5ab00b27039c91ea
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 156
  - 160
  blob: b39f476924d7685e34bb874806ba1366b92cd35b
  snippet_sha1: a02165d043433a90490cb2f7ae69a05cca200312
- path: crates/limedl-types/src/url_rewrite/mod.rs
  lines:
  - 48
  - 83
  blob: 6605f8ed55fa7fcb2523918272f62f5a99e80f33
  snippet_sha1: 1562e4f21dfd872a21a24bca432755f462c29d3b
- path: crates/limedl-types/src/url_rewrite/mod.rs
  lines:
  - 85
  - 141
  blob: 6605f8ed55fa7fcb2523918272f62f5a99e80f33
  snippet_sha1: 84cf37cdb20ebbee68fc47eea419eed47a16d709
- path: crates/limedl-types/src/url_rewrite/mod.rs
  lines:
  - 154
  - 185
  blob: 6605f8ed55fa7fcb2523918272f62f5a99e80f33
  snippet_sha1: 9e4b8c4118e2ecd7c0dfd491db2382b475ca1bf9
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 148
  - 156
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: fb6d64bd9c09a4a9c9daa6367c1044c3127b39df
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 365
  - 377
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 397f36bac8103773c53522b7c331691cbdd6a468
type: system
---
# Networking, HTTP Clients and Rate Control

Every outbound HTTP request in limedl is built through one module,
`http_client_factory` — since the engine split it lives in the `limedl-http` crate
alongside `http` (header/URL helpers) and `rate_limiter` — so proxy and
User-Agent behaviour is uniform. On top of that sit the global rate limiter, the
retry/backoff policy, the Alt-Svc/HTTP-3 cache and the URL-rewrite mirror
selection.
[[cite:crates/limedl-http/src/lib.rs:L1-L20]]

## HttpClientFactory

`build_http_client(settings)` is the convenience constructor;
`configure_client_builder` returns a `ClientBuilder` so callers can append their
own configuration — the CDN resolver chains `resolve_to_addrs` for DNS
acceleration, the updater chains its own `limedl-native/<version>` User-Agent.
Shared configuration applied by the factory (in non-expert mode):

| Setting | Value |
| --- | --- |
| Redirect policy | `Policy::limited(10)` |
| TCP | `tcp_nodelay(true)`, keepalive 60 s |
| Read timeout | 30 s |
| Connect timeout | 30 s |
| Pool | 20 idle per host, 120 s idle timeout |
| HTTP/2 | adaptive window enabled |

[[cite:crates/limedl-http/src/http_client_factory/mod.rs:L11-L53]]
[[cite:crates/limedl-engine/src/cdn/resolver.rs:L45-L66]]
[[cite:crates/limedl-native/src/update/mod.rs:L370-L384]]

Those four values are the defaults; with `scheduler.expert_mode` on, the same
function reads connect/read timeouts, idle-per-host and TCP keepalive from
`settings.scheduler.expert` instead.
[[cite:crates/limedl-http/src/http_client_factory/mod.rs:L25-L41]]

The read timeout is a stall detector between body bytes, so the default is
deliberately equal to the connect timeout: a high-latency or jittery link
legitimately produces multi-second gaps mid-body, and timing out on those turns a
slow mirror into a failed download. The authoritative per-request patience is
still `max_retries` plus the `RequestBudget` below, not the socket timeout.

Proxy behaviour follows `settings.proxy.mode` (`Disabled` is the default):

- `Disabled` → `builder.no_proxy()` (turns off the automatic system/env proxy);
- `System` → nothing added, leaving reqwest's system detection active;
- `Manual` → `Proxy::all(manual_url)`, an invalid URL failing as
  `DownloadError::InvalidProxy`.
[[cite:crates/limedl-http/src/http_client_factory/mod.rs:L55-L67]]
[[cite:crates/limedl-types/src/types/common.rs:L86-L91]]

**A bare `reqwest::Client::builder()` is forbidden** for engine traffic, because
it silently bypasses `settings.proxy`. This is why the bootstrap Dispatcher client
(tracker-list fetch, checksum probing), the librqbit BT `.torrent` client and the
updater all go through this module; the bootstrap call site keeps the comment
explaining the mistake it replaced. The only bare builder left is Dispatcher's
own fallback for a `Dispatcher` constructed without an HTTP client.
[[cite:crates/limedl-core/src/bootstrap.rs:L101-L114]]
[[cite:crates/limedl-bt/src/session.rs:L99-L105]]
[[cite:crates/limedl-bt/src/session.rs:L190-L196]]
[[cite:crates/limedl-core/src/dispatcher.rs:L379-L389]]

### The system-proxy feature guard

`ProxyMode::System` only reads the Windows registry / macOS system configuration
because reqwest's default feature set includes `system-proxy`
(`hyper-util/client-proxy-system`). If someone re-enables
`default-features = false` without listing `system-proxy`, the feature disappears
and system proxies are silently ignored — the crate still compiles and every other
proxy test still passes, so the symptom is only "system proxy doesn't work". The
workspace dependency therefore keeps default features on, and a regression test
slices the `reqwest = { … }` entry out of the root manifest and fails unless
`system-proxy` is enabled, either by leaving defaults on or by listing it
explicitly.
[[cite:crates/limedl-http/src/http_client_factory/tests.rs:L221-L273]]
[[cite:Cargo.toml:L25-L47]]

### User-Agent freshness

`normalize_user_agent` trims, substitutes the built-in default when empty, and
rejects values longer than 512 bytes or containing invalid header characters. It
lives in `limedl-types::settings` rather than next to the client builder, because
settings validation is foundation-layer work and the builder is not the only
caller.
[[cite:crates/limedl-types/src/settings/mod.rs:L21-L40]]

The built-in default is a browser-shaped Chrome/154 UA; it is a deliberate
"disguise" that some mirrors (e.g. Tsinghua TUNA) police by version — an
outdated or not-yet-released Chrome version is rejected as a fake browser
("software with uncommon characteristics"), while tool UAs pass. Since Chrome 96
the minor/build fields freeze to `.0.0.0`, so only the major needs bumping.
Updating it means editing `default_http_user_agent()` and the placeholder hint in
`tab_download.slint`; existing `settings.json` files keep the old value (no
automatic migration).
[[cite:crates/limedl-types/src/types/settings.rs:L340-L366]]
[[cite:crates/limedl-native/ui/components/settings/tab_download.slint:L100-L106]]

### HTTP/3 and the Alt-Svc cache

The root manifest enables reqwest's `http3` feature (pulling `h3`/`h3-quinn`/
`quinn` and turning on `rustls`), and `.cargo/config.toml` adds the required
`--cfg reqwest_unstable` to every target's rustflags — a missing cfg is a hard
`compile_error!` inside reqwest, so the cfg must be added whenever a new
`[target.*]` entry appears.
[[cite:Cargo.toml:L31-L38]]
[[cite:.cargo/config.toml:L1-L42]]

reqwest itself does no Alt-Svc negotiation, so upgrade is limedl's own work.
`limedl-store::alt_svc` is an `AltSvcManager` with an in-memory L1 `DashMap` and an
SQLite `alt_svc_cache` table (migration `add_alt_svc_cache`, indexed on
`expires_at_ms`) for cross-task, cross-session discovery; `Alt-Svc` headers seen on
any response are ingested via `record_from_response`, `clear` drops the origin, and
only `h3*` protocols are kept, preferring standard `h3` over drafts like `h3-29`.
[[cite:crates/limedl-store/src/alt_svc/mod.rs:L1-L22]]
[[cite:crates/limedl-store/src/alt_svc/mod.rs:L209-L282]]
[[cite:crates/limedl-store/src/database/schema.rs:L237-L254]]
[[cite:crates/limedl-engine/src/http_executor/run.rs:L131-L132]]

`should_attempt_http3` is the gate, and it is deliberately conservative: it
returns `None` unless the feature is enabled
(`scheduler.http3_alt_svc_enabled`, default on), the proxy mode is `Disabled` —
QUIC must not leak a proxied connection — and the origin is HTTPS with a valid,
unexpired record that is not currently in its backoff window.
[[cite:crates/limedl-store/src/alt_svc/mod.rs:L264-L282]]
[[cite:crates/limedl-types/src/types/settings.rs:L312-L336]]

Because reqwest has no h3→h1/h2 fallback, the executor implements it: both the
single-stream path and the chunk worker try exactly one HTTP/3 request
(`Version::HTTP_3`, 5 s timeout) before falling back to the normal retrying TCP
request; a failure or timeout trips a per-origin circuit breaker with exponential
backoff (5 min, 10 min, 30 min, 24 h) and `mark_success` clears it.
[[cite:crates/limedl-engine/src/http_executor/single.rs:L445-L508]]
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L597-L643]]
[[cite:crates/limedl-store/src/alt_svc/mod.rs:L41-L64]]

The manager is reloaded from SQLite at startup (`preload_from_db`) so a resumed
session keeps its learned alternatives.
[[cite:crates/limedl-core/src/manager.rs:L153-L158]]

### Response compression (gzip / brotli / zstd)

reqwest is built with the `gzip`, `brotli` and `zstd` features, so it advertises
`Accept-Encoding` on **every** request without one and transparently decompresses
any `Content-Encoding` response. It does not exempt `Range` requests, so a
compressed `206` would be decompressed and destroy the byte offsets, the
per-chunk `Content-Length` and the final checksum.
[[cite:Cargo.toml:L39-L46]]

`http::identity_encoding` therefore forces `Accept-Encoding: identity` on every
request where decompression would be wrong, and it is applied *after*
`apply_extra_headers` so a user-supplied `Accept-Encoding` cannot re-enable
compression on a request that would corrupt the file:

- probes: the `HEAD`, the `Range: bytes=0-0` fallback and the anti-hotlink
  candidates (a compressed probe would report the compressed length/ranges);
- `build_segment_request`, i.e. every parallel chunk request;
- the single-stream resume request (which carries a `Range`).

[[cite:crates/limedl-http/src/http/mod.rs:L205-L241]]
[[cite:crates/limedl-engine/src/http_executor/run.rs:L36-L56]]
[[cite:crates/limedl-engine/src/http_executor/run.rs:L82-L104]]
[[cite:crates/limedl-engine/src/http_executor/single.rs:L563-L591]]

Only the fresh single-stream GET (offset 0, no `Range`) negotiates compression.
The identity probe has already reported the true decoded length, so progress,
`guard_content_length` and the final checksum stay correct.
[[cite:crates/limedl-engine/src/http_executor/single.rs:L617-L620]]

`crates/limedl-core/src/tests/http_executor_tests/compression.rs` proves both
halves against a server that compresses whenever it is asked and counts how often
it did: single-stream downloads complete with the *decoded* checksum, and range
downloads never trigger a compressed response.
[[cite:crates/limedl-core/src/tests/http_executor_tests/compression.rs:L1-L20]]

## Global rate limiter

`RateLimiter` is a thread-safe token bucket shared across the whole app: the rate
is an `AtomicU64` for a lock-free unlimited fast path, and the bucket state
(`rate`, `capacity`, `tokens`, `last_refill`) sits behind a `parking_lot::Mutex`
held only for brief arithmetic — never across an await or a blocking call.
[[cite:crates/limedl-http/src/rate_limiter/mod.rs:L11-L28]]
[[cite:crates/limedl-http/src/rate_limiter/mod.rs:L70-L84]]

- Rate `0` means unlimited (`consume` returns immediately).
- Bucket capacity is `max(2 * rate, 1)`, and `0` when unlimited.
- `set_rate` refills tokens at the old rate before switching, preserving budget
  coherently.
- `try_consume` refills by elapsed time, and on a deficit it keeps the existing
  tokens and returns a sleep duration (at least 1 ns) instead of zeroing them —
  zeroing discarded partial progress and caused an infinite oscillation when a
  single request was larger than what one refill window could accumulate.
- `consume` is async (`tokio::time::sleep`); the `consume_blocking` thread variant
  is compiled only for tests and the `test-utils` feature.
[[cite:crates/limedl-http/src/rate_limiter/mod.rs:L44-L68]]
[[cite:crates/limedl-http/src/rate_limiter/mod.rs:L127-L155]]
[[cite:crates/limedl-http/src/rate_limiter/mod.rs:L86-L104]]

### Batching in the download path

Chunk workers do not consume the limiter per received chunk. `BatchLimiter`
accumulates bytes and chunk counts and consumes once when either 256 KiB or 8
chunks is reached, and `flush()` consumes any leftover bytes on every worker exit
path. The AIMD sampling window is independent of this batching: the scheduler
computes throughput itself as the delta of `downloaded_bytes` over the elapsed
wall time between its own samples.
[[cite:crates/limedl-engine/src/http_executor/mod.rs:L236-L272]]
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L377-L381]]
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L737-L741]]
[[cite:crates/limedl-engine/src/aimd/mod.rs:L39-L56]]
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L503-L506]]

## Retry and backoff

`request_with_retry(factory, token, max_retries, managed)` drives a request
attempt loop under a cancellation token:

- Every attempt is `select!`ed against cancellation, so a cancel returns
  `Interrupted` promptly — including the backoff sleep itself.
- A `403 Forbidden` gets a bounded body sniff (`ANTI_ABUSE_SNIFF_LIMIT`, 16 KiB);
  if it looks like a WAF/mirror anti-abuse page the request fails terminally with
  an actionable error rather than retrying or probing Referers.
- `classify_download_response` separates `Use`, `Retryable` and `Invalid`.
  Retryable responses increment the attempt, record an AIMD penalty and back off;
  terminal (`Invalid`) responses fail immediately. Only `408`, `429` and 5xx
  retry; `200`/`206` are used as-is and everything else is invalid.
- A `429 Too Many Requests` on a multi-threaded download aborts retries
  (`rate_limit_aborts`) so the chunked path can downgrade to single-thread instead
  of hammering the host — for both the response and the transport-error arm.
- `register_retry_penalty` sets the task to `Retrying`, records the error and
  marks an AIMD penalty for connection backpressure.

[[cite:crates/limedl-engine/src/retry/mod.rs:L25-L66]]
[[cite:crates/limedl-engine/src/retry/mod.rs:L68-L143]]
[[cite:crates/limedl-http/src/http/mod.rs:L18-L34]]
[[cite:crates/limedl-engine/src/retry/mod.rs:L153-L171]]

Backoff is `250 ms * 2^min(attempt, 8)` capped at 4 s — in practice 500 ms, 1 s,
2 s, 4 s, 4 s, … That base `backoff_delay` stays deterministic because it is what
the unit tests pin. `backoff_or_cancel` sleeps `jittered_backoff_delay`, which adds
uniform jitter in `[base, base * 1.25]` (`BACKOFF_JITTER_PERCENT = 25`): every
chunk of every task retries against the same host, so an unjittered schedule puts
them all back on the wire at the same instant — exactly in the 429/5xx case where
the extra load hurts most. The delay is never shortened.
[[cite:crates/limedl-engine/src/retry/mod.rs:L173-L217]]

`is_network_error` classifies a `DownloadError::Http` as transport-level when
reqwest reports `is_connect`, `is_timeout`, `is_body` **or `is_decode`**. The last
one matters because a connection that dies while the body is being read surfaces
as "error decoding response body", not as a connect error; without it a multi-URL
task would fail outright instead of failing over to the next mirror. Only a
transport-class failure with another candidate URL left moves on to the mirror;
every other error fails the task immediately.
[[cite:crates/limedl-engine/src/task_lifecycle/mod.rs:L563-L569]]
[[cite:crates/limedl-engine/src/task_lifecycle/mod.rs:L162-L171]]

### RequestBudget: why per-request retries are not enough

`max_retries` bounds one *request*, so it cannot bound the work a caller does when
a server keeps ending the body early: the caller simply re-requests from the new
offset, and that request gets a fresh retry counter. A server that always closes
the body immediately then turns the caller's loop into a livelock that neither
fails nor finishes.

`RequestBudget` (charged once per request through `charge`, with the unit of work
named in the error) closes that hole, and the two callers pick their own limit:

| Caller | Limit | Meaning |
| --- | --- | --- |
| chunk worker (`worker.rs`) | `MAX_SEGMENT_FETCHES_PER_CHUNK = 24` | separate segment requests per chunk; legitimate multi-response serving needs a handful |
| single-stream loop (`single.rs`) | `MAX_SINGLE_STREAM_REQUESTS = 64` | responses per download; a server that ends the body without moving the offset spins here |

Exceeding the budget fails the download with an actionable
"... was re-requested N times without completing; the server keeps ending the
response early" error instead of looping forever; the chunk worker releases its
claim first.
[[cite:crates/limedl-engine/src/http_executor/mod.rs:L274-L307]]
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L325-L326]]
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L435-L438]]
[[cite:crates/limedl-engine/src/http_executor/single.rs:L29-L35]]
[[cite:crates/limedl-engine/src/http_executor/single.rs:L156-L160]]

## URL rewriting and mirror selection

`url_rewrite` rewrites a download URL to configured mirrors/proxies (ghproxy,
hf-mirror, …). `matches_rule` ignores disabled rules and empty patterns, and
supports four match types:

- `Host` — parses the URL and compares the lowercased host: exact, `*.suffix`
  wildcard suffix, or `*`/`?` wildcard;
- `Prefix` — string prefix;
- `Regex` — regex match (invalid regex never matches);
- `Wildcard` — `*`/`?` glob over the whole URL.

[[cite:crates/limedl-types/src/url_rewrite/mod.rs:L48-L83]]

`rewrite_url` returns `[original]` when rewriting is disabled or no rule matches.
When a rule matches, it walks enabled rules in `order`, generates candidate URLs
from that rule's enabled targets in priority order, de-duplicated, and appends the
original URL at the end when `fallback_to_original` is set and it is not already
present. Two replacement modes exist:

- `PrefixProxy` — `base/<url>` (optionally percent-encoding the URL with the RFC
  3986 unreserved set);
- `Template` — regex `replace_all` with capture groups when the rule matches by
  regex, or `{url}` / `{raw_url}` placeholder substitution.

[[cite:crates/limedl-types/src/url_rewrite/mod.rs:L85-L141]]
[[cite:crates/limedl-types/src/url_rewrite/mod.rs:L154-L185]]

The Dispatcher uses this in `start` (filling `mirror_urls` when the caller
supplied none and more than one candidate came back) and exposes it as
`resolve_mirror_urls`.
[[cite:crates/limedl-core/src/dispatcher.rs:L148-L156]]
[[cite:crates/limedl-core/src/dispatcher.rs:L365-L377]]

Related pages: [[wiki:workflows/http-download-lifecycle]],
[[wiki:workflows/cdn-acceleration]],
[[wiki:workflows/scheduler-and-concurrency]],
[[wiki:desktop/self-update-and-distribution]].
