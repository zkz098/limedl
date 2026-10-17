//! Tray menu strings and desktop notifications.

use super::{Language, tr, tr_fmt};

/// Notification texts for task completion.
pub fn format_notification_completed(file_name: &str, lang: Language) -> (String, String) {
    (
        tr("Download Completed", lang).to_string(),
        tr_fmt("File saved: {}", lang, &[&file_name]),
    )
}

/// Notification texts for task failure.
pub fn format_notification_failed(
    file_name: &str,
    error: Option<&str>,
    lang: Language,
) -> (String, String) {
    let error = error.unwrap_or_else(|| tr("Network error", lang));
    (
        tr("Download Failed", lang).to_string(),
        tr_fmt("Task failed: {} ({})", lang, &[&file_name, &error]),
    )
}

/// Format the "new version available" system notification (title, body).
pub fn format_notification_update(version: &str, lang: Language) -> (String, String) {
    (
        tr_fmt("limedl v{} is available", lang, &[&version]),
        tr(
            "Open Settings → About to download and install the update.",
            lang,
        )
        .to_string(),
    )
}

/// System tray menu localized strings.
pub struct TrayMenuStrings {
    pub show_window: &'static str,
    pub pause_all: &'static str,
    pub resume_all: &'static str,
    pub speed_limit_toggle: &'static str,
    pub game_mode_toggle: &'static str,
    pub open_download_dir: &'static str,
    pub quit: &'static str,
    pub tooltip: &'static str,
}

pub fn get_tray_strings(lang: Language) -> TrayMenuStrings {
    TrayMenuStrings {
        show_window: tr("Show Main Window", lang),
        pause_all: tr("Pause All", lang),
        resume_all: tr("Resume All", lang),
        speed_limit_toggle: tr("Speed Limit (1 MB/s)", lang),
        game_mode_toggle: tr("Toggle Game Mode", lang),
        open_download_dir: tr("Open Download Directory", lang),
        quit: tr("Exit limedl", lang),
        tooltip: tr("limedl - Download Manager", lang),
    }
}

/// OS notification title shown when saving settings fails.
pub fn format_notification_settings_save_failed(lang: Language) -> &'static str {
    tr("Failed to save settings", lang)
}
