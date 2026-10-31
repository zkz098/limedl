---
id: architecture/overview
title: Workspace and System Architecture
status: verified
generated_at: 2026-10-31T14:51:29Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence:
- path: Cargo.toml
  lines:
  - 1
  - 20
  blob: 76657a09d63846e6e4df9e5ff601c82e5fe5f04e
  snippet_sha1: d3c350838efc75fdbca982d3948eab043bb29187
- path: crates/limedl-core/Cargo.toml
  lines:
  - 1
  - 10
  blob: 1df32cad19a6f1f2bfdf7a491b426c4091fcb7ed
  snippet_sha1: c37df5948258b92b982e0547378f129a560a41e1
- path: crates/limedl-types/Cargo.toml
  lines:
  - 16
  - 27
  blob: 840a2c4083e14fb8f102d99044f1a6a3ccda4756
  snippet_sha1: f7c56eabb10074ec5faa773a4c521d6a92d38205
- path: crates/limedl-disk/Cargo.toml
  lines:
  - 13
  - 18
  blob: 370ea396053ec5bb385694528101cf153b7a406b
  snippet_sha1: 7d17948b3300116437e360c1826bf5445f602fb5
- path: crates/limedl-store/Cargo.toml
  lines:
  - 13
  - 15
  blob: 42448a062e874438a628476211e6ad7d8f6256d0
  snippet_sha1: 2202ef8399c18cca304e0ebf9504e284cc8ee006
- path: crates/limedl-http/Cargo.toml
  lines:
  - 13
  - 18
  blob: d7a90757a8642a87750734d8f708e57ce1b9c579
  snippet_sha1: 5fd66a2a4132cb777afea7cff4f70f1ae0d65a04
- path: crates/limedl-engine/Cargo.toml
  lines:
  - 13
  - 17
  blob: 75a0e59d5288ce5f5673406082f9e173dbd63f98
  snippet_sha1: e025aa7c651fe6eed920d4b1a65767414b776075
- path: crates/limedl-bt/Cargo.toml
  lines:
  - 13
  - 22
  blob: 6f0ff40bcfbc9409789f2856b2f51b6f71ff6184
  snippet_sha1: 18572645966e669d8a08153305763b1cd46fe493
- path: crates/limedl-core/Cargo.toml
  lines:
  - 15
  - 31
  blob: 1df32cad19a6f1f2bfdf7a491b426c4091fcb7ed
  snippet_sha1: 205ff85d4b677a2ae92dd91150ed0956509c2062
- path: crates/limedl-rpc/Cargo.toml
  lines:
  - 13
  - 18
  blob: 6bad5d877b655ecee6305fa6ed30aa12ea4414be
  snippet_sha1: e50c38518fbef08d669a5fc639187eab4c1477d2
- path: crates/limedl-testkit/Cargo.toml
  lines:
  - 14
  - 21
  blob: ddbbcd22744c5f9efc5f7b04295d6242a1869e61
  snippet_sha1: 2cb49a91ab1a0db09f67850fb1fe541153831289
- path: crates/limedl-rpc/Cargo.toml
  lines:
  - 14
  - 18
  blob: 6bad5d877b655ecee6305fa6ed30aa12ea4414be
  snippet_sha1: d4f9efb33d7a8ba231fced8fc4fc8dbe2ae6d013
- path: crates/limedl-bt/Cargo.toml
  lines:
  - 13
  - 23
  blob: 6f0ff40bcfbc9409789f2856b2f51b6f71ff6184
  snippet_sha1: 5e956c01a72af11406e3022a735469e6ad3930ad
- path: crates/limedl-core/Cargo.toml
  lines:
  - 30
  - 30
  blob: 1df32cad19a6f1f2bfdf7a491b426c4091fcb7ed
  snippet_sha1: ce7e3df57b55231a552197e7dc90bbbcea72eb60
