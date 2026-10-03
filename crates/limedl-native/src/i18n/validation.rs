//! Settings field labels and validation error messages.

use super::Language;

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
    DiskOverridePath,
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
            (F::DiskOverridePath, Language::ZhCn) => "目录介质覆盖-路径",
            (F::DiskOverridePath, Language::ZhTw) => "目錄介質覆蓋-路徑",
            (F::DiskOverridePath, Language::EnUs) => "Media override directory",
        }
    }
}

/// "expected a number" validation error for `field`.
pub fn format_validation_number(lang: Language, field: SettingsField, value: &str) -> String {
    match lang {
        Language::ZhCn => format!("{} 格式错误: '{}' 请输入有效数字", field.label(lang), value),
        Language::ZhTw => format!("{} 格式錯誤: '{}' 請輸入有效數字", field.label(lang), value),
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
        Language::ZhCn => format!("{} 格式错误: '{}' 请输入有效整数", field.label(lang), value),
        Language::ZhTw => format!("{} 格式錯誤: '{}' 請輸入有效整數", field.label(lang), value),
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
pub fn format_validation_range(lang: Language, field: SettingsField, min: u32, max: u32) -> String {
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

/// "expected an absolute path" validation error for `field`.
///
/// A relative key can never match a download destination — the engine's lookup
/// compares normalized absolute paths — and Windows detection silently answers
/// "SSD" for one, so the editor must not persist it.
pub fn format_validation_absolute_path(lang: Language, field: SettingsField, value: &str) -> String {
    match lang {
        Language::ZhCn => format!(
            "{} 必须是绝对路径: '{value}'（例如 D:\\Downloads 或 \\\\NAS\\share）",
            field.label(lang)
        ),
        Language::ZhTw => format!(
            "{} 必須是絕對路徑: '{value}'（例如 D:\\Downloads 或 \\\\NAS\\share）",
            field.label(lang)
        ),
        Language::EnUs => format!(
            "{} must be an absolute path: '{value}' (e.g. D:\\Downloads or \\\\NAS\\share)",
            field.label(lang)
        ),
    }
}

/// Two rows of a list editor point at the same path.
pub fn format_validation_duplicate_path(lang: Language, value: &str) -> String {
    match lang {
        Language::ZhCn => format!("重复的路径: '{value}' — 同一目录只能配置一次"),
        Language::ZhTw => format!("重複的路徑: '{value}' — 同一目錄只能設定一次"),
        Language::EnUs => format!("Duplicate path: '{value}' — one entry per directory"),
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

/// A per-client Aria2 token row was left without a name.
pub fn format_validation_client_name_empty(lang: Language) -> String {
    match lang {
        Language::ZhCn => "客户端名称不能为空".to_string(),
        Language::ZhTw => "用戶端名稱不能為空".to_string(),
        Language::EnUs => "Client name cannot be empty".to_string(),
    }
}

/// Two per-client Aria2 rows share a name.
pub fn format_validation_client_name_duplicate(lang: Language, name: &str) -> String {
    match lang {
        Language::ZhCn => format!("客户端名称重复: '{name}'"),
        Language::ZhTw => format!("用戶端名稱重複: '{name}'"),
        Language::EnUs => format!("Duplicate client name: '{name}'"),
    }
}

/// A per-client Aria2 row has no token (its generation failed or it was
/// hand-edited).
pub fn format_validation_client_token_missing(lang: Language, name: &str) -> String {
    match lang {
        Language::ZhCn => format!("客户端 '{name}' 缺少令牌，请重新生成"),
        Language::ZhTw => format!("用戶端 '{name}' 缺少權杖，請重新產生"),
        Language::EnUs => format!("Client '{name}' has no token — regenerate it"),
    }
}

/// Default label for a freshly added Aria2 client row.
pub fn format_aria2_client_default_name(index: usize, lang: Language) -> String {
    match lang {
        Language::ZhCn => format!("客户端 {index}"),
        Language::ZhTw => format!("用戶端 {index}"),
        Language::EnUs => format!("Client {index}"),
    }
}
