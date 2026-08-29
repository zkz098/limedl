use limedl_core::types::DownloadState;

/// Supported languages in the limedl native desktop client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    ZhCn,
    EnUs,
}

impl Language {
    /// Parse from language code (e.g., "zh", "zh-CN", "zh_CN", "en", "en-US", "system").
    pub fn from_code(code: &str) -> Self {
        let code_trimmed = code.trim().to_lowercase();
        if code_trimmed.starts_with("zh") {
            Language::ZhCn
        } else if code_trimmed.starts_with("en") {
            Language::EnUs
        } else {
            Language::detect_system()
        }
    }

    /// Detect system locale automatically via `sys_locale`.
    pub fn detect_system() -> Self {
        if let Some(locale) = sys_locale::get_locale() {
            let loc = locale.to_lowercase();
            if loc.starts_with("zh") {
                return Language::ZhCn;
            }
        }
        Language::EnUs
    }

    /// Bundled translation locale code used by Slint (matches directory in lang/).
    pub fn as_code(&self) -> &'static str {
        match self {
            Language::ZhCn => "zh_CN",
            Language::EnUs => "en",
        }
    }

    /// Standard BCP-47 tag for serialization in settings.
    pub fn as_bcp47(&self) -> &'static str {
        match self {
            Language::ZhCn => "zh-CN",
            Language::EnUs => "en-US",
        }
    }

    /// Human-readable label for UI selection.
    #[allow(dead_code)]
    pub fn as_label(&self) -> &'static str {
        match self {
            Language::ZhCn => "简体中文 (zh-CN)",
            Language::EnUs => "English (en-US)",
        }
    }
}

/// Activate bundled translation catalog in Slint runtime.
/// Must be called after the first Slint component has been created.
pub fn apply_translation(lang: Language) {
    if let Err(e) = slint::select_bundled_translation(lang.as_code()) {
        tracing::warn!(
            "Failed to select Slint translation '{}': {e}",
            lang.as_code()
        );
    }
}

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
        (DownloadState::Downloading, Language::EnUs) => "Downloading",
        (DownloadState::Paused, Language::ZhCn) => "已暂停",
        (DownloadState::Paused, Language::EnUs) => "Paused",
        (DownloadState::Completed, Language::ZhCn) => "已完成",
        (DownloadState::Completed, Language::EnUs) => "Completed",
        (DownloadState::Failed, Language::ZhCn) => "失败",
        (DownloadState::Failed, Language::EnUs) => "Failed",
        (DownloadState::Canceled, Language::ZhCn) => "已取消",
        (DownloadState::Canceled, Language::EnUs) => "Canceled",
        (DownloadState::Queued, Language::ZhCn) => "排队中",
        (DownloadState::Queued, Language::EnUs) => "Queued",
        (DownloadState::Retrying, Language::ZhCn) => "重试中",
        (DownloadState::Retrying, Language::EnUs) => "Retrying",
        (DownloadState::Verifying, Language::ZhCn) => "校验中",
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
        Language::EnUs => format!("{mode_str} (Allocated: {allocated_threads} threads)"),
    }
}

/// Format seed / leech peer counts localized.
pub fn format_seed_leech(seed: Option<u64>, leech: Option<u64>, lang: Language) -> String {
    match (seed, leech) {
        (Some(s), Some(l)) => match lang {
            Language::ZhCn => format!("做种: {s} | 下载: {l}"),
            Language::EnUs => format!("Seeds: {s} | Peers: {l}"),
        },
        _ => String::new(),
    }
}

/// "Unknown" text localized.
pub fn format_unknown(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "未知",
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
            Language::EnUs => "No piece data".to_string(),
        };
    }
    match lang {
        Language::ZhCn => format!("{completed} / {total} 分片 ({percent:.1}%)"),
        Language::EnUs => format!("{completed} / {total} pieces ({percent:.1}%)"),
    }
}

/// Buffer pool not ready localized.
pub fn format_io_status_not_ready(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "智能缓冲池未就绪",
        Language::EnUs => "Smart buffer pool not ready",
    }
}

/// CDN status label localized.
pub fn format_cdn_status_label(is_testing: bool, lang: Language) -> &'static str {
    match (is_testing, lang) {
        (true, Language::ZhCn) => "测速中",
        (true, Language::EnUs) => "Testing",
        (false, Language::ZhCn) => "准备就绪",
        (false, Language::EnUs) => "Ready",
    }
}

/// CDN phase label localized.
pub fn format_cdn_phase_label(is_testing: bool, lang: Language) -> &'static str {
    match (is_testing, lang) {
        (true, Language::ZhCn) => "正在测量候选节点",
        (true, Language::EnUs) => "Measuring candidate edge nodes",
        (false, Language::ZhCn) => "测速完成",
        (false, Language::EnUs) => "Speedtest finished",
    }
}

