//! Slint color theme generator and token rewriter.
//!
//! Replaces `scripts/theme-tokens.ps1`, `scripts/generate-theme-slint.ps1`, and
//! `scripts/apply-theme-tokens.ps1`:
//! - `cargo xtask theme generate`: emits `crates/limedl-native/ui/theme.slint`
//! - `cargo xtask theme apply`: rewrites hex literals in `ui/*.slint` to `Theme.c<hex>`
//! - `cargo xtask theme check`: scans for unmapped hex literals across UI components

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use regex::Regex;

#[derive(Subcommand, Debug, Clone)]
pub enum ThemeCommand {
    /// Generate crates/limedl-native/ui/theme.slint from the color tables.
    Generate {
        /// Where to write the generated Slint theme.
        #[arg(long, default_value = "crates/limedl-native/ui/theme.slint")]
        out_file: PathBuf,
    },
    /// Rewrite hardcoded hex literals in ui/*.slint to Theme.c<hex> references.
    Apply {
        /// UI source directory.
        #[arg(long, default_value = "crates/limedl-native/ui")]
        ui_dir: PathBuf,
    },
    /// Scan ui/*.slint for unmapped hex literals.
    Check {
        /// UI source directory.
        #[arg(long, default_value = "crates/limedl-native/ui")]
        ui_dir: PathBuf,
    },
}

/// Identical in both schemes: rewriting it would only add indirection.
pub const KEEP_LITERAL: &[&str] = &["6b7280"];

/// Colors with identical values in both schemes stay as literals in the UI.
pub const UNCHANGED: &[&str] = &[
    "ffffff", "dc2626", "b91c1c", "fecaca", "fef2f2", "0b1104", "6b7280",
];

/// Static dark -> light mappings.
pub const STATIC_MAP: &[(&str, &str)] = &[
    // window / panel / card surfaces
    ("0d0f12", "f4f5f7"),
    ("12151a", "ffffff"),
    ("181b22", "ffffff"),
    ("111419", "f8f9fa"),
    ("111418", "f8f9fa"),
    ("0b0c0f", "e9ebef"),
    ("13161b", "f2f3f5"),
    ("14161b", "f0f1f4"),
    ("14171d", "eff1f4"),
    ("141922", "eef0f4"),
    ("15171c", "eff1f4"),
    ("15181d", "eff1f4"),
    ("15181e", "eff1f4"),
    ("15181f", "eff1f4"),
    ("151921", "eef0f3"),
    ("171a20", "f0f1f4"),
    ("171a21", "eff1f4"),
    ("171b21", "eef0f3"),
    ("191c22", "eff1f4"),
    ("1a1d23", "eceef2"),
    ("1a1e25", "eceef2"),
    ("1a1e26", "eceef1"),
    ("1d212a", "e9ecf0"),
    ("1e2229", "e9ecf0"),
    ("1f242e", "e8ebf0"),
    ("20242c", "e7eaef"),
    ("20242d", "e7eaef"),
    ("20252e", "e7eaef"),
    ("22262f", "e6e9ee"),
    ("222731", "e7eaef"),
    ("232832", "e6e9ee"),
    ("232a38", "e6e9ee"),
    ("242932", "e6e9ee"),
    ("242934", "e6e9ee"),
    ("252a34", "e5e8ee"),
    ("252b34", "e5e8ee"),
    ("252b36", "e5e8ee"),
    ("262a31", "e5e8ed"),
    ("262c37", "e4e7ed"),
    ("272d38", "dfe3ea"),
    ("272f3e", "dfe3ea"),
    ("2a2f3a", "dfe2e9"),
    ("2b313d", "dee2e9"),
    ("2b3342", "dfe3ec"),
    ("2e3542", "d9dde5"),
    ("323844", "d5dae3"),
    ("323845", "d5dae3"),
    ("343c4a", "d2d8e1"),
    ("3b4352", "cdd4df"),
    ("3f4756", "c9d0da"),
    ("202632", "e4e7ed"),
    ("181d26", "eef0f4"),
    // text
    ("f3f4f6", "1f2329"),
    ("d1d5db", "374151"),
    ("9ca3af", "6b7280"),
    ("6b7280", "6b7280"),
    ("8b95a5", "7c8698"),
    ("5d6778", "8a93a3"),
    ("4b5563", "9ca3af"),
    ("626c7d", "8a93a3"),
    ("e5e7eb", "374151"),
    ("ffffff", "1f2329"),
    // info / blue (semantic, not brand accent)
    ("38bdf8", "0284c7"),
    ("60a5fa", "2563eb"),
    ("112842", "dbeafe"),
    ("1d406b", "bfdbfe"),
    ("132a42", "e0f2fe"),
    ("255078", "93c5fd"),
    ("152c42", "e0f2fe"),
    // success / status green (semantic, not brand accent)
    ("4ade80", "16a34a"),
    ("22c55e", "16a34a"),
    ("86efac", "15803d"),
    ("142e1b", "dcfce7"),
    ("14351a", "dcfce7"),
    ("153920", "dcfce7"),
    ("22542e", "bbf7d0"),
    ("276238", "15803d"),
    // danger / red
    ("ef4444", "dc2626"),
    ("f87171", "dc2626"),
    ("991b1b", "b91c1c"),
    ("7f1d1d", "b91c1c"),
    ("201315", "fef2f2"),
    ("361214", "fee2e2"),
    ("3f1518", "fee2e2"),
    ("3b1216", "fee2e2"),
    ("5c1f23", "fecaca"),
    ("351417", "fee2e2"),
    ("fca5a5", "f87171"),
    // warning / amber (semantic)
    ("f59e0b", "d97706"),
    ("facc15", "eab308"),
    ("fcd34d", "fbbf24"),
    ("fbbf24", "d97706"),
    ("351d0b", "fef3c7"),
    ("3b280c", "fef3c7"),
    ("543615", "fde68a"),
    ("33200b", "fde68a"),
    ("2a1d08", "fef9c3"),
    // pink (overclock accents)
    ("ec4899", "db2777"),
    ("f472b6", "db2777"),
    ("31132b", "fce7f3"),
    // violet (semantic / swatch)
    ("8b5cf6", "7c3aed"),
    // overlays
    ("000000bb", "00000066"),
    ("000000cc", "00000066"),
];

