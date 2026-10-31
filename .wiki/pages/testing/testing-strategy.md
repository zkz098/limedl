---
id: testing/testing-strategy
title: Testing Strategy
status: verified
generated_at: 2026-10-31T15:15:00Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence:
- path: crates/limedl-core/src/lib.rs
  lines:
  - 89
  - 90
  blob: 8d1831bda64242c4d6dbe2820d6f8cab738ac5e0
  snippet_sha1: abdb6fa75f0ea0a9390cd1da39fe12700fee0f5a
- path: crates/limedl-core/src/tests/mod.rs
  lines:
  - 1
  - 25
  blob: 211792f478f3333837120f5e0a605b9ce56cc427
  snippet_sha1: 1fd4a14abe190f29645a9a4267c7c8dee9256f62
- path: crates/limedl-core/src/tests/mod.rs
  lines:
  - 13
  - 23
  blob: 211792f478f3333837120f5e0a605b9ce56cc427
  snippet_sha1: 9a8910e1000a476ecdfab88a15929b9dfb19d9f8
- path: crates/limedl-core/tests/logging_reload_repro.rs
  lines:
  - 1
  - 9
  blob: 0004c6115e958c95e5000bfc37dffad54ab0303b
  snippet_sha1: e9f6434790c84a26535a99048db665222629db4c
- path: .github/workflows/ci.yml
  lines:
  - 98
  - 106
  blob: 5c8150eda778c6d0b1f463635a371c86d2f145b4
  snippet_sha1: 52e2a7407b215528933677e6415b21ea86b9e0f8
- path: crates/limedl-core/src/tests/corruption_oracle_tests.rs
  lines:
  - 1
  - 17
  blob: aff39d8562614bb6cfcf865f71331a8677e9b8e5
  snippet_sha1: e1535835c4170b80789fd9611f24c7631ca92794
- path: crates/limedl-testkit/src/lib.rs
  lines:
  - 80
  - 82
  blob: 65923f8a06fba3128727da068809956042e16700
  snippet_sha1: ec731327a105e98cc45cfa6b469b2aa0e1b9cf34
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 155
  - 232
  blob: 1425cef6f504dffe584d6b9fd22fbdcb396efdab
  snippet_sha1: a7cdfdacf9d9fdae960ac28b11ee411791bce782
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 308
  - 409
  blob: 1425cef6f504dffe584d6b9fd22fbdcb396efdab
  snippet_sha1: 6e86bb61fad12ef0f4c1a3ce639d04a1795414bb
- path: crates/limedl-core/src/tests/persistence_tests.rs
  lines:
  - 504
  - 577
  blob: 7c5a077d81e82d5638e58ba4b049fd9e184b5495
  snippet_sha1: 8d813018b518995f6984765b2170dc135241f816
- path: crates/limedl-engine/src/persistence.rs
  lines:
  - 4
  - 31
  blob: dac6df18b1fbb4ac265f3454ec8096905dbe3057
  snippet_sha1: 21460e91e07ba832acc9dae97668159979866c34
- path: crates/limedl-engine/src/http_executor/finalize.rs
  lines:
  - 236
  - 260
  blob: 98508e0c71004d01fdccd3e13ccca55eeb40160b
  snippet_sha1: 7815ab29e5606e5f770d06e3136a3693a28c09ba
- path: crates/limedl-engine/src/http_executor/tests.rs
  lines:
  - 240
  - 298
  blob: db5543a7f695be26f10451c9e623aeee5e1bc6ba
  snippet_sha1: 3ef23035ba8985be1bea7195a6ab774a197bc18d
- path: crates/limedl-store/src/database/tests/schema_migrations.rs
  lines:
  - 125
  - 171
  blob: b5847180eefbdb6d6c42978acf3c7f97709128a3
  snippet_sha1: f393b829ff1deacb298af037670585340896c529
- path: crates/limedl-store/src/database/tests/connection.rs
  lines:
  - 176
  - 231
  blob: 5810235c67f60f1a3f4bc8452670cb4f1da15538
  snippet_sha1: a35f7870ebca3c71d47dfaac81fe886fc1327045
- path: crates/limedl-types/src/manifest.rs
  lines:
  - 100
  - 123
  blob: 54ad684831aca2c5fa70d6650426e2a12e96fd1e
  snippet_sha1: 589e056bc1439eaa562188a5f32ed14e53ac6471
- path: crates/limedl-types/src/manifest.rs
  lines:
  - 159
  - 173
  blob: 54ad684831aca2c5fa70d6650426e2a12e96fd1e
  snippet_sha1: c00ede77b577d2d373256ab1118461c2f5a69b77
- path: crates/limedl-store/src/database/tests/concurrency.rs
  lines:
  - 207
  - 213
  blob: 4557e57305bf2e6bccc0ce306d4fb6ec6eabc78c
  snippet_sha1: 509b588c6b815b7fa0a8b67e4dcc7f44400492b5
- path: crates/limedl-core/src/tests/adversarial_interception_tests.rs
  lines:
  - 1
  - 14
  blob: 0e7c50261945e876dbc0af2902519a35a295daf4
  snippet_sha1: 1b991aed9924877cd7449bc760d58fe81df14fec
- path: crates/limedl-core/src/tests/http_executor_tests/http_errors.rs
  lines:
  - 427
  - 584
  blob: d65460b62d051007ee3088304eb39c8eccf2f3b8
  snippet_sha1: cca676f84565bbc82522260bc217cb642ae3d446
- path: crates/limedl-engine/src/task_lifecycle/tests.rs
  lines:
  - 249
  - 299
  blob: 7c7fe3f380cce16376453a09b215105c45ba0b7c
  snippet_sha1: c064b3a0d3a0fb0605ae1707df827e5c31843ac3
