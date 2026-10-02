//! Tray menu strings and desktop notifications.

use super::Language;

/// Notification texts for task completion.
pub fn format_notification_completed(file_name: &str, lang: Language) -> (String, String) {
    match lang {
        Language::ZhCn => ("下载已完成".to_string(), format!("文件已保存: {file_name}")),
        Language::ZhTw => ("下載已完成".to_string(), format!("檔案已儲存: {file_name}")),
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
            format!("任务失败: {} ({})", file_name, error.unwrap_or("网络错误")),
        ),
        Language::ZhTw => (
            "下載失敗".to_string(),
            format!("任務失敗: {} ({})", file_name, error.unwrap_or("網路錯誤")),
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
            "打开 设置 → 关于 以下载并更新。".into(),
        ),
        Language::ZhTw => (
            format!("limedl 發現新版本 v{version}"),
            "開啟 設定 → 關於 以下載並更新。".into(),
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
    pub speed_limit_toggle: &'static str,
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
            speed_limit_toggle: "限速模式 (1 MB/s)",
            game_mode_toggle: "游戏模式开关",
            open_download_dir: "打开下载目录",
            quit: "退出 limedl",
            tooltip: "limedl - 下载管理器",
        },
        Language::ZhTw => TrayMenuStrings {
            show_window: "顯示主視窗",
            pause_all: "全部暫停",
            resume_all: "全部繼續",
            speed_limit_toggle: "限速模式 (1 MB/s)",
            game_mode_toggle: "遊戲模式開關",
            open_download_dir: "開啟下載目錄",
            quit: "結束 limedl",
            tooltip: "limedl - 下載管理器",
        },
        Language::EnUs => TrayMenuStrings {
            show_window: "Show Main Window",
            pause_all: "Pause All",
            resume_all: "Resume All",
            speed_limit_toggle: "Speed Limit (1 MB/s)",
            game_mode_toggle: "Toggle Game Mode",
            open_download_dir: "Open Download Directory",
            quit: "Exit limedl",
            tooltip: "limedl - Download Manager",
        },
    }
}
/// OS notification title shown when saving settings fails.
pub fn format_notification_settings_save_failed(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "保存设置失败",
        Language::ZhTw => "儲存設定失敗",
        Language::EnUs => "Failed to save settings",
    }
}