- path: crates/limedl-types/Cargo.toml
  lines:
  - 54
  - 57
  blob: 840a2c4083e14fb8f102d99044f1a6a3ccda4756
  snippet_sha1: 1b10e2c1946a7d39a34f06b341a618bc1b63cabf
- path: crates/limedl-store/Cargo.toml
  lines:
  - 13
  - 16
  blob: 42448a062e874438a628476211e6ad7d8f6256d0
  snippet_sha1: 08ec66e2636ed1e4f6c00da6c487731091ae48ec
- path: crates/limedl-rpc/Cargo.toml
  lines:
  - 14
  - 22
  blob: 6bad5d877b655ecee6305fa6ed30aa12ea4414be
  snippet_sha1: 2e3505c00ca1d37d697d74abbf39396101bcf949
- path: crates/limedl-testkit/Cargo.toml
  lines:
  - 22
  - 22
  blob: ddbbcd22744c5f9efc5f7b04295d6242a1869e61
  snippet_sha1: b27db12e00256f3086743dda8c98315a75c56b5b
- path: crates/limedl-core/Cargo.toml
  lines:
  - 81
  - 83
  blob: 1df32cad19a6f1f2bfdf7a491b426c4091fcb7ed
  snippet_sha1: 64682223969f057308a1639c6d9f5e9a12cc5e78
- path: crates/limedl-testkit/Cargo.toml
  lines:
  - 27
  - 38
  blob: ddbbcd22744c5f9efc5f7b04295d6242a1869e61
  snippet_sha1: b48a83d193445a53ad84d6ee0dc840ba1e0d8b08
- path: Cargo.toml
  lines:
  - 38
  - 46
  blob: 76657a09d63846e6e4df9e5ff601c82e5fe5f04e
  snippet_sha1: 668691eba5f196d0f2b26ca6ebce22452597206c
- path: crates/limedl-core/src/lib.rs
  lines:
  - 1
  - 64
  blob: 8d1831bda64242c4d6dbe2820d6f8cab738ac5e0
  snippet_sha1: 46565201ab7aa1c843f61ff08f311c326da2c7e4
- path: crates/limedl-core/src/lib.rs
  lines:
  - 12
  - 16
  blob: 8d1831bda64242c4d6dbe2820d6f8cab738ac5e0
  snippet_sha1: 276affb4c4f4b51d8e226109bc5a2eb48a125de2
- path: crates/limedl-core/src/lib.rs
  lines:
  - 58
  - 64
  blob: 8d1831bda64242c4d6dbe2820d6f8cab738ac5e0
  snippet_sha1: 84fb766d4cf5e9ad55f847c4b369b4e6a60390f8
- path: crates/limedl-core/src/lib.rs
  lines:
  - 85
  - 87
  blob: 8d1831bda64242c4d6dbe2820d6f8cab738ac5e0
  snippet_sha1: 369ddb17061744f6b6c8865db8ccc5717b0a0c2a
- path: crates/limedl-native/src/main.rs
  lines:
  - 113
  - 116
  blob: de0b47270c1be4ea918f8af5fa13252cae4f38e5
  snippet_sha1: 18dbb1deb1203241ff4672d56fc932d2d8a7b41e
- path: crates/limedl-server/src/lib.rs
  lines:
  - 62
  - 64
  blob: db33f43e46019b79e0d8eeb7a1198a4d39812a40
  snippet_sha1: 7926ca2891a5beedb7bcc5fe841e16e814306f0f
- path: crates/limedl-native/Cargo.toml
  lines:
  - 16
  - 42
  blob: 22145a24a678bb40d82d9a1e894710e74c1de8aa
  snippet_sha1: 937d8cad82c4de9d8b8155ccdbf6b8fe17b0ef51
- path: crates/limedl-server/Cargo.toml
  lines:
  - 20
  - 30
  blob: 6893e6c30f7243eaf34a51bb537afe0a24c4692d
  snippet_sha1: 2710cd40d1da06e45c57c27779adb26d42853e15
