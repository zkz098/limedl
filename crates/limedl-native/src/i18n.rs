use limedl_core::types::DownloadState;

/// Supported languages in the limedl native desktop client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    ZhCn,
    ZhTw,
    EnUs,
}

impl Language {
    /// Parse from language code (e.g., "zh", "zh-CN", "zh_CN", "en", "en-US", "system").
    pub fn from_code(code: &str) -> Self {
        let code_trimmed = code.trim().to_lowercase();
        if code_trimmed.starts_with("zh") {
            if code_trimmed.contains("tw")
                || code_trimmed.contains("hk")
                || code_trimmed.contains("mo")
                || code_trimmed.contains("hant")
            {
                Language::ZhTw
            } else {
                Language::ZhCn
            }
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
                if loc.contains("tw")
                    || loc.contains("hk")
                    || loc.contains("mo")
                    || loc.contains("hant")
                {
                    return Language::ZhTw;
                }
                return Language::ZhCn;
            }
        }
        Language::EnUs
    }

    /// Bundled translation locale code used by Slint (matches directory in lang/).
    pub fn as_code(&self) -> &'static str {
        match self {
            Language::ZhCn => "zh_CN",
            Language::ZhTw => "zh_TW",
            Language::EnUs => "en",
        }
    }

    /// Standard BCP-47 tag for serialization in settings.
    pub fn as_bcp47(&self) -> &'static str {
        match self {
            Language::ZhCn => "zh-CN",
            Language::ZhTw => "zh-TW",
            Language::EnUs => "en-US",
        }
    }

    /// Human-readable label for UI selection.
    #[allow(dead_code)]
    pub fn as_label(&self) -> &'static str {
        match self {
            Language::ZhCn => "简体中文 (zh-CN)",
            Language::ZhTw => "繁體中文 (zh-TW)",
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

/// CDN status label localized.
pub fn format_cdn_status_label(is_testing: bool, lang: Language) -> &'static str {
    match (is_testing, lang) {
        (true, Language::ZhCn) => "测速中",
        (true, Language::ZhTw) => "測速中",
        (true, Language::EnUs) => "Testing",
        (false, Language::ZhCn) => "准备就绪",
        (false, Language::ZhTw) => "準備就緒",
        (false, Language::EnUs) => "Ready",
    }
}

/// CDN phase label localized.
pub fn format_cdn_phase_label(is_testing: bool, lang: Language) -> &'static str {
    match (is_testing, lang) {
        (true, Language::ZhCn) => "正在测量候选节点",
        (true, Language::ZhTw) => "正在測量候選節點",
        (true, Language::EnUs) => "Measuring candidate edge nodes",
        (false, Language::ZhCn) => "测速完成",
        (false, Language::ZhTw) => "測速完成",
        (false, Language::EnUs) => "Speedtest finished",
    }
}

/// CDN benchmark node text localized.
pub fn format_cdn_default_node(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "直连 DNS (基准)",
        Language::ZhTw => "直連 DNS (基準)",
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
        Language::ZhTw => (
            "下載已完成".to_string(),
            format!("檔案已儲存: {file_name}"),
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
        Language::ZhTw => (
            "下載失敗".to_string(),
            format!(
                "任務失敗: {} ({})",
                file_name,
                error.unwrap_or("網路錯誤")
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

// ── New-task dialog helpers (checksum probe / torrent preview / batch) ──

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
        ("loading", _) => {
            match lang {
                Language::ZhCn => "正在解析种子文件...".to_string(),
                Language::ZhTw => "正在解析種子檔案...".to_string(),
                Language::EnUs => "Parsing torrent...".to_string(),
            }
        }
        ("error", _) => {
            match lang {
                Language::ZhCn => format!("解析失败: {detail}"),
                Language::ZhTw => format!("解析失敗: {detail}"),
                Language::EnUs => format!("Preview failed: {detail}"),
            }
        }
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

// ── In-app toast message helpers ─────────────────────────────────────

pub fn format_toast_task_added(file_name: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("已添加下载任务: {file_name}"),
        Language::ZhTw => format!("已新增下載任務: {file_name}"),
        Language::EnUs => format!("Download task added: {file_name}"),
    }
}

pub fn format_toast_task_add_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("添加下载任务失败: {err}"),
        Language::ZhTw => format!("新增下載任務失敗: {err}"),
        Language::EnUs => format!("Failed to add task: {err}"),
    }
}

pub fn format_toast_batch_done(ok: usize, total: usize, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("批量添加完成: {ok}/{total} 成功"),
        Language::ZhTw => format!("批次新增完成: {ok}/{total} 成功"),
        Language::EnUs => format!("Batch add finished: {ok}/{total} succeeded"),
    }
}

pub fn format_toast_settings_saved(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "设置已保存",
        Language::ZhTw => "設定已儲存",
        Language::EnUs => "Settings saved",
    }
}

pub fn format_toast_settings_save_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("保存设置失败: {err}"),
        Language::ZhTw => format!("儲存設定失敗: {err}"),
        Language::EnUs => format!("Failed to save settings: {err}"),
    }
}

pub fn format_toast_settings_invalid(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("设置校验失败: {err}"),
        Language::ZhTw => format!("設定驗證失敗: {err}"),
        Language::EnUs => format!("Invalid settings: {err}"),
    }
}

