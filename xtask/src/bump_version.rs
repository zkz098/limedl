//! Bumps the workspace version everywhere the release process expects it.
//!
//! Migrated from `scripts/bump-version.ps1`. The files it edits and the git
//! choreography (commit → push `main` → tag → push tag) are unchanged; only the
//! implementation moved, so a version bump is reviewable in one place next to
//! the release manifest that consumes the tag (`cargo xtask manifest` uses
//! `v<version>` in every artifact URL).
//!
//! ```text
//! cargo xtask bump-version patch            # 0.4.1 -> 0.4.2, commit + tag + push
//! cargo xtask bump-version minor --dry-run  # print the plan, touch nothing
//! cargo xtask bump-version patch --no-push  # edit files, skip git entirely
//! ```

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use regex::Regex;

/// Website sources that embed the current version. Missing files are skipped
/// (the website is packaged separately and may not be checked out).
const WEBSITE_FILES: [&str; 6] = [
    "website/src/components/Header.astro",
    "website/src/components/OsDownloadButton.astro",
    "website/src/pages/download.astro",
    "website/src/pages/en/download.astro",
    "website/src/content/docs/getting-started/installation.md",
    "website/src/content/docs/en/getting-started/installation.md",
];

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Level {
    Patch,
    Minor,
    Major,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Patch => "patch",
            Level::Minor => "minor",
            Level::Major => "major",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub dry_run: bool,
    pub no_push: bool,
}

