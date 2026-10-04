//! Cross-platform icon generation for desktop packaging.
//!
//! Replaces `scripts/gen-msix-assets.ps1`, sips in `scripts/make-macos-icon.sh`,
//! and imagemagick/python fallbacks in `scripts/package-deb.sh` and
//! `scripts/package-appimage.sh`.
//!
//! Subcommands:
//! - `cargo xtask gen-icons msix --out-dir <dir>`
//! - `cargo xtask gen-icons hicolor --out-dir <dir>`
//! - `cargo xtask gen-icons macos-iconset --out-dir <dir>`

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Subcommand;
use image::imageops::FilterType;
use image::ImageReader;

#[derive(Subcommand, Debug, Clone)]
pub enum IconsCommand {
    /// Generate 13 tile/store assets for Windows MSIX Appx packaging.
    Msix {
        /// Target directory (e.g. packaging/msix/staging/Assets).
        #[arg(long)]
        out_dir: PathBuf,
        /// Source image (PNG format, high resolution recommended).
        #[arg(long, default_value = "crates/limedl-native/ui/assets/logo.png")]
        source: PathBuf,
    },
    /// Generate Linux hicolor icon hierarchy (<size>x<size>/apps/limedl-native.png).
    Hicolor {
        /// Target hicolor root directory (e.g. dist/usr/share/icons/hicolor).
        #[arg(long)]
        out_dir: PathBuf,
        /// Source image (PNG format).
        #[arg(long, default_value = "crates/limedl-native/ui/assets/icon.png")]
        source: PathBuf,
    },
    /// Generate 10 standard PNGs in an AppIcon.iconset directory for macOS iconutil.
    MacosIconset {
        /// Target iconset directory (e.g. AppIcon.iconset).
        #[arg(long)]
        out_dir: PathBuf,
        /// Source image (PNG format).
        #[arg(long, default_value = "crates/limedl-native/ui/assets/icon.png")]
        source: PathBuf,
    },
}

pub const MSIX_SPECS: &[(&str, u32)] = &[
    ("Square44x44Logo.targetsize-44", 44),
    ("Square44x44Logo.targetsize-66", 66),
    ("Square44x44Logo.targetsize-88", 88),
    ("Square44x44Logo.targetsize-176", 176),
    ("Square44x44Logo.scale-100", 44),
    ("Square44x44Logo.scale-125", 55),
    ("Square44x44Logo.scale-150", 66),
    ("Square44x44Logo.scale-200", 88),
    ("Square150x150Logo.scale-100", 150),
    ("Square150x150Logo.scale-125", 188),
    ("Square150x150Logo.scale-150", 225),
    ("Square150x150Logo.scale-200", 300),
    ("StoreLogo.scale-100", 50),
];

pub const HICOLOR_SIZES: &[u32] = &[16, 32, 48, 64, 128, 256, 512];

pub const MACOS_SPECS: &[(&str, u32)] = &[
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
];

pub fn run(cmd: IconsCommand) -> Result<()> {
    match cmd {
        IconsCommand::Msix { out_dir, source } => gen_msix(&source, &out_dir),
        IconsCommand::Hicolor { out_dir, source } => gen_hicolor(&source, &out_dir),
        IconsCommand::MacosIconset { out_dir, source } => gen_macos_iconset(&source, &out_dir),
    }
}

fn load_source(source: &Path) -> Result<image::DynamicImage> {
    ImageReader::open(source)
        .with_context(|| format!("open source image {}", source.display()))?
        .decode()
        .with_context(|| format!("decode image {}", source.display()))
}

fn resize_and_save(img: &image::DynamicImage, size: u32, out_path: &Path) -> Result<()> {
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let resized = image::imageops::resize(img, size, size, FilterType::Lanczos3);
    resized
        .save_with_format(out_path, image::ImageFormat::Png)
        .with_context(|| format!("save {}", out_path.display()))?;
    Ok(())
}

fn gen_msix(source: &Path, out_dir: &Path) -> Result<()> {
    fs::create_dir_all(out_dir).with_context(|| format!("create {}", out_dir.display()))?;
    let img = load_source(source)?;
    println!(
        "generating {} MSIX assets from {} ({}x{}) into {}...",
        MSIX_SPECS.len(),
        source.display(),
        img.width(),
        img.height(),
        out_dir.display()
    );

    for (name, size) in MSIX_SPECS {
        let out_path = out_dir.join(format!("{name}.png"));
        resize_and_save(&img, *size, &out_path)?;
        println!("  wrote {} ({}x{})", out_path.display(), size, size);
    }
    println!("MSIX assets generated successfully in {}", out_dir.display());
    Ok(())
}

fn gen_hicolor(source: &Path, out_dir: &Path) -> Result<()> {
    let img = load_source(source)?;
    println!(
        "generating Linux hicolor icons from {} into {}...",
        source.display(),
        out_dir.display()
    );

    for size in HICOLOR_SIZES {
        let sub_dir = out_dir.join(format!("{size}x{size}")).join("apps");
        let out_path = sub_dir.join("limedl-native.png");
        resize_and_save(&img, *size, &out_path)?;
        println!("  wrote {}", out_path.display());
    }
    println!("hicolor icons generated successfully in {}", out_dir.display());
    Ok(())
}

fn gen_macos_iconset(source: &Path, out_dir: &Path) -> Result<()> {
    fs::create_dir_all(out_dir).with_context(|| format!("create {}", out_dir.display()))?;
    let img = load_source(source)?;
    println!(
        "generating macOS AppIcon.iconset from {} into {}...",
        source.display(),
        out_dir.display()
    );

    for (name, size) in MACOS_SPECS {
        let out_path = out_dir.join(name);
        resize_and_save(&img, *size, &out_path)?;
        println!("  wrote {} ({}x{})", out_path.display(), size, size);
    }
    println!("macOS iconset generated successfully in {}", out_dir.display());
    Ok(())
}
