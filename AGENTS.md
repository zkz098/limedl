# AGENTS.md — limedl

> Compact instruction file for OpenCode sessions. What an agent would miss from source alone.

## Environment

**Windows**: initialize MSVC before any Rust command:

```cmd
cmd.exe /k "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
```

## Toolchain

| Purpose         | Command                                                                                                                                                                |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Test (Rust)     | `cargo nextest run --manifest-path crates/limedl-<crate>/Cargo.toml` per crate (see the gate below)                                                                    |
| Version bump    | `pwsh scripts/bump-version.ps1 patch`                                                                                                                                  |
| Release preview | `git-cliff --config cliff.toml --strip header vX.Y.Z..vA.B.C`                                                                                                          |
| Fetch UI font   | `pwsh scripts/fetch-misans.ps1` (one-time, required before building limedl-native; font is not in git due to MiSans license)                                           |
| Sign / keys     | `cargo xtask sign <files>` · `cargo xtask guard <files>` (release gate) · `cargo xtask generate-key --out-dir <dir>` (see `.opencode/guides/subsystem-self-update.md`) |

## Releases

Pushing a `v*` tag triggers `.github/workflows/release.yml`. A `changelog` job generates
the GitHub release body **automatically** from Conventional Commits between the previous
tag and the released tag using **git-cliff** (`cliff.toml`), then injects it into the release
via `softprops/action-gh-release`. Commit types `test:`/`ci:`/`chore:`/`build:`/`style:` are omitted
from the notes; `feat:`/`fix:`/`perf:`/`refactor:`/`docs:` are grouped into sections. Keep commit
subjects Conventional (with meaningful `scope:`) so release notes stay readable.

Two categories of jobs upload artifacts:

| Job                                   | Artifacts                                                                                                                                                                                                                                                 |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `build-native` (Windows)              | **Desktop**: `limedl-native-v{V}-windows-x86_64-{setup.exe,portable.zip,msix}`                                                                                                                                                                             |
| `build-native-macos` (Apple silicon)  | **Desktop**: `limedl-native-v{V}-darwin-aarch64-portable.tar.gz` (ad-hoc-signed, un-notarized `limedl.app`)                                                                                                                                                 |
| `build-native-linux` (x86_64, glibc)  | **Desktop**: `limedl-native-v{V}-linux-x86_64-portable.tar.gz`                                                                                                                                                                                              |
| `native-manifest`                     | minisign signatures for every desktop artifact + `latest-native.json` (self-update manifest) + the signed MSIX. Sole writer of the manifest — see below                                                                                                     |

The desktop manifest is built by `native-manifest`, **not** by the platform jobs: it is a
single file whose `platforms` map must carry every platform, so if each job generated it
the last one to finish would drop the other platform's entries (and desynchronize the file
from its `.sig`). The job runs with `if: always()` and only advertises platforms whose legs
succeeded — a missing key is reported by the client as "no update" rather than an error.

Desktop releases are the Slint client (`limedl-native`) only: Windows, macOS (Apple silicon)
and Linux x86_64 desktop users get the Slint client.
macOS builds are ad-hoc signed and **not notarized** (no Apple Developer account in CI),
so a browser-downloaded copy needs right-click → Open once. The Linux build targets
`x86_64-unknown-linux-gnu`, so it needs glibc >= 2.39 (Ubuntu 24.04 / its derivatives).
Existing legacy installs migrate their data on first run of the Slint client
(`crates/limedl-native/src/migrate.rs`).

## Architecture

### Workspace

```
limedl/
├── crates/limedl-core/   # Pure download engine (lib: limedl_core)
├── crates/limedl-native/ # Lightweight native desktop UI based on Slint
└── xtask/                # Repo tooling: minisign keygen/sign/guard for the update channel
```

All Rust crates use edition 2024.

### Multi-platform

| Target                | Frontend            | Backend                 | Build                                                                                                          |
| --------------------- | ------------------- | ----------------------- | -------------------------------------------------------------------------------------------------------------- |
| Native Desktop (Win)  | Slint (Rust)        | `crates/limedl-native/` | `cargo run -p limedl-native` (needs `pwsh scripts/fetch-misans.ps1` once)                                      |
| Native Desktop (mac)  | Slint (Rust)        | `crates/limedl-native/` | `cargo run -p limedl-native`; release bundle via `bash scripts/package-macos.sh` (macOS host required)         |
| Native Desktop (Linux)| Slint (Rust)        | `crates/limedl-native/` | `cargo run -p limedl-native` (needs `libgtk-3-dev` for the tray); release tarball via `bash scripts/package-linux.sh` |

### Event system

`EventBus` = `tokio::sync::broadcast::channel<DownloadEvent>`. Each adapter subscribes independently:

- Desktop (Slint): `crates/limedl-native/src/main.rs` subscriber → UI state updates
- Aria2 RPC: direct `event_bus.subscribe()`

### Protocol routing

`DownloadBackend` trait (unified API) → `BackendRegistry` routes by TaskId prefix:

- `http:` → `DownloadManager`
- `bt:` → `IrontideBtBackend`

`Dispatcher` routes operations to `BackendRegistry`.

## Conventions

