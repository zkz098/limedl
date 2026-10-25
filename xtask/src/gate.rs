//! `cargo xtask gate` — the pre-commit gate, in the order CI runs it.
//!
//! `AGENTS.md` used to spell the gate out as ~25 lines of commands, and the two
//! drifted: the list never grew `cargo deny` / `cargo audit`, and every
//! `--manifest-path crates/limedl-core/Cargo.toml` silently skipped the suites
//! of the crates that were split out of it. Both failure modes are structural,
//! so they are fixed structurally:
//!
//! * the package list comes from `cargo metadata`, and [`TEST_GROUPS`] must
//!   cover every workspace member — `every_workspace_member_is_tested` fails
//!   otherwise, so a new crate cannot be forgotten;
//! * [`ci_parity`] re-reads `.github/workflows/ci.yml` and fails when a member
//!   is not named on a `nextest` / `llvm-cov` line there;
//! * the environment CI sets (`CARGO_BUILD_WARNINGS=deny`, the sparse registry
//!   protocol) is applied here instead of being remembered;
//! * the pinned toolchains are asserted, so "green locally" keeps meaning
//!   "green in CI" (`rustfmt` is the one that can silently reformat the tree).
//!
//! CI does **not** call this command. `ci.yml` stays the authority; this mirrors
//! it, and [`ci_parity`] is what keeps the mirror honest.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};

/// The `rustfmt` toolchain `ci.yml` pins.
///
/// `rustfmt`'s output changes between Rust releases, so CI formats with an
/// explicitly installed 1.99.0 instead of the job's `stable`. Formatting here
/// with a different one is what makes a locally-clean tree fail `check-rust`.
const RUSTFMT_TOOLCHAIN: &str = "1.99.0";

/// One `cargo nextest run` invocation.
///
/// Groups rather than one package per entry, because the engine crates are
/// selected together: `--features` binds to `limedl-core` and a per-package
/// `--manifest-path` would skip its siblings.
pub struct TestGroup {
    pub packages: &'static [&'static str],
    pub features: &'static [&'static str],
}

pub const TEST_GROUPS: &[TestGroup] = &[
    TestGroup {
        packages: &[
            "limedl-types",
            "limedl-disk",
            "limedl-store",
            "limedl-http",
            "limedl-engine",
            "limedl-bt",
            "limedl-core",
        ],
        features: &["limedl-core/test-utils", "limedl-core/aria2-rpc"],
    },
    TestGroup {
        packages: &["limedl-native"],
        features: &[],
    },
    TestGroup {
        packages: &["limedl-server"],
        features: &[],
    },
    TestGroup {
        packages: &["xtask"],
        features: &[],
    },
];

/// Packages whose tests need the MiSans font artifact (`cargo xtask fetch-font`).
const FONT_PACKAGES: &[&str] = &["limedl-native"];

#[derive(Debug, Default)]
pub struct Options {
    /// Also run the slow whole-workspace checks (`hack`, `machete`, `deny`, `audit`).
    pub full: bool,
    /// Keep going after a failure instead of stopping at the first one.
    pub keep_going: bool,
}

/// A gate step: a name for the summary and the process it runs.
pub struct Step {
    pub name: &'static str,
    pub program: &'static str,
    pub args: Vec<String>,
    /// `cargo-<tool>` that has to be on `PATH` before this step can run.
    pub tool: Option<&'static str>,
}

impl Step {
    fn cargo(name: &'static str, args: &[&str]) -> Self {
        Self {
            name,
            program: "cargo",
            args: args.iter().map(|arg| (*arg).to_string()).collect(),
            tool: None,
        }
    }

    /// A `cargo <tool>` step that only exists once `cargo-<tool>` is installed.
    fn cargo_tool(name: &'static str, tool: &'static str, args: &[&str]) -> Self {
        Self {
            tool: Some(tool),
            ..Self::cargo(name, args)
        }
    }

