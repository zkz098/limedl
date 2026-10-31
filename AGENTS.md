# AGENTS.md — limedl

## Environment

**Windows**: initialize MSVC before any Rust command:

```cmd
cmd.exe /k "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
```

## Toolchain

| Purpose         | Command                                                                                                                                                            |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Gate (all)      | `cargo xtask gate` · `cargo xtask gate --full` · `cargo xtask gate --check-ci` (see "Pre-commit verification gate")                                   |
| Test (Rust)     | `cargo nextest run -p limedl-types -p limedl-disk -p limedl-store -p limedl-http -p limedl-engine -p limedl-bt -p limedl-rpc -p limedl-testkit -p limedl-core --features "limedl-core/test-utils"` (engine), then `xtask`, `limedl-native`, `limedl-server` (see the gate below)                                                                |
| Feature matrix  | `cargo hack check --feature-powerset --all-targets --keep-going --manifest-path crates/limedl-core/Cargo.toml`                                                      |
| Unused deps     | `cargo machete --with-metadata`                                                                                                                                     |
| Version bump    | `cargo xtask bump-version patch`                                                                                                                                   |
| Release preview | `git-cliff --config cliff.toml --strip header vX.Y.Z..vA.B.C`                                                                                                      |
| Fetch UI font   | `cargo xtask fetch-font [--verify]` (one-time, required before building limedl-native; font is not in git due to MiSans license)                                  |
| Drive the UI    | `set "SLINT_EMIT_DEBUG_INFO=1" && set "SLINT_MCP_PORT=8080" && cargo run -p limedl-native --features slint/mcp` (see `docs/manual-smoke-testing.md`)              |
| Theme / tokens  | `cargo xtask theme generate` · `cargo xtask theme apply` · `cargo xtask theme check`                                                                               |
| Format          | `cargo fmt --all` · `cargo fmt --all -- --check` (CI pins rustfmt to 1.99.0; `newline_style = "Unix"` + `.gitattributes eol=lf`)                                  |
| Translations    | `cargo xtask i18n generate` · `cargo xtask i18n check` (see `docs/i18n.md`)                                                                                          |
| Generate icons  | `cargo xtask gen-icons [msix\|hicolor\|macos-iconset] --out-dir <dir>`                                                                                             |
| Sign / keys     | `cargo xtask sign <files>` · `cargo xtask guard <files>` (release gate) · `cargo xtask generate-key --out-dir <dir>` (see `docs/update-signing-key.md`)             |

## Architecture

```
limedl/
├── crates/limedl-types/  # Foundation layer: domain types, settings, errors, events, manifest, DownloadBackend
├── crates/limedl-disk/   # Disk/IO substrate: file_ops, buffer_pool, io_scheduler, disk services
├── crates/limedl-store/  # Persistence: SQLite task/chunk/manifest store, Alt-Svc cache
├── crates/limedl-http/   # HTTP policy: client factory, identity encoding, rate limiter
├── crates/limedl-engine/ # HTTP engine: chunk executor, scheduler, CDN, metalink, task lifecycle
├── crates/limedl-rpc/    # Aria2 JSON-RPC + WebSocket server (axum); sits *above* the engine
├── crates/limedl-testkit/# Test-only HTTP harness (deterministic content, fault injection)
├── crates/limedl-bt/     # BitTorrent backend (librqbit); the only crate that names the engine
├── crates/limedl-core/   # Engine + orchestration (lib: limedl_core): HTTP executor, scheduler, CDN, BT, Aria2 RPC
├── crates/limedl-native/ # Lightweight native desktop UI based on Slint
├── crates/limedl-server/ # Headless daemon: Aria2 JSON-RPC over the engine (NAS/server)
└── xtask/                # Repo tooling: minisign keygen/sign/guard, font fetch, version bump, theme, icons
```

The engine is split: `limedl-types`, `limedl-disk`, `limedl-store`,
`limedl-http`, `limedl-engine`, `limedl-bt`, `limedl-rpc` (above the facade —
`limedl-core` does not depend on it) and `limedl-testkit` (test-only).
`limedl-bt` is an *optional* dependency, so `--no-default-features` still builds
an engine with no BitTorrent protocol and no `librqbit` in the graph. `limedl-core` keeps a `pub use` shim per
module (`pub use limedl_types::{types, error, …}`) so every historical path —
including the ~12k lines of tests in `src/tests/` — still resolves; the remaining
`LAYERS` in `crates/limedl-core/src/tests/module_boundaries_tests.rs` remains as
the guard for the eight modules left in the facade: it fails on an upward edge,
on a module it cannot classify, and on an entry that no longer exists.

All Rust crates use edition 2024.

- **Events**: `EventBus` is a `tokio::sync::broadcast` channel; each adapter (Slint desktop, Aria2 RPC) subscribes independently.
- **Routing**: `BackendRegistry` routes by `TaskId` prefix (`http:` → `DownloadManager`, `bt:` → `RqbitBtBackend`); `Dispatcher` is the frontend-facing facade.
- **UI tests**: two layers in `crates/limedl-native` — in-process `src/ui_tests/` (part of the normal gate; `.slint` ids are the contract) and the MCP server (see `docs/manual-smoke-testing.md`).