- path: crates/limedl-core/src/tests/resume_corruption_tests.rs
  lines:
  - 1
  - 21
  blob: 04d6efe82db6db2a4ba07d793f51b3a59486ce31
  snippet_sha1: dc7c7753f34ef160d2d3279efa6e0d374a2888d0
- path: crates/limedl-core/src/tests/resume_corruption_tests.rs
  lines:
  - 169
  - 196
  blob: 04d6efe82db6db2a4ba07d793f51b3a59486ce31
  snippet_sha1: b52a79f81b7a8d134f9789ee8753fb03f8f3cac1
- path: crates/limedl-testkit/src/lib.rs
  lines:
  - 199
  - 209
  blob: 65923f8a06fba3128727da068809956042e16700
  snippet_sha1: 0edfbdf9e758a830b67ce2f72fd9dfcf5df7f3a6
- path: crates/limedl-testkit/src/tests.rs
  lines:
  - 60
  - 105
  blob: 4d1edddd99e64d3394ced9660997c7dc0f59e9ac
  snippet_sha1: c9afc8015d17fc9b0213e96ef08d1ea3fe04e2ad
- path: crates/limedl-core/src/tests/buffer_integrity_tests.rs
  lines:
  - 1
  - 23
  blob: 236b933ff95e1f0a9bbf0bccb229ac90a58d47a0
  snippet_sha1: c7f7d83d3128a143c54a6486a29ae3e31f0157a9
- path: crates/limedl-disk/src/file_ops/mod.rs
  lines:
  - 19
  - 20
  blob: fdc0466005070507b08a4f0c537b148733132a57
  snippet_sha1: 537efe607fe363227417868d153cae50edae0253
- path: crates/limedl-core/src/tests/persistence_tests.rs
  lines:
  - 411
  - 420
  blob: 7c5a077d81e82d5638e58ba4b049fd9e184b5495
  snippet_sha1: e1e9214000d9f36ea9e2b46c6461dc35b3ae2c14
- path: crates/limedl-engine/src/metalink/tests.rs
  lines:
  - 7
  - 61
  blob: 66bc40c311e6537226247116c0e8d80e198ae246
  snippet_sha1: 773d7b19421558acae8d37b3eb97a86a8bcbc85d
- path: crates/limedl-engine/src/metalink/tests.rs
  lines:
  - 146
  - 176
  blob: 66bc40c311e6537226247116c0e8d80e198ae246
  snippet_sha1: 521251de449342b46c0d13ebd31046f965e337b5
- path: crates/limedl-core/src/tests/http_executor_tests/compression.rs
  lines:
  - 1
  - 13
  blob: d61bc7661c9c417d6ca58acc00b0ae0874214a0b
  snippet_sha1: be1c4e607127ef18e9b513f9fd3551db707203af
- path: crates/limedl-testkit/src/lib.rs
  lines:
  - 220
  - 239
  blob: 65923f8a06fba3128727da068809956042e16700
  snippet_sha1: d7f8c31159baccb16e9bcf3bf05c5dfe6949c3b3
- path: crates/limedl-testkit/src/lib.rs
  lines:
  - 558
  - 561
  blob: 65923f8a06fba3128727da068809956042e16700
  snippet_sha1: 773983b7ba320466d88115a2ad6a9cb489c822da
- path: crates/limedl-core/src/tests/http_executor_tests/compression.rs
  lines:
  - 64
  - 148
  blob: d61bc7661c9c417d6ca58acc00b0ae0874214a0b
  snippet_sha1: 186aa71d8268a1e785789378106a98491cc9884a
- path: crates/limedl-http/src/http/mod.rs
  lines:
  - 237
  - 239
  blob: 9dc32195af6d3982fad1d090f95559e3839465e9
  snippet_sha1: 251f0d18f59da37e277121a9fce678bd9471e89c
- path: crates/limedl-core/src/tests/dispatcher_tests.rs
  lines:
  - 1
  - 9
  blob: ed883188b00a1e82527c20072a1706a7ad29331e
  snippet_sha1: 35284a701594b46c51edfb0c4c253ba498274201
- path: crates/limedl-core/src/tests/dispatcher_tests.rs
  lines:
  - 379
  - 666
  blob: ed883188b00a1e82527c20072a1706a7ad29331e
  snippet_sha1: fbda1f865a988cf1b46d02cb9c29962e9954d421
- path: crates/limedl-core/src/testing.rs
  lines:
  - 1
  - 44
  blob: 4aa513ad3fe5e8bfdee6986f0857e6e7d732b548
  snippet_sha1: e43f9d4555b458c37d6b26d4cb2982510c428eb1
- path: crates/limedl-core/src/testing.rs
  lines:
  - 160
  - 169
  blob: 4aa513ad3fe5e8bfdee6986f0857e6e7d732b548
  snippet_sha1: 08f4953b75c2cb9417ab5abcf67227f11cf7ad2f
- path: crates/limedl-rpc/src/tests.rs
  lines:
  - 909
  - 925
  blob: daff971e1802bd15737d31a9f03b2c72e4ebea12
  snippet_sha1: c92092dd59425f1df554f4f0c8ecd03f348b4c95
- path: crates/limedl-rpc/src/e2e_tests.rs
  lines:
  - 1
  - 8
  blob: 68213e81202fc594e5cfd0ba1e6755dadeb8592a
  snippet_sha1: 95caa1b035bb46d50f3baaac7b22985c8e9604f3
- path: crates/limedl-rpc/src/e2e_tests.rs
  lines:
  - 22
  - 41
  blob: 68213e81202fc594e5cfd0ba1e6755dadeb8592a
  snippet_sha1: bad4bc91982a69a0dc0c55513110bfc97a621606
