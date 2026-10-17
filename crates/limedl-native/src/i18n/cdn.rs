//! CDN acceleration labels.

use super::{Language, tr};

/// CDN status label localized.
pub fn format_cdn_status_label(is_testing: bool, lang: Language) -> &'static str {
    if is_testing {
        tr("Testing", lang)
    } else {
        tr("Ready", lang)
    }
}

/// CDN phase label localized.
pub fn format_cdn_phase_label(is_testing: bool, lang: Language) -> &'static str {
    if is_testing {
        tr("Measuring candidate edge nodes", lang)
    } else {
        tr("Speedtest finished", lang)
    }
}

/// CDN benchmark node text localized.
pub fn format_cdn_default_node(lang: Language) -> &'static str {
    tr("Direct DNS (Benchmark)", lang)
}

/// CDN status label shown before any speedtest ran / after clearing.
pub fn cdn_idle_label(lang: Language) -> &'static str {
    tr("Not Configured", lang)
}

/// CDN status label shown when a node is applied and ready.
pub fn cdn_ready_label(lang: Language) -> &'static str {
    tr("Ready", lang)
}

/// CDN speedtest failure fallback text.
pub fn cdn_test_failed_label(lang: Language) -> &'static str {
    tr("Speedtest failed", lang)
}

/// CDN phase shown while the IP ranges are being fetched.
pub fn cdn_phase_fetching_label(lang: Language) -> &'static str {
    tr("Fetching IP ranges", lang)
}

/// CDN phase label for a phase key emitted by the accelerator, falling back to
/// the raw key for phases this client does not know yet.
pub fn cdn_phase_label(phase: &str, lang: Language) -> String {
    match phase {
        "fetchingRanges" => cdn_phase_fetching_label(lang).to_string(),
        "screening" => tr("Screening latency", lang).to_string(),
        "measuringThroughput" => tr("Measuring bandwidth", lang).to_string(),
        other => other.to_string(),
    }
}

/// CDN status label shown after the user cancels a running speedtest.
pub fn cdn_cancelled_label(lang: Language) -> &'static str {
    tr("Cancelled", lang)
}
