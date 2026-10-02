//! Speed limit schedule summaries and toasts.

use super::Language;

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
    match lang {
        Language::ZhCn => format!("限速计划无效: {err}"),
        Language::ZhTw => format!("限速排程無效: {err}"),
        Language::EnUs => format!("Invalid speed limit schedule: {err}"),
    }
}

/// In-app toast for the tray speed-limit shortcut.
pub fn format_toast_speed_limit(enabled: bool, lang: Language) -> String {
    match (enabled, lang) {
        (true, Language::ZhCn) => "已开启全局限速 (1 MB/s)".to_string(),
        (true, Language::ZhTw) => "已開啟全域限速 (1 MB/s)".to_string(),
        (true, Language::EnUs) => "Global speed limit enabled (1 MB/s)".to_string(),
        (false, Language::ZhCn) => "已关闭全局限速".to_string(),
        (false, Language::ZhTw) => "已關閉全域限速".to_string(),
        (false, Language::EnUs) => "Global speed limit disabled".to_string(),
    }
}
