//! Speed limit schedule summaries and toasts.

use super::{Language, tr, tr_fmt};

/// Display summary for one speed-limit schedule row.
pub fn format_schedule_summary(
    start_hour: u32,
    end_hour: u32,
    limit_kb: u64,
    lang: Language,
) -> String {
    let wraps_marker = if start_hour >= end_hour { " (+1d)" } else { "" };
    let range = format!("{start_hour:02}:00 → {end_hour:02}:00{wraps_marker}");
    let limit = if limit_kb == 0 {
        // NOT `tr("Unlimited", lang)`: the speed-limit combo translates that
        // msgid as "不限", while a schedule row reads better as "不限速". Same
        // English source, two translations — the `msgctxt` case documented in
        // `docs/i18n.md`.
        match lang {
            Language::ZhCn => "不限速".to_string(),
            Language::ZhTw => "不限速".to_string(),
            Language::EnUs => "Unlimited".to_string(),
        }
    } else {
        format!("{limit_kb} KB/s")
    };
    format!("{range} · {limit}")
}

/// Toast shown when a schedule row cannot be parsed on save.
pub fn format_toast_schedule_invalid(err: &str, lang: Language) -> String {
    tr_fmt("Invalid speed limit schedule: {}", lang, &[&err])
}

/// In-app toast for the tray speed-limit shortcut.
pub fn format_toast_speed_limit(enabled: bool, lang: Language) -> String {
    if enabled {
        tr("Global speed limit enabled (1 MB/s)", lang).to_string()
    } else {
        tr("Global speed limit disabled", lang).to_string()
    }
}
