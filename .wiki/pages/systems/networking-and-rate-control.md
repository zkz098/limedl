---
id: systems/networking-and-rate-control
title: Networking, HTTP Clients and Rate Control
status: verified
generated_at: 2026-11-13T06:38:14Z
commit: c28b25a7355ded0aa81f07dfee7c66c53537eaf3
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
  blob: b81bed4e871719ca834a78d5fa058d62c97e8ea8
  snippet_sha1: 00ef9c62018942231e729f2006a3e350eb582e32
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
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: c3ce4043108e4e54a01291e287987b7b6ad013a9
- path: crates/limedl-bt/src/session.rs
  lines:
  - 99
  - 105
  blob: 6c6c108af4073d41ace3f87f18faac8a958fc6f3
  snippet_sha1: 896e0fbca2c69d9377b8ecc6bb5a3c835e206b85
- path: crates/limedl-bt/src/session.rs
  lines:
  - 190
  - 196
  blob: 6c6c108af4073d41ace3f87f18faac8a958fc6f3
  snippet_sha1: 5c20b7c1b9f6cc3a06fe699b266e491e358c0044
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 379
  - 389
  blob: 2f774d1c7b11e7af6fb302a7063501181b4f8a0b
  snippet_sha1: 17e3d51c005f6119b8f21d5d5d2c66216bf181e4
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
  blob: d6e99b280b6758f713130a1a7201dac7463dd565
  snippet_sha1: eab3a7e3d7c27a4441373ca0acd2e1222a70c0e6
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
  blob: d6e99b280b6758f713130a1a7201dac7463dd565
  snippet_sha1: 588f30824e32d93d4e1ce5c11b2d37cd209102da
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
  blob: e0e7b42b06644786a81d21043a4f502a105a1237
  snippet_sha1: cefb1cd2486b0adfced55268258a457b704d88bc
- path: crates/limedl-store/src/alt_svc/mod.rs
  lines:
  - 209
  - 282
  blob: e0e7b42b06644786a81d21043a4f502a105a1237
  snippet_sha1: 1348553b47abaab5fa4c74f98f14c7bf1ab95143
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
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: eec5aad3e2e353eeb5a21e5ec4a2724448763aa4
- path: crates/limedl-store/src/alt_svc/mod.rs
  lines:
  - 264
  - 282
  blob: e0e7b42b06644786a81d21043a4f502a105a1237
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
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: 8f978cd66470103c8eb09ca1312af97c35e10159
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 597
  - 643
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: 649e9f6f04e2c852d6d560a6de73f92abd9988f8
- path: crates/limedl-store/src/alt_svc/mod.rs
  lines:
  - 41
  - 64
  blob: e0e7b42b06644786a81d21043a4f502a105a1237
  snippet_sha1: d53138ecca24f98d8611d4ed7258768777c58aaa
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 153
  - 158
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: 30340bceeb34736b747d1ea2e39e1c145405c5f2
- path: Cargo.toml
  lines:
  - 39
  - 46
  blob: d6e99b280b6758f713130a1a7201dac7463dd565
  snippet_sha1: df765856bd896e5d5a56394c1b51a3a6f819fd06
- path: crates/limedl-http/src/http/mod.rs
  lines:
  - 205
  - 241
  blob: a079384e1c984f8817f512eac709db0c312119cf
  snippet_sha1: 10adb1f12fd17d1049ac675c31f06e26b86573b4
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 36
  - 56
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: 3879217884c1a08e8f5c2e8b8960af6749126946
- path: crates/limedl-engine/src/http_executor/run.rs
  lines:
  - 82
  - 104
  blob: beca9b99784caae27de5ae257efb53cdd74b0418
  snippet_sha1: 417e6dafd0bf4b98aa78d81d0f3f052e3e8a21e4
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 563
  - 591
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: 5385dcbe75eba4fb8294849dcf9ce2c7e215207d
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 617
  - 620
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: 23097f786712897d05d41528f4aae0b9031cf44c
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
  blob: 2f610ec272053d555c8433282b6e4a05141bdab0
  snippet_sha1: bb48f3fdcaaa59b2e92ae41b7c1fb8f77508ee60
- path: crates/limedl-http/src/rate_limiter/mod.rs
  lines:
  - 70
  - 84
  blob: 2f610ec272053d555c8433282b6e4a05141bdab0
  snippet_sha1: d086448a635695ae071eeeb572972ab2499027bd
- path: crates/limedl-http/src/rate_limiter/mod.rs
  lines:
  - 44
  - 68
  blob: 2f610ec272053d555c8433282b6e4a05141bdab0
  snippet_sha1: 92a217a2ab644f71ca6fee4797ffd61509b816e9
- path: crates/limedl-http/src/rate_limiter/mod.rs
  lines:
  - 127
  - 155
  blob: 2f610ec272053d555c8433282b6e4a05141bdab0
  snippet_sha1: eae8dcda47a4c39b9e95b9289dd368d5f12d9cca