- path: crates/limedl-native/src/main.rs
  lines:
  - 88
  - 92
  blob: de0b47270c1be4ea918f8af5fa13252cae4f38e5
  snippet_sha1: e05c1cb69e39cc541b93373ae835a4c0b852f98f
- path: crates/limedl-native/src/main.rs
  lines:
  - 102
  - 104
  blob: de0b47270c1be4ea918f8af5fa13252cae4f38e5
  snippet_sha1: 6ff38415937cfb46b8a3cf87e7bfdbb815b86455
- path: crates/limedl-native/src/main.rs
  lines:
  - 195
  - 212
  blob: de0b47270c1be4ea918f8af5fa13252cae4f38e5
  snippet_sha1: 7b36ee0dbc7e47feb2db0c6d16b13fc5faace4b4
- path: crates/limedl-types/src/manifest.rs
  lines:
  - 80
  - 123
  blob: 54ad684831aca2c5fa70d6650426e2a12e96fd1e
  snippet_sha1: 62ac65a4bb8a9cde5608fa86298ea5b95860b93f
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 20
  - 42
  blob: 1425cef6f504dffe584d6b9fd22fbdcb396efdab
  snippet_sha1: af2e309eddf1a64e955a2d00a326448cd914439f
- path: crates/limedl-types/src/types/task.rs
  lines:
  - 31
  - 59
  blob: cd48c61d6464160971b95ab16a139dc5d6e65232
  snippet_sha1: 228953973c66317d3bc591c646e2709f747edaae
- path: crates/limedl-types/src/types/task.rs
  lines:
  - 143
  - 198
  blob: cd48c61d6464160971b95ab16a139dc5d6e65232
  snippet_sha1: e123f822a33126c02408863efd2471e27850c810
- path: crates/limedl-types/src/types/task.rs
  lines:
  - 252
  - 287
  blob: cd48c61d6464160971b95ab16a139dc5d6e65232
  snippet_sha1: 29a659e84459d95949291736ab5e9b1b1d77e61a
- path: crates/limedl-types/src/protocol.rs
  lines:
  - 8
  - 50
  blob: 7911ac44f1635ad31f54f84e8326e5af07c5c4a6
  snippet_sha1: 87fc749b71defbc9e53bdaaf8a2f2dd7d3977a6d
- path: crates/limedl-core/src/backend_registry/mod.rs
  lines:
  - 9
  - 75
  blob: 7efae12f82ab6a73adf8c0c5771570a774137be5
  snippet_sha1: 4d1764687424c477904d59d3162e76df1e5cb955
- path: crates/limedl-types/src/event_bus/mod.rs
  lines:
  - 14
  - 81
  blob: 062c63d2325ed1412032c724c7c9d1c53924f7d5
  snippet_sha1: 71b7e5c1e5f95ccaca604d1f6f191c7a7763702e
- path: crates/limedl-native/src/main.rs
  lines:
  - 224
  - 228
  blob: de0b47270c1be4ea918f8af5fa13252cae4f38e5
  snippet_sha1: dc699ccb499cc4b1fc3dc6cadcde41372fd10112
- path: crates/limedl-native/src/event_stream/bus.rs
  lines:
  - 294
  - 347
  blob: d73a979c318114afb86bafa7df107869c6cdd01e
  snippet_sha1: 0f3ec0377cb5e0dbef324a57e682cc21c4504199
- path: crates/limedl-rpc/src/transport.rs
  lines:
  - 37
  - 68
  blob: eebb3201ea4dfede253ee19f6ac71a6bf9739c09
  snippet_sha1: ff793e3026b7c78a53e0c17690416ab3d56b5f11
- path: crates/limedl-native/src/event_stream/bus.rs
  lines:
  - 329
  - 344
  blob: d73a979c318114afb86bafa7df107869c6cdd01e
  snippet_sha1: 2954522d0cfc028ca75e5e5b0d8b77944075789c