## Releases

A `v*` tag triggers `.github/workflows/release.yml`. The release body is generated
by **git-cliff** from Conventional Commits, so keep commit subjects Conventional
with a meaningful `scope:`. `native-manifest` is the sole writer of
`latest-native.json` (the signed self-update manifest). `build-server` additionally
ships static musl binaries of `limedl-server` for `x86_64` and `aarch64` only, built
with `cargo zigbuild`; they are plain release assets and are not part of the update
manifest. `server-image` assembles those binaries into a multi-arch
`ghcr.io/zkz098/limedl-server` container image. The Linux desktop leg uses the same
`cargo zigbuild` with `--target x86_64-unknown-linux-gnu.2.17`, which keeps the
binary dynamically linked to the system libraries but lowers its glibc floor from
the `ubuntu-latest` host's 2.39 to 2.17; `scripts/check-glibc-floor.sh` fails the
release if a dependency raises it again.

## Conventions

- Rust structs: `#[serde(rename_all = "camelCase")]`. Enums: `#[serde(rename_all = "snake_case")]`.
- Native UI (Slint): use `Theme.c<hex>` tokens from `ui/theme.slint` (never hardcoded hex, except the literals `theme.rs` lists as identical in both schemes), `@tr(...)` for all user-visible strings in `.slint`, and `i18n::format_*` / `i18n::tr` helpers for Rust-side text. Both sides read the same `.po` catalogs; see `docs/i18n.md`.
- Settings: a field lives in `SettingsFormData` (`ui/types.slint`) and one row of the table in `src/bridge/forms/fields.rs`. The table's completeness test fails when `AppSettings` gains a leaf that no dialog field writes.
- Formatting: `cargo fmt --all` before committing — `check-rust` runs it as a gate, pinned to rustfmt 1.99.0 so a Rust release cannot reformat the tree unasked. `rustfmt.toml` is minimal on purpose (the options that would follow this repo's hand-laid tables are nightly-only and silently ignored on stable); where a table must stay hand-laid, use `#[rustfmt::skip]` with the reason next to it.
- Line endings: every text file is LF, in the repository **and** in the working tree (`.gitattributes`). On Windows that means the editor must keep LF (VS Code, RustRover and `rust-analyzer` default to it); a checkout made before the switch keeps CRLF on disk — refresh it once with `git rm --cached -r . && git reset --hard`.
- Build: `.cargo/config.toml` sets `target-cpu=x86-64-v3` and adds `--cfg reqwest_unstable` (required by reqwest's unstable `http3` feature) to every `[target.*]` rustflags — remember it when adding a new target.

## Documentation

Architecture and mechanism documentation lives in the loredex wiki
(`.wiki/pages/`); start at `.wiki/pages/quickstart.md`. Operational runbooks live in
`docs/` (index: `docs/README.md`); `docs/` runbooks are hand-maintained — update
them **after** changing the behavior they describe.

| Topic | Where |
| --- | --- |
| Engine architecture, protocol routing, event bus | `.wiki/pages/architecture/` |
| HTTP / BT / CDN workflows, scheduler & AIMD | `.wiki/pages/workflows/` |
| Persistence, settings, disk I/O, networking | `.wiki/pages/systems/` |
| Aria2 RPC, native UI, self-update | `.wiki/pages/integrations/`, `.wiki/pages/desktop/` |
| Testing (engine + Slint UI) | `.wiki/pages/testing/` |
| Build / CI / release operations, server deployment | `.wiki/pages/operations/`, `docs/ci-operations.md` |
| Self-update key rotation | `docs/update-signing-key.md` |
| Known issues & accepted warnings | `docs/troubleshooting.md` |
| Linux desktop build & packaging | `docs/desktop-build-and-packaging.md` |
| Manual smoke testing / MCP | `docs/manual-smoke-testing.md` |
| Headless server daemon (NAS / 软路由) | `docs/server-daemon.md` |
| Aria2 RPC interop testing | `docs/aria2-interop-testing.md` |
| Checksum algorithms, async file I/O | `docs/engine-dev-notes.md` |
| Translations (Slint `@tr` + Rust `i18n::tr`, the generated catalog) | `docs/i18n.md` |
| Regressions the test suite caught | `docs/test-regression-notes.md` |

## Pre-commit verification gate (MANDATORY)

Never commit while any check is red. CI runs every check on the whole workspace
and fails if **any** test fails, warning is emitted, or error is raised —
regardless of whether your own diff caused it. Fix all failures, warnings and
errors before committing, even pre-existing ones.

Run it with one command (Windows: init MSVC first, as above):

```powershell
cargo xtask gate            # ~4 min: fmt, clippy, doc, all test suites, theme check
cargo xtask gate --full     # + feature matrix, unused deps, license gate, RustSec audit
cargo xtask gate --check-ci # offline: does ci.yml still run every crate's tests?
cargo xtask gate --keep-going
```

`cargo xtask gate` (xtask/src/gate.rs) is the hand-written command list this
section used to carry, moved into code so it cannot drift again:

- **The package list comes from `cargo metadata`.** `TEST_GROUPS` must cover
  every workspace member and `ci.yml` must name each one on a `nextest` /
  `llvm-cov` line — `ci_parity` (run before every gate, and by
  `xtask::tests::ci_parity_accepts_the_real_workspace`) fails otherwise. This is
  not cosmetic: `--manifest-path crates/limedl-core/Cargo.toml` silently skipped
  269 tests the day `limedl-types` and `limedl-disk` were split out.
- **CI's environment is applied for you** (`CARGO_BUILD_WARNINGS=deny`, sparse
  registry protocol), and the `rustfmt` pin is checked: with `1.99.0` installed
  the gate formats through it exactly like `check-rust` does, otherwise it warns
  and uses the host toolchain.
- **`cargo doc` runs with `RUSTDOCFLAGS=-D warnings`.** Clippy compiles the tree
  but never resolves doc links or parses prose, so a doc comment pointing at a
  module that has since moved to another crate stays green forever — which is
  exactly what the crate split left behind (`crate::aria2_rpc` in `limedl-core`,
  `DownloadManager`/`TaskLifecycle` in `limedl-engine`). `--no-deps` keeps the
  step on this workspace and reuses clippy's artifacts.
- **It reports the command that failed**, not just a red step, so a failure can
  be re-run in isolation.
- **`--full` is the CI-parity profile.** `cargo deny check -W rejected bans
  licenses sources` and `cargo audit` are part of it — they live only in the
  `supply-chain` job, and a push has already gone red on four freshly published
  RustSec advisories that the old command list never ran.
- **CI does not call the gate.** `.github/workflows/ci.yml` is the authority;
  the gate mirrors it and `ci_parity` keeps the mirror honest.

What each step means:

- `cargo hack` walks every `limedl-core` feature combination —
  `--no-default-features` (BT engine absent) included — with
  `CARGO_BUILD_WARNINGS=deny` in force, so a warning in *any* combination fails.
- `cargo machete` runs with `--with-metadata`; its false positives are listed
  with reasons under `[package.metadata.cargo-machete]` in the crate that owns
  them.
- `cargo xtask theme check` is the token scanner: no new hardcoded hex in
  `ui/*.slint` components (alpha tints and scheme-identical colors are
  allowlisted in `xtask/src/theme.rs`).

See `docs/ci-operations.md` for the cache/RUSTFLAGS contract, the coverage gate
and the macOS linker exemption.

### The gate does not compile non-Windows code on a Windows host

On Windows, `cfg(not(windows))` items are never compiled, so
`cargo clippy --workspace --all-targets` cannot see an unused import/type/static
that only macOS and Linux reach, a `build.rs` dependency or feature only required
off Windows (`rfd`'s `xdg-portal` needs `tokio` or `async-std`), or a `-l<lib>`
whose `-dev` package is missing. This has already shipped two broken tagged
releases.

- **Prefer `#[cfg]` over runtime `CARGO_CFG_TARGET_OS` checks** in `build.rs` and
  `platform_*.rs`; a runtime branch still compiles the dead code on every host.
  To exercise the gate on Windows:
  `$env:CARGO_CFG_TARGET_OS = "macos"; cargo build -p limedl-native --target x86_64-pc-windows-msvc`
  (compile-only; it will not link the real target).
- **Treat a tagged release as the first real platform check** and follow CI to
  green before publishing, or push the tag only after `check-macos` / `check-rust`
  are green on that same commit.
- When adding a platform-gated module, re-read it asking "what does this file
  look like with `cfg(windows)` false?".

## Dependency discipline

After `cargo update` / `cargo add` / `cargo remove`, commit the changed
`Cargo.lock`. Uncommitted lockfile changes cause CI cache misses and stale
dependency resolution.

<!-- BEGIN loredex -->
## 仓库 Wiki(loredex)

本仓库在 `.wiki/` 下维护证据可回溯的 wiki(页面在 `.wiki/pages/*.md`;每条论断都带 `[[cite:path:Lx-Ly]]` 源码标记,并 pin 到 git 内容哈希)。

Agent 协议:

1. 先查 wiki 再读代码:`loredex search "<关键词>"` / `loredex show <页面>`(或 MCP `wiki_search` / `wiki_get_page`)。
2. 信任页面前先验证新鲜度:`loredex check`(或 MCP `wiki_check_freshness`);STALE 页面视为未验证,去读源码。
3. 修改代码后:`loredex auto` 自愈漂移引用,`loredex plan` 列出剩余写作任务。
4. 写页面时每条论断配一个行内 `[[cite:path:Lx-Ly]]`;无引用的正文会被拒绝。

请勿手改本区块;用 `loredex agents-md` 重新生成(`loredex agents-md --remove` 移除)。
<!-- END loredex -->