- path: crates/limedl-rpc/src/e2e_tests.rs
  lines:
  - 1236
  - 1337
  blob: 68213e81202fc594e5cfd0ba1e6755dadeb8592a
  snippet_sha1: 51439f279178fd510e7223aeb6b183d502e96602
- path: crates/limedl-rpc/src/e2e_tests.rs
  lines:
  - 2434
  - 2483
  blob: 68213e81202fc594e5cfd0ba1e6755dadeb8592a
  snippet_sha1: cfa5bc0cb109311def31daeb3347d227caf0d482
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 1
  - 21
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: d6343f03e152aa105725a5c793edc45f1e7573d2
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 27
  - 124
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: 445b44e36d68301ab0df4d733274a5f0817592bc
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 126
  - 162
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: 4106e8e894482729142d856f82532ed154f9ae99
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 163
  - 255
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: 19bba39e6607a753a02d5d0c2a24d58aa2cf65d5
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 256
  - 396
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: 0ed5d971a22cca9fcaefe2a131161aea0164ebdd
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 397
  - 557
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: a886da0d6a0d6e46e28b911f4eaede00b88b52ff
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 622
  - 692
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: 5ca6af259218e96f20c1141b38edbdfeb8f04735
- path: crates/limedl-rpc/src/tests.rs
  lines:
  - 872
  - 906
  blob: daff971e1802bd15737d31a9f03b2c72e4ebea12
  snippet_sha1: f1ee80a1868d350cb314ffe4901f7f60a6d0996d
- path: crates/limedl-rpc/src/tests.rs
  lines:
  - 960
  - 975
  blob: daff971e1802bd15737d31a9f03b2c72e4ebea12
  snippet_sha1: dd2a686d92e5ddc0cf3d8e2de9b9e64df7128e3b
- path: .github/workflows/ci.yml
  lines:
  - 246
  - 248
  blob: 5c8150eda778c6d0b1f463635a371c86d2f145b4
  snippet_sha1: 6c9298e41bd8d3496b6f27549d066350d9e6b8e8
- path: docs/aria2-interop-testing.md
  lines:
  - 12
  - 41
  blob: 57ce0f0b76a3264bc7db7d8bd38a045e73c81d49
  snippet_sha1: a94c082f3ad315fd95b93872e8723268a5386e22
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 576
  - 617
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: e54fe2b1ac0596bb789755fda627ea8bccccdcd8
- path: crates/limedl-rpc/src/oracle_tests.rs
  lines:
  - 1
  - 23
  blob: 8f5ea65b8ea7dd966d37dadcc0a1c6f1946b2de5
  snippet_sha1: 740c02ae9f60bde972ccb90a8950b95274409c6c
- path: .github/workflows/aria2-oracle.yml
  lines:
  - 1
  - 28
  blob: 364518b8f43f7fd0a03443cb5a1c86cc62dc0b34
  snippet_sha1: ac0a8a013a84ba2fc27784f0b2bcfa236e21ef1d
- path: .github/workflows/aria2-oracle.yml
  lines:
  - 35
  - 59
  blob: 364518b8f43f7fd0a03443cb5a1c86cc62dc0b34
  snippet_sha1: ebd2c14b1497f3e74f536227247a9b805f5f3bb9
- path: crates/limedl-rpc/src/oracle_tests.rs
  lines:
  - 29
  - 52
  blob: 8f5ea65b8ea7dd966d37dadcc0a1c6f1946b2de5
  snippet_sha1: 96011cf18dd3319d7c41033feb0cf800283a6e24
- path: crates/limedl-rpc/src/oracle_tests.rs
  lines:
  - 57
  - 145
  blob: 8f5ea65b8ea7dd966d37dadcc0a1c6f1946b2de5
  snippet_sha1: ccaf56a2a804dfb5222d6dc1804dbc4752b7f666
- path: crates/limedl-rpc/src/oracle_tests.rs
  lines:
  - 150
  - 240
  blob: 8f5ea65b8ea7dd966d37dadcc0a1c6f1946b2de5
  snippet_sha1: 4282b1fce6f39bf8c008e1a884e7d3904ce169c1
- path: crates/limedl-rpc/src/oracle_tests.rs
  lines:
  - 411
  - 446
  blob: 8f5ea65b8ea7dd966d37dadcc0a1c6f1946b2de5
  snippet_sha1: f0c03efe9b6fa35d4c39011de1f94d21caaa6726
- path: docs/aria2-interop-testing.md
  lines:
  - 172
  - 194
  blob: 57ce0f0b76a3264bc7db7d8bd38a045e73c81d49
  snippet_sha1: 76566c0101c8a41d0ce0f9e57fb49a3dfdf2f7d9
- path: crates/limedl-bt/src/tests/mod.rs
  lines:
  - 62
  - 106
  blob: f85fd0d459141e25c9514043e609c0326883141d
  snippet_sha1: cdb0a376acca0e74b361ce7dfb6fd71dce4988d6
- path: crates/limedl-bt/src/fixtures.rs
  lines:
  - 12
  - 23
  blob: 19d4dd5b3eb367f20a983cad9b1ae827edb2270e
  snippet_sha1: 40faaee15ee6f1d7e44130b05e8c34c5a6e8d519
- path: crates/limedl-bt/src/tests/queries.rs
  lines:
  - 1
  - 6
  blob: 84daa785d871b864cdcdb1102b67c60e2a3827aa
  snippet_sha1: f3dea2833bab9f90d9ba49f8630f9db79369bd33
- path: crates/limedl-server/src/cli.rs
  lines:
  - 92
  - 143
  blob: 708212ec73af32d6e5044a20486a2540665b2548
  snippet_sha1: 1cfff92b16d6b07e6e82e88fcd3127987057580c
