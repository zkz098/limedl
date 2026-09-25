use std::collections::HashSet;

use limedl_core::cdn::speed_test::SpeedTestResult;
use limedl_core::types::{
    AppSettings, MatchType, ReplacementMode, RewriteTarget, UrlRewriteRule, UrlRewriteSettings,
};
use slint::{Model, ModelRc, SharedString, VecModel};

use crate::i18n::{self, Language};
use crate::{CdnCandidateItem, LabsFormData, UrlRewriteRuleItem, UrlRewriteTargetItem};
use super::format_timestamp_ms;

#[allow(clippy::too_many_arguments)]
pub fn app_settings_to_labs_form(
    settings: &AppSettings,
    is_testing: bool,
    phase_label: &str,
    progress_percent: f32,
    progress_label: &str,
    speed_improvement: Option<&str>,
    latency_improvement: Option<&str>,
    default_node_text: Option<&str>,
    ranges_text: &str,
    show_advanced: bool,
    test_url: &str,
    matched_rule: &str,
    candidates: &[String],
    lang: Language,
) -> LabsFormData {
    let cdn = &settings.cdn_acceleration;
    let (status_type, status_label) = if is_testing {
        ("testing", match lang { Language::ZhCn => "测速中", Language::ZhTw => "測速中", Language::EnUs => "Testing" })
    } else if cdn.last_error.is_some() {
        ("error", match lang { Language::ZhCn => "测速失败", Language::ZhTw => "測速失敗", Language::EnUs => "Failed" })
    } else if cdn.active_ip.is_some() {
        ("ready", match lang { Language::ZhCn => "准备就绪", Language::ZhTw => "準備就緒", Language::EnUs => "Ready" })
    } else {
        ("idle", match lang { Language::ZhCn => "未配置", Language::ZhTw => "未配置", Language::EnUs => "Not Configured" })
    };

    let active_speed_text = cdn
        .active_speed_mbps
        .map(|s| format!("{s:.2} MB/s"))
        .unwrap_or_default();

    let last_test_time = cdn
        .last_test_at_ms
        .map(format_timestamp_ms)
        .unwrap_or_default();

    LabsFormData {
        cdn_enabled: cdn.enabled,
        cdn_provider: SharedString::from(if cdn.provider.is_empty() { "cloudflare" } else { &cdn.provider }),
        cdn_custom_test_url: SharedString::from(cdn.custom_test_url.as_deref().unwrap_or_default()),
        cdn_custom_cidrs: SharedString::from(cdn.custom_cidrs.as_deref().unwrap_or_default()),
        cdn_status_type: SharedString::from(status_type),
        cdn_status_label: SharedString::from(status_label),
        cdn_is_testing: is_testing,
        cdn_phase_label: SharedString::from(phase_label),
        cdn_progress_percent: progress_percent,
        cdn_progress_label: SharedString::from(progress_label),
        cdn_active_ip: SharedString::from(cdn.active_ip.as_deref().unwrap_or_default()),
        cdn_active_speed_text: SharedString::from(active_speed_text),
        cdn_last_test_time: SharedString::from(last_test_time),
        cdn_speed_improvement_text: SharedString::from(speed_improvement.unwrap_or_default()),
        cdn_latency_improvement_text: SharedString::from(latency_improvement.unwrap_or_default()),
        cdn_default_node_text: SharedString::from(default_node_text.unwrap_or_default()),
        cdn_last_error: SharedString::from(cdn.last_error.as_deref().unwrap_or_default()),
        cdn_show_advanced: show_advanced,
        cdn_manual_ip: SharedString::default(),
        cdn_manual_ip_error: SharedString::default(),
        cdn_ranges_text: SharedString::from(ranges_text),

        url_rewrite_enabled: settings.url_rewrite.enabled,
        url_rewrite_test_url: SharedString::from(test_url),
        url_rewrite_test_matched_rule: SharedString::from(matched_rule),
        url_rewrite_test_candidates_count: candidates.len() as i32,
        url_rewrite_test_result_1: SharedString::from(candidates.first().cloned().unwrap_or_default()),
        url_rewrite_test_result_2: SharedString::from(candidates.get(1).cloned().unwrap_or_default()),
        url_rewrite_test_result_3: SharedString::from(candidates.get(2).cloned().unwrap_or_default()),
    }
}