pub fn format_toast_setup_finished(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "初始设置已保存完成",
        Language::ZhTw => "初始設定已儲存完成",
        Language::EnUs => "Setup completed",
    }
}

pub fn format_toast_tracker_synced(count: usize, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("Tracker 列表同步成功: {count} 个"),
        Language::ZhTw => format!("Tracker 清單同步成功: {count} 個"),
        Language::EnUs => format!("Tracker list synced: {count} entries"),
    }
}

pub fn format_toast_tracker_sync_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("Tracker 同步失败: {err}"),
        Language::ZhTw => format!("Tracker 同步失敗: {err}"),
        Language::EnUs => format!("Tracker sync failed: {err}"),
    }
}

pub fn format_toast_link_copied(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "下载链接已复制到剪贴板",
        Language::ZhTw => "下載連結已複製到剪貼簿",
        Language::EnUs => "Download link copied to clipboard",
    }
}

pub fn format_toast_filename_copied(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "文件名已复制到剪贴板",
        Language::ZhTw => "檔案名稱已複製到剪貼簿",
        Language::EnUs => "File name copied to clipboard",
    }
}

pub fn format_toast_labs_saved(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "实验室设置已保存",
        Language::ZhTw => "實驗室設定已儲存",
        Language::EnUs => "Labs settings saved",
    }
}

pub fn format_toast_labs_save_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("保存实验室设置失败: {err}"),
        Language::ZhTw => format!("儲存實驗室設定失敗: {err}"),
        Language::EnUs => format!("Failed to save Labs settings: {err}"),
    }
}

pub fn format_toast_cdn_applied(ip: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("已应用 CDN 节点: {ip}"),
        Language::ZhTw => format!("已套用 CDN 節點: {ip}"),
        Language::EnUs => format!("CDN node applied: {ip}"),
    }
}

pub fn format_toast_cdn_cleared(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "已清除 CDN 加速配置",
        Language::ZhTw => "已清除 CDN 加速設定",
        Language::EnUs => "CDN acceleration cleared",
    }
}

pub fn format_toast_cdn_test_done(ip: Option<&str>, lang: Language) -> String {
    match (ip, lang) {
        (Some(ip), Language::ZhCn) => format!("CDN 测速完成，已锁定节点 {ip}"),
        (Some(ip), Language::ZhTw) => format!("CDN 測速完成，已鎖定節點 {ip}"),
        (Some(ip), Language::EnUs) => format!("CDN speedtest finished, node {ip} locked"),
        (None, Language::ZhCn) => "CDN 测速完成".to_string(),
        (None, Language::ZhTw) => "CDN 測速完成".to_string(),
        (None, Language::EnUs) => "CDN speedtest finished".to_string(),
    }
}

pub fn format_toast_cdn_test_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("CDN 测速失败: {err}"),
        Language::ZhTw => format!("CDN 測速失敗: {err}"),
        Language::EnUs => format!("CDN speedtest failed: {err}"),
    }
}

pub fn format_toast_aria2_rpc_started(port: u16, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("Aria2 RPC 已启动 (端口 {port})"),
        Language::ZhTw => format!("Aria2 RPC 已啟動 (連接埠 {port})"),
        Language::EnUs => format!("Aria2 RPC started (port {port})"),
    }
}

pub fn format_toast_aria2_rpc_stopped(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "Aria2 RPC 已停止",
        Language::ZhTw => "Aria2 RPC 已停止",
        Language::EnUs => "Aria2 RPC stopped",
    }
}

pub fn format_toast_autostart_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("自启动设置失败: {err}"),
        Language::ZhTw => format!("自啟動設定失敗: {err}"),
        Language::EnUs => format!("Autostart failed: {err}"),
    }
}

/// Settings field referenced by a validation error message.
///
/// The label is localized so an English UI never surfaces Chinese text from
/// Rust-side validation (Slint `@tr` only covers strings inside `.slint` files).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    MaxRetries,
    MaxParallelTasks,
    GlobalSpeedLimit,
    SchedulerMaxParallelThreads,
    SchedulerMaxThreadsPerTask,
    SchedulerMinThreadsPerTask,
    ListenPort,
    MaxPeersPerTorrent,
    BtMaxDownloads,
    BtMaxSeeds,
    BtMaxTorrents,
    BtActiveLimit,
    BtGlobalDownloadRateLimit,
    BtGlobalUploadRateLimit,
    BtUploadLimit,
    BtUploadRatioLimit,
    BtAntiLeechGraceSecs,
    BtAntiLeechRatio,
    BtAntiLeechBanSecs,
    BtAntiLeechMaxUploadSlots,
    BtMaxUploadSlotsPerTorrent,
    BtSmartBanMaxFailures,
    BtEvictionBanDurationSecs,
    BtDataContributionTimeoutSecs,
    IoBufferLimitMb,
    IoGameModeBufferMb,
    IoMaxParallelHdd,
    IoGameModeMaxParallel,
    IoSsdWriteCombineMb,
    LoggingRetentionCount,
    LoggingRetentionDays,
    Aria2Port,
    MaxInMemoryDownloads,
    SpeedLimitStartHour,
    SpeedLimitEndHour,
    SpeedLimitLimit,
}