pub struct AccentColor {
    pub lime: &'static str,
    pub amber: &'static str,
    pub sky: &'static str,
    pub violet: &'static str,
    pub monochrome: &'static str,
}

pub struct AccentEntry {
    pub key: &'static str,
    pub dark: AccentColor,
    pub light: AccentColor,
}

pub const ACCENT_MAP: &[AccentEntry] = &[
    AccentEntry {
        key: "84cc16",
        dark: AccentColor { lime: "84cc16", amber: "f59e0b", sky: "0ea5e9", violet: "8b5cf6", monochrome: "ffffff" },
        light: AccentColor { lime: "65a30d", amber: "d97706", sky: "0284c7", violet: "7c3aed", monochrome: "000000" },
    },
    AccentEntry {
        key: "a3e635",
        dark: AccentColor { lime: "a3e635", amber: "fbbf24", sky: "38bdf8", violet: "a78bfa", monochrome: "e5e5e5" },
        light: AccentColor { lime: "84cc16", amber: "f59e0b", sky: "0ea5e9", violet: "8b5cf6", monochrome: "27272a" },
    },
    AccentEntry {
        key: "65a30d",
        dark: AccentColor { lime: "65a30d", amber: "d97706", sky: "0284c7", violet: "7c3aed", monochrome: "d4d4d8" },
        light: AccentColor { lime: "4d7c0f", amber: "b45309", sky: "0369a1", violet: "6d28d9", monochrome: "3f3f46" },
    },
    AccentEntry {
        key: "365314",
        dark: AccentColor { lime: "365314", amber: "78350f", sky: "0c4a6e", violet: "4c1d95", monochrome: "3f3f46" },
        light: AccentColor { lime: "1a2e05", amber: "713f12", sky: "0c4a6e", violet: "2e1065", monochrome: "d4d4d8" },
    },
    AccentEntry {
        key: "26331a",
        dark: AccentColor { lime: "26331a", amber: "451a03", sky: "082f49", violet: "2e1065", monochrome: "27272a" },
        light: AccentColor { lime: "ecfccb", amber: "fef3c7", sky: "e0f2fe", violet: "ede9fe", monochrome: "f4f4f5" },
    },
    AccentEntry {
        key: "232b1d",
        dark: AccentColor { lime: "232b1d", amber: "3b280c", sky: "0c4a6e", violet: "26153b", monochrome: "27272a" },
        light: AccentColor { lime: "f0f7e2", amber: "fef3c7", sky: "e0f2fe", violet: "f5f3ff", monochrome: "f4f4f5" },
    },
    AccentEntry {
        key: "2a3322",
        dark: AccentColor { lime: "2a3322", amber: "422006", sky: "075985", violet: "3b1c66", monochrome: "27272a" },
        light: AccentColor { lime: "eaf6d9", amber: "fde68a", sky: "bae6fd", violet: "ddd6fe", monochrome: "e4e4e7" },
    },
    AccentEntry {
        key: "1c2618",
        dark: AccentColor { lime: "1c2618", amber: "451a03", sky: "082f49", violet: "221236", monochrome: "1c1c1f" },
        light: AccentColor { lime: "eaf6d9", amber: "fef3c7", sky: "e0f2fe", violet: "ede9fe", monochrome: "f0f0f2" },
    },
    AccentEntry {
        key: "4d5c41",
        dark: AccentColor { lime: "4d5c41", amber: "92400e", sky: "0369a1", violet: "6d28d9", monochrome: "71717a" },
        light: AccentColor { lime: "9db876", amber: "d97706", sky: "0284c7", violet: "8b5cf6", monochrome: "a1a1aa" },
    },
    AccentEntry {
        key: "0b1104",
        dark: AccentColor { lime: "0b1104", amber: "0b1104", sky: "0b1104", violet: "ffffff", monochrome: "000000" },
        light: AccentColor { lime: "0b1104", amber: "0b1104", sky: "0b1104", violet: "ffffff", monochrome: "ffffff" },
    },
];