pub fn update_app_settings_from_labs_form(settings: &mut AppSettings, form: &LabsFormData) {
    settings.cdn_acceleration.enabled = form.cdn_enabled;
    settings.cdn_acceleration.provider = form.cdn_provider.trim().to_string();
    let test_url = form.cdn_custom_test_url.trim().to_string();
    settings.cdn_acceleration.custom_test_url = if test_url.is_empty() { None } else { Some(test_url) };
    let cidrs = form.cdn_custom_cidrs.trim().to_string();
    settings.cdn_acceleration.custom_cidrs = if cidrs.is_empty() { None } else { Some(cidrs) };
    settings.url_rewrite.enabled = form.url_rewrite_enabled;
}

pub fn cdn_candidates_to_slint(
    candidates: &[SpeedTestResult],
    active_ip: &str,
) -> ModelRc<CdnCandidateItem> {
    let items: Vec<CdnCandidateItem> = candidates
        .iter()
        .map(|c| {
            let ip_str = c.ip.to_string();
            let is_active = !ip_str.is_empty() && ip_str == active_ip;
            let latency_text = format!("{:.1} ms", c.tcp_latency_ms);
            let throughput_text = match c.throughput_mbps {
                Some(tp) if tp > 0.0 => format!("{:.2} MB/s", tp),
                _ => "-".to_string(),
            };
            let is_failed = c.error.is_some();

            CdnCandidateItem {
                ip: SharedString::from(ip_str),
                latency_text: SharedString::from(latency_text),
                throughput_text: SharedString::from(throughput_text),
                throughput_mbps: c.throughput_mbps.unwrap_or(0.0) as f32,
                is_active,
                is_failed,
            }
        })
        .collect();

    ModelRc::new(VecModel::from(items))
}

pub fn match_type_to_str(m: MatchType) -> &'static str {
    match m {
        MatchType::Host => "host",
        MatchType::Prefix => "prefix",
        MatchType::Regex => "regex",
        MatchType::Wildcard => "wildcard",
    }
}

pub fn str_to_match_type(s: &str) -> MatchType {
    match s {
        "prefix" => MatchType::Prefix,
        "regex" => MatchType::Regex,
        "wildcard" => MatchType::Wildcard,
        _ => MatchType::Host,
    }
}

pub fn replacement_mode_to_str(m: ReplacementMode) -> &'static str {
    match m {
        ReplacementMode::PrefixProxy => "prefix_proxy",
        ReplacementMode::Template => "template",
    }
}

pub fn str_to_replacement_mode(s: &str) -> ReplacementMode {
    match s {
        "template" => ReplacementMode::Template,
        _ => ReplacementMode::PrefixProxy,
    }
}

pub fn url_rewrite_rules_to_slint(
    rules: &[UrlRewriteRule],
    expanded_ids: &HashSet<String>,
) -> ModelRc<UrlRewriteRuleItem> {
    let items: Vec<UrlRewriteRuleItem> = rules
        .iter()
        .map(|r| {
            let is_expanded = expanded_ids.contains(&r.id);
            let target_items: Vec<UrlRewriteTargetItem> = r
                .targets
                .iter()
                .map(|t| UrlRewriteTargetItem {
                    url_template: SharedString::from(&t.url_template),
                    enabled: t.enabled,
                    order: t.order as i32,
                })
                .collect();

            UrlRewriteRuleItem {
                id: SharedString::from(&r.id),
                name: SharedString::from(&r.name),
                enabled: r.enabled,
                match_type: SharedString::from(match_type_to_str(r.match_type)),
                pattern: SharedString::from(&r.pattern),
                replacement_mode: SharedString::from(replacement_mode_to_str(r.replacement_mode)),
                encode_url: r.encode_url,
                fallback_to_original: r.fallback_to_original,
                order: r.order as i32,
                targets: ModelRc::new(VecModel::from(target_items)),
                is_expanded,
            }
        })
        .collect();

    ModelRc::new(VecModel::from(items))
}