impl SettingsField {
    /// Localized field label used as the subject of a validation message.
    pub fn label(self, lang: Language) -> &'static str {
        use SettingsField as F;
        match (self, lang) {
            (F::MaxRetries, Language::ZhCn) => "最大重试次数",
            (F::MaxRetries, Language::ZhTw) => "最大重試次數",
            (F::MaxRetries, Language::EnUs) => "Max retries",
            (F::MaxParallelTasks, Language::ZhCn) => "最大并发任务数",
            (F::MaxParallelTasks, Language::ZhTw) => "最大並發任務數",
            (F::MaxParallelTasks, Language::EnUs) => "Max parallel tasks",
            (F::GlobalSpeedLimit, Language::ZhCn) => "全局限速",
            (F::GlobalSpeedLimit, Language::ZhTw) => "全域限速",
            (F::GlobalSpeedLimit, Language::EnUs) => "Global speed limit",
            (F::SchedulerMaxParallelThreads, Language::ZhCn) => "自动调度-最大并行线程",
            (F::SchedulerMaxParallelThreads, Language::ZhTw) => "自動排程-最大並行執行緒",
            (F::SchedulerMaxParallelThreads, Language::EnUs) => "Scheduler max parallel threads",
            (F::SchedulerMaxThreadsPerTask, Language::ZhCn) => "单任务最大线程",
            (F::SchedulerMaxThreadsPerTask, Language::ZhTw) => "單任務最大執行緒",
            (F::SchedulerMaxThreadsPerTask, Language::EnUs) => "Max threads per task",
            (F::SchedulerMinThreadsPerTask, Language::ZhCn) => "单任务最小线程",
            (F::SchedulerMinThreadsPerTask, Language::ZhTw) => "單任務最小執行緒",
            (F::SchedulerMinThreadsPerTask, Language::EnUs) => "Min threads per task",
            (F::ListenPort, Language::ZhCn) => "BT 监听端口",
            (F::ListenPort, Language::ZhTw) => "BT 監聽連接埠",
            (F::ListenPort, Language::EnUs) => "BT listen port",
            (F::MaxPeersPerTorrent, Language::ZhCn) => "每 Torrent 最大 Peers",
            (F::MaxPeersPerTorrent, Language::ZhTw) => "每 Torrent 最大 Peers",
            (F::MaxPeersPerTorrent, Language::EnUs) => "Max peers per torrent",
            (F::BtMaxDownloads, Language::ZhCn) => "BT 最大下载数",
            (F::BtMaxDownloads, Language::ZhTw) => "BT 最大下載數",
            (F::BtMaxDownloads, Language::EnUs) => "BT max downloads",
            (F::BtMaxSeeds, Language::ZhCn) => "BT 最大做种数",
            (F::BtMaxSeeds, Language::ZhTw) => "BT 最大做種數",
            (F::BtMaxSeeds, Language::EnUs) => "BT max seeds",
            (F::BtMaxTorrents, Language::ZhCn) => "BT 最大 Torrent 数",
            (F::BtMaxTorrents, Language::ZhTw) => "BT 最大 Torrent 數",
            (F::BtMaxTorrents, Language::EnUs) => "BT max torrents",
            (F::BtActiveLimit, Language::ZhCn) => "BT 活跃限制",
            (F::BtActiveLimit, Language::ZhTw) => "BT 活躍限制",
            (F::BtActiveLimit, Language::EnUs) => "BT active limit",
            (F::BtGlobalDownloadRateLimit, Language::ZhCn) => "BT 全局下载限速",
            (F::BtGlobalDownloadRateLimit, Language::ZhTw) => "BT 全域下載限速",
            (F::BtGlobalDownloadRateLimit, Language::EnUs) => "BT global download limit",
            (F::BtGlobalUploadRateLimit, Language::ZhCn) => "BT 全局上传限速",
            (F::BtGlobalUploadRateLimit, Language::ZhTw) => "BT 全域上傳限速",
            (F::BtGlobalUploadRateLimit, Language::EnUs) => "BT global upload limit",
            (F::BtUploadLimit, Language::ZhCn) => "做种上传限制",
            (F::BtUploadLimit, Language::ZhTw) => "做種上傳限制",
            (F::BtUploadLimit, Language::EnUs) => "Seeding upload limit",
            (F::BtUploadRatioLimit, Language::ZhCn) => "分享率限制",
            (F::BtUploadRatioLimit, Language::ZhTw) => "分享率限制",
            (F::BtUploadRatioLimit, Language::EnUs) => "Share ratio limit",
            (F::BtAntiLeechGraceSecs, Language::ZhCn) => "反吸血宽限期",
            (F::BtAntiLeechGraceSecs, Language::ZhTw) => "反吸血寬限期",
            (F::BtAntiLeechGraceSecs, Language::EnUs) => "Anti-leech grace period",
            (F::BtAntiLeechRatio, Language::ZhCn) => "反吸血分享率阈值",
            (F::BtAntiLeechRatio, Language::ZhTw) => "反吸血分享率閾值",
            (F::BtAntiLeechRatio, Language::EnUs) => "Anti-leech ratio threshold",
            (F::BtAntiLeechBanSecs, Language::ZhCn) => "反吸血封禁时长",
            (F::BtAntiLeechBanSecs, Language::ZhTw) => "反吸血封鎖時長",
            (F::BtAntiLeechBanSecs, Language::EnUs) => "Anti-leech ban duration",
            (F::BtAntiLeechMaxUploadSlots, Language::ZhCn) => "反吸血限槽模式槽位",
            (F::BtAntiLeechMaxUploadSlots, Language::ZhTw) => "反吸血限槽模式槽位",
            (F::BtAntiLeechMaxUploadSlots, Language::EnUs) => "Anti-leech upload slots",
            (F::BtMaxUploadSlotsPerTorrent, Language::ZhCn) => "每 Torrent 最大上传槽",
            (F::BtMaxUploadSlotsPerTorrent, Language::ZhTw) => "每 Torrent 最大上傳槽",
            (F::BtMaxUploadSlotsPerTorrent, Language::EnUs) => "Max upload slots per torrent",
            (F::BtSmartBanMaxFailures, Language::ZhCn) => "智能封禁阈值",
            (F::BtSmartBanMaxFailures, Language::ZhTw) => "智慧封鎖閾值",
            (F::BtSmartBanMaxFailures, Language::EnUs) => "Smart ban threshold",
            (F::BtEvictionBanDurationSecs, Language::ZhCn) => "驱逐封禁时长",
            (F::BtEvictionBanDurationSecs, Language::ZhTw) => "驅逐封鎖時長",
            (F::BtEvictionBanDurationSecs, Language::EnUs) => "Eviction ban duration",
            (F::BtDataContributionTimeoutSecs, Language::ZhCn) => "无贡献超时",
            (F::BtDataContributionTimeoutSecs, Language::ZhTw) => "無貢獻逾時",
            (F::BtDataContributionTimeoutSecs, Language::EnUs) => "No-contribution timeout",
            (F::IoBufferLimitMb, Language::ZhCn) => "IO 缓冲上限",
            (F::IoBufferLimitMb, Language::ZhTw) => "IO 快取上限",
            (F::IoBufferLimitMb, Language::EnUs) => "IO buffer limit",
            (F::IoGameModeBufferMb, Language::ZhCn) => "游戏模式缓冲",
            (F::IoGameModeBufferMb, Language::ZhTw) => "遊戲模式快取",
            (F::IoGameModeBufferMb, Language::EnUs) => "Game mode buffer",
            (F::IoMaxParallelHdd, Language::ZhCn) => "HDD 最大并行",
            (F::IoMaxParallelHdd, Language::ZhTw) => "HDD 最大並行",
            (F::IoMaxParallelHdd, Language::EnUs) => "Max parallel HDD transfers",
            (F::IoGameModeMaxParallel, Language::ZhCn) => "游戏模式最大并行",
            (F::IoGameModeMaxParallel, Language::ZhTw) => "遊戲模式最大並行",
            (F::IoGameModeMaxParallel, Language::EnUs) => "Game mode max parallel",
            (F::IoSsdWriteCombineMb, Language::ZhCn) => "SSD 合并缓冲",
            (F::IoSsdWriteCombineMb, Language::ZhTw) => "SSD 合併快取",
            (F::IoSsdWriteCombineMb, Language::EnUs) => "SSD write-combine buffer",
            (F::LoggingRetentionCount, Language::ZhCn) => "日志保留数量",
            (F::LoggingRetentionCount, Language::ZhTw) => "日誌保留數量",
            (F::LoggingRetentionCount, Language::EnUs) => "Log retention count",
            (F::LoggingRetentionDays, Language::ZhCn) => "日志保留天数",
            (F::LoggingRetentionDays, Language::ZhTw) => "日誌保留天數",
            (F::LoggingRetentionDays, Language::EnUs) => "Log retention days",
            (F::Aria2Port, Language::ZhCn) => "Aria2 端口",
            (F::Aria2Port, Language::ZhTw) => "Aria2 連接埠",
            (F::Aria2Port, Language::EnUs) => "Aria2 port",
            (F::MaxInMemoryDownloads, Language::ZhCn) => "内存保留记录数",
            (F::MaxInMemoryDownloads, Language::ZhTw) => "記憶體保留記錄數",
            (F::MaxInMemoryDownloads, Language::EnUs) => "In-memory record limit",
            (F::SpeedLimitStartHour, Language::ZhCn) => "限速计划-起始小时",
            (F::SpeedLimitStartHour, Language::ZhTw) => "限速排程-起始小時",
            (F::SpeedLimitStartHour, Language::EnUs) => "Speed limit schedule start hour",
            (F::SpeedLimitEndHour, Language::ZhCn) => "限速计划-结束小时",
            (F::SpeedLimitEndHour, Language::ZhTw) => "限速排程-結束小時",
            (F::SpeedLimitEndHour, Language::EnUs) => "Speed limit schedule end hour",
            (F::SpeedLimitLimit, Language::ZhCn) => "限速计划-限速值",
            (F::SpeedLimitLimit, Language::ZhTw) => "限速排程-限速值",
            (F::SpeedLimitLimit, Language::EnUs) => "Speed limit schedule rate",
        }
    }
}