- path: crates/limedl-http/src/rate_limiter/mod.rs
  lines:
  - 86
  - 104
  blob: 2f610ec272053d555c8433282b6e4a05141bdab0
  snippet_sha1: 2a263718bec5afe1b42a9a8ab87bfef60f590665
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 236
  - 272
  blob: 440aa6e7e0e75317a748ed5618b9c2869db2bb86
  snippet_sha1: 48c41fca55b0567f38e3e7bf502a18d2040cfc4c
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 377
  - 381
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: f9aec5dcc400da767702e03584952d031baeb9bb
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 737
  - 741
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: 768c553fa19c77a718f9ac27e26db0033ff4ce7a
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
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: ea599c35e8ae6e04f3ad1250afd57b6d288441eb
- path: crates/limedl-engine/src/retry/mod.rs
  lines:
  - 25
  - 31
  blob: 1eec19c69b09b31c4a2722ee386186411884d831
  snippet_sha1: 657e5fc738d18bbbb7a70a1ea292298037312de8
- path: crates/limedl-engine/src/retry/mod.rs
  lines:
  - 35
  - 66
  blob: 1eec19c69b09b31c4a2722ee386186411884d831
  snippet_sha1: 84321b97301e63f214dd78aa254a8c4b4b20c699
- path: crates/limedl-engine/src/retry/mod.rs
  lines:
  - 68
  - 143
  blob: 1eec19c69b09b31c4a2722ee386186411884d831
  snippet_sha1: 6aec5fd5779f4b98684d64fda365daa8501ffcb3
- path: crates/limedl-http/src/http/mod.rs
  lines:
  - 18
  - 34
  blob: a079384e1c984f8817f512eac709db0c312119cf
  snippet_sha1: eef0351bf02d1379b524e318a534b86006328725
- path: crates/limedl-engine/src/retry/mod.rs
  lines:
  - 153
  - 171
  blob: 1eec19c69b09b31c4a2722ee386186411884d831
  snippet_sha1: 4996383ca4606ccf98015e6d584db811f101ffc1
- path: crates/limedl-engine/src/retry/mod.rs
  lines:
  - 173
  - 217
  blob: 1eec19c69b09b31c4a2722ee386186411884d831
  snippet_sha1: fdae216ea32e6ae34e89a355f9f43778582fb3d1
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 563
  - 569
  blob: 06fa226e016db7406aab96ecc589b08052421836
  snippet_sha1: 5779a8454c026f482038eed3c6053466fd7fb415
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 162
  - 171
  blob: 06fa226e016db7406aab96ecc589b08052421836
  snippet_sha1: 91704024e4ab38faa6e4d252744c1c74a7f9e118
- path: crates/limedl-engine/src/http_executor/mod.rs
  lines:
  - 274
  - 307
  blob: 440aa6e7e0e75317a748ed5618b9c2869db2bb86
  snippet_sha1: 2f8ee01c59e92b67a27fc01b772fd7fa1e6bf227
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 325
  - 326
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: 920f16e55f56f53b8ff16b13fa93155bf68aeff8
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 435
  - 438
  blob: ce80a384ff8cae3049a7708004df3764c9b0d1cb
  snippet_sha1: 182df421e3e627e9eb7c9b95f4e8bd798b7b68df
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 29
  - 35
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: 386a4528311fa8ec28afd91a5ab00b27039c91ea
- path: crates/limedl-engine/src/http_executor/single.rs
  lines:
  - 156
  - 160
  blob: 7185f0766dff1e1e7b4780f816f60381a250c4bf
  snippet_sha1: a02165d043433a90490cb2f7ae69a05cca200312
- path: crates/limedl-types/src/url_rewrite/mod.rs
  lines:
  - 48
  - 83
  blob: 6b201723326ece1a02d539f36d214cca0dd651e3
  snippet_sha1: 1c5e770a0e1ade8f32944eb10fef7deae9bf5647
- path: crates/limedl-types/src/url_rewrite/mod.rs
  lines:
  - 85
  - 141
  blob: 6b201723326ece1a02d539f36d214cca0dd651e3
  snippet_sha1: 6c77795a6c4fc943ff17312f7bf0aeb90dc22a68
- path: crates/limedl-types/src/url_rewrite/mod.rs
  lines:
  - 154
  - 185
  blob: 6b201723326ece1a02d539f36d214cca0dd651e3
  snippet_sha1: 6ff13022e1efc2b3cb11e1b4aeeab6662b84c0e8
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 148
  - 156
  blob: 2f774d1c7b11e7af6fb302a7063501181b4f8a0b
  snippet_sha1: fb6d64bd9c09a4a9c9daa6367c1044c3127b39df
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 365
  - 377
  blob: 2f774d1c7b11e7af6fb302a7063501181b4f8a0b
  snippet_sha1: 7c9ca3d253008a11a8b6f210b866366670076edc
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
[[cite:crates/limedl-engine/src/manager.rs:L153-L158]]

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

[[cite:crates/limedl-engine/src/retry/mod.rs:L25-L31]]
[[cite:crates/limedl-engine/src/retry/mod.rs:L35-L66]]
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