- path: crates/limedl-rpc/src/transport.rs
  lines:
  - 63
  - 65
  blob: eebb3201ea4dfede253ee19f6ac71a6bf9739c09
  snippet_sha1: a67dbbe1987f970f76c153db3b9b40e5cf70219f
- path: crates/limedl-types/src/types/common.rs
  lines:
  - 6
  - 14
  blob: 22d9bc8410a3945fcd353adba848b94dd013180b
  snippet_sha1: 5aa462bd7a02db2c9974d37a0f71f120698420da
- path: crates/limedl-types/src/types/task.rs
  lines:
  - 10
  - 20
  blob: cd48c61d6464160971b95ab16a139dc5d6e65232
  snippet_sha1: f2c3ff69eff00637e84c8e20e9c41aa7a528a29f
- path: crates/limedl-core/Cargo.toml
  lines:
  - 50
  - 60
  blob: 1df32cad19a6f1f2bfdf7a491b426c4091fcb7ed
  snippet_sha1: 099072c1a3170c73f9f5243c503d6fd50328c94e
- path: crates/limedl-types/Cargo.toml
  lines:
  - 59
  - 70
  blob: 840a2c4083e14fb8f102d99044f1a6a3ccda4756
  snippet_sha1: f2c2fc0482ce993b24981be4203f65980fd01daa
- path: .cargo/config.toml
  lines:
  - 1
  - 42
  blob: 2f22eee2bc7e1af060093888b6066427fba7847e
  snippet_sha1: 891836d4c885c961ad8bf062d69a4641073f4194
- path: Cargo.toml
  lines:
  - 113
  - 126
  blob: 76657a09d63846e6e4df9e5ff601c82e5fe5f04e
  snippet_sha1: 4ab52a74aacfae723ee4be672b98a700c2853877
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 41
  - 52
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 0c8d4279093fad1c4440490f375d64bcaad1d984
- path: crates/limedl-server/src/lib.rs
  lines:
  - 85
  - 105
  blob: db33f43e46019b79e0d8eeb7a1198a4d39812a40
  snippet_sha1: b70a45dce1923306edf7e35506c6c63e6f6ef528
type: architecture
---
# Workspace and System Architecture

limedl is a Cargo workspace with twelve members: nine engine crates, two
frontends and the tooling crate. All use Rust edition 2024 and inherit the
workspace version and license from the root manifest. Every crate is
`publish = false` — limedl ships binaries, a container image and signed
self-update manifests, never crates.io packages. [[cite:Cargo.toml:L1-L20]]
[[cite:crates/limedl-core/Cargo.toml:L1-L10]]

## The engine crate graph

The engine used to be one 41.5k-line crate. It is now a stack of crates whose
dependency edges are the layering, so the compiler enforces what
`module_boundaries_tests` used to police by parsing source files:

| Crate | Owns | Engine-crate dependencies |
| --- | --- | --- |
| `limedl-types` | domain types, settings tree, `DownloadError`, `EventBus`, `Manifest`, `DownloadBackend`, checksum, url rewrite, `slot_guard` | none — "the bottom of the graph" [[cite:crates/limedl-types/Cargo.toml:L16-L27]] |
| `limedl-disk` | `file_ops` (preallocation, coalescing sink, fault hooks, media probe), `buffer_pool`, `io_scheduler`, disk services | types [[cite:crates/limedl-disk/Cargo.toml:L13-L18]] |
| `limedl-store` | `database` (SQLite task/chunk/manifest store + migrations), `alt_svc` cache | types [[cite:crates/limedl-store/Cargo.toml:L13-L15]] |
| `limedl-http` | `http` helpers, `http_client_factory` (the one `reqwest` policy set), `rate_limiter` | types [[cite:crates/limedl-http/Cargo.toml:L13-L18]] |
| `limedl-engine` | `http_executor`, `scheduler`, `cdn`, `metalink`, `download`, `task_lifecycle`, `persistence`, `aimd`, `manager_state` | types, disk, store, http [[cite:crates/limedl-engine/Cargo.toml:L13-L17]] |
| `limedl-bt` | the BitTorrent backend and its librqbit engine | types (`bt`), store (`bt`), http [[cite:crates/limedl-bt/Cargo.toml:L13-L22]] |
| `limedl-core` | facade: `manager`, `dispatcher`, `context`, `backend_registry`, `bootstrap`, `logging`, and the test fixtures | all of the above, plus optional `limedl-bt` and `limedl-testkit` [[cite:crates/limedl-core/Cargo.toml:L15-L31]] |
| `limedl-rpc` | Aria2 JSON-RPC + WebSocket server (`axum`) | core only [[cite:crates/limedl-rpc/Cargo.toml:L13-L18]] |
| `limedl-testkit` | the deterministic HTTP test server, fault injection, compression endpoints | types, disk [[cite:crates/limedl-testkit/Cargo.toml:L14-L21]] |

