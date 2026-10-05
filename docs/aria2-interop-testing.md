# Aria2 interoperability testing

How limedl verifies that real aria2 clients (AriaNg, Motrix, userscripts, CLI
tools) can drive its JSON-RPC server, and the plan for the layers that need a
real `aria2c` binary.

The server lives in `crates/limedl-core/src/aria2_rpc/`; the compatibility
contract spans the dispatcher (`dispatch.rs`), the status mapper
(`protocol.rs`) and the query/lifecycle handlers (`query.rs`, `download.rs`,
`options.rs`).

## Tier 1 — fixture contract tests (in the normal gate)

`crates/limedl-core/src/aria2_rpc/interop_tests.rs` encodes the **wire shapes
real clients send**, not the shapes limedl happens to expect. It is pure Rust,
needs no external binary, and runs with the existing core gate:

```sh
cargo nextest run --manifest-path crates/limedl-core/Cargo.toml \
  --features "test-utils,aria2-rpc" -E 'test(/interop_/)'
```

What it pins down:

| Test | What regressing looks like |
| --- | --- |
| `interop_list_methods_matches_the_routed_surface` | `system.listMethods` drifts from the routed handlers, or a method is advertised but not wired |
| `interop_ariang_multicall_shape` | nested calls stop being individually authenticated, or the single-element `[value]` wrapper changes |
| `interop_ariang_add_torrent_shape_keeps_options` | AriaNg's `[torrent, [], options]` shape silently drops `dir`/`out`/`pause` again |
| `interop_add_torrent_legacy_shape_keeps_options` | the older `[torrent, options]` shape stops working |
| `interop_add_uri_shape_keeps_options` | `addUri([urls, options])` options are dropped |
| `interop_tell_status_http_field_schema` / `..._bt_field_schema` | aria2's always-present `tellStatus` keys or the piece map disappear |
| `interop_get_servers_and_change_uri` | `getServers` shape or `changeUri`'s `[deleted, added]` reply drifts |
| `interop_change_position_returns_the_real_index` | `changePosition` reports an index it did not apply |
| `interop_shutdown_handshake` | `forceShutdown`/`getVersion` stop answering |

The suite was introduced after `addTorrent` was found to read `params[1]` as the
options object. AriaNg sends `[torrent, [], options]`, so every option was
dropped while limedl's own tests (which sent `[torrent, options]`) stayed green.
Any future change to a request shape should add a fixture in a client's actual
shape, not a hand-written one.

## Tier 2 — a real `aria2c` oracle (nightly / Linux job, not the PR gate)

Tier 1 cannot catch "aria2 itself returns a different shape". Tier 2 runs the
real binary as an oracle. It is deliberately **not** part of the required gate:
it needs a third-party binary, is network/port sensitive, and its output drifts
with the aria2 release.

### Mode A — golden capture (recommended first step)

Capture aria2's deterministic responses once, commit them, and assert them in a
normal Rust test. Deterministic methods (no live download needed):

