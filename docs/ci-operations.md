# CI operations

Operational contract for `.github/workflows/ci.yml`, `.github/workflows/release.yml`
and `.github/workflows/sign-check.yml`. Read this before editing a job.

## CI job naming convention

`name:` (the GitHub UI and check name) is uniformly
`<scope/action> (<platform>[, detail])`:

- The platform is always the **first token** inside the final parentheses, limited
  to `Linux` / `macOS` / `Windows` / `windows-x86_64`.
- The action vocabulary is limited to: Clippy / Tests / Coverage / Supply chain /
  Release notes / Sign & guard.
- **Never write `: ` in `name:`** — a YAML value containing `": "` must be quoted.
  Use a comma for sub-qualifiers: `Tests (Windows, limedl-core)`.
- **Job ids do not change with the display name.** Keep them stable so `needs:`,
  `gh run view` and documentation references keep working.

## CI cache / RUSTFLAGS contract

- **`setup-rust-toolchain` must pass `cache: false`.** The action's default
  (`cache: true`) runs `swatinem/rust-cache` internally, duplicating the explicit
  `swatinem/rust-cache` step in the workflow — i.e. two 1-2 GB entries per job.
  The repository cache ceiling is 10 GB; duplicate entries evict other jobs'
  entries and trigger LRU churn.
- **`rustflags: ""`.** The action otherwise exports `RUSTFLAGS=-D warnings`, and
  any `RUSTFLAGS` **replaces** `.cargo/config.toml`'s per-target rustflags
  (`target-cpu` / `rust-lld` all stop applying). Leaving it empty keeps
  `config.toml` the single flag source, so every step shares one fingerprint and
  a job no longer rebuilds the whole graph mid-run.
- Warnings are still fatal: `CARGO_BUILD_WARNINGS: deny` (cargo's
  `build.warnings`, orthogonal to RUSTFLAGS) plus clippy's `-- -D warnings`, set
  on **every** Rust job including `test-windows-native`. That job used to be an
  exception because linking the Skia-based `limedl-native` with rust-lld warned
  about Skia's duplicate ICU symbol `ubrk_getLocaleByType` (converged with
  `/FORCE:MULTIPLE`); both the Skia dependency and the renderer are gone, so the
  exception and the flag were removed with them.
- The rust-cache key is composed of the Rust toolchain + the RUST*/CARGO*/CC*/
  CFLAGS*/CXX*/CMAKE* environment variables + `.cargo/config.toml` + external
  dependency hashes, and **does not include source**. It is only written back
  when the restore is incomplete (key mismatch); a full hit is not overwritten.
- **Desktop release builds are deliberately uncached.** They were once warmed by
  `.github/workflows/warm-release-cache.yml` on `main`, but that workflow cost
  ~50 min per crates-changing push for two legs while the measured release gain
  was Windows ≈ 0 (13m05s warm vs 13m16s cold — that leg is link-bound, not
  dependency-compile-bound) and macOS only 4-9 min. More importantly rust-cache
  caches dependencies but not workspace crates, so a source-only push cannot
  change the key and the binary is rebuilt anyway. The workflow was deleted and
  the three `release.yml` legs no longer carry rust-cache: a tag release now pays
  a cold compile (v0.4.0 fully cold measured 1481 s for
  `cargo build --release -p limedl-native` on macOS).
- **The Rust jobs share one `ci-debug` cache key per OS** (`shared-key: ci-debug`).
  `shared-key` replaces the job-id segment while rust-cache still appends the
  runner OS/arch, so Windows / Linux / macOS keep separate entries. The real
  consolidation happens on Windows: the three jobs' envHash and lockHash are
  identical (measured `fc9b2beb`/`b0be5515`), and only rust-cache's default
  job-id key kept them apart, storing the same dependency graph three times.
  Linux / macOS each have one job, so `shared-key` just unifies the name and
  shares automatically if another job is added.
  **Exactly one job writes each key** (Windows = `test-windows-native`,
  Linux = `check-rust`, macOS = `check-macos`); the others pass `save-if: false`.
  Multiple writers race on the same key after each lockfile bump (actions/cache
  can only reserve a key once) and each writes back a generation; those duplicate
  entries pushed the repo to 11.7 GB, evicted the Linux entry and made the
  coverage leg cold-compile llvm-cov. Writers also require
  `github.ref == 'refs/heads/main'`: a cache saved on a PR branch is visible only
  to that branch yet still counts against the 10 GB quota.
- **A job-level env var splits rust-cache's envHash.** A variable like
  `CARGO_BUILD_WARNINGS: deny` (prefix-matches the list above) prevents two jobs
  from sharing a key (measured: `check-rust` = `357705c9`, `supply-chain` =
  `df9a423c`). When consolidating keys (`shared-key` or `add-job-id-key: false`),
  attach such variables to the specific step's `env:` so the cache step never
  sees them.

## CI test execution / build speed

- **Rust tests all run under `cargo nextest`** (installed via
  `taiki-e/install-action`, version pinned in the workflow). nextest starts an
  independent process per test, which both parallelizes execution and removes the
  sibling-test interference a single libtest process per binary exposes.
  `cargo nextest run` **does not run doctests** — the workspace has none today;
  if one is added, add a `cargo test --doc` step to `check-rust`. The coverage
  job also uses nextest (`cargo llvm-cov nextest`) with `--no-fail-fast` so one
  failing test cannot shorten the coverage union.
- **`[profile.test] debug = false`** (root `Cargo.toml`): every CI job compiles
  and links test binaries, and dependencies already skip debug info via
  `[profile.dev.package."*"]`, so the workspace crates' own line tables are pure
  overhead. For local breakpoints/line numbers use `cargo test --profile dev`;
  the coverage `cargo llvm-cov nextest --cargo-profile dev` also exists for lcov
  line attribution (`--profile` selects the nextest profile; the Cargo profile is
  `--cargo-profile`). Release artifacts use a separate profile and are unaffected.
