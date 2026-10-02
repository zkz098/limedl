//! Disk media reporting and the per-directory override editor.

use super::*;

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
        Some(media) => {
            let name = format_disk_type_name(media, lang);
            match lang {
                Language::ZhCn => format!("当前检测: {name}"),
                Language::ZhTw => format!("目前偵測: {name}"),
                Language::EnUs => format!("Detected: {name}"),
            }
        }
        None => match lang {
            Language::ZhCn => "请输入绝对路径（如 D:\\Downloads 或 \\\\NAS\\share）".to_string(),
            Language::ZhTw => "請輸入絕對路徑（如 D:\\Downloads 或 \\\\NAS\\share）".to_string(),
            Language::EnUs => {
                "Enter an absolute path (e.g. D:\\Downloads or \\\\NAS\\share)".to_string()
            }
        },
    }
}

/// Toast shown when the override rows cannot be turned into settings on save.
pub fn format_toast_disk_override_invalid(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("目录介质覆盖无效: {err}"),
        Language::ZhTw => format!("目錄介質覆蓋無效: {err}"),
        Language::EnUs => format!("Invalid directory media override: {err}"),
    }
}
