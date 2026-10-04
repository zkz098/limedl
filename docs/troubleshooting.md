# Troubleshooting

Known issues and accepted warnings. New issues with a non-zero exit code are
still real problems.

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
   [`openwiki/systems/networking-and-rate-control.md`](../openwiki/systems/networking-and-rate-control.md)).
2. Existing `settings.json` does **not** follow automatically (the user's custom
   value is preserved); clear the Default User-Agent in Settings or type a new
   browser UA.
3. If it still 403s after the UA update, compare `curl -4` / `curl -6`: when the
   IPv4 subnet is flagged by TUNA you need a different network or IPv6.
4. This is server policy, not a limedl Referer problem; the code already detects
   anti-abuse pages and skips pointless Referer probing (see
   [`openwiki/workflows/http-download-lifecycle.md`](../openwiki/workflows/http-download-lifecycle.md)).

## Accepted warnings (do not need fixing)

These used to be documented here and all came from the retired Tauri shell
(`src-tauri/`), which has been removed:

- **`LNK4078` "multiple `.rsrc` sections"** — came from `src-tauri/build.rs`
  embedding a ComCtl32 v6 manifest while `tauri_build::build()` embedded one
  through `tauri-winres`. `crates/limedl-native/build.rs` only calls `winres` for
  icon/version metadata, so the duplicate section cannot occur. There is no custom
  manifest code to preserve.
- **`quick-xml` RUSTSEC-2026-0194 / RUSTSEC-2026-0195** — accepted advisories for
  `quick-xml 0.39.x`, pinned by `wayland-scanner 0.31.10` in the Slint/winit Linux
  dependency tree. The tree now resolves `wayland-scanner 0.31.11` →
  `quick-xml 0.41.0`, which is patched (`patched = [">= 0.41.0"]`), so `deny.toml`
  no longer ignores them and CI runs a plain `cargo audit`.
- **`winreg` `multiple-versions` warning** — came from `auto-launch` (via
  `tauri-plugin-autostart`) and `embed-resource` (via `tauri-winres`). `winreg`
  now appears exactly once (`0.52.0`, used by
  `crates/limedl-native/src/autostart.rs`), so `cargo deny` reports no duplicate.
