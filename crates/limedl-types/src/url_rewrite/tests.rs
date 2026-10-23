use super::*;
use crate::types::{MatchType, ReplacementMode, RewriteTarget, UrlRewriteRule, UrlRewriteSettings};

#[test]
fn test_wildcard_matching() {
    assert!(wildcard_match("*.github.com", "api.github.com"));
    assert!(wildcard_match(
        "https://github.com/*/releases/*",
        "https://github.com/user/repo/releases/v1.0"
    ));
    assert!(!wildcard_match(
        "https://github.com/*/releases/*",
        "https://gitlab.com/user/repo/releases/v1.0"
    ));
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
    assert_eq!(
        result,
        vec!["https://cdn.example.com/https://example.com/downloads/setup.exe"]
    );
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