/// Emit the full Slint markup for `ui/theme.slint`.
pub fn generate_theme_slint() -> String {
    let mut out = String::new();
    out.push_str(
        r#"// Auto-generated color theme for the native UI. See xtask/src/theme.rs
// for the source mapping tables (static dark -> light values + brand accent ramps).
//
// Usage rules:
//  - 'mode' is set from Rust (settings.appearance.color_mode).
//  - 'accent' is set from Rust (settings.appearance.theme_color).
//  - 'dark' resolves the effective scheme: explicit choice, or the OS scheme
//    in system mode (via the std-widgets Palette global).
//  - Property names are the DARK hex values (c<hex>) so every call site maps
//    1:1 back to the literal it replaced. Do not introduce new hardcoded
//    hex colors in components; add a token here instead. Brand-accent tokens
//    (buttons/selection/brand text) follow 'accent'; status colors
//    (success/warning/danger/info) intentionally do not.
import { Palette } from "std-widgets.slint";

export enum ColorModePref { System, Light, Dark }

export enum ThemeAccent { Lime, Amber, Sky, Violet, Monochrome }

export global Theme {
    /// User preference from settings (system/light/dark).
    in-out property <ColorModePref> mode: ColorModePref.System;

    /// Brand accent from settings (lime/amber/sky/violet/monochrome).
    in-out property <ThemeAccent> accent: ThemeAccent.Lime;

    /// Effective dark flag used by every color token below.
    // Note: builtin ColorScheme enum uses lowercase variants.
    in-out property <bool> dark: mode == ColorModePref.Dark
        || (mode == ColorModePref.System && Palette.color-scheme == ColorScheme.dark);

"#,
    );

    for (hex, light) in STATIC_MAP {
        out.push_str(&format!("    /// dark #{hex} / light #{light}\n"));
        out.push_str(&format!("    in property <color> c{hex}: dark ? #{hex} : #{light};\n"));
    }

    for entry in ACCENT_MAP {
        let hex = entry.key;
        let dark_expr = format!(
            "accent == ThemeAccent.Amber ? #{} : accent == ThemeAccent.Sky ? #{} : accent == ThemeAccent.Violet ? #{} : accent == ThemeAccent.Monochrome ? #{} : #{}",
            entry.dark.amber, entry.dark.sky, entry.dark.violet, entry.dark.monochrome, entry.dark.lime
        );
        let light_expr = format!(
            "accent == ThemeAccent.Amber ? #{} : accent == ThemeAccent.Sky ? #{} : accent == ThemeAccent.Violet ? #{} : accent == ThemeAccent.Monochrome ? #{} : #{}",
            entry.light.amber, entry.light.sky, entry.light.violet, entry.light.monochrome, entry.light.lime
        );
        out.push_str(&format!("    /// brand accent (was dark #{hex}) — follows theme_color\n"));
        out.push_str(&format!("    in property <color> c{hex}: dark ? ({dark_expr}) : ({light_expr});\n"));
    }

    out.push_str("}\n");
    out
}

pub fn run(cmd: ThemeCommand) -> Result<()> {
    match cmd {
        ThemeCommand::Generate { out_file } => {
            let content = generate_theme_slint();
            if let Some(parent) = out_file.parent() {
                fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
            }
            fs::write(&out_file, content).with_context(|| format!("write {}", out_file.display()))?;
            println!("wrote Slint theme -> {}", out_file.display());
            Ok(())
        }
        ThemeCommand::Apply { ui_dir } => apply_tokens(&ui_dir),
        ThemeCommand::Check { ui_dir } => {
            let unmapped = scan_unmapped(&ui_dir)?;
            if unmapped.is_empty() {
                println!("all hex colors in {} are mapped", ui_dir.display());
                Ok(())
            } else {
                println!("unmapped hex colors found ({}):", unmapped.len());
                for (color, files) in &unmapped {
                    println!("  #{color} in: {}", files.join(", "));
                }
                bail!("unmapped colors remain in UI components");
            }
        }
    }
}

/// Recursively find all `.slint` files under `dir`, excluding `theme.slint`.
fn find_slint_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(find_slint_files(&path)?);
        } else if path.extension().is_some_and(|ext| ext == "slint")
            && path.file_name().is_some_and(|n| n != "theme.slint")
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Apply tokens across `.slint` files in `ui_dir`.
fn apply_tokens(ui_dir: &Path) -> Result<()> {
    let key_map = build_key_map();
    let token_re = Regex::new(r"#([0-9a-fA-F]{3,8})\b")?;
    let slint_files = find_slint_files(ui_dir)?;
    let mut updated = 0;

    for file in &slint_files {
        let text = fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
        let mut result = replace_tokens(&text, &token_re, &key_map);

        if result != text {
            // Ensure `import { Theme }` is present
            if !result.contains("import { Theme") && !result.contains("import {Theme") {
                result = insert_import(&result, &theme_import(file));
            }

            fs::write(file, &result).with_context(|| format!("write {}", file.display()))?;
            println!("updated {}", file.display());
            updated += 1;
        }
    }

    println!("updated {updated} Slint file(s)");
    report_unmapped(ui_dir)?;

    Ok(())
}

