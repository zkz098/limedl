# AGENTS.md — limedl

## Environment

**Windows**: initialize MSVC before any Rust command:

```cmd
cmd.exe /k "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
```

## Toolchain

| Purpose         | Command                                                                                                                                                            |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Test (Rust)     | `cargo nextest run --manifest-path crates/limedl-<crate>/Cargo.toml` per crate (see the gate below)                                                                |
| Version bump    | `cargo xtask bump-version patch`                                                                                                                                   |
| Release preview | `git-cliff --config cliff.toml --strip header vX.Y.Z..vA.B.C`                                                                                                      |
| Fetch UI font   | `cargo xtask fetch-font [--verify]` (one-time, required before building limedl-native; font is not in git due to MiSans license)                                  |
| Drive the UI    | `set "SLINT_EMIT_DEBUG_INFO=1" && set "SLINT_MCP_PORT=8080" && cargo run -p limedl-native --features slint/mcp` (see `docs/manual-smoke-testing.md`)              |
| Theme / tokens  | `cargo xtask theme generate` · `cargo xtask theme apply` · `cargo xtask theme check`                                                                               |
| Generate icons  | `cargo xtask gen-icons [msix\|hicolor\|macos-iconset] --out-dir <dir>`                                                                                             |
| Sign / keys     | `cargo xtask sign <files>` · `cargo xtask guard <files>` (release gate) · `cargo xtask generate-key --out-dir <dir>` (see `docs/update-signing-key.md`)             |

## Architecture

```
limedl/
├── crates/limedl-core/   # Pure download engine (lib: limedl_core)
├── crates/limedl-native/ # Lightweight native desktop UI based on Slint
└── xtask/                # Repo tooling: minisign keygen/sign/guard, font fetch, version bump, theme, icons
```

All Rust crates use edition 2024.

- **Events**: `EventBus` is a `tokio::sync::broadcast` channel; each adapter (Slint desktop, Aria2 RPC) subscribes independently.
- **Routing**: `BackendRegistry` routes by `TaskId` prefix (`http:` → `DownloadManager`, `bt:` → `IrontideBtBackend`); `Dispatcher` is the frontend-facing facade.
- **UI tests**: two layers in `crates/limedl-native` — in-process `src/ui_tests/` (part of the normal gate; `.slint` ids are the contract) and the MCP server (see `docs/manual-smoke-testing.md`).

## Releases

A `v*` tag triggers `.github/workflows/release.yml`. The release body is generated
by **git-cliff** from Conventional Commits, so keep commit subjects Conventional
with a meaningful `scope:`. `native-manifest` is the sole writer of
`latest-native.json` (the signed self-update manifest).

## Conventions

- Rust structs: `#[serde(rename_all = "camelCase")]`. Enums: `#[serde(rename_all = "snake_case")]`.
- Native UI (Slint): use `Theme.c<hex>` tokens from `ui/theme.slint` (never hardcoded hex), `@tr(...)` for all user-visible strings in `.slint`, and `i18n::format_*` helpers for Rust-side text.
- Build: `.cargo/config.toml` sets `target-cpu=x86-64-v3` and adds `--cfg reqwest_unstable` (required by reqwest's unstable `http3` feature) to every `[target.*]` rustflags — remember it when adding a new target.

## Documentation

Architecture and mechanism documentation lives in the generated OpenWiki
(`openwiki/`); start at `openwiki/quickstart.md`. Operational runbooks live in
`docs/` (index: `docs/README.md`).

**After finishing a task, update this repository's OpenWiki for changes since its
last successful run** — start an OpenWiki `update` run (the `openwiki_begin`
update flow; CLI: `openwiki code --update`). OpenWiki is **not** refreshed by CI
in this repository, so the agent must run the update itself; the wiki would
otherwise drift from the code just changed.

- OpenWiki pages are **generated** — do not hand-edit them; update source code
  and re-run the update so they are regenerated.
- `docs/` runbooks are hand-maintained — update them **after** changing the
  behavior they describe.

| Topic | Where |
| --- | --- |
| Engine architecture, protocol routing, event bus | `openwiki/architecture/` |
| HTTP / BT / CDN workflows, scheduler & AIMD | `openwiki/workflows/` |
| Persistence, settings, disk I/O, networking | `openwiki/systems/` |
| Aria2 RPC, native UI, self-update | `openwiki/integrations/`, `openwiki/desktop/` |
| Testing (engine + Slint UI) | `openwiki/testing/` |
| Build / CI / release operations | `docs/ci-operations.md` |
| Self-update key rotation | `docs/update-signing-key.md` |
| Known issues & accepted warnings | `docs/troubleshooting.md` |
| Linux desktop build & packaging | `docs/desktop-build-and-packaging.md` |
| Manual smoke testing / MCP | `docs/manual-smoke-testing.md` |
| Aria2 RPC interop testing | `docs/aria2-interop-testing.md` |
| Checksum algorithms, async file I/O | `docs/engine-dev-notes.md` |
| Regressions the test suite caught | `docs/test-regression-notes.md` |

## Pre-commit verification gate (MANDATORY)

Never commit while any check is red. CI runs every check on the whole workspace
and fails if **any** test fails, warning is emitted, or error is raised —
regardless of whether your own diff caused it. Fix all failures, warnings and
errors before committing, even pre-existing ones.

Run the full gate locally (Windows: init MSVC first). The commands mirror
`.github/workflows/ci.yml` one for one, so a green local run means a green CI run:

```powershell
cmd.exe /k "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
$env:CARGO_BUILD_WARNINGS="deny"
$env:CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse"
cargo clippy --workspace --all-targets -- -D warnings

cargo nextest run --manifest-path crates/limedl-core/Cargo.toml --features "test-utils,aria2-rpc"
cargo nextest run --manifest-path xtask/Cargo.toml
cargo nextest run --manifest-path crates/limedl-native/Cargo.toml
```

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

<!-- OPENWIKI:START -->

## OpenWiki

This repository has a generated `openwiki/` evidence index. It is optional just-in-time context, not required startup reading.

- Do not enumerate, preload, or search wikis at task start. Use retrieval when the user asks for it, when unfamiliar architecture or dependency behavior materially affects the task, or when source inspection leaves an important uncertainty. Stop once the question is grounded.
- When those conditions apply and OpenWiki retrieval tools are available, use `openwiki_search` for just-in-time context and `openwiki_read` for the relevant complete sections. If search returns `workspace_required`, ask which listed workspace to use and retry with its ID.
- Use `openwiki_list_workspaces` or `openwiki_list_wikis` when workspace membership itself needs to be discovered.
- If the retrieval tools are unavailable, read `openwiki/quickstart.md` and follow its links to the relevant pages.
- Treat source code and tests as authoritative. A brief's unknowns and review items are verification gaps, not automatic requirements.
- Prefer the narrowest quiet validation that proves the changed behavior. Preserve complete failure output.

The scheduled OpenWiki GitHub Actions workflow refreshes the repository wiki. Do not hand-edit generated OpenWiki pages unless explicitly asked; prefer updating source code/docs and letting OpenWiki regenerate.

<!-- OPENWIKI:END -->
