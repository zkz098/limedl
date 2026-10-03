//! In-app toast text for every feature area.

use super::{Language, format_priority_label};

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

pub fn format_toast_aria2_token_copied(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "令牌已复制",
        Language::ZhTw => "權杖已複製",
        Language::EnUs => "Token copied",
    }
}

pub fn format_toast_aria2_token_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("生成令牌失败: {err}"),
        Language::ZhTw => format!("產生權杖失敗: {err}"),
        Language::EnUs => format!("Failed to generate token: {err}"),
    }
}

pub fn format_toast_autostart_failed(err: &str, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("自启动设置失败: {err}"),
        Language::ZhTw => format!("自啟動設定失敗: {err}"),
        Language::EnUs => format!("Autostart failed: {err}"),
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
