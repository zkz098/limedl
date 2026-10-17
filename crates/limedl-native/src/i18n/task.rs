//! Task list and status formatting (eta, state, threads, priority, io status).

use super::{DownloadState, Language, tr, tr_fmt};

/// Format ETA seconds localized.
pub fn format_eta(eta: Option<u64>, lang: Language) -> String {
    // `None` and `Some(0)` both mean "no estimate yet".
    let Some(seconds) = eta.filter(|seconds| *seconds > 0) else {
        return String::new();
    };
    if seconds >= 86400 {
        tr_fmt(
            "{}d {}h left",
            lang,
            &[&(seconds / 86400), &((seconds % 86400) / 3600)],
        )
    } else if seconds >= 3600 {
        tr_fmt(
            "{}h {}m left",
            lang,
            &[&(seconds / 3600), &((seconds % 3600) / 60)],
        )
    } else if seconds >= 60 {
        tr_fmt("{}m {}s left", lang, &[&(seconds / 60), &(seconds % 60)])
    } else {
        tr_fmt("{}s left", lang, &[&seconds])
    }
}

/// Format download state label localized.
///
/// Kept as a local three-language table rather than a catalog lookup: the UI
/// shows the same English source as a longer status label (`@tr("Downloading")`
/// is "正在下载"), while a list cell reads better as "下载中". Same msgid, two
/// translations — the `msgctxt` case documented in `docs/i18n.md`; the other
/// variants agree with the catalog and are listed here only to keep the table
/// exhaustive.
pub fn format_state_label(state: &DownloadState, lang: Language) -> &'static str {
    match (state, lang) {
        (DownloadState::Downloading, Language::ZhCn) => "下载中",
        (DownloadState::Downloading, Language::ZhTw) => "下載中",
        (DownloadState::Downloading, Language::EnUs) => "Downloading",
        (DownloadState::Paused, Language::ZhCn) => "已暂停",
        (DownloadState::Paused, Language::ZhTw) => "已暫停",
        (DownloadState::Paused, Language::EnUs) => "Paused",
        (DownloadState::Completed, Language::ZhCn) => "已完成",
        (DownloadState::Completed, Language::ZhTw) => "已完成",
        (DownloadState::Completed, Language::EnUs) => "Completed",
        (DownloadState::Failed, Language::ZhCn) => "失败",
        (DownloadState::Failed, Language::ZhTw) => "失敗",
        (DownloadState::Failed, Language::EnUs) => "Failed",
        (DownloadState::Canceled, Language::ZhCn) => "已取消",
        (DownloadState::Canceled, Language::ZhTw) => "已取消",
        (DownloadState::Canceled, Language::EnUs) => "Canceled",
        (DownloadState::Queued, Language::ZhCn) => "排队中",
        (DownloadState::Queued, Language::ZhTw) => "排隊中",
        (DownloadState::Queued, Language::EnUs) => "Queued",
        (DownloadState::Retrying, Language::ZhCn) => "重试中",
        (DownloadState::Retrying, Language::ZhTw) => "重試中",
        (DownloadState::Retrying, Language::EnUs) => "Retrying",
        (DownloadState::Verifying, Language::ZhCn) => "校验中",
        (DownloadState::Verifying, Language::ZhTw) => "校驗中",
        (DownloadState::Verifying, Language::EnUs) => "Verifying",
    }
}

/// Format inspector thread allocation string localized.
pub fn format_threads_text(
    thread_mode: Option<&str>,
    allocated_threads: usize,
    lang: Language,
) -> String {
    // "Default" is a mode identifier, not display text, so it is not translated.
    tr_fmt(
        "{} (Allocated: {} threads)",
        lang,
        &[&thread_mode.unwrap_or("Default"), &allocated_threads],
    )
}

/// Format seed / leech peer counts localized.
pub fn format_seed_leech(seed: Option<u64>, leech: Option<u64>, lang: Language) -> String {
    match (seed, leech) {
        (Some(seed), Some(leech)) => tr_fmt("Seeds: {} | Peers: {}", lang, &[&seed, &leech]),
        _ => String::new(),
    }
}

/// "Unknown" text localized.
pub fn format_unknown(lang: Language) -> &'static str {
    tr("Unknown", lang)
}