/// The `Theme.c<hex>` replacement for every color token `keep_set` does not keep.
fn build_key_map() -> BTreeMap<String, String> {
    let keep_set: HashSet<&str> = KEEP_LITERAL.iter().copied().collect();
    let mut key_map: BTreeMap<String, String> = BTreeMap::new();

    for (k, _) in STATIC_MAP {
        if !keep_set.contains(k) {
            key_map.insert(k.to_ascii_lowercase(), format!("Theme.c{k}"));
        }
    }
    for e in ACCENT_MAP {
        if !keep_set.contains(e.key) {
            key_map.insert(e.key.to_ascii_lowercase(), format!("Theme.c{}", e.key));
        }
    }

    key_map
}

/// Rewrite every hex literal in `text` that `key_map` knows about.
fn replace_tokens(text: &str, token_re: &Regex, key_map: &BTreeMap<String, String>) -> String {
    token_re
        .replace_all(text, |caps: &regex::Captures| {
            let hex = caps[1].to_ascii_lowercase();
            match key_map.get(&hex) {
                Some(replacement) => replacement.clone(),
                None => caps[0].to_string(),
            }
        })
        .into_owned()
}

/// The `import { Theme }` statement a file in `file`'s directory needs.
fn theme_import(file: &Path) -> String {
    let is_component = file
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|n| n == "components");
    let rel = if is_component { "../theme.slint" } else { "theme.slint" };
    format!("import {{ Theme }} from \"{rel}\";")
}

/// Insert `import_stmt` right after the last top-level import of `text`.
fn insert_import(text: &str, import_stmt: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut last_import_idx = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("import ") {
            last_import_idx = Some(i);
        } else if last_import_idx.is_some() && !trimmed.is_empty() && !trimmed.starts_with("//") {
            // Stopped seeing imports
            break;
        }
    }

    let mut new_lines = Vec::with_capacity(lines.len() + 1);
    match last_import_idx {
        Some(idx) => {
            for (i, line) in lines.into_iter().enumerate() {
                new_lines.push(line.to_string());
                if i == idx {
                    new_lines.push(import_stmt.to_string());
                }
            }
        }
        None => {
            new_lines.push(import_stmt.to_string());
            new_lines.extend(lines.into_iter().map(String::from));
        }
    }

    let mut result = new_lines.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Print the hex literals that `apply` could not map, for a human to triage.
fn report_unmapped(ui_dir: &Path) -> Result<()> {
    let unmapped = scan_unmapped(ui_dir)?;
    if !unmapped.is_empty() {
        println!("\nUNMAPPED colors remaining ({}):", unmapped.len());
        for (color, files) in &unmapped {
            println!("  #{color} in: {}", files.join(", "));
        }
    } else {
        println!("no unmapped colors remaining");
    }
    Ok(())
}

/// Scan `ui_dir` for hex literals not in `UNCHANGED`.
pub fn scan_unmapped(ui_dir: &Path) -> Result<BTreeMap<String, Vec<String>>> {
    let unchanged_set: HashSet<&str> = UNCHANGED.iter().copied().collect();
    let re = Regex::new(r"#([0-9a-fA-F]{3,8})\b")?;
    let slint_files = find_slint_files(ui_dir)?;
    let mut unmapped: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for file in &slint_files {
        let file_name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.display().to_string());
        let text = fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;

        for cap in re.captures_iter(&text) {
            let hex = cap[1].to_ascii_lowercase();
            if (hex.len() == 6 || hex.len() == 8) && !unchanged_set.contains(hex.as_str()) {
                let entry = unmapped.entry(hex).or_default();
                if !entry.contains(&file_name) {
                    entry.push(file_name.clone());
                }
            }
        }
    }

    Ok(unmapped)
}
