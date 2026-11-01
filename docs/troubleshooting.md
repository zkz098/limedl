# Troubleshooting

Known issues and accepted warnings. New issues with a non-zero exit code are
still real problems. Mechanism documentation lives in the wiki: durable progress
and schema migrations in
[`.wiki/pages/systems/persistence-and-recovery.md`](../.wiki/pages/systems/persistence-and-recovery.md),
startup hardening in
[`.wiki/pages/desktop/native-ui-architecture.md`](../.wiki/pages/desktop/native-ui-architecture.md),
and the Aria2 default in
[`.wiki/pages/integrations/aria2-rpc-server.md`](../.wiki/pages/integrations/aria2-rpc-server.md).

## Mirror anti-abuse `403` (e.g. Tsinghua TUNA)

**Symptom**: the browser can download the same URL but limedl returns
`403 Forbidden`, with
`http status 403 Forbidden (server anti-abuse check rejected this client; update the default User-Agent in Settings or try another mirror)`.

**Cause**: the TUNA edge (`mirrors.tuna.tsinghua.edu.cn`) classifies clients by
UA: a UA that claims to be a browser but whose version is outdated or not yet
released is treated as "uncommon software" (a disguised browser) and rejected.
Measured 2026-09, with Chrome 154 stable at the time:

| UA | Result |
| --- | --- |
| `Chrome/124` (old built-in default) | GET 403; HEAD 200 (HEAD is exempt, so probing looked fine) |
| `Chrome/137`–`Chrome/141` | 206 OK |
| `Chrome/143`, `Chrome/145` | 403 (unreleased versions are rejected too) |
| `curl/8.7.1`, `aria2/1.37.0`, `limedl/0.3.13` | 206 (tool UAs take another path) |
| IPv4 egress | all UAs 403 when the whole subnet is flagged; IPv6 works |

**Handling**:

1. Bump the built-in UA: change
   `crates/limedl-core/src/types/settings.rs::default_http_user_agent()` to the
   current stable Chrome major, and sync the UI copy and i18n (see
   [`.wiki/pages/systems/networking-and-rate-control.md`](../.wiki/pages/systems/networking-and-rate-control.md)).
2. Existing `settings.json` does **not** follow automatically (the user's custom
   value is preserved); clear the Default User-Agent in Settings or type a new
   browser UA.
3. If it still 403s after the UA update, compare `curl -4` / `curl -6`: when the
   IPv4 subnet is flagged by TUNA you need a different network or IPv6.
4. This is server policy, not a limedl Referer problem; the code already detects
   anti-abuse pages and skips pointless Referer probing (see
   [`.wiki/pages/workflows/http-download-lifecycle.md`](../.wiki/pages/workflows/http-download-lifecycle.md)).

## CDN service tests reach the real network (known flake)

`cdn::tests::service_tests::{monitor_test_emits_progress_and_complete_ready,
monitor_test_emits_complete_on_error}` are the only engine tests that talk to the
public internet: `CdnService::start_test` runs the real accelerator, which fetches
Cloudflare's IP ranges (10 s cap) and then probes candidate IPs and the default
node with the production HTTP client. There is no injection point for the IP-range
provider or the speed-test strategy (see the comment block in
`crates/limedl-engine/src/retry/tests.rs`), so the tests cannot be made
hermetic without adding one.

Two consequences worth knowing before blaming a code change:

- Both carry `#[timeout(30_000)]`, which is now exactly the client's `read_timeout`
  (30 s). A probe that connects and then stalls is only released by that timeout,
  so under parallel load the ntest watchdog can fire first and report an opaque
  "the function call took 30000 ms" instead of the test's own assertion. Observed
  once locally (2026-10-05, WSL2, full-suite run) where a re-run passed; CI has
  been green.
- The tests themselves tolerate either outcome of the accelerator run (`Ready` or
  `Error: …`), so a slow or unreachable Cloudflare API makes them slower, not
  wrong — unless the watchdog pre-empts them.

The durable fix is to give the accelerator injectable range/speed-test strategies
(and run these two tests against a local fixture server). Until then, treat a
failure here as environment-sensitive and re-run before investigating.

## A download keeps the machine awake (or sleep is not inhibited)

`settings.json` → `power.preventSleep` (default on, Settings → Appearance) holds
an OS keep-awake lock **only while a task is running**: the last task to finish
releases it, and the display is free to turn off throughout. It never covers the
laptop lid or a manual suspend.

If the machine will not sleep after the queue drains, the lock is still held —
check with `systemd-inhibit --list` (Linux, `WHO` is `limedl`),
`pmset -g assertions` (macOS, `limedl: active downloads`) or `powercfg /requests`
(Windows, as administrator). On Windows the flag is per-thread, so an
`ES_SYSTEM_REQUIRED` requirement that outlives its release means the lock was
taken and released by different threads; `power.rs` logs an error in that case.

In the other direction, nothing is logged as broken when sleep is simply *not*
inhibited on Linux:

```
系统电源管理: 申请防休眠锁失败，下载将继续但系统可能休眠: logind Inhibit:
org.freedesktop.DBus.Error.AccessDenied: Permission denied
```

That is logind refusing the `Inhibit` lock (polkit's `inhibit-block-sleep` /
`inhibit-block-idle` needs an *active* local session), not a limedl bug —
`systemd-inhibit --what=idle:sleep --why=test sleep 1` fails the same way in the
same session (reproduced under WSL2, where the session is not active). It is
logged once per download session, and the download continues regardless. There is
also no fallback for a system without logind: such a host keeps its own power
policy.

## Accepted warnings (do not need fixing)

These used to be documented here and all came from the retired Tauri shell
(`src-tauri/`), which has been removed:

- **`LNK4078` "multiple `.rsrc` sections"** — came from `src-tauri/build.rs`
  embedding a ComCtl32 v6 manifest while `tauri_build::build()` embedded one
  through `tauri-winres`. `crates/limedl-native/build.rs` embeds exactly one
  resource object — `limedl-native.rc` (icon, VERSIONINFO), compiled by
  `embed-resource` — so the duplicate section cannot occur. There is no custom
  manifest code to preserve.
- **`quick-xml` RUSTSEC-2026-0194 / RUSTSEC-2026-0195** — accepted advisories for
  `quick-xml 0.39.x`, pinned by `wayland-scanner 0.31.10` in the Slint/winit Linux
  dependency tree. The tree now resolves `wayland-scanner 0.31.11` →
  `quick-xml 0.41.0`, which is patched (`patched = [">= 0.41.0"]`), so `deny.toml`
  no longer ignores them and CI runs a plain `cargo audit`.
- **`winreg` `multiple-versions` warning** — previously came from `auto-launch` and
  `embed-resource`. `winreg` has been completely replaced by the official
  `windows-registry` crate in `crates/limedl-native`, eliminating the dependency
  entirely. Note that `winreg 0.56` is back as `embed-resource`'s own
  build-dependency (Windows build scripts only, never linked into the app), a
  single version — so the warning stays gone; `cargo deny` in the `supply-chain`
  job is what proves it.