- path: crates/limedl-server/src/config.rs
  lines:
  - 126
  - 192
  blob: 8ef47e880dcb943c9f06542eca334f8bcd2a1dcc
  snippet_sha1: 8187bedfe1ae86f620eb7d2bd68d446e1280b960
- path: crates/limedl-server/src/lib.rs
  lines:
  - 194
  - 213
  blob: db33f43e46019b79e0d8eeb7a1198a4d39812a40
  snippet_sha1: d999fca22fa1b0c08ece7f700845a9bc73a9cd26
- path: crates/limedl-server/tests/daemon.rs
  lines:
  - 1
  - 126
  blob: adbce0f5cb9fd81e192a2ca17f7f10af3c332c2c
  snippet_sha1: 8837b2e65ed26a8936cb347dc3a1e61830863b0c
- path: crates/limedl-server/src/lib.rs
  lines:
  - 86
  - 94
  blob: db33f43e46019b79e0d8eeb7a1198a4d39812a40
  snippet_sha1: a156857ec1fe4b67d754372156132bc9479226bb
- path: crates/limedl-server/src/lib.rs
  lines:
  - 135
  - 141
  blob: db33f43e46019b79e0d8eeb7a1198a4d39812a40
  snippet_sha1: d76306f32c923fb46f11aacb2404630a9bff3365
- path: .github/workflows/ci.yml
  lines:
  - 596
  - 603
  blob: 5c8150eda778c6d0b1f463635a371c86d2f145b4
  snippet_sha1: 901c5378cc4165c563a176b529c89789a04ea0a8
- path: crates/limedl-server/src/lib.rs
  lines:
  - 188
  - 192
  blob: db33f43e46019b79e0d8eeb7a1198a4d39812a40
  snippet_sha1: cf8bd048c12aebd77866fe887989bd22a5ee2a54
- path: .github/workflows/ci.yml
  lines:
  - 396
  - 402
  blob: 5c8150eda778c6d0b1f463635a371c86d2f145b4
  snippet_sha1: 24ff3d72650a9bc26cf518b3117fbd4f418e3510
- path: .github/workflows/ci.yml
  lines:
  - 436
  - 448
  blob: 5c8150eda778c6d0b1f463635a371c86d2f145b4
  snippet_sha1: a0b7aef69bd0141448a8908b973e8e485a32ac97
- path: .github/workflows/ci.yml
  lines:
  - 475
  - 477
  blob: 5c8150eda778c6d0b1f463635a371c86d2f145b4
  snippet_sha1: 938944dc4171ac354eff9b01764dcfd0a32f731c
- path: .github/workflows/ci.yml
  lines:
  - 505
  - 508
  blob: 5c8150eda778c6d0b1f463635a371c86d2f145b4
  snippet_sha1: fe213d8357a2c76009c9af6e5520ac30a80e9805
- path: sonar-project.properties
  lines:
  - 54
  - 56
  blob: b94b995f2572baaa24363962787c936892c95a6b
  snippet_sha1: a029c63a69da2977822ee0802fcd1336e5bbcd57
- path: sonar-project.properties
  lines:
  - 83
  - 84
  blob: b94b995f2572baaa24363962787c936892c95a6b
  snippet_sha1: 8a988f6a51837c0c85ccc03c4a64f36acf82674a
type: testing
---
# Testing Strategy

limedl's engine tests live beside the code (unit tests inside each crate) or in
`crates/limedl-core/src/tests/` (cross-module integration, declared by the
`#[cfg(test)] mod tests` in `lib.rs`), with two dedicated integration binaries
under `crates/limedl-core/tests/`. The UI layer has its own in-process suite,
documented on testing/slint-ui-testing; the headless daemon has an end-to-end
test in its own crate (below).
[[cite:crates/limedl-core/src/lib.rs:L89-L90]]
[[cite:crates/limedl-core/src/tests/mod.rs:L1-L25]]

## Test layout conventions

1. Small `#[cfg(test)] mod tests` blocks stay inline in the production file.
2. A larger block moves to a sibling `tests.rs` — e.g.
   `crates/limedl-types/src/settings/tests.rs`,
   `crates/limedl-engine/src/http_executor/tests.rs`,
   `crates/limedl-engine/src/scheduler/tests.rs` — leaving only
   `#[cfg(test)] mod tests;` in the production file; tests still reach private
   items via `use super::*;`.
3. Cross-module E2E tests that need a mock server or several subsystems go in
   `crates/limedl-core/src/tests/`, declared by `lib.rs`'s `mod tests`.
   [[cite:crates/limedl-core/src/tests/mod.rs:L1-L25]]
4. A test file that grows large is split into a same-named directory
   (`mod.rs` holds imports and shared fixtures, `<scenario>.rs` holds tests), as
   done for `manager_tests/`, `http_executor_tests/` and `scheduler_tests/` in
   the core suite, and for `crates/limedl-bt/src/tests/`,
   `crates/limedl-disk/src/buffer_pool/tests/` and
   `crates/limedl-store/src/database/tests/` in the split crates.
   [[cite:crates/limedl-core/src/tests/mod.rs:L13-L23]]
5. Tests that take a process-global slot (notably a `tracing_subscriber` global
   init) get their **own integration binary**, because libtest runs sibling
   tests in one process and a second `init()` panics. That is why
   `tests/logging_reload_repro.rs` (clean process) and
   `tests/logging_preinstalled_subscriber.rs` (pre-installed subscriber) are
   separate files.
   [[cite:crates/limedl-core/tests/logging_reload_repro.rs:L1-L9]]

## nextest process isolation

