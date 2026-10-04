---
type: operations
title: Build, Tooling, CI and Release Operations
description: The operational surface of limedl — build prerequisites and rustflags, the xtask tooling, the mandatory pre-commit gate and its Windows blind spot, the CI job graph with nextest/coverage/sonar/supply-chain, and the tag-driven release pipeline.
tags: [operations, build, ci, release, xtask, tooling]
sources:
  - id: openwiki-source-4905fab56ecf9fa5e1ebbf3f
    resource: repo://.cargo/config.toml
  - id: openwiki-source-ca5b77738a5ec463872c3294
    resource: repo://.github/actions/fetch-misans/action.yml
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
  - id: openwiki-source-44d192e16032f18d847f0ff6
    resource: repo://xtask/src/bump_version.rs
  - id: openwiki-source-ca864fd40fa4107ed35f840f
    resource: repo://xtask/src/fetch_font.rs
  - id: openwiki-source-3e467e67d349677035f0363f
    resource: repo://xtask/src/main.rs
generated: { by: "pi", at: "2026-10-04T03:21:09.297Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T10:20:09.270Z
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
`[target.*]` sets `target-cpu=x86-64-v3` for desktop targets (x86-64-v2 for NAS
musl targets) and adds `--cfg reqwest_unstable`, which reqwest's HTTP/3 feature
requires at compile time. Add both when introducing a new target.

Evidence: `repo://AGENTS.md#L3-L21`, `repo://.cargo/config.toml#L1-L45`.

## xtask tooling

`cargo xtask` is the repository's tooling entry point:

| Command | Purpose |
| --- | --- |
| `generate-key --out-dir <dir>` | Create a minisign keypair and print the `PUBKEY_B64` value + `gh secret set` commands |
| `sign <files...>` | Write minisign `<file>.sig` next to each file |
| `verify <files...>` | Verify `.sig` against a public key |
| `guard <files...>` | Release guard: signing key must match the client's `PUBKEY_B64` and every signature must verify |
| `manifest --version ...` | Generate the self-update `latest-native.json` |
| `fetch-font [--verify]` | Fetch/verify the pinned MiSans VF font |
| `bump-version <patch\|minor\|major>` | Bump version, update Cargo.lock + website, commit, tag and push |

Evidence: `repo://xtask/src/main.rs#L71-L140`.

`bump-version` updates the workspace `Cargo.toml` (first `version = "x.y.z"`),
`Cargo.lock` and the website, then creates a `chore: bump version to X` commit and
pushes commit + tag. `--dry-run` prints the plan and `--no-push` skips git.

Evidence: `repo://xtask/src/bump_version.rs#L57-L135`.

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
CDN hiccup read as four unrelated failures.

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
```

Notes that matter:

- Warnings are fatal through `CARGO_BUILD_WARNINGS=deny` (cargo's own
  `build.warnings`, orthogonal to RUSTFLAGS) plus clippy's `-- -D warnings`.
  Deliberately **not** `RUSTFLAGS="-D warnings"`, which would override
  `.cargo/config.toml`'s per-target flags.
- Tests run per crate, not `--workspace`: only `limedl-core`'s tests need the
  `test-utils,aria2-rpc` features, and `xtask` is in the gate because it owns the
  release-guard tests.
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
stable).

Evidence: `repo://AGENTS.md#L82-L105`,
`repo://.github/workflows/ci.yml#L451-L472`.

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

Evidence: `repo://AGENTS.md#L106-L125`.

## CI job graph

`.github/workflows/ci.yml` runs these jobs:

| Job | Platform | What it runs |
| --- | --- | --- |
| `font` | Linux | Fetch MiSans VF once, upload `misans-vf` |
| `check-windows` | Windows | `cargo clippy --workspace --all-targets -- -D warnings` |
| `test-windows-core` | Windows | nextest for `limedl-core` (with features) and `xtask` |
| `test-windows-native` | Windows | nextest for `limedl-native` |
| `check-macos` | macOS | clippy → core nextest → native nextest |
| `check-rust` | Linux | coverage (core + native lcov) → clippy → SonarCloud → native nextest |
| `supply-chain` | Linux | `cargo deny check -W rejected bans licenses sources` + `cargo audit` |

Windows is split into three parallel jobs because its native job is the critical
path and clippy cannot share build artifacts with test builds.

Evidence: `repo://.github/workflows/ci.yml#L148-L336`,
`repo://.github/workflows/ci.yml#L336-L560`.

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
3. `build-native` (Windows), `build-native-macos` and `build-native-linux` each
   build and package their artifacts and upload them to the release.
4. `native-manifest` is the **sole writer** of `latest-native.json`: it runs with
   `if: always()`, merges only the platform legs that succeeded, signs every
   artifact, generates and signs the manifest, and runs `cargo xtask guard`. A
   missing platform key is handled by the client as "no update" instead of an
   error.

The signing key never appears in the tree; `cargo xtask guard` fails the release
if the CI secret's derived public key does not match the client's `PUBKEY_B64`.

Evidence: `repo://.github/workflows/release.yml#L1-L37`,
`repo://.github/workflows/release.yml#L77-L128`,
`repo://.github/workflows/release.yml#L512-L640`.

The `Signing check` workflow is the manual counterpart: it signs a throwaway file
and runs `guard` without publishing, so a key rotation can be validated on demand.

Evidence: `repo://.github/workflows/sign-check.yml#L1-L47`.

## Dependency discipline

After `cargo update`/`cargo add`/`cargo remove`, commit the changed
`Cargo.lock`. Uncommitted lockfile changes cause CI cache misses and stale
dependency resolution.

One dependency is pinned for correctness rather than version hygiene: `irontide`
is `=1.7.0` because the BT backend depends on a wide slice of the engine API and
upstream's git repository was removed, so crates.io is the only source of truth.
Moving off it is a migration, not a version bump.

Evidence: `repo://AGENTS.md#L126-L133`, `repo://Cargo.toml#L70-L74`.

Related pages: [Self-Update and Distribution Channels](../desktop/self-update-and-distribution.md),
[Testing Strategy](../testing/testing-strategy.md),
[limedl Wiki Quickstart](../quickstart.md).