/// "expected a number" validation error for `field`.
pub fn format_validation_number(lang: Language, field: SettingsField, value: &str) -> String {
    match lang {
        Language::ZhCn => format!(
            "{} 格式错误: '{}' 请输入有效数字",
            field.label(lang),
            value
        ),
        Language::ZhTw => format!(
            "{} 格式錯誤: '{}' 請輸入有效數字",
            field.label(lang),
            value
        ),
        Language::EnUs => format!(
            "Invalid {}: '{}' — please enter a valid number",
            field.label(lang),
            value
        ),
    }
}

/// "expected an integer" validation error for `field`.
pub fn format_validation_integer(lang: Language, field: SettingsField, value: &str) -> String {
    match lang {
        Language::ZhCn => format!(
            "{} 格式错误: '{}' 请输入有效整数",
            field.label(lang),
            value
        ),
        Language::ZhTw => format!(
            "{} 格式錯誤: '{}' 請輸入有效整數",
            field.label(lang),
            value
        ),
        Language::EnUs => format!(
            "Invalid {}: '{}' — please enter a valid integer",
            field.label(lang),
            value
        ),
    }
}

/// "expected a TCP port" validation error for `field`.
pub fn format_validation_port(lang: Language, field: SettingsField, value: &str) -> String {
    match lang {
        Language::ZhCn => format!(
            "{} 格式错误: '{}' 请输入 0-65535 的端口号",
            field.label(lang),
            value
        ),
        Language::ZhTw => format!(
            "{} 格式錯誤: '{}' 請輸入 0-65535 的連接埠號",
            field.label(lang),
            value
        ),
        Language::EnUs => format!(
            "Invalid {}: '{}' — please enter a port in 0-65535",
            field.label(lang),
            value
        ),
    }
}

