//! Rust-side translation catalog generation.
//!
//! `cargo xtask i18n generate` reads the gettext catalogs under
//! `crates/limedl-native/lang/<locale>/LC_MESSAGES/limedl-native.po` and writes
//! `crates/limedl-native/src/i18n/catalog.rs`, the lookup table behind
//! `i18n::tr` / `i18n::tr_fmt`.
//!
//! Both halves of the desktop UI share those catalogs: the Slint side goes
//! through `@tr` (slint-build bundles the same `.po` files) and the Rust side
//! through the generated table. A translator therefore edits one file per
//! locale, and a message used from both sides is translated once — which is what
//! `format_toast_update_ready` and the About tab's update row now do.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Subcommand;

#[derive(Subcommand, Debug, Clone)]
pub enum I18nCommand {
    /// Regenerate `src/i18n/catalog.rs` from the `.po` catalogs.
    Generate {
        /// Directory holding `<locale>/LC_MESSAGES/limedl-native.po`.
        #[arg(long, default_value = "crates/limedl-native/lang")]
        lang_dir: PathBuf,
        /// Where to write the generated Rust catalog.
        #[arg(long, default_value = "crates/limedl-native/src/i18n/catalog.rs")]
        out_file: PathBuf,
    },
    /// Fail when `src/i18n/catalog.rs` is out of sync with the `.po` catalogs.
    Check {
        /// Directory holding `<locale>/LC_MESSAGES/limedl-native.po`.
        #[arg(long, default_value = "crates/limedl-native/lang")]
        lang_dir: PathBuf,
        /// The generated catalog to compare against.
        #[arg(long, default_value = "crates/limedl-native/src/i18n/catalog.rs")]
        out_file: PathBuf,
    },
}

pub fn run(cmd: I18nCommand) -> Result<()> {
    match cmd {
        I18nCommand::Generate { lang_dir, out_file } => {
            let content = generate_catalog(&lang_dir)?;
            if let Some(parent) = out_file.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
            fs::write(&out_file, content)
                .with_context(|| format!("write {}", out_file.display()))?;
            println!("wrote Rust translation catalog -> {}", out_file.display());
            Ok(())
        }
        I18nCommand::Check { lang_dir, out_file } => {
            let generated = generate_catalog(&lang_dir)?;
            let on_disk = fs::read_to_string(&out_file)
                .with_context(|| format!("read {}", out_file.display()))?;
            if normalize(&generated) == normalize(&on_disk) {
                println!("{} is up to date", out_file.display());
                Ok(())
            } else {
                bail!(
                    "{} is out of sync with the .po catalogs; run `cargo xtask i18n generate`",
                    out_file.display()
                )
            }
        }
    }
}

/// Locale order of the generated `translations` array.
///
/// Must match `limedl_native::i18n::Language::catalog_index`; the
/// `catalog_columns_follow_language_order` test in the native crate is what
/// enforces it.
const LOCALES: [&str; 3] = ["zh_CN", "zh_TW", "en"];

/// Line-ending-insensitive comparison, so a CRLF checkout does not fail `check`.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Build the whole generated `catalog.rs` from the catalogs under `lang_dir`.
pub fn generate_catalog(lang_dir: &Path) -> Result<String> {
    let catalogs: Vec<BTreeMap<String, String>> = LOCALES
        .iter()
        .map(|locale| {
            let path = lang_dir
                .join(locale)
                .join("LC_MESSAGES")
                .join("limedl-native.po");
            parse_po(&path)
        })
        .collect::<Result<_>>()?;

    // The union of every msgid, so an entry that is missing from one catalog
    // still resolves (to the English source) instead of breaking the lookup.
    // A `BTreeMap` iterates in sorted order, which is what `tr`'s binary search
    // relies on.
    let mut msgids: BTreeMap<&str, ()> = BTreeMap::new();
    for catalog in &catalogs {
        for msgid in catalog.keys() {
            msgids.insert(msgid.as_str(), ());
        }
    }

    let mut out = String::from(HEADER);
    for msgid in msgids.keys() {
        // A missing or empty translation falls back to the English source, which
        // is the msgid itself — the same fallback gettext has.
        let mut translations = [*msgid; 3];
        for (index, catalog) in catalogs.iter().enumerate() {
            if let Some(value) = catalog.get(*msgid).filter(|value| !value.is_empty()) {
                translations[index] = value.as_str();
            }
        }
        out.push_str(&format!(
            "    Entry {{ msgid: {}, translations: [{}, {}, {}] }},\n",
            rust_string(msgid),
            rust_string(translations[0]),
            rust_string(translations[1]),
            rust_string(translations[2]),
        ));
    }
    out.push_str("];\n");
    Ok(out)
}