    /// The command as a copy-pasteable line. Only quoting for readability —
    /// nothing here is re-parsed by a shell.
    fn rendered(&self) -> String {
        let mut out = self.program.to_string();
        for arg in &self.args {
            out.push(' ');
            if arg.contains(' ') {
                out.push('"');
                out.push_str(arg);
                out.push('"');
            } else {
                out.push_str(arg);
            }
        }
        out
    }
}

pub fn run(root: &Path, options: &Options) -> Result<()> {
    // Cheap, offline, and the one check that catches the structural failure
    // this command exists to prevent — so it runs before anything slow.
    ci_parity(root)?;

    let steps = build_steps(options);
    println!(
        "gate: {} steps ({} profile)",
        steps.len(),
        if options.full { "full" } else { "quick" }
    );
    host_notes(root)?;

    let mut failures: Vec<(&'static str, String, u128)> = Vec::new();
    let mut skipped: Vec<(&'static str, &'static str)> = Vec::new();
    let mut passed = 0usize;
    let started = Instant::now();

    for step in &steps {
        if let Some(tool) = step.tool
            && !tool_is_installed(tool)
        {
            // A missing tool is not a passing check. It is not a failing one
            // either: the step never ran. Say which and how to fix it, and keep
            // the summary honest about what was not verified.
            println!(
                "\n==> {}\n    skipped: cargo-{tool} is not installed\n    \
                 $ cargo install cargo-{tool} --locked",
                step.name
            );
            skipped.push((step.name, tool));
            continue;
        }
        println!("\n==> {}\n    $ {}", step.name, step.rendered());
        let step_started = Instant::now();
        let mut command = Command::new(step.program);
        command
            .args(&step.args)
            .current_dir(root)
            .env("CARGO_BUILD_WARNINGS", "deny")
            .env("CARGO_REGISTRIES_CRATES_IO_PROTOCOL", "sparse");
        scrub_cargo_metadata(&mut command);
        let status = command
            .status()
            .with_context(|| format!("spawn `{}` (is it installed?)", step.rendered()))?;
        let elapsed = step_started.elapsed().as_millis();

        if status.success() {
            passed += 1;
            println!("<== {} ok ({} ms)", step.name, elapsed);
            continue;
        }

        failures.push((step.name, step.rendered(), elapsed));
        println!("<== {} FAILED ({} ms)", step.name, elapsed);
        if !options.keep_going {
            break;
        }
    }

    println!("\n──────── gate summary ────────");
    println!(
        "{passed} passed, {} failed, {} not run, in {} s",
        failures.len(),
        skipped.len(),
        started.elapsed().as_secs()
    );
    for (name, tool) in &skipped {
        println!("  - {name} not run: cargo install cargo-{tool} --locked");
    }
    if failures.is_empty() {
        if skipped.is_empty() {
            println!("all green — this is what `ci.yml` runs, on this host");
        } else {
            println!(
                "every step that ran is green; the steps listed above did not run, \
                 so CI is still the only run that covered them"
            );
        }
        return Ok(());
    }
    for (name, command, elapsed) in &failures {
        println!("  ✗ {name} ({} ms)\n    $ {command}", elapsed);
    }
    println!("    re-run a single step by copying its command above");
    bail!("{} gate step(s) failed", failures.len())
}

/// Remove the metadata block `cargo run` hands to its child.
///
/// This is not cosmetic. Cargo subcommands decide how to parse their arguments
/// by asking "am I being run by cargo?", and they answer it from these
/// variables: `cargo-machete` uses `argh::cargo_from_env()` (which drops the
/// repeated `machete` argv item) only when `CARGO` is set **and**
/// `CARGO_PKG_NAME` is not. Spawned from `cargo run -p xtask`, `CARGO_PKG_NAME`
/// *is* set, so it fell back to `argh::from_env()`, took `machete` for a
/// directory and failed with `IO error for operation on machete`. Scrubbing the
/// block makes every step behave exactly like the same command typed in a
/// shell — which is the whole promise of the printed command.
fn scrub_cargo_metadata(command: &mut Command) {
    for key in cargo_metadata_keys(std::env::vars().map(|(key, _)| key)) {
        command.env_remove(key);
    }
}

/// The keys [`scrub_cargo_metadata`] removes: `CARGO_PKG_*`, plus the ones that
/// would otherwise make a tool look at xtask's manifest instead of the tree.
pub fn cargo_metadata_keys(keys: impl Iterator<Item = String>) -> Vec<String> {
    const EXTRA: &[&str] = &["CARGO_MANIFEST_DIR", "CARGO_CRATE_NAME", "OUT_DIR"];
    keys.filter(|key| key.starts_with("CARGO_PKG_") || EXTRA.contains(&key.as_str()))
        .collect()
}

#[cfg(test)]
pub fn steps(options: &Options) -> Vec<Step> {
    build_steps(options)
}

fn build_steps(options: &Options) -> Vec<Step> {
    let mut steps = vec![
        rustfmt_step(),
        Step::cargo(
            "clippy",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
    ];
    for group in TEST_GROUPS {
        let mut args = vec!["nextest".to_string(), "run".to_string()];
        for package in group.packages {
            args.push("-p".to_string());
            args.push((*package).to_string());
        }
        if !group.features.is_empty() {
            args.push("--features".to_string());
            args.push(group.features.join(","));
        }
        steps.push(Step {
            name: "nextest",
            program: "cargo",
            args,
            tool: Some("nextest"),
        });
    }
    steps.push(Step::cargo(
        "theme",
        &["run", "-q", "-p", "xtask", "--", "theme", "check"],
    ));

    if options.full {
        steps.extend([
            Step::cargo_tool(
                "hack",
                "hack",
                &[
                    "hack",
                    "check",
                    "--feature-powerset",
                    "--all-targets",
                    "--keep-going",
                    "--manifest-path",
                    "crates/limedl-core/Cargo.toml",
                ],
            ),
            Step::cargo_tool("machete", "machete", &["machete", "--with-metadata"]),
            Step::cargo_tool(
                "deny",
                "deny",
                &[
                    "deny", "check", "-W", "rejected", "bans", "licenses", "sources",
                ],
            ),
            Step::cargo_tool("audit", "audit", &["audit"]),
        ]);
    }

    steps
}

/// `cargo fmt --check`, against CI's pinned `rustfmt` when it is installed.
///
/// Falling back to the host toolchain is a warning rather than an error on
/// purpose: a contributor without `rustup toolchain install 1.99.0` should still
/// get a usable gate, and the warning names the one command that closes the gap.
fn rustfmt_step() -> Step {
    if toolchain_is_installed(RUSTFMT_TOOLCHAIN) {
        // `+<toolchain>` is one argument, not two: `cargo + 1.99.0` is parsed as
        // an empty toolchain name.
        return Step {
            name: "fmt",
            program: "cargo",
            tool: None,
            args: vec![
                format!("+{RUSTFMT_TOOLCHAIN}"),
                "fmt".to_string(),
                "--all".to_string(),
                "--".to_string(),
                "--check".to_string(),
            ],
        };
    }
    println!(
        "warning: rustfmt {RUSTFMT_TOOLCHAIN} (the version ci.yml pins) is not installed;\n         \
         formatting with the host toolchain instead. `rustup toolchain install {RUSTFMT_TOOLCHAIN} \n         \
         --profile minimal --component rustfmt` makes this check match CI."
    );
    Step::cargo("fmt", &["fmt", "--all", "--", "--check"])
}

/// Is `cargo-<tool>` runnable? Resolved through `PATH` the same way the step's
/// own `cargo <tool>` invocation will resolve it.
fn tool_is_installed(tool: &str) -> bool {
    Command::new(format!("cargo-{tool}"))
        .arg("--version")
        .output()
        .is_ok()
}

fn toolchain_is_installed(toolchain: &str) -> bool {
    let Ok(output) = Command::new("rustup").args(["toolchain", "list"]).output() else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.starts_with(toolchain))
}

/// Facts about the host that change what a green run means.
fn host_notes(root: &Path) -> Result<()> {
    if cfg!(windows) && std::env::var_os("VSINSTALLDIR").is_none() {
        println!(
            "warning: no MSVC environment detected (VSINSTALLDIR unset).\n         \
             Run the vcvarsall.bat line from AGENTS.md first, or linking will fail."
        );
    }
    if !cfg!(unix) {
        println!(
            "note: this host compiles `cfg(windows)` code; `cfg(not(windows))` code is\n      \
             only type-checked through `cargo clippy --workspace` — see AGENTS.md."
        );
    }
    let needs_font = TEST_GROUPS
        .iter()
        .any(|group| group.packages.iter().any(|p| FONT_PACKAGES.contains(p)));
    let font = root.join("crates/limedl-native/assets/fonts/MiSansVF.ttf");
    if needs_font && !font.is_file() {
        println!(
            "warning: {} is missing; `limedl-native` will not build.\n         \
             Run `cargo xtask fetch-font` once (the font is not in git).",
            font.display()
        );
    }
    Ok(())
}

/// Workspace member package names, straight from cargo.
fn workspace_members(root: &Path) -> Result<Vec<String>> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .context("run `cargo metadata`")?;
    if !output.status.success() {
        bail!(
            "`cargo metadata` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parse `cargo metadata` output")?;
    let mut members: Vec<String> = metadata["packages"]
        .as_array()
        .context("`packages` missing from cargo metadata")?
        .iter()
        .filter(|package| !package_listing_is_ignored(package))
        .filter_map(|package| package["name"].as_str().map(str::to_string))
        .collect();
    members.sort();
    Ok(members)
}

/// `cargo metadata --no-deps` lists workspace members only, but the xtask
/// package itself is a member and is also a tool — it has no tests-worth of
/// source, yet it does have a suite, so nothing is filtered today. Kept as a
/// single place to exclude a package if one is ever added.
fn package_listing_is_ignored(_package: &serde_json::Value) -> bool {
    false
}

/// Every workspace member must be selected by [`TEST_GROUPS`] and named on a
/// test command in `ci.yml`.
pub fn ci_parity(root: &Path) -> Result<()> {
    ci_parity_for(root, &workspace_members(root)?)
}

/// Test seam: [`ci_parity`] with an injected member list, so the drift checks
/// can be exercised against a fake workspace.
#[cfg(test)]
pub fn ci_parity_for_test(root: &Path, members: &[String]) -> Result<()> {
    ci_parity_for(root, members)
}

fn ci_parity_for(root: &Path, members: &[String]) -> Result<()> {
    let tested: Vec<&str> = TEST_GROUPS
        .iter()
        .flat_map(|group| group.packages.iter().copied())
        .collect();

    let untested: Vec<&String> = members
        .iter()
        .filter(|member| !tested.contains(&member.as_str()))
        .collect();
    if !untested.is_empty() {
        bail!(
            "workspace members with no `cargo xtask gate` test group: {}\n\
             Add them to `TEST_GROUPS` in xtask/src/gate.rs.",
            untested
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let stale: Vec<&&str> = tested
        .iter()
        .filter(|package| !members.iter().any(|member| member == *package))
        .collect();
    if !stale.is_empty() {
        bail!(
            "`TEST_GROUPS` names packages that are not workspace members: {}",
            stale
                .iter()
                .map(|package| **package)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let ci = std::fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .context("read .github/workflows/ci.yml")?;
    let test_lines: Vec<&str> = ci
        .lines()
        .filter(|line| line.contains("nextest") || line.contains("llvm-cov"))
        .collect();
    let missing: Vec<&String> = members
        .iter()
        .filter(|member| !test_lines.iter().any(|line| line.contains(member.as_str())))
        .collect();
    if !missing.is_empty() {
        bail!(
            "workspace members whose tests CI never runs: {}\n\
             `-p <package>` (or a `--manifest-path`) has to appear on a `nextest` or\n\
             `llvm-cov` line in .github/workflows/ci.yml.",
            missing
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    println!(
        "ci parity ok: {} workspace members, all selected here and on a test line in ci.yml",
        members.len()
    );
    Ok(())
}
