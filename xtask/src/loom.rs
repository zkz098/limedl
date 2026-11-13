//! Run Loom permutation tests with `.cargo/config.toml` flags preserved.
//!
//! Passing `RUSTFLAGS="--cfg limedl_loom"` directly replaces `[target.*]`
//! `rustflags` from `.cargo/config.toml`, which drops `--cfg reqwest_unstable`
//! and `target-cpu` settings. This runner merges the host target's configured
//! flags with `--cfg limedl_loom` into `CARGO_ENCODED_RUSTFLAGS` and compiles
//! into `target/loom` so the regular `target/debug` fingerprints stay intact.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Crates that carry `#[cfg(all(test, limedl_loom))]` concurrency suites.
const LOOM_PACKAGES: &[&str] = &["limedl-types", "limedl-disk", "limedl-http"];

pub fn run(root: &Path, clippy_only: bool, extra_args: &[String]) -> Result<()> {
    let host = detect_host_triple()?;
    let config_path = root.join(".cargo/config.toml");
    let config_text = fs::read_to_string(&config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    let encoded_flags = build_loom_encoded_rustflags(&config_text, &host);
    let target_dir = root.join("target/loom");

    run_cargo(
        root,
        &target_dir,
        &encoded_flags,
        &build_clippy_args(LOOM_PACKAGES),
        "clippy",
    )?;
    if clippy_only {
        return Ok(());
    }

    let mut test_args = build_test_args(LOOM_PACKAGES);
    if !extra_args.is_empty() {
        test_args.push("--".to_owned());
        test_args.extend(extra_args.iter().cloned());
    }
    run_cargo(root, &target_dir, &encoded_flags, &test_args, "test")
}

fn build_clippy_args(packages: &[&str]) -> Vec<String> {
    let mut args = vec!["clippy".to_owned()];
    for pkg in packages {
        args.push("-p".to_owned());
        args.push((*pkg).to_owned());
    }
    args.extend([
        "--lib".to_owned(),
        "--tests".to_owned(),
        "--".to_owned(),
        "-D".to_owned(),
        "warnings".to_owned(),
    ]);
    args
}

fn build_test_args(packages: &[&str]) -> Vec<String> {
    let mut args = vec!["test".to_owned()];
    for pkg in packages {
        args.push("-p".to_owned());
        args.push((*pkg).to_owned());
    }
    args.extend(["--lib".to_owned(), "loom_tests".to_owned()]);
    args
}

fn run_cargo(
    root: &Path,
    target_dir: &Path,
    encoded_flags: &str,
    subcommand_args: &[String],
    label: &str,
) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut cmd = Command::new(&cargo);
    cmd.current_dir(root)
        .env("CARGO_ENCODED_RUSTFLAGS", encoded_flags)
        .env_remove("RUSTFLAGS")
        .env("CARGO_TARGET_DIR", target_dir);
    if std::env::var_os("CARGO_BUILD_WARNINGS").is_none() {
        cmd.env("CARGO_BUILD_WARNINGS", "deny");
    }
    cmd.args(subcommand_args);

    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn cargo {label} under --cfg limedl_loom"))?;
    if !status.success() {
        bail!("cargo {label} under --cfg limedl_loom exited with {status}");
    }
    Ok(())
}

fn detect_host_triple() -> Result<String> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let output = Command::new(&rustc)
        .arg("-vV")
        .output()
        .context("run `rustc -vV` to detect host target")?;
    if !output.status.success() {
        bail!("`rustc -vV` exited with {}", output.status);
    }
    let stdout = String::from_utf8(output.stdout).context("parse `rustc -vV` as UTF-8")?;
    for line in stdout.lines() {
        if let Some(triple) = line.strip_prefix("host: ") {
            return Ok(triple.trim().to_owned());
        }
    }
    bail!("`rustc -vV` output did not include a `host:` line");
}

fn build_loom_encoded_rustflags(config_toml: &str, host_triple: &str) -> String {
    let mut flags = extract_target_rustflags(config_toml, host_triple);
    flags.push("--cfg".to_owned());
    flags.push("limedl_loom".to_owned());
    flags.join("\x1f")
}

/// Extract `[target.<host_triple>].rustflags` string entries from `.cargo/config.toml`.
fn extract_target_rustflags(config_toml: &str, host_triple: &str) -> Vec<String> {
    let header = format!("[target.{host_triple}]");
    let mut in_target = false;
    let mut collecting_array = false;
    let mut flags = Vec::new();

    for raw_line in config_toml.lines() {
        let line = strip_toml_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && !collecting_array {
            in_target = line == header;
            continue;
        }
        if !in_target {
            continue;
        }
        if let Some(rest) = line.strip_prefix("rustflags") {
            let Some(rhs) = rest.trim_start().strip_prefix('=') else {
                continue;
            };
            collecting_array = true;
            if collect_quoted_items(rhs, &mut flags) {
                break;
            }
            continue;
        }
        if collecting_array && collect_quoted_items(line, &mut flags) {
            break;
        }
    }

    flags
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            '#' if !in_quotes => return &line[..idx],
            _ => {}
        }
    }
    line
}

/// Append all double-quoted string literals on `line` to `out`; returns `true` when `]` closes the array.
fn collect_quoted_items(line: &str, out: &mut Vec<String>) -> bool {
    let mut chars = line.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if ch == ']' {
            return true;
        }
        if ch != '"' {
            continue;
        }
        let start = idx + 1;
        let mut end = start;
        for (q_idx, q_ch) in chars.by_ref() {
            if q_ch == '"' {
                end = q_idx;
                break;
            }
        }
        out.push(line[start..end].to_owned());
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_linux_and_windows_rustflags_from_repo_config() {
        let root = crate::repo_root().expect("repo root");
        let cfg = fs::read_to_string(root.join(".cargo/config.toml")).expect("read config.toml");

        assert_eq!(
            extract_target_rustflags(&cfg, "x86_64-unknown-linux-gnu"),
            vec!["-C", "target-cpu=x86-64-v3", "--cfg", "reqwest_unstable"]
        );
        assert_eq!(
            extract_target_rustflags(&cfg, "x86_64-pc-windows-msvc"),
            vec![
                "-C",
                "target-cpu=x86-64-v3",
                "-C",
                "linker=rust-lld",
                "--cfg",
                "reqwest_unstable",
            ]
        );
        assert_eq!(
            extract_target_rustflags(&cfg, "aarch64-apple-darwin"),
            vec![
                "-C",
                "target-cpu=apple-m1",
                "-A",
                "linker_messages",
                "--cfg",
                "reqwest_unstable",
            ]
        );
    }

    #[test]
    fn appends_limedl_loom_cfg_into_encoded_rustflags() {
        let cfg = r#"
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "target-cpu=x86-64-v3", "--cfg", "reqwest_unstable"]
"#;
        assert_eq!(
            build_loom_encoded_rustflags(cfg, "x86_64-unknown-linux-gnu"),
            "-C\x1ftarget-cpu=x86-64-v3\x1f--cfg\x1freqwest_unstable\x1f--cfg\x1flimedl_loom"
        );
    }
}