The order is the direction: a crate may depend only on the ones above it.
`limedl-bt` and `limedl-engine` are siblings (neither depends on the other), and
`limedl-rpc` is the only crate that sits *above* the facade.
[[cite:crates/limedl-engine/Cargo.toml:L13-L17]]
[[cite:crates/limedl-bt/Cargo.toml:L13-L22]]
[[cite:crates/limedl-rpc/Cargo.toml:L14-L18]]

### One crate per heavy dependency

The split's most useful property is that each expensive third-party tree is
reachable from exactly one place:

- **`librqbit`** (the BitTorrent engine, 100+ crates of DHT, peer-wire and
  `aws-lc` if it leaked downward) is named only by `limedl-bt`. `limedl-core`
  takes that crate as an *optional* dependency behind the `bt` feature, so
  `--no-default-features` builds an engine with no BT protocol.
  [[cite:crates/limedl-bt/Cargo.toml:L13-L23]]
  [[cite:crates/limedl-core/Cargo.toml:L30-L30]]
  [[cite:crates/limedl-types/Cargo.toml:L54-L57]]
- **`rusqlite`** (bundled SQLite, the most expensive C build in the graph) is
  named only by `limedl-store`. [[cite:crates/limedl-store/Cargo.toml:L13-L16]]
- **`axum`/`tower-http`/`argon2`** are production-named only by `limedl-rpc`
  (plus `axum` in the test-only `limedl-testkit`), never by the engine — the
  engine crates carry `axum` only as a dev-dependency for their own ad-hoc test
  servers. [[cite:crates/limedl-rpc/Cargo.toml:L14-L22]]
  [[cite:crates/limedl-testkit/Cargo.toml:L22-L22]]
  [[cite:crates/limedl-core/Cargo.toml:L81-L83]]
- **`flate2`/`brotli`/`zstd`** encoders exist only in `limedl-testkit`; the
  decoder side arrives through `reqwest`'s features.
  [[cite:crates/limedl-testkit/Cargo.toml:L27-L38]]
  [[cite:Cargo.toml:L38-L46]]

### What `limedl-core` still is

`limedl-core` is the facade: the router (`Dispatcher`), the manager, the
assembly point (`bootstrap`/`SystemContext`), logging, and the fixtures that need
a real manager. Its `lib.rs` is mostly `pub use` statements — one per extracted
layer, re-exported as *modules* rather than as item globs so that every
historical path (`crate::types::TaskId`, `crate::http_executor::…`,
`crate::file_ops::…`) keeps resolving inside the engine and, for the types and
modules it still re-exports, outside it. That shim is why the ~11.5k lines of
engine tests moved without a single edit. [[cite:crates/limedl-core/src/lib.rs:L1-L64]]

