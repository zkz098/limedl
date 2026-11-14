//! Repository tooling for limedl.
//!
//! ```text
//! cargo xtask fetch-font [--verify]          # pinned MiSans VF (build prerequisite)
//! cargo xtask bump-version <patch|minor|major>
//! cargo xtask theme generate
//! cargo xtask gen-icons [msix|hicolor|macos-iconset] --out-dir <dir>
//! cargo xtask loom
//! ```

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

mod bump_version;
mod fetch_font;
mod icons;
mod loom;
mod theme;

#[derive(Parser)]
#[command(
    name = "xtask",
    about = "limedl repository tooling (font fetch, version bump, theme, icons, loom)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fetch the pinned MiSans VF font into `crates/limedl-native/assets/fonts/`.
    FetchFont {
        /// Re-download even when the local copy already matches the pinned
        /// size/hash.
        #[arg(long)]
        force: bool,
        /// Check the local copy only; never touch the network. The CI jobs that
        /// receive the font as an artifact run this.
        #[arg(long)]
        verify: bool,
        /// Take the extracted `MiSansVF.ttf` from this local file or URL
        /// instead of Xiaomi's CDN (the pinned hash is still enforced).
        /// Defaults to `$LIMEDL_MISANS_TTF`.
        #[arg(long)]
        from_path: Option<String>,
        /// Alternate URL for the `MiSans.zip` archive (a mirror).
        #[arg(long, default_value = fetch_font::DEFAULT_ZIP_URL)]
        zip_url: String,
    },
    /// Bump the workspace version in Cargo.toml, Cargo.lock and website/.
    BumpVersion {
        /// Which version component to increment.
        #[arg(value_enum)]
        level: bump_version::Level,
        /// Edit the files but skip commit/tag/push.
        #[arg(long)]
        no_push: bool,
        /// Print what would change and exit without touching any file.
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate `theme.slint` with 5-hue ThemeAccent ramps (`check`/`apply` live in `cargo-warden`).
    #[command(subcommand)]
    Theme(theme::ThemeCommand),
    /// Generate desktop packaging icons (MSIX, Linux hicolor, macOS iconset).
    #[command(subcommand)]
    GenIcons(icons::IconsCommand),
    /// Run `clippy` and Loom permutation tests under `--cfg limedl_loom`.
    Loom {
        /// Run `cargo clippy` under `--cfg limedl_loom` without executing tests.
        #[arg(long)]
        clippy_only: bool,
        /// Extra arguments forwarded to `cargo test -- ...` (e.g. `--nocapture`).
        #[arg(last = true)]
        test_args: Vec<String>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::FetchFont {
            force,
            verify,
            from_path,
            zip_url,
        } => fetch_font::run(
            &repo_root()?,
            &fetch_font::Options {
                force,
                verify,
                from_path,
                zip_url,
            },
        ),
        Command::BumpVersion {
            level,
            no_push,
            dry_run,
        } => bump_version::run(
            &repo_root()?,
            level,
            &bump_version::Options { dry_run, no_push },
        ),
        Command::Theme(cmd) => theme::run(cmd),
        Command::GenIcons(cmd) => icons::run(cmd),
        Command::Loom {
            clippy_only,
            test_args,
        } => loom::run(&repo_root()?, clippy_only, &test_args),
    }
}

/// The workspace root: `xtask` lives one level below it.
fn repo_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask sits one level below the repo root")
}

/// Lowercase hex SHA-256 — used by the pinned font hash check.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Writing into a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}