/// "value out of range" validation error for `field`.
pub fn format_validation_range(
    lang: Language,
    field: SettingsField,
    min: u32,
    max: u32,
) -> String {
    match lang {
        Language::ZhCn => format!("{} 必须在 {min}-{max} 之间", field.label(lang)),
        Language::ZhTw => format!("{} 必須在 {min}-{max} 之間", field.label(lang)),
        Language::EnUs => format!("{} must be between {min} and {max}", field.label(lang)),
    }
}

/// Port 0 is rejected because it would let the OS pick an unpredictable port.
pub fn format_validation_port_zero(lang: Language, field: SettingsField) -> String {
    match lang {
        Language::ZhCn => format!("{} 不能为 0", field.label(lang)),
        Language::ZhTw => format!("{} 不能為 0", field.label(lang)),
        Language::EnUs => format!("{} cannot be 0", field.label(lang)),
    }
}

/// Manual proxy mode requires a proxy URL.
pub fn format_proxy_url_required(lang: Language) -> String {
    match lang {
        Language::ZhCn => "代理模式为 manual 时必须填写代理 URL".to_string(),
        Language::ZhTw => "代理模式為 manual 時必須填寫代理 URL".to_string(),
        Language::EnUs => "A proxy URL is required when the proxy mode is manual".to_string(),
    }
}

/// Invalid IP literal typed into the CDN manual override field.
pub fn format_invalid_ip(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "无效的 IP 地址格式",
        Language::ZhTw => "無效的 IP 位址格式",
        Language::EnUs => "Invalid IP address format",
    }
}

/// CDN status label shown before any speedtest ran / after clearing.
pub fn cdn_idle_label(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "未配置",
        Language::ZhTw => "未配置",
        Language::EnUs => "Not Configured",
    }
}

/// CDN status label shown when a node is applied and ready.
pub fn cdn_ready_label(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "准备就绪",
        Language::ZhTw => "準備就緒",
        Language::EnUs => "Ready",
    }
}

/// CDN speedtest failure fallback text.
pub fn cdn_test_failed_label(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "测速失败",
        Language::ZhTw => "測速失敗",
        Language::EnUs => "Speedtest failed",
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
        Language::ZhCn => format!(
            "已用缓存: {allocated} / 上限: {capacity} (活跃缓冲槽: {active_buffers} 个)"
        ),
        Language::ZhTw => format!(
            "已用快取: {allocated} / 上限: {capacity} (活躍快取槽: {active_buffers} 個)"
        ),
        Language::EnUs => format!(
            "Buffer in use: {allocated} / limit: {capacity} ({active_buffers} active slots)"
        ),
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

/// OS notification title shown when saving settings fails.
pub fn format_notification_settings_save_failed(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "保存设置失败",
        Language::ZhTw => "儲存設定失敗",
        Language::EnUs => "Failed to save settings",
    }
}

/// Priority label localized (high / normal / low).
pub fn format_priority_label(priority: limedl_core::types::Priority, lang: Language) -> &'static str {
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

/// Toast confirming a priority change (used by the priority popup menu).
pub fn format_toast_priority_set(
    file_name: &str,
    priority: limedl_core::types::Priority,
    lang: Language,
) -> String {
    let label = format_priority_label(priority, lang);
    match lang {
        Language::ZhCn => format!("已设置优先级: {label} — {file_name}"),
        Language::ZhTw => format!("已設定優先級: {label} — {file_name}"),
        Language::EnUs => format!("Priority set to {label} — {file_name}"),
    }
}

/// Toast shown when changing the priority failed.
pub fn format_toast_priority_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("设置优先级失败: {err}"),
        Language::ZhTw => format!("設定優先級失敗: {err}"),
        Language::EnUs => format!("Failed to set priority: {err}"),
    }
}