/// CDN benchmark node text localized.
pub fn format_cdn_default_node(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "直连 DNS (基准)",
        Language::EnUs => "Direct DNS (Benchmark)",
    }
}

/// Notification texts for task completion.
pub fn format_notification_completed(file_name: &str, lang: Language) -> (String, String) {
    match lang {
        Language::ZhCn => (
            "下载已完成".to_string(),
            format!("文件已保存: {file_name}"),
        ),
        Language::EnUs => (
            "Download Completed".to_string(),
            format!("File saved: {file_name}"),
        ),
    }
}

/// Notification texts for task failure.
pub fn format_notification_failed(
    file_name: &str,
    error: Option<&str>,
    lang: Language,
) -> (String, String) {
    match lang {
        Language::ZhCn => (
            "下载失败".to_string(),
            format!(
                "任务失败: {} ({})",
                file_name,
                error.unwrap_or("网络错误")
            ),
        ),
        Language::EnUs => (
            "Download Failed".to_string(),
            format!(
                "Task failed: {} ({})",
                file_name,
                error.unwrap_or("Network error")
            ),
        ),
    }
}

/// System tray menu localized strings.
/// Format the "new version available" system notification (title, body).
pub fn format_notification_update(version: &str, lang: Language) -> (String, String) {
    match lang {
        Language::ZhCn => (
            format!("limedl 发现新版本 v{version}"),
            "打开 设置 → 关于 以下载并安装更新。".into(),
        ),
        Language::EnUs => (
            format!("limedl v{version} is available"),
            "Open Settings → About to download and install the update.".into(),
        ),
    }
}

pub struct TrayMenuStrings {
    pub show_window: &'static str,
    pub pause_all: &'static str,
    pub resume_all: &'static str,
    pub game_mode_toggle: &'static str,
    pub open_download_dir: &'static str,
    pub quit: &'static str,
    pub tooltip: &'static str,
}

pub fn get_tray_strings(lang: Language) -> TrayMenuStrings {
    match lang {
        Language::ZhCn => TrayMenuStrings {
            show_window: "显示主窗口",
            pause_all: "全部暂停",
            resume_all: "全部继续",
            game_mode_toggle: "游戏模式开关",
            open_download_dir: "打开下载目录",
            quit: "退出 limedl",
            tooltip: "limedl - 下载管理器",
        },
        Language::EnUs => TrayMenuStrings {
            show_window: "Show Main Window",
            pause_all: "Pause All",
            resume_all: "Resume All",
            game_mode_toggle: "Toggle Game Mode",
            open_download_dir: "Open Download Directory",
            quit: "Exit limedl",
            tooltip: "limedl - Download Manager",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_parsing() {
        assert_eq!(Language::from_code("zh"), Language::ZhCn);
        assert_eq!(Language::from_code("zh-CN"), Language::ZhCn);
        assert_eq!(Language::from_code("zh_CN"), Language::ZhCn);
        assert_eq!(Language::from_code("en"), Language::EnUs);
        assert_eq!(Language::from_code("en-US"), Language::EnUs);
        assert_eq!(Language::from_code("en_GB"), Language::EnUs);
    }

    #[test]
    fn test_format_eta_localized() {
        assert_eq!(format_eta(Some(45), Language::ZhCn), "剩余 45秒");
        assert_eq!(format_eta(Some(45), Language::EnUs), "45s left");
        assert_eq!(format_eta(Some(125), Language::ZhCn), "剩余 2分5秒");
        assert_eq!(format_eta(Some(125), Language::EnUs), "2m 5s left");
        assert_eq!(format_eta(Some(3665), Language::ZhCn), "剩余 1小时1分");
        assert_eq!(format_eta(Some(3665), Language::EnUs), "1h 1m left");
        assert_eq!(format_eta(Some(90000), Language::ZhCn), "剩余 1天1小时");
        assert_eq!(format_eta(Some(90000), Language::EnUs), "1d 1h left");
    }

    #[test]
    fn test_state_labels() {
        assert_eq!(format_state_label(&DownloadState::Downloading, Language::ZhCn), "下载中");
        assert_eq!(format_state_label(&DownloadState::Downloading, Language::EnUs), "Downloading");
        assert_eq!(format_state_label(&DownloadState::Completed, Language::ZhCn), "已完成");
        assert_eq!(format_state_label(&DownloadState::Completed, Language::EnUs), "Completed");
    }

    #[test]
    fn test_tray_menu_strings() {
        let zh = get_tray_strings(Language::ZhCn);
        assert_eq!(zh.show_window, "显示主窗口");
        let en = get_tray_strings(Language::EnUs);
        assert_eq!(en.show_window, "Show Main Window");
    }
}