The BT crate is re-exported under its old module name
(`#[cfg(feature = "bt")] pub use limedl_bt as bt_backend;`) precisely so callers
keep writing `limedl_core::bt_backend::LazyBtBackend`. The testkit is re-exported
the same way (`pub use limedl_testkit as test_harness`).
[[cite:crates/limedl-core/src/lib.rs:L12-L16]]
[[cite:crates/limedl-core/src/lib.rs:L58-L64]]

The one historical path that does **not** resolve any more is
`limedl_core::aria2_rpc::…`: the RPC layer is a crate above the facade, and the
comment in `lib.rs` records that deliberately — no shim, so the dependency runs
one way. [[cite:crates/limedl-core/src/lib.rs:L85-L87]]
The same arrangement holds in Cargo: `limedl-rpc` lists `limedl-core` as its only
engine dependency. [[cite:crates/limedl-rpc/Cargo.toml:L14-L18]]

## Two layers, one engine

The defining split is that the engine knows nothing about the UI. `limedl-core`
and the crates under it depend on no GUI crate; the desktop client and the
headless server are both frontends over them, and both call the same
`bootstrap(state_dir)`. See [[wiki:architecture/bootstrap-and-services]].
[[cite:crates/limedl-native/src/main.rs:L113-L116]]
[[cite:crates/limedl-server/src/lib.rs:L62-L64]]

`limedl-native` depends on `limedl-core` and `limedl-rpc` separately (the RPC
server is a frontend concern, not an engine one) and compiles the Slint renderer
pair FemtoVG/OpenGL plus the software rasterizer. The renderer is a compile-time
choice; the software fallback exists because a machine whose GL stack fails to
initialize (RDP session, VM, broken driver) would otherwise have no renderer able
to open a window. The advertised `NAME` is the *preferred* renderer, not a
guarantee that the GPU path was taken. The crate also enables tokio's `signal`
feature for the `cfg(unix)` SIGTERM/SIGINT watcher in `main.rs`.
[[cite:crates/limedl-native/Cargo.toml:L16-L42]]

`limedl-server` depends on the same engine crates plus `limedl-rpc`, `clap` and
`anyhow`/`tracing`. It has no Slint, tray or dialog dependency, which is what
lets the release pipeline ship it as a single static musl binary for NAS and
soft-router targets. See [[wiki:integrations/headless-server-daemon]].
[[cite:crates/limedl-server/Cargo.toml:L20-L30]]

The client owns the concerns that are meaningless inside the engine — crash
reporting, the single-instance claim and its activation channel, the tray and the
window platform glue. See [[wiki:desktop/native-ui-architecture]].
[[cite:crates/limedl-native/src/main.rs:L88-L92]]
[[cite:crates/limedl-native/src/main.rs:L102-L104]]
[[cite:crates/limedl-native/src/main.rs:L195-L212]]

## Durable progress is an engine invariant

Download state has two progress counters per chunk: the *received* one
(`ChunkManifest::downloaded`/`completed`), which the scheduler and the UI follow,
and the *durable* one (`ChunkManifest::durable_downloaded`, read through
`durable_bytes()`), which only the write buffer advances after a flush and which
is the only thing the database stores. `DownloadCore::durable_bytes` carries the
task-level twin of that split. That is what makes a crash recoverable instead of
silently resuming past data that never reached the file; the clamp in
`durable_bytes()` means a stale counter can only ever *under*-report. The
mechanism is documented in [[wiki:systems/persistence-and-recovery]] and
[[wiki:workflows/http-download-lifecycle]].
[[cite:crates/limedl-types/src/manifest.rs:L80-L123]]
[[cite:crates/limedl-engine/src/download/managed.rs:L20-L42]]

It is also why the manifest type lives in `limedl-types` while the counters that
advance it live in `limedl-engine`: the schema is shared with the store, the
mutation is engine policy.

## Task identity and protocol routing