- `aria2.getVersion` (feature list / version format)
- `system.listMethods` (limedl must be a subset of aria2's list)
- `system.listNotifications`
- `aria2.getGlobalOption` (key set)
- `aria2.getSessionInfo`
- malformed-request / unknown-method error objects

Capture script (run manually, commit the JSON under
`crates/limedl-core/tests/fixtures/aria2/`):

```sh
aria2c --enable-rpc --rpc-listen-port=6800 --rpc-secret=oracle \
  --no-conf --quiet &
# wait for the port, then:
curl -s http://127.0.0.1:6800/jsonrpc -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"aria2.getVersion","params":["token:oracle"]}'
```

The test then asserts key sets and value **types** (aria2 serialises every
value as a string), not exact values, so a version bump does not break it.
Record the capturing aria2 version in the fixture file.

### Mode B — live dual-run diff (implemented, nightly)

`crates/limedl-core/src/aria2_rpc/oracle_tests.rs` starts a real
`aria2c --enable-rpc` beside limedl's server, sends both the same requests, and
reports how the responses differ. It is the executable form of the deviation
table below.

Run it locally:

```sh
ARIA2_ORACLE_BIN=$(command -v aria2c) cargo nextest run \
  --manifest-path crates/limedl-core/Cargo.toml \
  --features "test-utils,aria2-rpc" \
  -E 'test(/aria2_rpc::oracle_tests::/)' --success-output=final
```

When `ARIA2_ORACLE_BIN` is unset every oracle test returns early, so the normal
core gate never needs aria2. A set-but-unusable path panics, so the job cannot
pass by accident.

**Policy: allowlisted gaps only.** The oracle asserts the client-facing
contract: limedl must answer every AriaNg option key (`aria2Options.js`), the
always-present `tellStatus` keys, and `system.listMethods`/`listNotifications`
must differ from aria2 only by the documented entries. Anything outside those
allowlists fails the run; the remaining aria2-only differences are printed as
allowlisted notes by `--success-output=final`. The `ARIA2_ORACLE_BIN` opt-in is
unchanged: unset, every test returns early, so the normal gate needs no aria2
binary.

`Aria2Oracle` owns the operational contract: a free port, `--no-conf`,
`--enable-dht=false`, a `TempDir` for `--dir`, a `getVersion` readiness poll with
a deadline, and a `Drop` that kills the child.

Coverage (all offline, against `test_harness::TestServer`):

| Test | Compares |
| --- | --- |
| `oracle_list_methods_diff` | method set; today the only diff is `aria2.multicall` (extra alias) |
| `oracle_list_notifications_diff` | notification set (currently equal) |
| `oracle_get_version_shape` | `version`/`enabledFeatures` shapes; feature diff reported |
| `oracle_error_objects` | unknown method / missing params / bad GID return an error object |
| `oracle_tell_status_keys` | `tellStatus` key set and value types |
| `oracle_get_option_keys` / `oracle_get_global_option_keys` | option key sets (report-only) |
| `oracle_change_position_returns_integer` | the reply is an integer |
| `oracle_transport_gaps` | HTTP GET/JSONP and batch POST support |

CI: `.github/workflows/aria2-oracle.yml` runs on a nightly `schedule` and
`workflow_dispatch` (never on PRs) on ubuntu-latest. It reads the Linux
`ci-debug` rust-cache entry with `save-if: false`, so it reuses `check-rust`'s
artifacts instead of paying a cold build.

### Observed baseline (aria2 1.37.0)

Captured by the first oracle run; a change here shows up in the report diff:

- `system.listMethods`: aria2 has 36 methods; limedl implements all 36 (including
  `aria2.addMetalink`) and adds the `aria2.multicall` alias (37 total).
- `system.listNotifications`: identical (6).
- `getVersion`: `GZip` and `Metalink` are now shared.
  limedl additionally advertises `Brotli` / `Zstd` (non-standard strings) and
  correctly omits `SFTP` / `XML-RPC` / `Firefox3 Cookie`, which
  aria2 lists because it serves them and limedl does not.
- `tellStatus` (paused HTTP): limedl omits `numPieces` / `pieceLength` /
  `bitfield` until chunks are planned; aria2 always emits the first two.
- Error responses: aria2 returns `code: 1` for **every** failure (unauthorized,
  unknown method, missing params, bad GID); limedl now matches on the wire and
  only keeps `-32700`/`-32600` for wire-level parse/version failures.
  `oracle_error_objects` asserts the code is 1 on both servers.
- `getOption` / `getGlobalOption`: limedl answers AriaNg's complete global and
task key sets. The values are real where the engine has an equivalent and
documented aria2 defaults otherwise; the remaining aria2-only global keys are
the largest allowlisted difference (reported, not failing).
- Transports: aria2 accepts HTTP GET/JSONP and a top-level JSON-RPC batch;
  limedl rejects both (GET → 400 from the WebSocket-only route).

Keep the oracle out of the Windows/macOS required gates: Windows needs an extra
package manager step and those host gates are already the flakiest. A Linux-only
nightly workflow gives the signal with the least maintenance.

Per-platform fetch if a job ever needs it:

| OS | Install |
| --- | --- |
| Linux (Ubuntu/Debian) | `apt-get install -y aria2` |
| macOS | `brew install aria2` |
| Windows | `choco install aria2` or unpack the official win zip and put `aria2c.exe` on `PATH` |

### Oracle risks to design around

- **Version drift** — pin the archive version in Mode A's fixture metadata; in
  Mode B compare shapes, not values.
- **Port conflicts / startup race** — pick a free port, poll `/jsonrpc` until
  `getVersion` answers before driving it.
- **Downloads need a real origin** — point aria2 at the same in-process
  `test_harness::TestServer` limedl uses; never the public internet.
- **Timeouts** — aria2's scheduler differs; use per-method deadlines and
  compare a *paused* task's status, which is stable.

## Known, intentional deviations

These are the allowlist entries Tier 2 must honour. They are behavioural
choices, not bugs; each one is documented where it is implemented.

| Area | limedl behaviour | Why |
| --- | --- | --- |
| `changePosition` | maps the requested index onto the three priority levels and returns the **real** resulting index | limedl's queue is `(priority, created_at)`, not positional. Arbitrary drags among >3 same-priority tasks collapse. |
| BT `tellStatus.dir` / `getOption.dir` | report the BT backend's default output dir, not the per-task `dir` | the BT backend does not track a per-task output dir yet |
| `addTorrent` `out` / `select-file` | parsed but not applied at start for BT | use `aria2.changeOption` after metadata; `select-file` at start needs a BT-backend change |
| `errorCode` | `"0"` (no error) or `"1"` (failure) | limedl does not persist aria2 exit-status codes; the reason stays in `errorMessage` |
| `tellStatus` piece map | `numPieces` / `pieceLength` / `bitfield` appear once chunks are planned | aria2 always emits `numPieces`/`pieceLength`; limedl only has them after the range probe |
| `numStoppedTotal` | mirrors the current stopped count | no lifetime counter |
| `getVersion.enabledFeatures` | truthful: `Async DNS`, `BitTorrent`, `GZip`, `Brotli`, `Zstd`, `HTTPS`, `Message Digest`, `Metalink` | aria2's `SFTP`/`XML-RPC`/`Firefox3 Cookie` are not advertised because limedl does not implement them; `Brotli`/`Zstd` are truthful non-standard additions. Locked by `interop_get_version_is_truthful`. |
| HTTP response compression | gzip/brotli/zstd are decompressed only on the plain single-stream GET; probes and every `Range` request force `Accept-Encoding: identity` | transparently decompressing a `206` destroys byte offsets, `Content-Length` and the checksum. See `http::identity_encoding` and `tests/http_executor_tests/compression.rs`. |
| `getGlobalOption` / `changeGlobalOption` | answers AriaNg's full global key set; `dir`, `max-concurrent-downloads`, `split`, `max-tries`, `user-agent`, `max-overall-*-limit`, and the mappable `bt-*`/`enable-*` keys are real, the rest are documented aria2 defaults | engine settings model; aria2-only keys AriaNg never reads are omitted |
| `getOption` | answers AriaNg's full task key set; several fields stay fixed placeholders (`min-split-size`, `max-tries`, …) | not all surfaced by the engine |
| `aria2.shutdown` / `forceShutdown` | acknowledge + warn; exit the process when `exit_on_shutdown` is set (the headless daemon) | the desktop is a managed subsystem and the UI owns exit |
| `changeUri` for BT | unsupported | torrent sources come from trackers/DHT; HTTP `changeUri` is supported |
| HTTP GET / JSONP transports | not served (POST, including a top-level JSON-RPC batch, + WebSocket only) | deliberate scope; add if browser/userscript clients need it |
| HTTPS RPC, HTTP Basic auth | not served; use a reverse proxy for TLS. A configurable bind address (aria2's `--rpc-listen-all`) *is* supported, but a non-loopback bind requires authentication | security posture |
| GID prefix matching | a unique prefix resolves; an ambiguous one is refused | aria2 accepts abbreviated GIDs; refusing an ambiguous prefix is safer than guessing |

## Tier 3 — real client smoke test

For release candidates, drive the desktop app with AriaNg (or another real
dashboard) using the existing UI/MCP tooling in
[`manual-smoke-testing.md`](manual-smoke-testing.md). This is the only layer
that exercises the browser's CORS, WebSocket reconnect and actual client
parsing; keep it a manual checklist rather than CI.

## Where to add coverage

- New request shape seen from a client → a Tier 1 fixture in `interop_tests.rs`.
- New `tellStatus` field → add it to the schema test's key list.
- New/changed method → update `EXPECTED_METHODS` and the router; the contract
  test fails otherwise.
- New intentional deviation → add a row above and, once Tier 2 exists, an
  allowlist entry.
