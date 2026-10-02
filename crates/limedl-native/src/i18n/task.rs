//! Task list and status formatting (eta, state, threads, priority, io status).

use super::{DownloadState, Language};

/// Format ETA seconds localized.
pub fn format_eta(eta: Option<u64>, lang: Language) -> String {
    match eta {
        Some(s) if s > 0 => match lang {
            Language::ZhCn => {
                if s >= 86400 {
                    let d = s / 86400;
                    let h = (s % 86400) / 3600;
                    format!("剩余 {d}天{h}小时")
                } else if s >= 3600 {
                    let h = s / 3600;
                    let m = (s % 3600) / 60;
                    format!("剩余 {h}小时{m}分")
                } else if s >= 60 {
                    let m = s / 60;
                    let sec = s % 60;
                    format!("剩余 {m}分{sec}秒")
                } else {
                    format!("剩余 {s}秒")
                }
            }
            Language::ZhTw => {
                if s >= 86400 {
                    let d = s / 86400;
                    let h = (s % 86400) / 3600;
                    format!("剩餘 {d}天{h}小時")
                } else if s >= 3600 {
                    let h = s / 3600;
                    let m = (s % 3600) / 60;
                    format!("剩餘 {h}小時{m}分")
                } else if s >= 60 {
                    let m = s / 60;
                    let sec = s % 60;
                    format!("剩餘 {m}分{sec}秒")
                } else {
                    format!("剩餘 {s}秒")
                }
            }
            Language::EnUs => {
                if s >= 86400 {
                    let d = s / 86400;
                    let h = (s % 86400) / 3600;
                    format!("{d}d {h}h left")
                } else if s >= 3600 {
                    let h = s / 3600;
                    let m = (s % 3600) / 60;
                    format!("{h}h {m}m left")
                } else if s >= 60 {
                    let m = s / 60;
                    let sec = s % 60;
                    format!("{m}m {sec}s left")
                } else {
                    format!("{s}s left")
                }
            }
        },
        _ => String::new(),
    }
}

/// Format download state label localized.
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
    let mode_str = thread_mode.unwrap_or("Default");
    match lang {
        Language::ZhCn => format!("{mode_str} (已分配: {allocated_threads} 线程)"),
        Language::ZhTw => format!("{mode_str} (已分配: {allocated_threads} 執行緒)"),
        Language::EnUs => format!("{mode_str} (Allocated: {allocated_threads} threads)"),
    }
}

/// Format seed / leech peer counts localized.
pub fn format_seed_leech(seed: Option<u64>, leech: Option<u64>, lang: Language) -> String {
    match (seed, leech) {
        (Some(s), Some(l)) => match lang {
            Language::ZhCn => format!("做种: {s} | 下载: {l}"),
            Language::ZhTw => format!("做種: {s} | 下載: {l}"),
            Language::EnUs => format!("Seeds: {s} | Peers: {l}"),
        },
        _ => String::new(),
    }
}

/// "Unknown" text localized.
pub fn format_unknown(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "未知",
        Language::ZhTw => "未知",
        Language::EnUs => "Unknown",
    }
}

/// Piece map summary localized.
pub fn format_piece_map_summary(
    completed: usize,
    total: usize,
    percent: f64,
    lang: Language,
) -> String {
    if total == 0 {
        return match lang {
            Language::ZhCn => "暂无分片数据".to_string(),
            Language::ZhTw => "暫無分片資料".to_string(),
            Language::EnUs => "No piece data".to_string(),
        };
    }
    match lang {
        Language::ZhCn => format!("{completed} / {total} 分片 ({percent:.1}%)"),
        Language::ZhTw => format!("{completed} / {total} 分片 ({percent:.1}%)"),
        Language::EnUs => format!("{completed} / {total} pieces ({percent:.1}%)"),
    }
}

/// Buffer pool not ready localized.
pub fn format_io_status_not_ready(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "智能缓冲池未就绪",
        Language::ZhTw => "智慧快取池未就緒",
        Language::EnUs => "Smart buffer pool not ready",
    }
}
/// Disk probe found no mount point.
pub fn format_no_disk_detected(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "未检测到磁盘信息",
        Language::ZhTw => "未偵測到磁碟資訊",
        Language::EnUs => "No disk information detected",
    }
}

/// Localized disk type name used in the IO baseline summary.
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
    match lang {
        Language::ZhCn => {
            format!("已用缓存: {allocated} / 上限: {capacity} (活跃缓冲槽: {active_buffers} 个)")
        }
        Language::ZhTw => {
            format!("已用快取: {allocated} / 上限: {capacity} (活躍快取槽: {active_buffers} 個)")
        }
        Language::EnUs => format!(
            "Buffer in use: {allocated} / limit: {capacity} ({active_buffers} active slots)"
        ),
    }
}
/// Priority label localized (high / normal / low).
pub fn format_priority_label(
    priority: limedl_core::types::Priority,
    lang: Language,
) -> &'static str {
    use limedl_core::types::Priority;
    match (priority, lang) {
        (Priority::High, Language::ZhCn) => "高",
        (Priority::High, Language::ZhTw) => "高",
        (Priority::High, Language::EnUs) => "High",
        (Priority::Normal, Language::ZhCn) => "普通",
        (Priority::Normal, Language::ZhTw) => "普通",
        (Priority::Normal, Language::EnUs) => "Normal",
        (Priority::Low, Language::ZhCn) => "低",
        (Priority::Low, Language::ZhTw) => "低",
        (Priority::Low, Language::EnUs) => "Low",
    }
}
/// "%APPDATA%-style" OS description shown in the About tab.
///
/// Windows reports a friendly product name + build (registry), other platforms
/// fall back to the compile-time target description.
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
    match lang {
        Language::ZhCn => "下载任务",
        Language::ZhTw => "下載任務",
        Language::EnUs => "Download Task",
    }
}
/// In-app toast for task terminal events (works alongside the OS notification).
pub fn format_toast_state(file_name: &str, state: &DownloadState, lang: Language) -> String {
    match (state, lang) {
        (DownloadState::Completed, Language::ZhCn) => format!("下载完成: {file_name}"),
        (DownloadState::Completed, Language::ZhTw) => format!("下載完成: {file_name}"),
        (DownloadState::Completed, Language::EnUs) => format!("Completed: {file_name}"),
        (DownloadState::Failed, Language::ZhCn) => format!("下载失败: {file_name}"),
        (DownloadState::Failed, Language::ZhTw) => format!("下載失敗: {file_name}"),
        (DownloadState::Failed, Language::EnUs) => format!("Failed: {file_name}"),
        _ => format_state_label(state, lang).to_string(),
    }
}