Every download is identified by a strongly-typed `TaskId` whose variants are
`Http(Uuid)` and `Bt(InfoHash)`. `InfoHash` is `limedl-types`' own 20-byte
newtype, not the BitTorrent engine's `Id20`: naming the engine's type here would
have dragged the whole BT engine under the foundation layer, so the BT crate
converts at its boundary instead (the orphan rule is spelled out as the reason
the conversion cannot be a `From` impl here). The wire string form is
`http:<uuid-hyphenated>` or `bt:<hex info-hash>`; `TaskId` itself deliberately
does not implement `Serialize`/`Deserialize`, because nothing persists it
directly — summaries carry a `String` id and the Aria2 GID map is in-memory.
`TaskId::kind()` is the routing key.

New requests are classified by `StartDownloadRequest::classify_kind`: an explicit
`kind` wins, otherwise `magnet:` and `.torrent` URLs (including local paths with a
`.torrent` extension) go to BT and `http://`/`https://` goes to HTTP. `http:` /
`bt:` prefixes on the wire are parsed back by `TaskId::from_wire_string`.
[[cite:crates/limedl-types/src/types/task.rs:L31-L59]]
[[cite:crates/limedl-types/src/types/task.rs:L143-L198]]
[[cite:crates/limedl-types/src/types/task.rs:L252-L287]]

The `DownloadBackend` trait is the uniform protocol interface: start, pause,
resume, cancel, remove, purge, open-in-explorer, status, list, update_settings,
optional set_priority, and shutdown. `BackendRegistry` stores each backend twice —
once as a `dyn DownloadBackend` keyed by `TaskKind` for routing, and once as
`Arc<dyn Any>` keyed by `TypeId` for protocol-specific downcasts (`get_typed::<T>`).
`BackendRegistry::dispatch(task_id)` is therefore a simple `task_id.kind()` lookup.
The trait lives in `limedl-types` because `limedl-bt` implements it while sitting
in the middle of the stack; the registry lives in `limedl-core` because only the
facade assembles backends. [[cite:crates/limedl-types/src/protocol.rs:L8-L50]]
[[cite:crates/limedl-core/src/backend_registry/mod.rs:L9-L75]]

## The EventBus

`EventBus` is a thin wrapper over `tokio::sync::broadcast::Sender<DownloadEvent>`
and is cheap to clone. `DownloadEvent` carries strongly typed engine payloads, not
wire JSON: `Updated { summary }`, `Progress { progress }`,
`Aria2Notification { event_name, gid }`, `CdnProgress`, `CdnComplete` and
`Warning`. `publish` does exactly one thing — `tx.send(event)` — and logs a
warning if there are no subscribers. Serialization to JSON happens at the
boundaries that actually need it (Aria2 conversion, SQLite columns), never on the
bus. [[cite:crates/limedl-types/src/event_bus/mod.rs:L14-L81]]

Frontend emission lives in each adapter, not in `publish`:

- The desktop subscribes once in `main()` —
  `event_stream::start_event_bus_listener(&ctx, core.event_bus.subscribe())` —
  and the listener task in `event_stream/bus.rs` updates the Slint store and
  repaints. [[cite:crates/limedl-native/src/main.rs:L224-L228]]
  [[cite:crates/limedl-native/src/event_stream/bus.rs:L294-L347]]
- The Aria2 WebSocket adapter (served from `limedl-rpc`, used by both the desktop
  and the daemon) subscribes per connection and forwards only
  `Aria2Notification` to connected clients, ignoring every other event.
  [[cite:crates/limedl-rpc/src/transport.rs:L37-L68]]

Both adapters must handle `RecvError::Lagged` explicitly: a `while let Ok(...)`
loop would exit permanently on lag. The desktop resynchronizes by calling
`Dispatcher::list()` and replacing the whole store; the Aria2 adapter simply
continues. [[cite:crates/limedl-native/src/event_stream/bus.rs:L329-L344]]
[[cite:crates/limedl-rpc/src/transport.rs:L63-L65]]

## Cross-cutting conventions