/// Toast shown when the user tries to deselect every BT file.
pub fn format_toast_bt_files_keep_one(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "至少需要保留一个文件",
        Language::ZhTw => "至少需要保留一個檔案",
        Language::EnUs => "At least one file must stay selected",
    }
}

/// Toast shown when updating the BT file selection failed.
pub fn format_toast_bt_files_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("更新 torrent 文件选择失败: {err}"),
        Language::ZhTw => format!("更新 torrent 檔案選擇失敗: {err}"),
        Language::EnUs => format!("Failed to update torrent file selection: {err}"),
    }
}

/// Toast shown after migrating data from the Tauri edition.
pub fn format_toast_tauri_migration(files: usize, lang: Language) -> String {
    match lang {
        Language::ZhCn => {
            format!("已从旧版 (Tauri) 导入 {files} 个文件：设置与任务记录已迁移")
        }
        Language::ZhTw => {
            format!("已從舊版 (Tauri) 匯入 {files} 個檔案：設定與任務記錄已遷移")
        }
        Language::EnUs => format!(
            "Imported {files} file(s) from the previous (Tauri) edition: settings and task history migrated"
        ),
    }
}

/// Toast shown after the "clear completed" action removed `count` records.
pub fn format_toast_clear_completed(count: usize, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("已清除 {count} 条已完成记录"),
        Language::ZhTw => format!("已清除 {count} 條已完成記錄"),
        Language::EnUs => format!("Cleared {count} completed record(s)"),
    }
}

/// Toast shown when "clear completed" found nothing to remove.
pub fn format_toast_clear_completed_none(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "没有已完成的记录需要清除",
        Language::ZhTw => "沒有已完成的記錄需要清除",
        Language::EnUs => "No completed records to clear",
    }
}

/// Toast shown when enumerating the task list failed.
pub fn format_toast_clear_completed_failed(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "清除已完成记录失败",
        Language::ZhTw => "清除已完成記錄失敗",
        Language::EnUs => "Failed to clear completed records",
    }
}

/// Toast shown after a successful factory reset (the app then restarts).
pub fn format_toast_factory_reset_done(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "已恢复出厂设置，正在重启...",
        Language::ZhTw => "已恢復原廠設定，正在重啟...",
        Language::EnUs => "Factory reset complete, restarting…",
    }
}

/// Toast shown when the factory reset could not delete the data directory.
pub fn format_toast_factory_reset_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("恢复出厂设置失败: {err}"),
        Language::ZhTw => format!("恢復原廠設定失敗: {err}"),
        Language::EnUs => format!("Factory reset failed: {err}"),
    }
}

pub fn format_toast_update_downloading(version: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("正在下载更新 v{version}..."),
        Language::ZhTw => format!("正在下載更新 v{version}..."),
        Language::EnUs => format!("Downloading update v{version}..."),
    }
}

pub fn format_toast_update_ready(version: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("v{version} 已下载并验证完成，重启应用后生效。"),
        Language::ZhTw => format!("v{version} 已下載並驗證完成，重啟應用程式後生效。"),
        Language::EnUs => format!("v{version} downloaded and verified. Restart to apply."),
    }
}

pub fn format_toast_update_installer_launched(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "安装程序已启动，即将退出应用以完成更新。",
        Language::ZhTw => "安裝程式已啟動，即將結束應用程式以完成更新。",
        Language::EnUs => "Installer launched. Exiting to complete update.",
    }
}

pub fn format_toast_update_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("更新失败: {err}"),
        Language::ZhTw => format!("更新失敗: {err}"),
        Language::EnUs => format!("Update failed: {err}"),
    }
}

pub fn format_toast_update_store_triggered(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "正在打开应用商店更新...",
        Language::ZhTw => "正在開啟應用程式商店更新...",
        Language::EnUs => "Opening Microsoft Store to update...",
    }
}

pub fn format_toast_update_not_found(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "未找到可用更新信息，请重新检查更新。",
        Language::ZhTw => "未找到可用更新資訊，請重新檢查更新。",
        Language::EnUs => "No update information found. Please check for updates again.",
    }
}

pub fn format_toast_update_up_to_date(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "当前已是最新版本。",
        Language::ZhTw => "目前已是最新版本。",
        Language::EnUs => "You are already on the latest version.",
    }
}

pub fn format_toast_update_available(version: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("发现新版本 v{version}，可在关于页面下载更新。"),
        Language::ZhTw => format!("發現新版本 v{version}，可在關於頁面下載更新。"),
        Language::EnUs => format!("New version v{version} available. Go to About to update."),
    }
}

pub fn format_toast_update_check_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("检查更新失败: {err}"),
        Language::ZhTw => format!("檢查更新失敗: {err}"),
        Language::EnUs => format!("Failed to check for updates: {err}"),
    }
}

pub fn format_toast_update_restart_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("重启失败: {err}"),
        Language::ZhTw => format!("重啟失敗: {err}"),
        Language::EnUs => format!("Failed to restart: {err}"),
    }
}

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

/// Default name of a freshly added custom URL rewrite rule.
pub fn new_rewrite_rule_name(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "新建自定义规则",
        Language::ZhTw => "新建自訂規則",
        Language::EnUs => "New Custom Rule",
    }
}