#[allow(dead_code)]
pub fn slint_to_url_rewrite_rules(models: &[UrlRewriteRuleItem]) -> Vec<UrlRewriteRule> {
    models
        .iter()
        .map(|m| {
            let mut targets = Vec::new();
            for i in 0..m.targets.row_count() {
                if let Some(t) = m.targets.row_data(i) {
                    targets.push(RewriteTarget {
                        url_template: t.url_template.to_string(),
                        enabled: t.enabled,
                        order: t.order as u32,
                    });
                }
            }

            UrlRewriteRule {
                id: m.id.to_string(),
                name: m.name.to_string(),
                enabled: m.enabled,
                match_type: str_to_match_type(m.match_type.as_str()),
                pattern: m.pattern.to_string(),
                replacement_mode: str_to_replacement_mode(m.replacement_mode.as_str()),
                encode_url: m.encode_url,
                fallback_to_original: m.fallback_to_original,
                order: m.order as u32,
                targets,
            }
        })
        .collect()
}

pub fn evaluate_url_rewrite(rules: &[UrlRewriteRule], test_url: &str) -> (String, Vec<String>) {
    let trimmed = test_url.trim();
    if trimmed.is_empty() {
        return (String::new(), Vec::new());
    }

    let mut matched_rule_name = String::new();
    let mut enabled_rules: Vec<&UrlRewriteRule> = rules.iter().filter(|r| r.enabled).collect();
    enabled_rules.sort_by_key(|r| r.order);

    for rule in &enabled_rules {
        if limedl_core::url_rewrite::matches_rule(trimmed, rule) {
            matched_rule_name = rule.name.clone();
            break;
        }
    }

    let settings = UrlRewriteSettings {
        enabled: true,
        rules: rules.to_vec(),
    };
    let candidates = limedl_core::url_rewrite::rewrite_url(trimmed, &settings);

    (matched_rule_name, candidates)
}

pub fn create_url_rewrite_preset(preset_key: &str, lang: Language) -> Option<UrlRewriteRule> {
    let presets = i18n::get_rewrite_preset_names(lang);
    match preset_key {
        "github" => Some(UrlRewriteRule {
            id: format!("preset-gh-{}", uuid::Uuid::new_v4().simple()),
            name: presets.github.to_string(),
            enabled: true,
            match_type: MatchType::Host,
            pattern: "*.github.com".to_string(),
            replacement_mode: ReplacementMode::PrefixProxy,
            encode_url: true,
            fallback_to_original: true,
            order: 0,
            targets: vec![
                RewriteTarget {
                    url_template: "https://ghproxy.net".to_string(),
                    enabled: true,
                    order: 0,
                },
                RewriteTarget {
                    url_template: "https://mirror.ghproxy.cc".to_string(),
                    enabled: true,
                    order: 1,
                },
            ],
        }),
        "huggingface" => Some(UrlRewriteRule {
            id: format!("preset-hf-{}", uuid::Uuid::new_v4().simple()),
            name: presets.huggingface.to_string(),
            enabled: true,
            match_type: MatchType::Regex,
            pattern: r"^https://huggingface\.co/(.*)$".to_string(),
            replacement_mode: ReplacementMode::Template,
            encode_url: false,
            fallback_to_original: true,
            order: 1,
            targets: vec![
                RewriteTarget {
                    url_template: "https://hf-mirror.com/$1".to_string(),
                    enabled: true,
                    order: 0,
                },
            ],
        }),
        "civitai" => Some(UrlRewriteRule {
            id: format!("preset-civitai-{}", uuid::Uuid::new_v4().simple()),
            name: presets.civitai.to_string(),
            enabled: true,
            match_type: MatchType::Host,
            pattern: "*.civitai.com".to_string(),
            replacement_mode: ReplacementMode::PrefixProxy,
            encode_url: true,
            fallback_to_original: true,
            order: 2,
            targets: vec![
                RewriteTarget {
                    url_template: "https://civitai.work".to_string(),
                    enabled: true,
                    order: 0,
                },
            ],
        }),
        _ => None,
    }
}
