//! Per-directory media override editor (`io_baseline.disk_type_overrides`).
//!
//! Rows live in the UI model until Save, exactly like the speed-limit schedule
//! editor: what the user typed is the authoritative state while the dialog is
//! open, and the persisted map is rebuilt from it on every successful save.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use limedl_core::file_ops::{is_usable_override_key, normalize_media_path};
use limedl_core::types::{AppSettings, DiskType};
use slint::SharedString;

use crate::DiskTypeOverrideItem;
use crate::i18n::{self, Language};

/// Picker index of "force SSD" — the first entry of the editor's combo.
pub const MEDIA_SSD_INDEX: i32 = 0;
/// Picker index of "force HDD" — the second entry of the editor's combo.
pub const MEDIA_HDD_INDEX: i32 = 1;

/// Does a media picker index mean HDD?
pub fn index_is_hdd(index: i32) -> bool {
    index == MEDIA_HDD_INDEX
}

/// Does a persisted media type mean HDD in the editor?
///
/// `Network` is the *auto* answer rather than a forceable mode, so a hand-edited
/// settings file containing one shows up as HDD — the thing a pinned remote path
/// usually wants — and stays editable.
pub fn media_is_hdd(media: DiskType) -> bool {
    media != DiskType::Ssd
}

/// Text state of one override row as typed by the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskTypeOverrideText {
    pub path: String,
    pub is_hdd: bool,
}

impl Default for DiskTypeOverrideText {
    fn default() -> Self {
        Self {
            path: String::new(),
            // A row is usually added because something was mis-detected as an
            // SSD (a NAS share, a WSL path), so HDD is the useful default.
            is_hdd: true,
        }
    }
}

/// Validate + convert the editor rows into the settings map.
///
/// Keys keep the user's spelling — the engine normalizes both sides when it
/// looks an override up — while the *normalized* form is used only to detect
/// duplicates, so `D:\dl` and `d:\dl\` cannot become two rows fighting over one
/// directory. Returns a localized message for the first bad row.
pub fn parse_disk_type_overrides(
    rows: &[DiskTypeOverrideText],
    lang: Language,
) -> Result<HashMap<String, DiskType>, String> {
    let mut overrides = HashMap::new();
    let mut seen = HashSet::new();
    for row in rows {
        let path = row.path.trim();
        if !is_usable_override_key(path) {
            return Err(i18n::format_validation_absolute_path(
                lang,
                i18n::SettingsField::DiskOverridePath,
                path,
            ));
        }
        if !seen.insert(normalize_media_path(path)) {
            return Err(i18n::format_validation_duplicate_path(lang, path));
        }
        let media = if row.is_hdd {
            DiskType::Hdd
        } else {
            DiskType::Ssd
        };
        overrides.insert(path.to_string(), media);
    }
    Ok(overrides)
}

/// Build the Slint model items for the editor.
pub fn disk_override_rows_to_slint(
    rows: &[DiskTypeOverrideText],
    lang: Language,
) -> Vec<DiskTypeOverrideItem> {
    rows.iter()
        .map(|row| {
            let path = row.path.trim();
            // Showing the auto-detected media next to the picker is what tells a
            // user whether a row still has work to do — this is also the only
            // place a `\\wsl$` path reports its resolved host volume.
            let detected = is_usable_override_key(path)
                .then(|| limedl_core::file_ops::detect_disk_type(Path::new(path)));
            DiskTypeOverrideItem {
                path: SharedString::from(row.path.as_str()),
                media_idx: if row.is_hdd {
                    MEDIA_HDD_INDEX
                } else {
                    MEDIA_SSD_INDEX
                },
                detected_text: SharedString::from(i18n::format_disk_override_detected(detected, lang)),
            }
        })
        .collect()
}

/// Convert persisted settings into the editor's row state.
///
/// Sorted by normalized path: `HashMap` order is arbitrary, so without this the
/// list would reshuffle on every open and save.
pub fn disk_override_rows_from_settings(settings: &AppSettings) -> Vec<DiskTypeOverrideText> {
    let mut overrides: Vec<(&String, DiskType)> = settings
        .io_baseline
        .disk_type_overrides
        .iter()
        .map(|(path, media)| (path, *media))
        .collect();
    overrides.sort_by_key(|(path, _)| normalize_media_path(path));
    overrides
        .into_iter()
        .map(|(path, media)| DiskTypeOverrideText {
            path: path.clone(),
            is_hdd: media_is_hdd(media),
        })
        .collect()
}
