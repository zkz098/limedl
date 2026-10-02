use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use regex::Regex;
use reqwest::Url;

use super::types::{MatchType, ReplacementMode, RewriteTarget, UrlRewriteRule, UrlRewriteSettings};

/// Percent-encode every byte except RFC 3986 unreserved characters
/// (alphanumerics and `-`, `_`, `.`, `~`).
pub const URL_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Robust wildcard pattern matching supporting `*` (any sequence) and `?` (any single character).
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p_chars: Vec<char> = pattern.chars().collect();
    let t_chars: Vec<char> = text.chars().collect();
    let mut p_idx = 0;
    let mut t_idx = 0;
    let mut star_idx = None;
    let mut match_idx = 0;

    while t_idx < t_chars.len() {
        if p_idx < p_chars.len() && (p_chars[p_idx] == '?' || p_chars[p_idx] == t_chars[t_idx]) {
            p_idx += 1;
            t_idx += 1;
        } else if p_idx < p_chars.len() && p_chars[p_idx] == '*' {
            star_idx = Some(p_idx);
            p_idx += 1;
            match_idx = t_idx;
        } else if let Some(star) = star_idx {
            p_idx = star + 1;
            match_idx += 1;
            t_idx = match_idx;
        } else {
            return false;
        }
    }

    while p_idx < p_chars.len() && p_chars[p_idx] == '*' {
        p_idx += 1;
    }

    p_idx == p_chars.len()
}

/// Check whether the given URL matches a specific rewrite rule.
pub fn matches_rule(url: &str, rule: &UrlRewriteRule) -> bool {
    if !rule.enabled || rule.pattern.trim().is_empty() {
        return false;
    }

    let pattern = rule.pattern.trim();

    match rule.match_type {
        MatchType::Host => {
            let parsed = match Url::parse(url) {
                Ok(p) => p,
                Err(_) => return false,
            };
            let host = match parsed.host_str() {
                Some(h) => h.to_lowercase(),
                None => return false,
            };
            let pat_lower = pattern.to_lowercase();

            if let Some(suffix) = pat_lower.strip_prefix("*.") {
                host == suffix || host.ends_with(&pat_lower[1..])
            } else if pat_lower.contains('*') || pat_lower.contains('?') {
                wildcard_match(&pat_lower, &host)
            } else {
                host == pat_lower
            }
        }
        MatchType::Prefix => url.starts_with(pattern),
        MatchType::Regex => match Regex::new(pattern) {
            Ok(re) => re.is_match(url),
            Err(_) => false,
        },
        MatchType::Wildcard => wildcard_match(pattern, url),
    }
}

/// Produce the list of candidate URLs to try for a download according to configured rewrite rules.
///
/// If rewriting is disabled or no rule matches, returns a single-element vector containing the original URL.
/// When a rule matches:
/// - Candidate URLs are generated from active targets in priority order.
/// - If `fallback_to_original` is true, the original URL is appended to the end of the list.
pub fn rewrite_url(url: &str, settings: &UrlRewriteSettings) -> Vec<String> {
    if !settings.enabled {
        return vec![url.to_string()];
    }

    for rule in enabled_rules_sorted(settings) {
        if !matches_rule(url, rule) {
            continue;
        }
        if let Some(candidates) = candidates_for_rule(url, rule) {
            return candidates;
        }
    }

    vec![url.to_string()]
}

/// Enabled rules, in evaluation order.
fn enabled_rules_sorted(settings: &UrlRewriteSettings) -> Vec<&UrlRewriteRule> {
    let mut rules: Vec<&UrlRewriteRule> = settings.rules.iter().filter(|r| r.enabled).collect();
    rules.sort_by_key(|r| r.order);
    rules
}

/// Candidate URLs for one matching rule, or `None` when the rule yields nothing.
fn candidates_for_rule(url: &str, rule: &UrlRewriteRule) -> Option<Vec<String>> {
    let targets = active_targets_sorted(rule);
    if targets.is_empty() {
        return None;
    }

    let encoded_url = if rule.encode_url {
        utf8_percent_encode(url, URL_ENCODE_SET).to_string()
    } else {
        url.to_string()
    };

    let mut candidates: Vec<String> = Vec::new();
    for target in targets {
        let generated = render_target(url, rule, target, &encoded_url);
        if !generated.is_empty() && !candidates.contains(&generated) {
            candidates.push(generated);
        }
    }

    if rule.fallback_to_original && !candidates.iter().any(|c| c == url) {
        candidates.push(url.to_string());
    }

    (!candidates.is_empty()).then_some(candidates)
}

/// Enabled targets of `rule`, in priority order.
fn active_targets_sorted(rule: &UrlRewriteRule) -> Vec<&RewriteTarget> {
    let mut targets: Vec<&RewriteTarget> = rule
        .targets
        .iter()
        .filter(|t| t.enabled && !t.url_template.trim().is_empty())
        .collect();
    targets.sort_by_key(|t| t.order);
    targets
}

/// Render one target into a candidate URL.
fn render_target(
    url: &str,
    rule: &UrlRewriteRule,
    target: &RewriteTarget,
    encoded_url: &str,
) -> String {
    match rule.replacement_mode {
        ReplacementMode::PrefixProxy => {
            let base = target.url_template.trim().trim_end_matches('/');
            let target_url = if rule.encode_url { encoded_url } else { url };
            format!("{base}/{target_url}")
        }
        ReplacementMode::Template => {
            let template = target.url_template.trim();
            if rule.match_type == MatchType::Regex
                && let Ok(re) = Regex::new(rule.pattern.trim())
            {
                re.replace_all(url, template).to_string()
            } else {
                apply_template(template, encoded_url, url)
            }
        }
    }
}

/// Substitute `{url}` / `{raw_url}` placeholders in a template.
fn apply_template(template: &str, encoded_url: &str, url: &str) -> String {
    template
        .replace("{url}", encoded_url)
        .replace("{raw_url}", url)
}

#[cfg(test)]
mod tests;