Tests run under `cargo nextest`, which executes each test in its own process.
That parallelizes the suite and removes the cross-test global-state interference
a single libtest process per binary exposes. The coverage legs use
`cargo llvm-cov nextest` for the same reason, with `--no-fail-fast` so one
failing test cannot shorten the coverage union. `cargo nextest run` does **not**
run doctests; the workspace has none today, and adding one requires a
`cargo test --doc` step in CI and the gate.
[[cite:.github/workflows/ci.yml:L98-L106]]

## The corruption oracle

The strongest tests in the repo target the user report "download finishes but the
SHA does not match". `corruption_oracle_tests.rs` exercises the **full** engine
path (HTTP probe → chunked range download → buffered writes → finalize), then
re-reads the assembled file from disk and hashes it **independently** with
SHA-256, comparing against the server's known hash. This is deliberately
stronger than asserting `state == Completed`, which cannot see a corrupt file.
The server content is deterministic (seeded PRNG, seed 42), so a failure in any
iteration is a real engine nondeterminism — a bug report, not a flaky test — and
the corrupt temp file should be preserved for diagnosis.
[[cite:crates/limedl-core/src/tests/corruption_oracle_tests.rs:L1-L17]]
[[cite:crates/limedl-testkit/src/lib.rs:L80-L82]]

## Crash consistency and durable progress

The tests that guard the durable-vs-received split (see
systems/persistence-and-recovery) are deliberately layered, because the failure
they prevent — resuming past data that never reached the file — is invisible
from the outside:

- **Pure range math** (`crates/limedl-engine/src/download/managed.rs`):
  `record_progress_on_managed` moves only the received counters;
  `record_durable_bytes` is the only writer of the durable ones; a coalesced
  write is credited to *every* chunk it overlaps; `durable_bytes()` is clamped
  and never exceeds the received count; and a manifest built like a loaded one
  starts `durable == received`.
  [[cite:crates/limedl-engine/src/download/managed.rs:L155-L232]]
  [[cite:crates/limedl-engine/src/download/managed.rs:L308-L409]]
- **Persist rules** (`crates/limedl-core/src/tests/persistence_tests.rs`):
  `persisted_progress_is_the_durable_progress` runs `DownloadManager::persist`
  with a chunk that is received-complete but only partly flushed and asserts the
  stored row says 40 bytes and `completed = false`;
  `incremental_persist_of_a_completed_but_unflushed_chunk_stays_incomplete`
  drives the same substitution through `persist_manifest_snapshot`, the function
  of the 300 ms incremental cycle, and asserts the row says 25 bytes. Both go
  through a real `Database`.
  [[cite:crates/limedl-core/src/tests/persistence_tests.rs:L504-L577]]
  [[cite:crates/limedl-engine/src/persistence.rs:L4-L31]]
