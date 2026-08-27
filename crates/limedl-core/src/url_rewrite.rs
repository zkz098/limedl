use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use regex::Regex;
use reqwest::Url;

use super::types::{MatchType, ReplacementMode, UrlRewriteRule, UrlRewriteSettings};

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

    let mut rules: Vec<&UrlRewriteRule> = settings.rules.iter().filter(|r| r.enabled).collect();
    rules.sort_by_key(|r| r.order);

    for rule in rules {
        if matches_rule(url, rule) {
            let mut active_targets: Vec<_> = rule
                .targets
                .iter()
                .filter(|t| t.enabled && !t.url_template.trim().is_empty())
                .collect();
            active_targets.sort_by_key(|t| t.order);

            if active_targets.is_empty() {
                continue;
            }

            let encoded_url = if rule.encode_url {
                utf8_percent_encode(url, URL_ENCODE_SET).to_string()
            } else {
                url.to_string()
            };

            let mut candidates: Vec<String> = Vec::new();

            for target in active_targets {
                let generated = match rule.replacement_mode {
                    ReplacementMode::PrefixProxy => {
                        let base = target.url_template.trim().trim_end_matches('/');
                        let target_url = if rule.encode_url {
                            &encoded_url
                        } else {
                            url
                        };
                        format!("{base}/{target_url}")
                    }
                    ReplacementMode::Template => {
                        let template = target.url_template.trim();
                        if rule.match_type == MatchType::Regex {
                            if let Ok(re) = Regex::new(rule.pattern.trim()) {
                                re.replace_all(url, template).to_string()
                            } else {
                                template
                                    .replace("{url}", &encoded_url)
                                    .replace("{raw_url}", url)
                            }
                        } else {
                            template
                                .replace("{url}", &encoded_url)
                                .replace("{raw_url}", url)
                        }
                    }
                };

                if !generated.is_empty() && !candidates.contains(&generated) {
                    candidates.push(generated);
                }
            }

            if rule.fallback_to_original && !candidates.iter().any(|c| c == url) {
                candidates.push(url.to_string());
            }

            if !candidates.is_empty() {
                return candidates;
            }
        }
    }

    vec![url.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MatchType, ReplacementMode, RewriteTarget, UrlRewriteRule, UrlRewriteSettings};

    #[test]
    fn test_wildcard_matching() {
        assert!(wildcard_match("*.github.com", "api.github.com"));
        assert!(wildcard_match("https://github.com/*/releases/*", "https://github.com/user/repo/releases/v1.0"));
        assert!(!wildcard_match("https://github.com/*/releases/*", "https://gitlab.com/user/repo/releases/v1.0"));
        assert!(wildcard_match("file_???.zip", "file_001.zip"));
        assert!(!wildcard_match("file_???.zip", "file_1.zip"));
    }

    #[test]
    fn test_disabled_settings_returns_original() {
        let settings = UrlRewriteSettings {
            enabled: false,
            rules: vec![UrlRewriteRule {
                id: "rule1".into(),
                name: "Rule 1".into(),
                enabled: true,
                match_type: MatchType::Host,
                pattern: "github.com".into(),
                replacement_mode: ReplacementMode::PrefixProxy,
                targets: vec![RewriteTarget {
                    url_template: "https://mirror.example.com".into(),
                    enabled: true,
                    order: 0,
                }],
                encode_url: true,
                fallback_to_original: true,
                order: 0,
            }],
        };

        let result = rewrite_url("https://github.com/user/repo/file.zip", &settings);
        assert_eq!(result, vec!["https://github.com/user/repo/file.zip"]);
    }

    #[test]
    fn test_host_matching_and_prefix_proxy() {
        let settings = UrlRewriteSettings {
            enabled: true,
            rules: vec![UrlRewriteRule {
                id: "gh".into(),
                name: "GitHub Mirror".into(),
                enabled: true,
                match_type: MatchType::Host,
                pattern: "*.github.com".into(),
                replacement_mode: ReplacementMode::PrefixProxy,
                targets: vec![
                    RewriteTarget {
                        url_template: "https://mirror1.example.com".into(),
                        enabled: true,
                        order: 0,
                    },
                    RewriteTarget {
                        url_template: "https://mirror2.example.com/".into(),
                        enabled: true,
                        order: 1,
                    },
                ],
                encode_url: true,
                fallback_to_original: true,
                order: 0,
            }],
        };

        let url = "https://raw.github.com/user/repo/file.zip";
        let result = rewrite_url(url, &settings);
        assert_eq!(
            result,
            vec![
                "https://mirror1.example.com/https%3A%2F%2Fraw.github.com%2Fuser%2Frepo%2Ffile.zip",
                "https://mirror2.example.com/https%3A%2F%2Fraw.github.com%2Fuser%2Frepo%2Ffile.zip",
                "https://raw.github.com/user/repo/file.zip"
            ]
        );
    }

    #[test]
    fn test_regex_matching_and_template_replacement() {
        let settings = UrlRewriteSettings {
            enabled: true,
            rules: vec![UrlRewriteRule {
                id: "hf".into(),
                name: "Hugging Face Mirror".into(),
                enabled: true,
                match_type: MatchType::Regex,
                pattern: r"^https://huggingface\.co/(.*)".into(),
                replacement_mode: ReplacementMode::Template,
                targets: vec![RewriteTarget {
                    url_template: "https://hf-mirror.com/$1".into(),
                    enabled: true,
                    order: 0,
                }],
                encode_url: false,
                fallback_to_original: true,
                order: 0,
            }],
        };

        let url = "https://huggingface.co/bert-base-uncased/resolve/main/pytorch_model.bin";
        let result = rewrite_url(url, &settings);
        assert_eq!(
            result,
            vec![
                "https://hf-mirror.com/bert-base-uncased/resolve/main/pytorch_model.bin",
                "https://huggingface.co/bert-base-uncased/resolve/main/pytorch_model.bin"
            ]
        );
    }

    #[test]
    fn test_prefix_matching() {
        let settings = UrlRewriteSettings {
            enabled: true,
            rules: vec![UrlRewriteRule {
                id: "pfx".into(),
                name: "Prefix Rule".into(),
                enabled: true,
                match_type: MatchType::Prefix,
                pattern: "https://example.com/downloads/".into(),
                replacement_mode: ReplacementMode::PrefixProxy,
                targets: vec![RewriteTarget {
                    url_template: "https://cdn.example.com".into(),
                    enabled: true,
                    order: 0,
                }],
                encode_url: false,
                fallback_to_original: false,
                order: 0,
            }],
        };

        let url = "https://example.com/downloads/setup.exe";
        let result = rewrite_url(url, &settings);
        assert_eq!(result, vec!["https://cdn.example.com/https://example.com/downloads/setup.exe"]);
    }

    #[test]
    fn test_invalid_regex_does_not_panic() {
        let settings = UrlRewriteSettings {
            enabled: true,
            rules: vec![UrlRewriteRule {
                id: "invalid_re".into(),
                name: "Bad Regex".into(),
                enabled: true,
                match_type: MatchType::Regex,
                pattern: "[unclosed".into(),
                replacement_mode: ReplacementMode::Template,
                targets: vec![RewriteTarget {
                    url_template: "https://fallback.com".into(),
                    enabled: true,
                    order: 0,
                }],
                encode_url: false,
                fallback_to_original: true,
                order: 0,
            }],
        };

        let url = "https://example.com/file.zip";
        let result = rewrite_url(url, &settings);
        assert_eq!(result, vec!["https://example.com/file.zip"]);
    }
}