/// Localized display names for the built-in URL rewrite presets.
pub struct RewritePresetNames {
    pub github: &'static str,
    pub huggingface: &'static str,
    pub civitai: &'static str,
}

/// Built-in URL rewrite preset names localized (a preset-created rule keeps
/// this name until the user renames it).
pub fn get_rewrite_preset_names(lang: Language) -> RewritePresetNames {
    match lang {
        Language::ZhCn => RewritePresetNames {
            github: "GitHub 镜像代理",
            huggingface: "Hugging Face 镜像",
            civitai: "Civitai 镜像",
        },
        Language::ZhTw => RewritePresetNames {
            github: "GitHub 鏡像代理",
            huggingface: "Hugging Face 鏡像",
            civitai: "Civitai 鏡像",
        },
        Language::EnUs => RewritePresetNames {
            github: "GitHub Mirror Proxy",
            huggingface: "Hugging Face Mirror",
            civitai: "Civitai Mirror",
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_parsing() {
        assert_eq!(Language::from_code("zh"), Language::ZhCn);
        assert_eq!(Language::from_code("zh-CN"), Language::ZhCn);
        assert_eq!(Language::from_code("zh_CN"), Language::ZhCn);
        assert_eq!(Language::from_code("zh-TW"), Language::ZhTw);
        assert_eq!(Language::from_code("zh_TW"), Language::ZhTw);
        assert_eq!(Language::from_code("zh-HK"), Language::ZhTw);
        assert_eq!(Language::from_code("en"), Language::EnUs);
        assert_eq!(Language::from_code("en-US"), Language::EnUs);
        assert_eq!(Language::from_code("en_GB"), Language::EnUs);
    }

    #[test]
    fn test_format_eta_localized() {
        assert_eq!(format_eta(Some(45), Language::ZhCn), "剩余 45秒");
        assert_eq!(format_eta(Some(45), Language::ZhTw), "剩餘 45秒");
        assert_eq!(format_eta(Some(45), Language::EnUs), "45s left");
        assert_eq!(format_eta(Some(125), Language::ZhCn), "剩余 2分5秒");
        assert_eq!(format_eta(Some(125), Language::ZhTw), "剩餘 2分5秒");
        assert_eq!(format_eta(Some(125), Language::EnUs), "2m 5s left");
        assert_eq!(format_eta(Some(3665), Language::ZhCn), "剩余 1小时1分");
        assert_eq!(format_eta(Some(3665), Language::ZhTw), "剩餘 1小時1分");
        assert_eq!(format_eta(Some(3665), Language::EnUs), "1h 1m left");
        assert_eq!(format_eta(Some(90000), Language::ZhCn), "剩余 1天1小时");
        assert_eq!(format_eta(Some(90000), Language::ZhTw), "剩餘 1天1小時");
        assert_eq!(format_eta(Some(90000), Language::EnUs), "1d 1h left");
    }

    #[test]
    fn test_state_labels() {
        assert_eq!(format_state_label(&DownloadState::Downloading, Language::ZhCn), "下载中");
        assert_eq!(format_state_label(&DownloadState::Downloading, Language::ZhTw), "下載中");
        assert_eq!(format_state_label(&DownloadState::Downloading, Language::EnUs), "Downloading");
        assert_eq!(format_state_label(&DownloadState::Completed, Language::ZhCn), "已完成");
        assert_eq!(format_state_label(&DownloadState::Completed, Language::ZhTw), "已完成");
        assert_eq!(format_state_label(&DownloadState::Completed, Language::EnUs), "Completed");
    }

    #[test]
    fn test_settings_field_labels_localized() {
        assert_eq!(
            SettingsField::ListenPort.label(Language::ZhCn),
            "BT 监听端口"
        );
        assert_eq!(
            SettingsField::ListenPort.label(Language::ZhTw),
            "BT 監聽連接埠"
        );
        assert_eq!(SettingsField::ListenPort.label(Language::EnUs), "BT listen port");
        // Every label must be non-empty in all languages.
        let fields = [
            SettingsField::MaxRetries,
            SettingsField::IoBufferLimitMb,
            SettingsField::Aria2Port,
        ];
        for field in fields {
            assert!(!field.label(Language::ZhCn).is_empty());
            assert!(!field.label(Language::ZhTw).is_empty());
            assert!(!field.label(Language::EnUs).is_empty());
        }
    }

    #[test]
    fn test_validation_messages_localized() {
        let zh = format_validation_integer(Language::ZhCn, SettingsField::ListenPort, "abc");
        assert!(zh.contains("BT 监听端口") && zh.contains("'abc'"));
        let zh_tw = format_validation_integer(Language::ZhTw, SettingsField::ListenPort, "abc");
        assert!(zh_tw.contains("BT 監聽連接埠") && zh_tw.contains("'abc'"));
        let en = format_validation_integer(Language::EnUs, SettingsField::ListenPort, "abc");
        assert!(en.contains("BT listen port") && en.contains("'abc'"));
        // No CJK characters may leak into the English message.
        assert!(!en.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)));

        let port_en = format_validation_port(Language::EnUs, SettingsField::Aria2Port, "70000");
        assert!(port_en.contains("0-65535"));
        let required_en = format_proxy_url_required(Language::EnUs);
        assert!(!required_en.is_empty());
        assert!(!required_en.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)));
    }

    #[test]
    fn test_tray_menu_strings() {
        let zh = get_tray_strings(Language::ZhCn);
        assert_eq!(zh.show_window, "显示主窗口");
        let zh_tw = get_tray_strings(Language::ZhTw);
        assert_eq!(zh_tw.show_window, "顯示主視窗");
        let en = get_tray_strings(Language::EnUs);
        assert_eq!(en.show_window, "Show Main Window");
        assert!(en.speed_limit_toggle.contains("Speed Limit"));
        assert!(zh.speed_limit_toggle.contains("限速"));
        assert!(zh_tw.speed_limit_toggle.contains("限速"));
    }

    #[test]
    fn test_pick_torrent_filter() {
        assert_eq!(pick_torrent_filter(Language::ZhCn), "种子文件 (*.torrent)");
        assert_eq!(pick_torrent_filter(Language::ZhTw), "種子檔案 (*.torrent)");
        assert_eq!(pick_torrent_filter(Language::EnUs), "Torrent Files (*.torrent)");
    }

    #[test]
    fn test_all_slint_tr_strings_in_po_catalogs() {
        use std::collections::HashMap;
        use std::path::{Path, PathBuf};

        let base_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let ui_dir = base_dir.join("ui");
        let zh_po = base_dir.join("lang/zh_CN/LC_MESSAGES/limedl-native.po");
        let zh_tw_po = base_dir.join("lang/zh_TW/LC_MESSAGES/limedl-native.po");
        let en_po = base_dir.join("lang/en/LC_MESSAGES/limedl-native.po");

        fn parse_po(path: &Path) -> HashMap<String, String> {
            let content = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("Failed to read PO file {:?}: {e}", path));
            let mut entries = HashMap::new();
            let mut current_id: Option<String> = None;
            let mut mode = None;

            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("msgid \"") {
                    if let Some(s) = rest.strip_suffix('"') {
                        current_id = Some(s.replace("\\\"", "\""));
                        mode = Some("id");
                    }
                } else if let Some(rest) = trimmed.strip_prefix("msgstr \"") {
                    if let Some(s) = rest.strip_suffix('"') {
                        if let Some(id) = &current_id {
                            entries.insert(id.clone(), s.replace("\\\"", "\""));
                        }
                        mode = Some("str");
                    }
                } else if let Some(stripped) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                    let val = stripped.replace("\\\"", "\"");
                    match mode {
                        Some("id") => {
                            if let Some(id) = &mut current_id {
                                id.push_str(&val);
                            }
                        }
                        Some("str") => {
                            if let Some(str_val) = current_id.as_ref().and_then(|id| entries.get_mut(id)) {
                                str_val.push_str(&val);
                            }
                        }
                        _ => {}
                    }
                } else if trimmed.is_empty() {
                    current_id = None;
                    mode = None;
                }
            }
            entries
        }

        let zh_entries = parse_po(&zh_po);
        let zh_tw_entries = parse_po(&zh_tw_po);
        let en_entries = parse_po(&en_po);

        let mut slint_files = Vec::new();
        fn collect_slint(dir: &Path, list: &mut Vec<PathBuf>) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        collect_slint(&p, list);
                    } else if p.extension().is_some_and(|ext| ext == "slint") {
                        list.push(p);
                    }
                }
            }
        }
        collect_slint(&ui_dir, &mut slint_files);

        fn extract_tr_strings(text: &str) -> Vec<String> {
            let mut results = Vec::new();
            let mut rest = text;
            while let Some(pos) = rest.find("@tr(") {
                rest = &rest[pos + 4..];
                let trimmed = rest.trim_start();
                if let Some(inner) = trimmed.strip_prefix('"') {
                    let mut escaped = false;
                    let mut end_idx = None;
                    for (idx, ch) in inner.char_indices() {
                        if escaped {
                            escaped = false;
                        } else if ch == '\\' {
                            escaped = true;
                        } else if ch == '"' {
                            end_idx = Some(idx);
                            break;
                        }
                    }
                    if let Some(idx) = end_idx {
                        let msg = &inner[..idx];
                        results.push(msg.replace("\\\"", "\"").replace("\\n", "\n"));
                        rest = &inner[idx + 1..];
                    }
                }
            }
            results
        }

        let mut missing_zh = Vec::new();
        let mut missing_zh_tw = Vec::new();
        let mut missing_en = Vec::new();

        for file in &slint_files {
            let content = std::fs::read_to_string(file).expect("read slint file");
            for msgid in extract_tr_strings(&content) {
                if !zh_entries.contains_key(&msgid) {
                    missing_zh.push((file.clone(), msgid.clone()));
                }
                if !zh_tw_entries.contains_key(&msgid) {
                    missing_zh_tw.push((file.clone(), msgid.clone()));
                }
                if !en_entries.contains_key(&msgid) {
                    missing_en.push((file.clone(), msgid));
                }
            }
        }

        assert!(
            missing_zh.is_empty(),
            "Missing translations in zh_CN PO catalog: {missing_zh:#?}"
        );
        assert!(
            missing_zh_tw.is_empty(),
            "Missing translations in zh_TW PO catalog: {missing_zh_tw:#?}"
        );
        assert!(
            missing_en.is_empty(),
            "Missing translations in en PO catalog: {missing_en:#?}"
        );
    }
}
