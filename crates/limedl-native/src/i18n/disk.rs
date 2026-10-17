//! Disk media reporting and the per-directory override editor.

use super::{Language, format_disk_type_name, tr, tr_fmt};

/// How one override row's path is currently auto-detected.
///
/// Shown next to the media picker so a user can see whether a row is actually
/// needed: `None` means the path is not usable as a key yet (blank, or not
/// absolute), which the editor reports instead of a misleading "SSD".
pub fn format_disk_override_detected(
    media: Option<limedl_core::types::DiskType>,
    lang: Language,
) -> String {
    match media {
        Some(media) => tr_fmt("Detected: {}", lang, &[&format_disk_type_name(media, lang)]),
        None => tr(
            "Enter an absolute path (e.g. D:\\Downloads or \\\\NAS\\share)",
            lang,
        )
        .to_string(),
    }
}

/// Toast shown when the override rows cannot be turned into settings on save.
pub fn format_toast_disk_override_invalid(err: &str, lang: Language) -> String {
    tr_fmt("Invalid directory media override: {}", lang, &[&err])
}