const HEADER: &str = r#"//! Generated by `cargo xtask i18n generate` — do not edit by hand.
//!
//! Mirrors `crates/limedl-native/lang/<locale>/LC_MESSAGES/limedl-native.po`.
//! The `translations` array is indexed by `Language::catalog_index`, and the
//! entries are sorted by `msgid` for `i18n::tr`'s binary search. `cargo xtask
//! i18n check` and the `i18n_catalog_matches_repo_catalogs` test fail when this
//! file is stale, which is the "do not edit by hand" enforcement.

/// One catalog entry: the English source string and its translations.
pub(crate) struct Entry {
    pub(crate) msgid: &'static str,
    pub(crate) translations: [&'static str; 3],
}

/// Every message in the bundled catalogs, sorted by `msgid`.
#[rustfmt::skip]
pub(crate) static CATALOG: &[Entry] = &[
"#;

/// Parse a `.po` catalog into `msgid -> msgstr`.
///
/// Deliberately minimal: the catalogs have no `msgctxt`/`msgid_plural` entries
/// (asserted below), so only the two keywords, their continuation lines and the
/// standard C escapes are handled.
fn parse_po(path: &Path) -> Result<BTreeMap<String, String>> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut entries = BTreeMap::new();
    let mut msgid: Option<String> = None;
    let mut msgstr: Option<String> = None;
    let mut field = Field::None;

    fn flush(
        entries: &mut BTreeMap<String, String>,
        msgid: &mut Option<String>,
        msgstr: &mut Option<String>,
    ) {
        if let (Some(id), Some(value)) = (msgid.take(), msgstr.take())
            && !id.is_empty()
        {
            entries.insert(id, value);
        }
    }

    for line in content.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            flush(&mut entries, &mut msgid, &mut msgstr);
            field = Field::None;
            continue;
        }
        if let Some(rest) = line.strip_prefix("msgid ") {
            // A new msgid terminates the previous entry.
            flush(&mut entries, &mut msgid, &mut msgstr);
            msgid = Some(po_string(rest, path)?);
            field = Field::Msgid;
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            msgstr = Some(po_string(rest, path)?);
            field = Field::Msgstr;
        } else if line.starts_with('"') {
            // Continuation of the current keyword.
            let text = po_string(line, path)?;
            match field {
                Field::Msgid => {
                    if let Some(id) = msgid.as_mut() {
                        id.push_str(&text);
                    }
                }
                Field::Msgstr => {
                    if let Some(value) = msgstr.as_mut() {
                        value.push_str(&text);
                    }
                }
                Field::None => {}
            }
        } else if line.starts_with("msgctxt") || line.starts_with("msgid_plural") {
            bail!(
                "{}: msgctxt/msgid_plural entries are not supported by the catalog generator",
                path.display()
            );
        } else {
            bail!("{}: unrecognized .po line: {line}", path.display());
        }
    }
    flush(&mut entries, &mut msgid, &mut msgstr);
    Ok(entries)
}

#[derive(Clone, Copy)]
enum Field {
    None,
    Msgid,
    Msgstr,
}

/// Decode one `"…"` .po string, unescaping the C escapes gettext writes.
fn po_string(raw: &str, path: &Path) -> Result<String> {
    let raw = raw.trim();
    let inner = raw
        .strip_prefix('"')
        .and_then(|raw| raw.strip_suffix('"'))
        .with_context(|| format!("{}: malformed .po string: {raw}", path.display()))?;

    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            // Unknown escapes are kept verbatim, matching gettext.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => bail!(
                "{}: trailing backslash in .po string: {raw}",
                path.display()
            ),
        }
    }
    Ok(out)
}

/// Render `value` as a Rust string literal.
fn rust_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}