- **Finalize guard** (`http_executor`): `ensure_core_fully_durable` accepts a
  fully durable download and a chunk-less single-stream one, and refuses a
  download with a shortfall with a message naming the missing byte count ("40
  byte(s)") and "refusing to publish". It takes the already-held core guard on
  purpose — calling `lock_core()` there deadlocks, since the `parking_lot`
  mutex is not reentrant.
  [[cite:crates/limedl-engine/src/http_executor/finalize.rs:L236-L260]]
  [[cite:crates/limedl-engine/src/http_executor/tests.rs:L240-L298]]
- **Migrations and quarantine** (`crates/limedl-store/src/database/tests`):
  `partially_applied_migration_is_recovered_on_open` opens a database whose schema
  is ahead of its `user_version`, `add_column_if_missing_is_idempotent` pins the
  no-op, and the connection tests cover the non-database quarantine and the
  lock-contention classifier.
  [[cite:crates/limedl-store/src/database/tests/schema_migrations.rs:L125-L171]]
  [[cite:crates/limedl-store/src/database/tests/connection.rs:L176-L231]]

### Fixture invariant: `end` is inclusive

`ChunkManifest::end` is the **last byte** of the chunk — `plan_chunks` emits
`end = start + chunk_size - 1` and starts the next chunk at `end + 1` — so a
chunk's size is `byte_len() == end - start + 1`, and "complete" means
`durable_bytes() >= byte_len()`. A fixture that writes `end` as an exclusive
bound (`start: 0, end: 500` with `downloaded: 500, completed: true`) is a
501-byte chunk that can never complete; it passes only while `completed` is
stored verbatim, and breaks once the row derives completeness from the durable
count. New fixtures must state `end` as the last byte, never one past it.
[[cite:crates/limedl-types/src/manifest.rs:L100-L123]]
[[cite:crates/limedl-types/src/manifest.rs:L159-L173]]
[[cite:crates/limedl-store/src/database/tests/concurrency.rs:L207-L213]]

### Adversarial servers

`adversarial_interception_tests.rs` uses two hostile hosts that return wrong
bytes for valid ranges: `range-shifted` (content shifted relative to the
advertised `Content-Range`) and `range-bitflip` (first byte of each range
flipped). Both keep the byte counts identical, so every chunk is accepted and the
download reaches finalize — only the checksum comparison can expose the problem.
With an explicit `expected_checksum`, the download must end in `Failed`.
[[cite:crates/limedl-core/src/tests/adversarial_interception_tests.rs:L1-L14]]

### Mid-stream body interruption

`http_executor_tests/http_errors.rs` serves a 64 KiB file whose **first** GET
sends 16 KiB and then aborts the body stream with a `ConnectionReset` error; the
resumed request is answered from the `Range` header. The test
(`stream_interruption_resumes_and_completes`) requires the download to finish
with the exact on-disk bytes instead of failing with "error decoding response
body" — the single-stream loop must treat a broken body as a retryable transport
error and resume from its durable offset. The classification it depends on is
pinned by `task_lifecycle::tests::is_network_error_http_decode_is_true`, which
asserts a reqwest *decode* error counts as a network error (so a multi-URL task
fails over to the next mirror) while an HTTP builder error does not.
[[cite:crates/limedl-core/src/tests/http_executor_tests/http_errors.rs:L427-L584]]
[[cite:crates/limedl-engine/src/task_lifecycle/tests.rs:L249-L299]]

### Resume and buffer integrity

`resume_corruption_tests.rs` pauses at staggered points, resumes, finishes, and
re-reads the file to require SHA-256 equality. Resume is a prime suspect because
the engine resets its incremental hasher on pause and re-derives incomplete-chunk
offsets from the manifest; the tests avoid racing a fast localhost download by
throttling the origin instead of the pausing loop (`pause_at_fraction` uses the
bandwidth endpoint). The multi-threaded case pauses on the first progress
(`pause_on_any_progress`) through `TestServer::file_url_range_bandwidth`, a
range-capable endpoint that throttles each chunk connection so the transfer is
still in flight when the pause lands — without it a 17 MiB localhost download
could finish and enter `Verifying` between the progress poll and the pause call.
[[cite:crates/limedl-core/src/tests/resume_corruption_tests.rs:L1-L21]]
[[cite:crates/limedl-core/src/tests/resume_corruption_tests.rs:L169-L196]]
[[cite:crates/limedl-testkit/src/lib.rs:L199-L209]]

The harness endpoint itself is covered by unit tests in
`crates/limedl-testkit/src/tests.rs`
(`range_bandwidth_endpoint_serves_throttled_ranges`,
`range_bandwidth_endpoint_serves_full_file_without_range`).
[[cite:crates/limedl-testkit/src/tests.rs:L60-L105]]

`buffer_integrity_tests.rs` injects deterministic I/O failures targeted by file
pointer through `crate::file_ops::fault` (gated on
`cfg(any(test, feature = "test-utils"))`), so parallel tests writing to other
files are never disturbed. It proves two things: a background flush failure sets
the buffer's error flag (`has_degraded()`, `flush_all` errors) without corrupting
already-written bytes, and a real download using the write-combining buffer ends
in `Failed` rather than a silent `Completed`.
[[cite:crates/limedl-core/src/tests/buffer_integrity_tests.rs:L1-L23]]
[[cite:crates/limedl-disk/src/file_ops/mod.rs:L19-L20]]

## Mock-server integration corpus

`crates/limedl-core/src/tests/` also holds the scenario suites:

- `manager_tests/` — startup and validation, thread resolution, progress,
  checksum, eviction, lifecycle, queries, scheduling.
- `http_executor_tests/` — single stream, multi stream, chunk workers, mirrors
  and abuse, HTTP errors, rate limiting, response compression.
- `scheduler_tests/` and `scheduler_concurrency_tests.rs` — limits, lifecycle,
  scheduling.
- `persistence_tests.rs` / `persistence_e2e_tests.rs` — restart recovery,
  including `downloading_survives_restart_without_stale_claims`.
  [[cite:crates/limedl-core/src/tests/persistence_tests.rs:L411-L420]]
- `checksum_e2e_tests.rs`, `mirror_e2e_tests.rs`, `cdn_e2e_tests.rs`,
  `integrity_tests.rs`, `protocol_tests.rs`, `bootstrap_tests.rs`,
  `bt_backend_tests.rs`, `settings_roundtrip_tests.rs`.

[[cite:crates/limedl-core/src/tests/mod.rs:L1-L25]]

Metalink parsing moved with its module into the engine crate:
`crates/limedl-engine/src/metalink/tests.rs` covers Metalink 4.0/3.0 and
Metalink/HTTP header parsing (including the `best_checksum` and
`sorted_mirror_urls` helpers), mirror scoring, and pool leasing/concurrency
limits. Disk-type detection likewise lives in `limedl-disk`
(`file_ops/disk_detect_tests.rs`).
[[cite:crates/limedl-engine/src/metalink/tests.rs:L7-L61]]
[[cite:crates/limedl-engine/src/metalink/tests.rs:L146-L176]]

### Response compression tests

`http_executor_tests/compression.rs` covers the gzip/brotli/zstd paths negotiated
by reqwest's decompression features. The test server exposes
`/file/encoded/{enc}` (single-stream) and `/file/encoded-range/{enc}`
(range-capable but returns the whole compressed body as a `200` when asked),
compresses only when the request's `Accept-Encoding` allows it, and counts how
often it did (`TestServer::encoded_responses`).
[[cite:crates/limedl-core/src/tests/http_executor_tests/compression.rs:L1-L13]]
[[cite:crates/limedl-testkit/src/lib.rs:L220-L239]]
[[cite:crates/limedl-testkit/src/lib.rs:L558-L561]]

Each encoding is exercised twice:

- single-stream: the download must complete with the checksum of the **decoded**
  content, the reported length must be the decoded size, and the server must have
  compressed at least once;
- range: a 4-thread download of a 9 MiB file must complete with the decoded
  checksum and the server must have compressed **zero** times — proving
  `identity_encoding` kept every segment request byte-exact.

[[cite:crates/limedl-core/src/tests/http_executor_tests/compression.rs:L64-L148]]
[[cite:crates/limedl-http/src/http/mod.rs:L237-L239]]

### Dispatcher facade tests

`crates/limedl-core/src/tests/dispatcher_tests.rs` is the `Dispatcher` facade
matrix: lifecycle `Updated` emission for cancel/remove/purge/pause/resume,
`status`/`list`/`has_active_downloads` aggregation, degraded branches when a
service is absent, `save_settings` synchronization to the ConcurrencyManager and
BufferPool, CDN clear when disabled, mirror resolution and tracker-list
normalization.
[[cite:crates/limedl-core/src/tests/dispatcher_tests.rs:L1-L9]]
[[cite:crates/limedl-core/src/tests/dispatcher_tests.rs:L379-L666]]

After the crate split the `make_manager` / `inject_download` fixtures moved from
this file into the published `crate::testing` module (gated on
`cfg(any(test, feature = "test-utils"))`) so the Aria2 RPC tests in `limedl-rpc`
reuse them to cover `resolve_gid`, GID-cache eviction and
`changeOption` against a real `DownloadManager` instead of duplicating a
`ManagedDownload` fixture.
[[cite:crates/limedl-core/src/testing.rs:L1-L44]]
[[cite:crates/limedl-core/src/testing.rs:L160-L169]]
[[cite:crates/limedl-rpc/src/tests.rs:L909-L925]]

### Aria2 RPC E2E

`crates/limedl-rpc/src/e2e_tests.rs` boots a real `Aria2RpcServer` over a live
`bootstrap()` `CoreSystems` on a random port and drives it with real HTTP POST
requests: the handler matrix, `system.multicall` response shape, secret gating on
every method, CORS fallback **and wildcard**, magnet-to-BT routing,
`addMetalink` (a base64 Metalink document creates a task per file and reports
status), `changeOption` matrices, `keys` filtering, `getUris`, and the
terminal-task eviction scenarios. Because the CORS test now covers
`allow_any_origin`, it also asserts that a wildcard origin is never paired with
`Access-Control-Allow-Credentials`.
[[cite:crates/limedl-rpc/src/e2e_tests.rs:L1-L8]]
[[cite:crates/limedl-rpc/src/e2e_tests.rs:L22-L41]]
[[cite:crates/limedl-rpc/src/e2e_tests.rs:L1236-L1337]]
[[cite:crates/limedl-rpc/src/e2e_tests.rs:L2434-L2483]]

### Aria2 interoperability fixtures (Tier 1)

`crates/limedl-rpc/src/interop_tests.rs` is the client-contract layer: rather
than the hand-written request shapes in `e2e_tests.rs`, it encodes the shapes
real clients send. It exists because a shape mismatch can pass limedl's own
tests while breaking a real client — `addTorrent` used to read `params[1]` as
the options object, so AriaNg's `[torrent, [], options]` silently dropped
`dir`/`out`/`pause`/`select-file` while the `[torrent, options]` tests stayed
green. [[cite:crates/limedl-rpc/src/interop_tests.rs:L1-L21]]

The suite asserts:

- `system.listMethods` equals the routed handler set exactly (37 entries), and
  every advertised method is reachable (a routed method's error must not be
  method-not-found; since every domain error is code 1, that is recognized by
  its message). [[cite:crates/limedl-rpc/src/interop_tests.rs:L27-L124]]
- AriaNg's multicall shape (no outer token, per-call tokens) yields the
  single-element `[value]` wrapper.
  [[cite:crates/limedl-rpc/src/interop_tests.rs:L126-L162]]
- `addTorrent([torrent, [], options])` and `[torrent, options]` both keep the
  options, and `addUri([urls, options])` keeps `dir`/`out`.
  [[cite:crates/limedl-rpc/src/interop_tests.rs:L163-L255]]
- `tellStatus` carries aria2's always-present keys plus the HTTP piece map and
  the BT metadata object, and stays silent about `errorCode` until terminal.
  [[cite:crates/limedl-rpc/src/interop_tests.rs:L256-L396]]
- `getServers`, `changeUri` (`[deleted, added]`) and `changePosition` (the real
  resulting index) match their aria2 reply shapes.
  [[cite:crates/limedl-rpc/src/interop_tests.rs:L397-L557]]
- `getGlobalOption` and `getOption` answer AriaNg's complete global and task key
  sets (`interop_get_global_option_covers_ariang_keys` /
  `interop_get_option_covers_ariang_task_keys`).
  [[cite:crates/limedl-rpc/src/interop_tests.rs:L622-L692]]
- A top-level JSON-RPC batch returns one response per element; an
  all-notification batch returns nothing and an empty array is Invalid Request
  (`batch_request_returns_one_response_per_element` and friends in
  `crates/limedl-rpc/src/tests.rs`).
  [[cite:crates/limedl-rpc/src/tests.rs:L872-L906]]
- An abbreviated GID resolves only when its prefix is unique
  (`resolve_gid_accepts_a_unique_prefix_and_refuses_an_ambiguous_one`).
  [[cite:crates/limedl-rpc/src/tests.rs:L960-L975]]

It is pure Rust with no external binary, so it runs inside the normal nextest
gate, which lists `-p limedl-rpc` among the packages. The Tier 2
(real `aria2c` oracle) and Tier 3 (AriaNg smoke test) layers, together with the
list of intentional deviations, live in `docs/aria2-interop-testing.md`.
[[cite:.github/workflows/ci.yml:L246-L248]]
[[cite:docs/aria2-interop-testing.md:L12-L41]]
`interop_get_version_is_truthful` additionally locks `aria2.getVersion` to the
real version and the truthful feature list (no XML-RPC/Firefox3 Cookie/SFTP;
Metalink, GZip, Brotli and Zstd present).
[[cite:crates/limedl-rpc/src/interop_tests.rs:L576-L617]]

### Aria2 oracle (Tier 2, opt-in)

`crates/limedl-rpc/src/oracle_tests.rs` is the layer above the fixtures: it
starts a real `aria2c --enable-rpc` beside limedl's server and reports how the
same requests differ, which is the only way to catch "aria2 itself returns
something else".

The suite is **opt-in** via `ARIA2_ORACLE_BIN`. Unset, every test returns early,
so the normal gate never needs a third-party binary; set but unusable, it
panics, so a job cannot pass by accident. A separate nightly/manual workflow
(`.github/workflows/aria2-oracle.yml`) sets it on Linux.
[[cite:crates/limedl-rpc/src/oracle_tests.rs:L1-L23]]
[[cite:.github/workflows/aria2-oracle.yml:L1-L28]]
[[cite:.github/workflows/aria2-oracle.yml:L35-L59]]

Differences are **allowlisted**: `GapReport` fails the run on an aria2 key
limedl is required to answer (AriaNg's option keys, the always-present
`tellStatus` keys), a `listMethods`/`listNotifications` regression, an error
object that is not code 1, or a batch that is not an array; the remaining
aria2-only differences are printed as allowlisted notes (nextest needs
`--success-output=final` to show captured output from passing tests). The job
also fails when the oracle cannot start or a server stops answering. The
`Aria2Oracle` guard handles the operational contract — free port, `--no-conf`,
`--enable-dht=false`, a `TempDir`, a readiness poll and a `Drop` that kills the
child. The readiness poll tolerates the connection-refused state that is expected
until `aria2c` binds its port: it calls a non-panicking `rpc_try` (returning
`None` on transport or parse errors) in a bounded retry loop, and checks
`child.try_wait()` each round so an early child exit fails with the real status
instead of a readiness timeout.
[[cite:crates/limedl-rpc/src/oracle_tests.rs:L29-L52]]
[[cite:crates/limedl-rpc/src/oracle_tests.rs:L57-L145]]
[[cite:crates/limedl-rpc/src/oracle_tests.rs:L150-L240]]
[[cite:crates/limedl-rpc/src/oracle_tests.rs:L411-L446]]
[[cite:docs/aria2-interop-testing.md:L172-L194]]

### BT backend tests

`crates/limedl-bt/src/tests/` builds the librqbit-backed `RqbitBtBackend` via
`make_backend()` with all network features disabled (DHT, UPnP, IPv6, uTP) and
`listen_port = Some(0)`, so tests bind no fixed port and need no network; tests
that genuinely require DHT/tracker are marked `#[ignore]`. Torrent fixtures are
hand-written bencode in `crates/limedl-bt/src/fixtures.rs`
(`multi_file_torrent_bytes`, `single_file_torrent_bytes` with all-zero piece
hashes, since parsing only needs structure). Any test that goes through
`block_in_place` must use `#[tokio::test(flavor = "multi_thread")]`, because
`block_in_place` panics on a current-thread runtime.
[[cite:crates/limedl-bt/src/tests/mod.rs:L62-L106]]
[[cite:crates/limedl-bt/src/fixtures.rs:L12-L23]]
[[cite:crates/limedl-bt/src/tests/queries.rs:L1-L6]]

## Headless server daemon

The `limedl-server` crate adds two layers:

- **Unit tests** for the parts that need no engine: CLI parsing, the
  `LogLevelArg` → `LogLevel` mapping, config precedence/trimming, absolute-path
  validation for `--download-dir`, the explicit data-dir override, and the
  instance lock's exclusivity (a second lock on the same directory fails, and the
  lock is reusable after release).
  [[cite:crates/limedl-server/src/cli.rs:L92-L143]]
  [[cite:crates/limedl-server/src/config.rs:L126-L192]]
  [[cite:crates/limedl-server/src/lib.rs:L194-L213]]
