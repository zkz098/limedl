---
type: operations
title: Build, Tooling, CI and Release Operations
description: The operational surface of limedl — build prerequisites and rustflags, the xtask tooling, the mandatory pre-commit gate and its Windows blind spot, the CI job graph with nextest/coverage/sonar/supply-chain, and the tag-driven release pipeline including the static musl server artifacts.
tags: [operations, build, ci, release, xtask, tooling]
sources:
  - id: openwiki-source-4905fab56ecf9fa5e1ebbf3f
    resource: repo://.cargo/config.toml
  - id: openwiki-source-ca5b77738a5ec463872c3294
    resource: repo://.github/actions/fetch-misans/action.yml
  - id: openwiki-source-06de9eea8068258882d65c0b
    resource: repo://.github/workflows/aria2-oracle.yml
  - id: openwiki-source-164e2da859b5277df81c7d94
    resource: repo://.github/workflows/ci.yml
  - id: openwiki-source-4d1d392666be6dfdd7a91a2e
    resource: repo://.github/workflows/release.yml
  - id: openwiki-source-baaa372232bcec4ef0e42d4a
    resource: repo://.github/workflows/sign-check.yml
  - id: openwiki-source-8037e2358a2c4f9b2c722a11
    resource: repo://AGENTS.md
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-cb3b278da9fc4917fdb881e9
    resource: repo://crates/limedl-core/src/aria2_rpc/oracle_tests.rs
  - id: openwiki-source-6824d268ea5edfcd81cb1a8b
    resource: repo://scripts/check-glibc-floor.sh
  - id: openwiki-source-feafbe9db788653e845840b8
    resource: repo://sonar-project.properties
  - id: openwiki-source-44d192e16032f18d847f0ff6
    resource: repo://xtask/src/bump_version.rs
  - id: openwiki-source-ca864fd40fa4107ed35f840f
    resource: repo://xtask/src/fetch_font.rs
  - id: openwiki-source-3e467e67d349677035f0363f
    resource: repo://xtask/src/main.rs
generated: { by: "pi", at: "2026-10-07T03:53:23.435Z" }
verified:
  - by: openwiki/0.7.1
    at: 2026-10-07T03:53:23.435Z
---

# Build, Tooling, CI and Release Operations

## Build prerequisites