pub fn run(root: &Path, level: Level, opts: &Options) -> Result<()> {
    let cargo_path = root.join("Cargo.toml");
    let cargo = fs::read_to_string(&cargo_path)
        .with_context(|| format!("read {}", cargo_path.display()))?;

    let current = parse_version(&cargo)?;
    let new = next_version(&current, level)?;
    println!("{current} -> {new} ({})", level.as_str());

    if opts.dry_run {
        println!("[dry-run] Would update:");
        println!("  Cargo.toml : {current} -> {new}");
        println!("  Cargo.lock : {current} -> {new}");
        println!("  website    : {current} -> {new}");
        return Ok(());
    }

    // Cargo.toml — first `version = "x.y.z"` at the start of a line, which is
    // `[workspace.package] version` (workspace dependencies are table entries).
    let cargo_version = Regex::new(r#"(?m)^version\s*=\s*"[^"]+""#).expect("valid regex");
    let updated = cargo_version.replacen(&cargo, 1, format!(r#"version = "{new}""#));
    fs::write(&cargo_path, updated.as_bytes())
        .with_context(|| format!("write {}", cargo_path.display()))?;
    println!("  Updated: Cargo.toml");

    // Cargo.lock — only the workspace's own packages carry the version. The
    // suffix class is deliberately open (`limedl-*`) so a newly added workspace
    // crate cannot be forgotten, and the check below makes a miss fatal here
    // instead of in a tagged release whose `--locked` legs refuse to run.
    let lock_path = root.join("Cargo.lock");
    if lock_path.is_file() {
        let lock = fs::read_to_string(&lock_path)
            .with_context(|| format!("read {}", lock_path.display()))?;
        let limedl_version =
            Regex::new(r#"(?m)(name = "limedl(?:-[a-z]+)?"\r?\nversion = ")[^"]+(")"#)
                .expect("valid regex");
        let updated = limedl_version.replace_all(&lock, |caps: &regex::Captures<'_>| {
            format!("{}{new}{}", &caps[1], &caps[2])
        });
        let stale = stale_limedl_lock_entries(&updated, &new);
        if !stale.is_empty() {
            bail!(
                "Cargo.lock still lists {}; bump every workspace crate in the lock \
                 before tagging",
                stale.join(", ")
            );
        }
        fs::write(&lock_path, updated.as_bytes())
            .with_context(|| format!("write {}", lock_path.display()))?;
        println!("  Updated: Cargo.lock");
    }

    // website/package.json — first `"version": "x.y.z"`.
    let web_pkg_path = root.join("website/package.json");
    if web_pkg_path.is_file() {
        let web_pkg = fs::read_to_string(&web_pkg_path)
            .with_context(|| format!("read {}", web_pkg_path.display()))?;
        let pkg_version = Regex::new(r#""version":\s*"[^"]+""#).expect("valid regex");
        let updated = pkg_version.replacen(&web_pkg, 1, format!(r#""version": "{new}""#));
        fs::write(&web_pkg_path, updated.as_bytes())
            .with_context(|| format!("write {}", web_pkg_path.display()))?;
        println!("  Updated: website/package.json");
    }

    // Website pages/buttons link the release tag (`vX.Y.Z`); replacing the
    // v-prefixed form first is deliberate, so a bare replacement can never
    // leave a stale tag behind.
    for rel in WEBSITE_FILES {
        let path = root.join(rel);
        if !path.is_file() {
            continue;
        }
        let content = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let updated = content
            .replace(&format!("v{current}"), &format!("v{new}"))
            .replace(&current, &new);
        fs::write(&path, updated).with_context(|| format!("write {}", path.display()))?;
        println!("  Updated: {rel}");
    }

    if opts.no_push {
        return Ok(());
    }

    let tag = format!("v{new}");
    git(root, &["add", "Cargo.toml", "Cargo.lock", "website"])?;
    git(root, &["commit", "-m", &format!("chore: bump version to {new}")])?;
    git(root, &["push", "origin", "main"])?;
    git(root, &["tag", &tag, "-m", &tag])?;
    git(root, &["push", "origin", &tag])?;
    println!("Pushed commit + tag {tag}");
    Ok(())
}

fn parse_version(cargo: &str) -> Result<String> {
    let re = Regex::new(r#"(?m)^version\s*=\s*"(\d+\.\d+\.\d+)""#).expect("valid regex");
    let captures = re
        .captures(cargo)
        .context("Could not parse version from Cargo.toml")?;
    Ok(captures[1].to_string())
}

/// Workspace packages (`limedl*`, never `xtask`) whose lock entry is not at
/// `version`, formatted as `name (version)`.
fn stale_limedl_lock_entries(lock: &str, version: &str) -> Vec<String> {
    let entry = Regex::new(r#"(?m)^name = "(limedl[a-z-]*)"\r?\nversion = "([^"]+)""#)
        .expect("valid regex");
    entry
        .captures_iter(lock)
        .filter(|captures| &captures[2] != version)
        .map(|captures| format!("{} ({})", &captures[1], &captures[2]))
        .collect()
}

fn next_version(current: &str, level: Level) -> Result<String> {
    let parts: Vec<&str> = current.split('.').collect();
    if parts.len() != 3 {
        bail!("version {current} is not major.minor.patch");
    }
    let mut numbers = [0u32; 3];
    for (slot, part) in numbers.iter_mut().zip(&parts) {
        *slot = part
            .parse()
            .with_context(|| format!("version component {part:?} is not a number"))?;
    }
    match level {
        Level::Major => {
            numbers[0] += 1;
            numbers[1] = 0;
            numbers[2] = 0;
        }
        Level::Minor => {
            numbers[1] += 1;
            numbers[2] = 0;
        }
        Level::Patch => numbers[2] += 1,
    }
    Ok(format!("{}.{}.{}", numbers[0], numbers[1], numbers[2]))
}

fn git(root: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if !status.success() {
        bail!("git {} failed ({status})", args.join(" "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "limedl-xtask-bump-{tag}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A miniature repo with every file the bump touches.
    fn fixture() -> std::path::PathBuf {
        let root = temp_dir("fixture");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace.package]\nversion = \"0.4.1\"\n\n[workspace.dependencies]\ntokio = { version = \"1\" }\n",
        )
        .unwrap();
        fs::write(
            root.join("Cargo.lock"),
            "[[package]]\nname = \"limedl-core\"\nversion = \"0.4.1\"\n\n\
             [[package]]\nname = \"limedl-native\"\nversion = \"0.4.1\"\n\n\
             [[package]]\nname = \"limedl-server\"\nversion = \"0.4.1\"\n\n\
             [[package]]\nname = \"xtask\"\nversion = \"0.0.0\"\n\n\
             [[package]]\nname = \"serde\"\nversion = \"0.4.1\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("website")).unwrap();
        fs::write(root.join("website/package.json"), "{\n  \"version\": \"0.4.1\"\n}\n").unwrap();
        for rel in WEBSITE_FILES {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "download v0.4.1 now, version 0.4.1 is current\n").unwrap();
        }
        root
    }

    #[test]
    fn bumps_each_level() {
        assert_eq!(next_version("0.4.1", Level::Patch).unwrap(), "0.4.2");
        assert_eq!(next_version("0.4.1", Level::Minor).unwrap(), "0.5.0");
        assert_eq!(next_version("0.4.1", Level::Major).unwrap(), "1.0.0");
        assert!(next_version("0.4", Level::Patch).is_err());
        assert!(next_version("0.4.x", Level::Patch).is_err());
    }

    #[test]
    fn no_push_edits_every_target() {
        let root = fixture();
        run(&root, Level::Minor, &Options { dry_run: false, no_push: true }).unwrap();

        let cargo = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("version = \"0.5.0\""));
        // The dependency entry keeps its own version.
        assert!(cargo.contains("tokio = { version = \"1\" }"), "{cargo}");

        let lock = fs::read_to_string(root.join("Cargo.lock")).unwrap();
        assert!(lock.contains("name = \"limedl-core\"\nversion = \"0.5.0\""));
        assert!(lock.contains("name = \"limedl-native\"\nversion = \"0.5.0\""));
        assert!(lock.contains("name = \"limedl-server\"\nversion = \"0.5.0\""));
        // A non-workspace package with a coincidentally equal version is untouched.
        assert!(lock.contains("name = \"serde\"\nversion = \"0.4.1\""), "{lock}");
        // `xtask` pins its own literal version, so it is never rewritten.
        assert!(lock.contains("name = \"xtask\"\nversion = \"0.0.0\""), "{lock}");

        let pkg = fs::read_to_string(root.join("website/package.json")).unwrap();
        assert!(pkg.contains("\"version\": \"0.5.0\""));

        for rel in WEBSITE_FILES {
            let text = fs::read_to_string(root.join(rel)).unwrap();
            assert_eq!(text, "download v0.5.0 now, version 0.5.0 is current\n", "{rel}");
        }
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn stale_lock_entries_are_reported() {
        let lock = "[[package]]\nname = \"limedl-core\"\nversion = \"0.5.0\"\n\n\
                    [[package]]\nname = \"limedl-server\"\nversion = \"0.4.1\"\n\n\
                    [[package]]\nname = \"xtask\"\nversion = \"0.0.0\"\n";
        assert_eq!(
            stale_limedl_lock_entries(lock, "0.5.0"),
            vec!["limedl-server (0.4.1)"]
        );
        assert!(stale_limedl_lock_entries(lock, "0.4.1").contains(&"limedl-core (0.5.0)".to_string()));
    }

    /// A workspace crate the rewrite regex cannot see must fail the bump instead of
    /// leaving a lagging `Cargo.lock` behind for a tagged `--locked` release leg.
    #[test]
    fn refuses_to_bump_when_the_lock_still_lags() {
        let root = fixture();
        let lock = fs::read_to_string(root.join("Cargo.lock")).unwrap();
        fs::write(
            root.join("Cargo.lock"),
            format!("{lock}[[package]]\nname = \"limedl-extra-crate\"\nversion = \"0.4.1\"\n"),
        )
        .unwrap();

        let error = run(&root, Level::Minor, &Options { dry_run: false, no_push: true })
            .expect_err("a lagging workspace lock entry must fail the bump");
        assert!(error.to_string().contains("limedl-extra-crate"), "{error}");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn dry_run_touches_nothing() {
        let root = fixture();
        run(&root, Level::Major, &Options { dry_run: true, no_push: true }).unwrap();
        let cargo = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("version = \"0.4.1\""));
        fs::remove_dir_all(&root).ok();
    }
}