- **One end-to-end test** (`tests/daemon.rs`) that starts a real daemon against a
  temp data directory on a reserved port and proves the wiring the RPC crate's
  tests cannot: `bootstrap` → force-enabled RPC → token enforcement
  (a request without `token:` is `Unauthorized`) → `aria2.shutdown` →
  `registry.shutdown_all()` → the daemon task exits cleanly within a timeout. The
  shutdown future is injected (`std::future::pending()`), so the only way the
  process may stop is the JSON-RPC call the test sends. Readiness is polled with a
  fallible `rpc_try` that treats a refused connection as "not ready yet", because
  the engine bootstraps and binds its port asynchronously; only the request after
  readiness may panic.
  [[cite:crates/limedl-server/tests/daemon.rs:L1-L126]]
  [[cite:crates/limedl-server/src/lib.rs:L86-L94]]
  [[cite:crates/limedl-server/src/lib.rs:L135-L141]]

It runs in the Linux `check-rust` job only. The Windows/macOS legs compile the
crate (including the `cfg(not(unix))` Ctrl+C path) through the workspace clippy
step but do not execute the port-binding test, because the daemon targets musl
Linux and a bootstrapped engine on those runners would add flake for no signal.
[[cite:.github/workflows/ci.yml:L596-L603]]
[[cite:crates/limedl-server/src/lib.rs:L188-L192]]

## Coverage

`check-rust` runs `cargo llvm-cov nextest` for the engine packages with
`--fail-under-lines 85` (pinned `cargo-llvm-cov@0.9.0`), then a second lcov for
`limedl-native`. cargo-llvm-cov ignores `tests/` directories and
`tests.rs`/`*_tests.rs` files by default, so the metric counts **product code
only**. A `cargo test --doc` step would be needed if a doctest is ever added.
[[cite:.github/workflows/ci.yml:L396-L402]]
[[cite:.github/workflows/ci.yml:L436-L448]]
[[cite:.github/workflows/ci.yml:L475-L477]]
[[cite:.github/workflows/ci.yml:L505-L508]]
`limedl-server` has no lcov report and is listed in Sonar's
`sonar.coverage.exclusions`, so its lines are not counted as 0% — its value is in
the end-to-end test, not per-line coverage.
[[cite:sonar-project.properties:L54-L56]]
[[cite:sonar-project.properties:L83-L84]]

Related pages: testing/slint-ui-testing, integrations/headless-server-daemon,
integrations/aria2-rpc-server, workflows/metalink-and-mirror-selection,
operations/build-release-and-ci, workflows/http-download-lifecycle.
