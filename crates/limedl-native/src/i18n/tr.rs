//! Runtime lookup in the generated translation catalog.
//!
//! Rust-side text (toasts, settings validation) resolves through the same
//! gettext catalogs the Slint `@tr` strings use; see `catalog.rs` for the
//! generated table and `xtask/src/i18n.rs` for the generator.

use std::fmt::{self, Write as _};

use super::Language;
use super::catalog::CATALOG;

/// Translate `msgid` — the English source string — into `lang`.
///
/// The msgid doubles as the fallback: a key that is absent from the catalogs (or
/// whose translation is empty) yields the English source, which is what gettext
/// does. `msgid` is `&'static str` because every call site passes a literal, and
/// the `every_rust_message_exists_in_the_catalogs` test fails on a key that is
/// missing from the generated table, so the fallback is a safety net rather than
/// a normal path.
pub fn tr(msgid: &'static str, lang: Language) -> &'static str {
    match CATALOG.binary_search_by(|entry| entry.msgid.cmp(msgid)) {
        Ok(index) => CATALOG[index].translations[lang.catalog_index()],
        Err(_) => msgid,
    }
}

/// Translate and fill: each `{}` in the message takes the next argument.
///
/// Placeholders are positional rather than named so a message can be shared with
/// the Slint side (`@tr("… {}", arg)`) and so translations stay conventional for
/// gettext. A `{}` with no argument left is emitted verbatim, and arguments with
/// no `{}` left are dropped, so a translation that reorders or omits a
/// placeholder degrades to visible text instead of panicking.
pub fn tr_fmt(msgid: &'static str, lang: Language, args: &[&dyn fmt::Display]) -> String {
    let template = tr(msgid, lang);
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    for arg in args {
        let Some(index) = rest.find("{}") else { break };
        out.push_str(&rest[..index]);
        let _ = write!(out, "{arg}");
        rest = &rest[index + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the catalog column order against the `Language` variants, which the
    /// generator cannot check on its own (`xtask` does not know the enum).
    #[test]
    fn catalog_columns_follow_language_order() {
        assert_eq!(tr("Settings saved", Language::ZhCn), "设置已保存");
        assert_eq!(tr("Settings saved", Language::ZhTw), "設定已儲存");
        assert_eq!(tr("Settings saved", Language::EnUs), "Settings saved");
    }

    #[test]
    fn tr_fmt_substitutes_positionally() {
        let msg = tr_fmt("Batch add finished: {}/{} succeeded", Language::EnUs, &[&3, &5]);
        assert_eq!(msg, "Batch add finished: 3/5 succeeded");
    }

    #[test]
    fn tr_fmt_keeps_unfilled_placeholders_and_ignores_spare_arguments() {
        assert_eq!(tr_fmt("Settings saved", Language::EnUs, &[&1]), "Settings saved");
        assert_eq!(
            tr_fmt("Batch add finished: {}/{} succeeded", Language::EnUs, &[&1]),
            "Batch add finished: 1/{} succeeded"
        );
    }

    #[test]
    fn a_missing_key_falls_back_to_the_english_source() {
        // A plain binding rather than a literal at the call site: the
        // `every_rust_message_exists_in_the_catalogs` scan in `tests.rs`
        // collects `tr("…")` literals, and this key is deliberately absent.
        const ABSENT: &str = "a message that is not in any catalog";
        assert_eq!(tr(ABSENT, Language::ZhCn), ABSENT);
    }
}