- **Serialization**: Rust structs use `#[serde(rename_all = "camelCase")]` and
  enums use `#[serde(rename_all = "snake_case")]` throughout `types/`. This is
  what the JSON settings file and the Aria2 wire responses rely on.
  [[cite:crates/limedl-types/src/types/common.rs:L6-L14]]
  [[cite:crates/limedl-types/src/types/task.rs:L10-L20]]
- **Feature flags**: `limedl-core` defaults to `bt`, which pulls in the optional
  `limedl-bt` and forwards `bt` to `limedl-types`/`limedl-store` (a type-level
  switch there: `TaskKind::Bt`/`TaskId::Bt` must not exist when BT is absent, or
  every gated match arm in the engine becomes non-exhaustive). `test-utils`
  enables `limedl-testkit` and forwards the `test-utils` seam to
  `limedl-disk`/`limedl-store`/`limedl-http`/`limedl-engine`. There is no
  `aria2-rpc` feature any more: the RPC layer is a crate.
  [[cite:crates/limedl-core/Cargo.toml:L50-L60]]
  [[cite:crates/limedl-types/Cargo.toml:L59-L70]]
- **Target flags**: `.cargo/config.toml` adds `target-cpu=x86-64-v3` (desktop)
  and `--cfg reqwest_unstable` for every target; the HTTP/3 feature in reqwest
  hard-fails to compile without that cfg. New targets must carry both. The two
  musl entries (`x86_64`, `aarch64`) are the headless server's, with the x86_64
  one lowered to `x86-64-v2` for older NAS CPUs. The desktop Linux release reuses
  the `x86_64-unknown-linux-gnu` entry through `cargo zigbuild --target
  x86_64-unknown-linux-gnu.2.17`, which needs no separate `[target.*]` entry.
  [[cite:.cargo/config.toml:L1-L42]]
- **Release profile**: the workspace release profile optimizes for size, but
  `limedl-native` is overridden to `opt-level = 3` because rendering is CPU/GPU
  heavy rather than I/O bound. [[cite:Cargo.toml:L113-L126]]
- **Layering guard**: `module_boundaries_tests` in `limedl-core` no longer keeps
  a `LAYERS` table — seven of the layers it used to police are crates now, and an
  upward edge there is a compile error. What the test still guards is a cycle
  *inside* the facade's seven remaining modules (`manager`, `dispatcher`,
  `backend_registry`, `context`, `logging`, `testing`, `bootstrap`), an import of
  `bootstrap` from below (which would invert the assembly order), and a parser
  that silently stops finding modules.

## Runtime topology

```
Slint desktop (limedl-native)
  UI callbacks -> Dispatcher -> BackendRegistry -> DownloadManager / LazyBtBackend
    -> EventBus::publish -> broadcast -> desktop subscriber -> Slint model -> repaint

Aria2 RPC (limedl-rpc, optional, local port)
  AriaNg / Motrix -> JSON-RPC / WebSocket -> Aria2RpcServer -> Dispatcher -> same backends
    -> EventBus -> desktop subscriber (UI stays in sync)

limedl-server (headless daemon)
  AriaNg / Motrix -> JSON-RPC / WebSocket -> Aria2RpcServer -> Dispatcher -> same backends
    (no UI subscriber; the RPC endpoint is the only frontend)
```

All three arrows terminate at the same `Dispatcher`
([[cite:crates/limedl-core/src/dispatcher.rs:L41-L52]]), which is why the frontends
see identical state and none re-implements lifecycle or settings logic. The RPC
server is constructed by whichever frontend wants it — the engine's `bootstrap`
never starts one; the daemon force-enables it after `bootstrap` returns.
[[cite:crates/limedl-server/src/lib.rs:L85-L105]]

Related pages: [[wiki:architecture/bootstrap-and-services]],
[[wiki:architecture/protocol-routing-and-dispatcher]],
[[wiki:integrations/headless-server-daemon]],
[[wiki:desktop/native-ui-architecture]],
[[wiki:operations/build-release-and-ci]].
