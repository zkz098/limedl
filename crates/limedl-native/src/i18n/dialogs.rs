//! New-task dialog, batch and detected-link strings.

use super::Language;

/// Checksum probe status line for the new-task dialog.
/// state: "probing" | "found" | "missing" | "not_http"
pub fn format_probe_status(state: &str, hash: &str, lang: Language) -> String {
    match state {
        "probing" => match lang {
            Language::ZhCn => "正在探测校验和...".to_string(),
            Language::ZhTw => "正在探測校驗值...".to_string(),
            Language::EnUs => "Detecting checksum...".to_string(),
        },
        "found" => format!("SHA-256: {hash}"),
        "missing" => match lang {
            Language::ZhCn => "未找到可用的校验和文件".to_string(),
            Language::ZhTw => "未找到可用的校驗檔案".to_string(),
            Language::EnUs => "No checksum file found".to_string(),
        },
        "not_http" => match lang {
            Language::ZhCn => "仅 HTTP 链接支持校验和探测".to_string(),
            Language::ZhTw => "僅 HTTP 連結支援校驗探測".to_string(),
            Language::EnUs => "Checksum detection is only available for HTTP links".to_string(),
        },
        _ => String::new(),
    }
}

/// Torrent preview section header/status for the new-task dialog.
/// state: "loading" | "error" | "summary"
pub fn format_preview_status(state: &str, detail: &str, lang: Language) -> String {
    match (state, lang) {
        ("loading", _) => match lang {
            Language::ZhCn => "正在解析种子文件...".to_string(),
            Language::ZhTw => "正在解析種子檔案...".to_string(),
            Language::EnUs => "Parsing torrent...".to_string(),
        },
        ("error", _) => match lang {
            Language::ZhCn => format!("解析失败: {detail}"),
            Language::ZhTw => format!("解析失敗: {detail}"),
            Language::EnUs => format!("Preview failed: {detail}"),
        },
        _ => String::new(),
    }
}

/// Torrent file selection summary, e.g. "24 files · 12.3 GB".
pub fn format_preview_summary(file_count: usize, size_text: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("{} 个文件 · {}", file_count, size_text),
        Language::ZhTw => format!("{} 個檔案 · {}", file_count, size_text),
        Language::EnUs => format!("{} files · {}", file_count, size_text),
    }
}

/// "Select at least one file" error shown when all torrent files are unchecked.
pub fn no_files_selected_text(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "请至少选择一个文件",
        Language::ZhTw => "請至少選擇一個檔案",
        Language::EnUs => "Select at least one file",
    }
}

/// Batch link count label under the batch textarea.
pub fn format_batch_count(count: usize, lang: Language) -> String {
    if count == 0 {
        return match lang {
            Language::ZhCn => "未识别到有效链接".to_string(),
            Language::ZhTw => "未辨識到有效連結".to_string(),
            Language::EnUs => "No valid links detected".to_string(),
        };
    }
    match lang {
        Language::ZhCn => format!("将添加 {} 个任务", count),
        Language::ZhTw => format!("將新增 {} 個任務", count),
        Language::EnUs => format!("Will add {} tasks", count),
    }
}

/// Batch submit progress/result line.
pub fn format_batch_status(done: usize, total: usize, lang: Language) -> String {
    if done < total {
        return match lang {
            Language::ZhCn => format!("正在提交... {}/{}", done, total),
            Language::ZhTw => format!("正在提交... {}/{}", done, total),
            Language::EnUs => format!("Submitting... {}/{}", done, total),
        };
    }
    if done == 0 {
        return match lang {
            Language::ZhCn => "未识别到有效链接".to_string(),
            Language::ZhTw => "未辨識到有效連結".to_string(),
            Language::EnUs => "No valid links detected".to_string(),
        };
    }
    match lang {
        Language::ZhCn => format!("批量提交完成: {}/{} 成功", done, total),
        Language::ZhTw => format!("批次提交完成: {}/{} 成功", done, total),
        Language::EnUs => format!("Batch submitted: {}/{} succeeded", done, total),
    }
}
/// Clipboard monitor toast for a single detected download link.
pub fn format_detected_link(url: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("检测到下载链接: {url}"),
        Language::ZhTw => format!("偵測到下載連結: {url}"),
        Language::EnUs => format!("Download link detected: {url}"),
    }
}

/// Clipboard monitor toast for multiple detected download links.
pub fn format_detected_batch(count: usize, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("检测到 {count} 个批量下载链接"),
        Language::ZhTw => format!("偵測到 {count} 個批次下載連結"),
        Language::EnUs => format!("Detected {count} batch download links"),
    }
}

/// Title of the native file picker used to choose a .torrent file.
pub fn pick_torrent_title(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "选择 Torrent 种子文件",
        Language::ZhTw => "選擇 Torrent 種子檔案",
        Language::EnUs => "Select Torrent File",
    }
}

/// Filter label for the native file picker when choosing a .torrent file.
pub fn pick_torrent_filter(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "种子文件 (*.torrent)",
        Language::ZhTw => "種子檔案 (*.torrent)",
        Language::EnUs => "Torrent Files (*.torrent)",
    }
}

/// Title of the native file picker used to choose a download directory.
pub fn pick_download_dir_title(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "选择下载保存目录",
        Language::ZhTw => "選擇下載儲存目錄",
        Language::EnUs => "Select Download Folder",
    }
}

/// Title of the native file picker used to choose the log directory.
pub fn pick_log_dir_title(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "选择日志保存目录",
        Language::ZhTw => "選擇日誌儲存目錄",
        Language::EnUs => "Select Log Folder",
    }
}

/// Title and body of the fatal startup-error dialog.
///
/// Shown before settings are available, so the caller passes the language
/// detected from the OS locale rather than the configured one. `detail` is the
/// underlying error and `log_path` the crash log the same report was written to.
pub fn format_startup_failure(
    lang: Language,
    detail: &str,
    log_path: &str,
) -> (String, String) {
    let title = match lang {
        Language::ZhCn => "limedl 无法启动",
        Language::ZhTw => "limedl 無法啟動",
        Language::EnUs => "limedl could not start",
    };
    let body = match lang {
        Language::ZhCn => format!(
            "启动时发生错误：\n\n{detail}\n\n详细信息已写入：\n{log_path}"
        ),
        Language::ZhTw => format!(
            "啟動時發生錯誤：\n\n{detail}\n\n詳細資訊已寫入：\n{log_path}"
        ),
        Language::EnUs => format!(
            "limedl failed to start:\n\n{detail}\n\nDetails were written to:\n{log_path}"
        ),
    };
    (title.to_string(), body)
}