- **Coverage is a hard gate.** `check-rust` runs
  `cargo llvm-cov nextest ... --fail-under-lines 85` for `limedl-core`; below 85 %
  fails the job (Sonar's quality gate cannot carry custom conditions on the Free
  plan, so it is only informational). The 85 comes from a pinned 0.9.0 measurement
  on Linux (86.90 % / 15376 lines, run 37088319623) with ~2 pp margin: the
  numerator jitters ~20 lines per run (CI 13362, local 13343; retry/eviction
  timing paths are nondeterministic under instrumentation). The number counts
  **product code only**: since cargo-llvm-cov 0.6.22 the default ignore regex
  drops `tests/` directories and `tests.rs`/`*_tests.rs` files.
  **Do not re-baseline from a single local `cargo llvm-cov` run:** the old gate
  read "87.37 % of 14561 lines", but 14561 was the 0.6.21 denominator — 0.6.x
  instrumented the whole dependency graph via RUSTFLAGS while 0.7.0+ uses a rustc
  wrapper that only instruments the needed crates (PR #471). The same tree is
  14561 with a fresh 0.6.21 build and 15376 with a fresh 0.9.0, and the wrapper's
  injected flags do not enter cargo's fingerprint, so re-measuring 0.9.0 after a
  0.6.21 run silently reuses the old artifacts. To re-baseline, use
  `cargo llvm-cov clean` plus the pinned tool, or read the number CI prints.
  The tool is pinned to `cargo-llvm-cov@0.9.0`.
  The same step also generates a second lcov for `limedl-native`, using
  `--ignore-filename-regex 'crates.limedl-core'` to drop the core lines llvm-cov
  instruments anyway (otherwise core's lines are imported twice). Both steps pass
  `--no-cfg-coverage`: on the native side `tiny-xlib` turns `cfg(coverage)` into
  the nightly-only `#![feature(coverage_attribute)]` (stable fails with E0554),
  and identical RUSTFLAGS let the second step reuse the first step's instrumented
  shared dependencies. Reports are written per crate
  (`crates/limedl-core/lcov-core.info`, `crates/limedl-native/lcov-native.info`):
  Sonar's Rust analyzer resolves `sonar.rust.lcov.reportPaths` per Cargo manifest
  module and only looks in that module's own base dir. `scripts/`, `website/` and
  `xtask` are excluded in `sonar.coverage.exclusions` — llvm-cov cannot instrument
  PS1/Astro and a 0 % score would only drag the denominator down.
- **Coverage is line-only for now.** cargo-llvm-cov's `--branch` is unstable and
  stable toolchains reject it (`--branch flag requires nightly toolchain`), so the
  LCOV has no BRDA and Sonar has no condition coverage. Branch coverage requires
  switching those two llvm-cov steps to a pinned nightly; see the ci.yml comment.
- **Windows jobs exclude build paths from Defender real-time scanning**
  (`Add-MpPreference -ExclusionPath`, best-effort, failure does not fail the job):
  Defender scans every multi-GB file cargo writes into `target/` and is the main
  Windows-vs-Linux penalty. Do not export `RUSTFLAGS`/`CARGO*`/`CC*`/`CMAKE*`
  environment variables when adding Windows steps, or the rust-cache key splits.

## Local pre-commit gate

The commands mirror `.github/workflows/ci.yml` one for one (same crate list, same
features, same nextest version), so a green local run means a green CI run.

- Warnings are fatal through `CARGO_BUILD_WARNINGS=deny` (cargo's own
  `build.warnings`, orthogonal to RUSTFLAGS) plus clippy's `-- -D warnings`.
  Deliberately **not** `RUSTFLAGS="-D warnings"`: that variable overrides
  `.cargo/config.toml`'s per-target rustflags (`target-cpu`, `rust-lld`,
  `/FORCE:MULTIPLE`), which CI keeps authoritative with `rustflags: ""`.
- Tests run **per crate, not `--workspace`**: only `limedl-core`'s tests are
  meaningful without the `test-utils,aria2-rpc` features, a workspace-wide run
  would lose them, and it would build the UI crate just to check its flags.
  `xtask` is in the gate because it holds the release-guard tests
  (`cargo xtask guard` runs in `release.yml`, and its default `--update-rs` path
  broke a release once).
- Pin the nextest version CI installs
  (`cargo install cargo-nextest --locked --version 0.9.144`). `cargo nextest run`
  does not execute doctests; the workspace has none today, so if one is ever
  added, add a `cargo test --doc` step to the gate and to CI's `check-rust` job
  together.
- Windows: initialize MSVC first (`vcvarsall.bat x64`).

## macOS linker note (`-A linker_messages`)

One macOS-only diagnostic is exempted rather than fixed: Apple's `ld` notes
`__eh_frame section too large (max 16MB)` for the `limedl-native` test binary.
Compact unwind only reserves a 24-bit hint for an FDE's offset in `__eh_frame`,
so past 16 MB the linker falls back to plain DWARF unwinding — a performance
note, not a defect. `.cargo/config.toml` therefore passes `-A linker_messages` on
both Apple targets, keeping that note out of cargo's `build.warnings = deny`.

It is about the size of the test binary, not a dependency: dropping the flag
after the Skia removal (run 36972236669) failed the macOS native test step with
`error: warnings are denied by build.warnings configuration`, with nothing linking
Skia. `cargo clippy` never links, which is why only the test step trips it. Every
rustc/clippy warning stays fatal; a *real* macOS linker warning is now only
printed, so read the `check-macos` log when a link looks suspicious.
