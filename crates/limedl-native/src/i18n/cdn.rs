//! CDN acceleration labels.

use super::*;

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

/// CDN phase shown while the IP ranges are being fetched.
pub fn cdn_phase_fetching_label(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "获取网段",
        Language::ZhTw => "獲取網段",
        Language::EnUs => "Fetching IP ranges",
    }
}

/// CDN phase label for a phase key emitted by the accelerator, falling back to
/// the raw key for phases this client does not know yet.
pub fn cdn_phase_label(phase: &str, lang: Language) -> String {
    match phase {
        "fetchingRanges" => cdn_phase_fetching_label(lang).to_string(),
        "screening" => match lang {
            Language::ZhCn => "延迟初筛",
            Language::ZhTw => "延遲初篩",
            Language::EnUs => "Screening latency",
        }
        .to_string(),
        "measuringThroughput" => match lang {
            Language::ZhCn => "带宽测速",
            Language::ZhTw => "頻寬測速",
            Language::EnUs => "Measuring bandwidth",
        }
        .to_string(),
        other => other.to_string(),
    }
}

/// CDN status label shown after the user cancels a running speedtest.
pub fn cdn_cancelled_label(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "已取消",
        Language::ZhTw => "已取消",
        Language::EnUs => "Cancelled",
    }
}