- Rust structs: `#[serde(rename_all = "camelCase")]`. Enums: `#[serde(rename_all = "snake_case")]`.
- Native UI (Slint): use `Theme.c<hex>` tokens from `ui/theme.slint` (never hardcoded hex), `@tr(...)` for all user-visible strings in `.slint`, and `i18n::format_*` helpers for Rust-side text. See `.opencode/guides/subsystem-native-ui.md`.
- Build: `.cargo/config.toml` sets `target-cpu=x86-64-v3`.

## Guides

Read the relevant guide **before** modifying any subsystem. Update it **after**.

| Core guides                                 | Rust subsystem guides                                                        |
| ------------------------------------------- | ---------------------------------------------------------------------------- |
| `.opencode/guides/architecture-overview.md` | `subsystem-download-manager.md` (HTTP + checksum + rate limiter + data flow) |
| `.opencode/guides/troubleshooting.md`       | `subsystem-bt-backend.md`                                                    |
| `.opencode/guides/testing-guide.md`         | `subsystem-cdn-accelerator.md`                                               |
|                                             | `subsystem-aria2-rpc.md`                                                     |
|                                             | `subsystem-database.md`                                                      |
|                                             | `subsystem-buffer-pool.md` (includes file_ops)                               |
|                                             | `subsystem-settings.md`                                                      |
|                                             | `subsystem-event-bus.md`                                                     |
|                                             | `subsystem-protocol-registry.md`                                             |
|                                             | `subsystem-http-client-factory.md`                                           |
|                                             | `subsystem-self-update.md` (native updater)                                  |
|                                             | `subsystem-native-ui.md` (Slint desktop client)                              |

## Pre-commit verification gate (MANDATORY)

Never commit while any check is red. CI runs every check on the whole workspace
and fails if **any** test fails, warning is emitted, or error is raised —
regardless of whether your own diff caused it.

Therefore: **fix all failures, warnings, and errors before committing, even if
they pre-date your change or were not introduced by you.** Leaving a broken test
or warning "for later" blocks the entire pipeline and hides real regressions.

Run the full gate locally (Windows: init MSVC first). The Rust commands mirror
`.github/workflows/ci.yml` **one for one** — same crate list, same features, same
nextest version — so a green local run means a green CI run:

```powershell
# Rust — clippy/build/test under -D warnings
cmd.exe /k "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
$env:RUSTFLAGS="-D warnings"
$env:CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse"
cargo clippy --workspace --all-targets

# Tests run under nextest, not `cargo test`: each test gets its own process, so
# the suite runs in parallel and cross-test global-state interference (shared
# temp dirs, env vars, LazyLock) surfaces locally instead of in CI. Pin the same
# version CI installs. One-time:
#   cargo install cargo-nextest --locked --version 0.9.144
# Per crate, NOT `--workspace`: only limedl-core's tests are meaningful without
# the `test-utils,aria2-rpc` features, and a workspace-wide run would both lose
# them and link the Skia UI binary just to check its flags.
cargo nextest run --manifest-path crates/limedl-core/Cargo.toml --features "test-utils,aria2-rpc"
cargo nextest run --manifest-path crates/limedl-native/Cargo.toml
```

`cargo nextest run` does not execute doctests. The workspace has none today; if
one is ever added, add a `cargo test --doc` step to the gate and to CI's
`check-rust` job together.

### The gate does not compile non-Windows code on a Windows host

This is the gate's biggest blind spot and it has already shipped two broken
tagged releases. On Windows, `cfg(not(windows))` items are never compiled, so
`cargo clippy --workspace --all-targets` cannot see:

- an unused import / type alias / static / const that only macOS and Linux
  reach (`-D warnings` turns each into a red CI job);
- a `build.rs` that compiles a dependency the crate only declares for Windows
  (`winres`), or a dependency feature that is required but not enabled
  (`rfd`'s `xdg-portal` needs `tokio` or `async-std`, checked by `rfd`'s own
  build script);
- a `-l<lib>` that the runner has no `-dev` package for.

Cross-checking locally is not possible without a cross toolchain (`ring`/`cc`
and the GTK `-sys` crates need a Linux compiler), so:

1. **Prefer `#[cfg]` over runtime checks in `build.rs` and `platform_*.rs`.**
   `if std::env::var("CARGO_CFG_TARGET_OS") == Ok("windows")` looks equivalent
   to `#[cfg(windows)]` but is evaluated at run time — the dead code is still
   compiled on every host. When a runtime decision really is needed, the gate
   can be exercised on Windows with
   `$env:CARGO_CFG_TARGET_OS = "macos"; cargo build -p limedl-native --target x86_64-pc-windows-msvc`
   (compile-only; it will not link the real target).
2. **Treat a tagged release as the first real platform check** and follow CI to
   green before publishing, or push the tag only after `check-macos` /
   `check-rust` are green on that same commit.
3. When adding a platform-gated module, re-read it asking "what does this file
   look like with `cfg(windows)` false?" — the file header comment convention in
   `platform_win.rs` (which exports are shared, which are Windows-only) exists
   for exactly this review.

Only commit once every check above is green. If a failure is environmental
(e.g. a Linux-only script on Windows), fix the code so it is platform-neutral or
otherwise reruns green in CI rather than committing around it.

## Dependency discipline

After `cargo update`/`cargo add`/`cargo remove`, commit changed lockfiles:

```powershell
git diff --stat Cargo.lock
git add Cargo.lock
```

Uncommitted lockfile changes cause CI cache misses and stale dependency resolution.
