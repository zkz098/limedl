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

### Mode B — live dual-run diff (strongest, heaviest)

Start `aria2c --enable-rpc` and limedl's server on two ports, send the same
request to both, and diff the responses after normalization:

- normalize volatile fields: `gid`, `downloadSpeed`, `uploadSpeed`,
  `completedLength`, `dir`, `sessionId`, `numStopped*`, timestamps;
- assert limedl's key set is a **superset** of aria2's for each state;
- allowlist the intentional deviations below.

CI placement (sketch, `.github/workflows/ci.yml`):

```yaml
  check-aria2-interop:
    name: Aria2 oracle diff (Linux)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@...
      - run: sudo apt-get update && sudo apt-get install -y aria2
      - run: aria2c --version
      - run: ARIA2_ORACLE_BIN=$(command -v aria2c) cargo nextest run
          --manifest-path crates/limedl-core/Cargo.toml
          --features "test-utils,aria2-rpc" -E 'test(/oracle/)'
```

Keep it out of the Windows/macOS required gates: Windows needs an extra package
manager step (`choco install aria2` / a release zip) and the host-side gate is
already the flakiest. A Linux-only nightly job gives the signal with the least
maintenance.

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
| `numStoppedTotal` | mirrors the current stopped count | no lifetime counter |
| `getVersion` | reports `0.1.0` and lists `XML-RPC` / `Firefox3 Cookie` | **stale**; limedl has no XML-RPC endpoint. To be corrected, and then asserted by Tier 1. |
| `getGlobalOption` / `changeGlobalOption` | a fixed subset (`dir`, `max-concurrent-downloads`, …); `max-overall-download-limit` is accepted but ignored | engine settings model |
| `getOption` | several fields are fixed placeholders (`min-split-size`, `max-tries`, …) | not surfaced by the engine |
| `aria2.shutdown` / `forceShutdown` | acknowledge + warn; do not exit | limedl is a managed subsystem; the UI owns exit |
| `addMetalink`, `changeUri` for BT | unsupported | Metalink is out of scope for now; torrent sources come from trackers/DHT |
| HTTP GET / JSONP / Batch transports | not served (POST + WebSocket only) | deliberate scope; add if browser/userscript clients need it |
| HTTPS RPC, Basic auth, `--rpc-listen-all` | not served (loopback + token only) | security posture |
| GID prefix matching | exact GID only | not needed by current clients |

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
