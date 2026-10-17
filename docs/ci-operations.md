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
- The rust-cache key is `v0-rust-<shared-key>-<os>-<arch>-<envHash>-<lockHash>`.
  `envHash` covers the installed Rust toolchain versions plus every
  `RUST*`/`CARGO*`/`CC*`/`CFLAGS*`/`CXX*`/`CMAKE*` environment variable, and
  **does not include source**. `lockHash` hashes each workspace member's
  `Cargo.toml` (with `package.version` normalised to `0.0.0` and path
  dependencies blanked), the external packages of `Cargo.lock` (also parsed, not
  read as bytes), and the **raw bytes** of `.cargo/config.toml` /
  `rust-toolchain*`. Consequences worth remembering:
  - A version bump does **not** invalidate the cache (`0.0.0` normalisation).
  - A `Cargo.lock`/dependency change appends a new `lockHash`, so rust-cache
    writes a **new generation** while the old one survives only until GitHub's
    repository-wide LRU eviction. The restore is a prefix match
    (`restoreKey` = key minus `lockHash`), so dependencies are reused rather than
    rebuilt cold.
  - Because `.cargo/config.toml` is hashed as raw bytes, Windows CRLF checkout
    gives Windows a different `lockHash` than Linux/macOS on the same commit
    (measured `75a17e6d` vs `73b11a8a`, run 37568494078): harmless today because
    each OS keeps its own entries, but do not expect cross-OS key equality.
  - The cache is only written back when the restore is incomplete (key
    mismatch); a full hit is not overwritten.
- **Every writer passes `cache-on-failure: true`.** The action's post step is
  gated on `success() || CACHE_ON_FAILURE == 'true'`; without the flag a run that
  changes the lockfile and then fails a test pays the multi-minute save twice
  (once red, once on the fix). The Windows save alone measured 127-179 s for
  ~1.2 GB. A failing test does not invalidate dependency artifacts, so caching
  them stays correct.
- **Superseded generations are collected by the `prune-cache` job** in
  `ci.yml`. It runs only on `main` (the only ref that saves anyway), after all
  three writers (`always()`, so a failed writer still leaves the previous
  generation as the newest), and keeps only the most recently created entry per
  key prefix — the only entry a bare prefix restore can ever pick — deleting the
  rest through the `actions/cache` REST API. `actions: write` is scoped to that
  job. Measured 2026-10-07 before the job existed: 5 `v0-rust-*` entries /
  8.54 GB against the 10 GB ceiling, of which 2.73 GB were superseded. When
  changing the GC, keep two invariants: never delete the newest entry per prefix,
  and never select outside `v0-rust-` (the `Linux-sonar` and `misans-vf` entries
  are rolling caches with their own lifecycle).
- **The Linux `ci-debug` entry is legitimately the largest** (2917 MiB vs
  1197 MiB on Windows and 1660 MiB on macOS). `cargo llvm-cov` defaults its build
  tree to `<target>/llvm-cov-target` (`CARGO_LLVM_COV_TARGET_DIR` /
  `CARGO_LLVM_COV_BUILD_DIR`), and rust-cache caches the whole `target/` dir.
  That tree is **not** a duplicate of `target/debug`: coverage runs with
  `--cargo-profile dev` while every other nextest step uses the default `test`
  profile, and `[profile.test] debug = false` makes their dependency artifacts
  hash differently, so the two directories hold different profiles. The dev
  graph is required to link the instrumented test binaries, and cargo-llvm-cov
  v0.9.0's rustc wrapper only adds instrumentation flags for workspace members
  (`__CARGO_LLVM_COV_RUSTC_WRAPPER_CRATE_NAMES`, from `src/wrapper.rs`), never
  for dependencies. Measured 2026-10-13: `target` = 12 G pre-clean, of which
  `target/debug/deps` = 5.4 G and the rest is the dev graph. Pointing
  `CARGO_LLVM_COV_TARGET_DIR` / `BUILD_DIR` at `target/` would therefore *move*
  the dev rlibs into `target/debug` without removing bytes — only the duplicated
  `build/` and `.fingerprint` entries (~0.2-0.3 GiB) would go, at the cost of a
  one-time full dev rebuild. Rejected on 2026-10-13. The only real size lever is
  running the `limedl-native` / `limedl-server` nextest steps with
  `--cargo-profile dev` so they reuse the coverage graph, which trades away the
  `[profile.test] debug = false` link-time win and runs instrumented test
  binaries (`.profraw` litter). Do not re-open this without new measurements.
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

## Feature matrix and dependency hygiene

Two small checks live in jobs that already exist, so neither adds a cache key
(all their own env is step-scoped, and neither compiles dependencies that the
job was not already building).

- **Feature matrix (`check-rust`, cargo-hack 0.6.45).**
  `cargo hack check --feature-powerset --all-targets --keep-going` over
  `limedl-core` expands to 10 combinations of `{bt, aria2-rpc, test-utils}` and
  runs under the job's `CARGO_BUILD_WARNINGS=deny`, so a warning in *any*
  combination is fatal. Only `limedl-core` has features; native/server/xtask have
  none, so running it there would only burn runner time.
  It exists because `--no-default-features` (the BT engine absent) is a
  documented configuration — the commit that added the `bt` feature promised it —
  and nothing ever built it: by the time the step was added it produced ten
  denied `irrefutable_let_patterns` warnings (`let TaskId::Http(id) = task_id
  else` is irrefutable when the enum has one variant) plus an ungated BT test
  module. If you add a `#[cfg(feature = …)]` branch, run this locally; the
  `TaskId::as_http()` helper exists so HTTP-only call sites stay refutable in
  every configuration.
  Measured 54 s warm; every combination reuses the dependency graph the coverage
  and lint steps already built.
- **Unused dependencies (`supply-chain`, cargo-machete 0.9.2).**
  `cargo machete --with-metadata` is static analysis — no workspace build,
  ~0.1 s — and that job already runs with `cache-targets: false`. Use the
  metadata mode: the default mode misses a dev-dependency that no source file
  names (it found neither of the two removed when the step was introduced).
  False positives go in `[package.metadata.cargo-machete] ignored` in the crate
  that owns them, each with the reason (build-script dependencies and
  feature-unification-only dependencies are the two recurring classes — see
  `crates/limedl-native/Cargo.toml`). A new entry there means "verify by hand
  that the crate is still needed", not "silence the tool".
- **Not adopted: `cargo udeps`.** It needs a nightly toolchain plus a full
  build, and rust-cache keys include the toolchain version, so it would add a
  multi-GB cache generation against the 10 GB ceiling this repo already fights
  (see `prune-cache`). Its extra accuracy over machete covers proc-macro-only
  dependencies, which this workspace has none of.
- **`cargo hack` and `cargo machete` are pinned** in `ci.yml` like `nextest` and
  `cargo-llvm-cov`: the tool's own release can change what the gate accepts
  without a code change.

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
- `cargo hack` and `cargo machete` are part of the gate (see the section above).
  `CARGO_BUILD_WARNINGS=deny` must be set for the `cargo hack` run exactly as for
  clippy, or the feature-matrix step stops being fatal on warnings. The
  `--keep-going` flag makes it report *every* broken combination rather than
  stopping at the first, matching the gate's preference for complete failure
  output.
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