| Platform | Requirement |
| --- | --- |
| Windows | Initialize MSVC before any Rust command: `vcvarsall.bat x64` |
| All | `cargo xtask fetch-font` once (the MiSans VF font is embedded at compile time) |
| Linux | `libfontconfig1-dev` (Slint's font stack probes it via pkg-config) |
| All | `cargo-nextest` pinned to the version CI installs |

`.cargo/config.toml` is the single source of truth for target flags: every
`[target.*]` sets `target-cpu=x86-64-v3` for desktop targets (x86-64-v2 for the
NAS musl targets) and adds `--cfg reqwest_unstable`, which reqwest's HTTP/3
feature requires at compile time. Add both when introducing a new target. The
Linux desktop release reuses `[target.x86_64-unknown-linux-gnu]` through
`cargo zigbuild --target x86_64-unknown-linux-gnu.2.17`, so the suffixed
cargo-zigbuild target needs no separate entry.

Evidence: `repo://AGENTS.md#L3-L21`, `repo://.cargo/config.toml#L1-L96`.

## xtask tooling

`cargo xtask` is the repository's tooling entry point:

| Command | Purpose |
| --- | --- |
| `generate-key --out-dir <dir>` | Create a Minisign keypair and an ML-DSA-65 keypair, print the `PUBKEY_B64`/`PQC_PUBKEY_B64` values + `gh secret set` commands |
| `sign <files...>` | Write minisign `<file>.sig` and (when `LIMEDL_PQC_SIGNING_KEY` is set) ML-DSA-65 `<file>.pqc.sig` next to each file |
| `verify <files...>` | Verify `.sig` and `.pqc.sig` against a public key |
| `guard <files...>` | Release guard: both signing secrets must match the client's `PUBKEY_B64`/`PQC_PUBKEY_B64` and every signature must verify |
| `manifest --version ...` | Generate the self-update `latest-native.json` |
| `fetch-font [--verify]` | Fetch/verify the pinned MiSans VF font |
| `bump-version <patch\|minor\|major>` | Bump version, update Cargo.lock + website, commit, tag and push |
| `theme generate\|apply\|check` | Generate `theme.slint` from the token tables, apply tokens, or check for unmapped colors |
| `gen-icons msix\|hicolor\|macos-iconset` | Generate desktop packaging icons |

Evidence: `repo://xtask/src/main.rs#L89-L193`.

`bump-version` updates the workspace `Cargo.toml` (first `version = "x.y.z"`),
every `limedl*` workspace package in `Cargo.lock` and the website, then creates a
`chore: bump version to X` commit and pushes commit + tag. `--dry-run` prints the
plan and `--no-push` skips git.

The lock rewrite matches the whole `limedl*` class instead of a hard-coded list of
crate names, and its result is re-checked: if any `limedl*` lock entry still lags,
the bump fails *before* committing or tagging. That guard exists because v0.4.7 was
tagged with `limedl-server` still at `0.4.6` in the lock — the release legs run
`cargo zigbuild --locked`, so both musl server jobs and the GHCR image failed 25 s
in with "cannot update the lock file". A workspace crate the rewrite cannot express
now fails loudly here instead of silently at tag time. `xtask` is unaffected: it
pins its own literal `0.0.0`.

Evidence: `repo://xtask/src/bump_version.rs#L57-L151`.

## The font artifact flow

`MiSansVF.ttf` is embedded into the `limedl-native` binary at compile time and
**cannot be committed** — the MiSans license forbids redistributing the file,
only works that embed it. `cargo xtask fetch-font` downloads it from Xiaomi's
CDN, pinned by size (`20_093_424`) and SHA-256
(`0ddef906…a115e79`), with a transport ladder (`--http2` → `--http1.1` →
`--http1.1 --ipv4`), chunked range fetch and escape hatches
(`LIMEDL_MISANS_TTF`, `--from-path`, `--zip-url`, `--verify`).

In CI the `font` job fetches it once and publishes the `misans-vf` artifact;
every job that compiles the UI crate restores that artifact and runs
`fetch-font --verify` instead of hitting the CDN. This exists because a cold
cache previously meant four runners downloading from Xiaomi simultaneously, so a
CDN hiccup read as four unrelated failures. The headless server needs no font —
its release job deliberately skips the artifact.

Evidence: `repo://xtask/src/fetch_font.rs#L44-L57`,
`repo://.github/actions/fetch-misans/action.yml#L1-L46`,
`repo://.github/workflows/ci.yml#L108-L147`.

## The mandatory pre-commit gate

CI fails on any test failure, warning or error regardless of whether a given
diff caused it, so the same gate must be green locally before committing:

```powershell
$env:CARGO_BUILD_WARNINGS="deny"
$env:CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse"
cargo clippy --workspace --all-targets -- -D warnings

cargo nextest run --manifest-path crates/limedl-core/Cargo.toml --features "test-utils,aria2-rpc"
cargo nextest run --manifest-path xtask/Cargo.toml
cargo nextest run --manifest-path crates/limedl-native/Cargo.toml
cargo nextest run --manifest-path crates/limedl-server/Cargo.toml
```

Notes that matter:

- Warnings are fatal through `CARGO_BUILD_WARNINGS=deny` (cargo's own
  `build.warnings`, orthogonal to RUSTFLAGS) plus clippy's `-- -D warnings`.
  Deliberately **not** `RUSTFLAGS="-D warnings"`, which would override
  `.cargo/config.toml`'s per-target flags.
- Tests run per crate, not `--workspace`: `limedl-core`'s tests need the
  `test-utils,aria2-rpc` features, `xtask` is in the gate because it owns the
  release-guard tests, and `limedl-server`'s suite carries the daemon's
  end-to-end test.
- `cargo nextest run` does not execute doctests; the workspace has none, and
  adding one requires a matching `cargo test --doc` step in the gate and CI.

Coverage is a hard gate in CI: `cargo llvm-cov nextest` on `limedl-core` with
`--fail-under-lines 85`. The tool is pinned to `cargo-llvm-cov@0.9.0` because the
0.6 → 0.9 change to instrumentation changed the denominator; re-baselining needs a
clean `cargo llvm-cov clean` with the pinned tool. `cargo-llvm-cov` ignores
`tests/` directories and `tests.rs`/`*_tests.rs` files, so the metric is product
code. A second lcov is generated for `limedl-native` with
`--ignore-filename-regex 'crates.limedl-core'` and `--no-cfg-coverage` (the latter
because `tiny-xlib` turns `cfg(coverage)` into a nightly-only feature under
stable). `limedl-server` has no lcov report and is excluded from the Sonar
coverage metric instead of being counted as 0%.

Evidence: `repo://AGENTS.md#L99-L112`,
`repo://.github/workflows/ci.yml#L451-L472`,
`repo://sonar-project.properties#L60-L61`.

### The gate cannot see non-Windows code from Windows

On a Windows host, `cfg(not(windows))` items are never compiled, so
`cargo clippy --workspace --all-targets` misses:

- unused imports/types/statics that only macOS and Linux reach;
- a `build.rs` that compiles a Windows-only dependency, or a feature that must
  be enabled (`rfd`'s `xdg-portal` needs `tokio` or `async-std`);
- a `-l<lib>` with no `-dev` package on the runner.

Mitigations: prefer `#[cfg]` over runtime `CARGO_CFG_TARGET_OS` checks so dead
code is not compiled everywhere; treat the first tagged release as the real
platform check; re-read platform-gated modules asking "what does this look like
with `cfg(windows)` false?". The `sign-check` workflow and macOS-only linker note
exemption are covered in the release page.

Evidence: `repo://AGENTS.md#L113-L131`.

## CI job graph

`.github/workflows/ci.yml` runs these jobs:

| Job | Platform | What it runs |
| --- | --- | --- |
| `font` | Linux | Fetch MiSans VF once, upload `misans-vf` |
| `check-windows` | Windows | `cargo clippy --workspace --all-targets -- -D warnings` |
| `test-windows-core` | Windows | nextest for `limedl-core` (with features) and `xtask` |
| `test-windows-native` | Windows | nextest for `limedl-native` |
| `check-macos` | macOS | clippy → core nextest → native nextest |
| `check-rust` | Linux | coverage (core + native lcov) → clippy → SonarCloud → native nextest → `limedl-server` nextest |
| `supply-chain` | Linux | `cargo deny check -W rejected bans licenses sources` + `cargo audit` |

`limedl-server` is exercised **only** on the Linux leg. Its end-to-end test binds
a real port and bootstraps the engine, so running it on the Windows/macOS legs
would add flake for a daemon that targets musl Linux; those legs still *compile*
it (including the `cfg(not(unix))` Ctrl+C path) through `cargo clippy
--workspace --all-targets`.

A separate `.github/workflows/aria2-oracle.yml` runs the Tier 2 aria2 oracle
(`aria2_rpc/oracle_tests.rs`) on a nightly `schedule` and on `workflow_dispatch` —
never on push or pull request. It is a standalone workflow rather than a `ci.yml`
job on purpose: a `schedule` on `ci.yml` would run the whole Windows/macOS/Linux
matrix nightly, and the oracle needs a third-party `aria2` binary whose output
drifts with its release. It is a **reader** of the Linux `ci-debug` cache entry
(`save-if: false`), so it reuses `check-rust`'s artifacts instead of a cold build.
The oracle now asserts the client-facing contract against allowlists (AriaNg
option keys, always-present `tellStatus` keys, the documented
`listMethods`/`listNotifications` delta, code 1 error objects and JSON-RPC
batch); it fails on anything outside them, when the oracle cannot start, or when
a server stops answering. The remaining aria2-only differences are printed as
allowlisted notes.

Windows is split into three parallel jobs because its native job is the critical
path and clippy cannot share build artifacts with test builds.

Evidence: `repo://.github/workflows/ci.yml#L148-L336`,
`repo://.github/workflows/ci.yml#L336-L522`,
`repo://.github/workflows/aria2-oracle.yml#L1-L45`,
`repo://crates/limedl-core/src/aria2_rpc/oracle_tests.rs#L1-L68`.

### Cache and RUSTFLAGS contract

Every job passes `cache: false` to `actions-rust-lang/setup-rust-toolchain`
(because it runs rust-cache internally) and then runs `swatinem/rust-cache`
explicitly exactly once. The Rust jobs share one `ci-debug` key per OS via
`shared-key`; rust-cache still appends the runner OS/arch, so Windows, Linux and
macOS stay separate.

Additional rules:

- **Exactly one writer per shared key**, gated on `github.ref == 'refs/heads/main'`;
  everyone else passes `save-if: false`. Multiple writers race after a lockfile
  bump and recreate duplicate entries, which is what pushed the repo against
  GitHub's 10 GB ceiling and caused LRU eviction of the Linux caches.
- `rustflags: ""` leaves RUSTFLAGS unset so `.cargo/config.toml` stays
  authoritative. A job-scoped env var such as `CARGO_BUILD_WARNINGS` splits the
  cache key, so it is attached to steps rather than the job when key sharing
  matters.
- Windows jobs exclude build paths from Defender real-time scanning
  (best-effort), which is the dominant Windows-vs-Linux build penalty.

Evidence: `repo://.github/workflows/ci.yml#L40-L93`,
`repo://.github/workflows/ci.yml#L148-L336`,
`repo://.github/workflows/ci.yml#L336-L515`.

## Release pipeline

Pushing a `v*` tag triggers `.github/workflows/release.yml`:

1. `font` fetches the font once.
2. `changelog` computes the previous tag and generates the release body with
   **git-cliff** (`cliff.toml`) from Conventional Commits. `test:`, `ci:`,
   `chore:`, `build:` and `style:` are omitted from the notes;
   `feat:`/`fix:`/`perf:`/`refactor:`/`docs:` are grouped. Keep commit subjects
   Conventional so notes stay readable.
3. `build-native` (Windows) and `build-native-macos` build and package their
   artifacts. `build-native-linux` builds with
   `cargo zigbuild --target x86_64-unknown-linux-gnu.2.17` and runs
   `scripts/check-glibc-floor.sh`, so the portable/deb/AppImage keep a glibc 2.17
   floor instead of the runner's 2.39 while still linking the distro's dynamic
   system libraries. All three legs upload to the release.
4. `build-server` cross-compiles the headless `limedl-server` for
   `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` with
   `cargo zigbuild` (zig bundles the musl C toolchain `aws-lc-sys` needs, so no
   Docker image is required) and uploads tarballs that also contain the systemd
   unit and env example. They are not signed, not guarded and not part of
   `latest-native.json`.
5. `server-image` assembles those same prebuilt static binaries into a thin
   Alpine image (no engine rebuild), pushes one image per architecture, and
   merges them into a multi-arch `ghcr.io/zkz098/limedl-server` manifest. The
   `latest` tag is skipped for an alpha/beta/rc release. QEMU is used only for
   the image's Alpine `apk add` layer.
6. `native-manifest` is the **sole writer** of `latest-native.json`: it runs with
   `if: always()`, merges only the platform legs that succeeded, dual-signs every
   artifact (Minisign + ML-DSA-65), generates and dual-signs the manifest, and
   runs `cargo xtask guard`. A missing platform key is handled by the client as
   "no update" instead of an error. It lists `build-server` in `needs` only so it
   flips the release's `prerelease` flag to `false` last.

Both `cargo zigbuild` legs install Zig from the PyPI `ziglang` wheel pinned to
`0.16.0` and pass `pip --only-binary ":all:"`; the wheel ships no `zig` console
script, so the packaged binary is symlinked onto `PATH` for cargo-zigbuild. The
pin and the binary-only install are what the supply-chain rules require and what
keeps a release reproducible.

The signing keys never appear in the tree; `cargo xtask guard` fails the release
if either CI secret's derived public key does not match the client's
`PUBKEY_B64`/`PQC_PUBKEY_B64`.

Evidence: `repo://.github/workflows/release.yml#L1-L37`,
`repo://.github/workflows/release.yml#L77-L128`,
`repo://.github/workflows/release.yml#L548-L639`,
`repo://.github/workflows/release.yml#L640-L734`,
`repo://.github/workflows/release.yml#L735-L877`.

The `Signing check` workflow is the manual counterpart: it signs a throwaway file
and runs `guard` without publishing, so a rotation of either the Minisign or the
ML-DSA-65 key can be validated on demand.

Evidence: `repo://.github/workflows/sign-check.yml#L1-L48`.

### The Linux glibc floor

Lowering the desktop Linux floor is a release concern of its own, because the
Slint build cannot be static (it links `libfontconfig.so.1` and `dlopen`s
X11/Wayland/GL), so musl is not an option. `cargo zigbuild --target
x86_64-unknown-linux-gnu.2.17` keeps the binary dynamically linked but makes the
linker resolve its *own* libc references against glibc 2.17 — Rust's minimum for
the gnu target. `scripts/check-glibc-floor.sh` reads the ELF's `GLIBC_*` symbol
versions with `objdump` and fails the release if any exceeds 2.17, so a future
dependency cannot silently raise the floor again.

Evidence: `repo://.github/workflows/release.yml#L471-L480`,
`repo://scripts/check-glibc-floor.sh#L1-L60`,

## Dependency discipline

After `cargo update`/`cargo add`/`cargo remove`, commit the changed
`Cargo.lock`. Uncommitted lockfile changes cause CI cache misses and stale
dependency resolution.

One dependency is pinned for correctness rather than version hygiene: `irontide`
is `=1.7.0` because the BT backend depends on a wide slice of the engine API and
upstream's git repository was removed, so crates.io is the only source of truth.
Moving off it is a migration, not a version bump.

Evidence: `repo://AGENTS.md#L140-L147`, `repo://Cargo.toml#L68-L71`.

Related pages: [Self-Update and Distribution Channels](../desktop/self-update-and-distribution.md),
[Headless Server Daemon](../integrations/headless-server-daemon.md),
[Server Deployment and Packaging](server-deployment-and-packaging.md),
[Testing Strategy](../testing/testing-strategy.md),
[limedl Wiki Quickstart](../quickstart.md).