/// Piece map summary localized.
pub fn format_piece_map_summary(
    completed: usize,
    total: usize,
    percent: f64,
    lang: Language,
) -> String {
    if total == 0 {
        return tr("No piece data", lang).to_string();
    }
    tr_fmt(
        "{} / {} pieces ({}%)",
        lang,
        &[&completed, &total, &format!("{percent:.1}")],
    )
}

/// Buffer pool not ready localized.
pub fn format_io_status_not_ready(lang: Language) -> &'static str {
    tr("Smart buffer pool not ready", lang)
}

/// Disk probe found no mount point.
pub fn format_no_disk_detected(lang: Language) -> &'static str {
    tr("No disk information detected", lang)
}

/// Localized disk type name used in the IO baseline summary.
///
/// Kept as a local three-language table rather than a catalog lookup: the disk
/// override editor's ComboBox shows the same English source as a short badge
/// (`@tr("SSD")` / `@tr("HDD")`), while this summary needs the descriptive
/// "SSD 固态硬盘". Same msgid, two translations — the `msgctxt` case documented
/// in `docs/i18n.md`.
pub fn format_disk_type_name(disk: limedl_core::types::DiskType, lang: Language) -> &'static str {
    match (disk, lang) {
        (limedl_core::types::DiskType::Ssd, Language::ZhCn) => "SSD 固态硬盘",
        (limedl_core::types::DiskType::Ssd, Language::ZhTw) => "SSD 固態硬碟",
        (limedl_core::types::DiskType::Ssd, Language::EnUs) => "SSD",
        (limedl_core::types::DiskType::Hdd, Language::ZhCn) => "HDD 机械硬盘",
        (limedl_core::types::DiskType::Hdd, Language::ZhTw) => "HDD 機械硬碟",
        (limedl_core::types::DiskType::Hdd, Language::EnUs) => "HDD",
        // Remote locations: the media is unknown, which is different from
        // "detected as SSD". WSL paths resolve to their host volume and only
        // land here when that resolution failed. No parentheses, because the
        // settings panel wraps this name in its own (`Z: (…)`).
        (limedl_core::types::DiskType::Network, Language::ZhCn) => "网络位置·介质未知",
        (limedl_core::types::DiskType::Network, Language::ZhTw) => "網路位置·介質未知",
        (limedl_core::types::DiskType::Network, Language::EnUs) => "Network share — media unknown",
    }
}

/// Buffer pool status line shown in the IO settings tab.
pub fn format_io_status_line(
    allocated: &str,
    capacity: &str,
    active_buffers: u64,
    lang: Language,
) -> String {
    tr_fmt(
        "Buffer in use: {} / limit: {} ({} active slots)",
        lang,
        &[&allocated, &capacity, &active_buffers],
    )
}

/// Priority label localized (high / normal / low).
pub fn format_priority_label(
    priority: limedl_core::types::Priority,
    lang: Language,
) -> &'static str {
    use limedl_core::types::Priority;
    let msgid = match priority {
        Priority::High => "High",
        Priority::Normal => "Normal",
        Priority::Low => "Low",
    };
    tr(msgid, lang)
}

/// "%APPDATA%-style" OS description shown in the About tab.
///
/// Windows reports a friendly product name + build (registry), other platforms
/// fall back to the compile-time target description. The parts are platform
/// identifiers, so there is nothing to translate.
pub fn format_platform_description(os: &str, arch: &str, renderer: &str) -> String {
    format!("{arch} / {os} ({renderer})")
}

/// In-app toast for a download warning, prefixed with the affected task name.
///
/// The message body comes from core (English identifiers such as "disk full"),
/// so only the surrounding text is localized.
pub fn format_warning_with_file(file_name: &str, message: &str) -> String {
    format!("{file_name}: {message}")
}

/// Fallback display name for a task started without a resolved file name.
pub fn format_unnamed_task(lang: Language) -> &'static str {
    tr("Download Task", lang)
}

/// In-app toast for task terminal events (works alongside the OS notification).
pub fn format_toast_state(file_name: &str, state: &DownloadState, lang: Language) -> String {
    match state {
        DownloadState::Completed => tr_fmt("Completed: {}", lang, &[&file_name]),
        DownloadState::Failed => tr_fmt("Failed: {}", lang, &[&file_name]),
        _ => format_state_label(state, lang).to_string(),
    }
}
