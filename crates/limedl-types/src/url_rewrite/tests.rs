use super::*;
use crate::types::{MatchType, ReplacementMode, RewriteTarget, UrlRewriteRule, UrlRewriteSettings};
use proptest::prelude::*;
use std::collections::HashSet;

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

/// Regression: the `*` branch must be checked before literal equality, or a
/// `*` in the text is consumed as a literal match of the pattern's `*`.
#[test]
fn wildcard_handles_stars_in_the_text() {
    assert!(wildcard_match("*", "*0"));
    assert!(wildcard_match("*0", "*0"));
    assert!(wildcard_match("a*b", "a*b"));
    assert!(wildcard_match("*", "***"));
    assert!(wildcard_match("a?b", "a*b"));
}

fn any_text() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..64).prop_map(|chars| chars.into_iter().collect())
}

fn literal_text() -> impl Strategy<Value = String> {
    prop::collection::vec(
        any::<char>().prop_filter("not a wildcard", |c| *c != '*' && *c != '?'),
        0..64,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

fn rule(
    match_type: MatchType,
    pattern: &str,
    templates: &[&str],
    fallback: bool,
    encode_url: bool,
) -> UrlRewriteRule {
    UrlRewriteRule {
        id: "prop".into(),
        name: "prop".into(),
        enabled: true,
        match_type,
        pattern: pattern.into(),
        replacement_mode: ReplacementMode::PrefixProxy,
        targets: templates
            .iter()
            .enumerate()
            .map(|(order, template)| RewriteTarget {
                url_template: (*template).into(),
                enabled: true,
                order: order as u32,
            })
            .collect(),
        encode_url,
        fallback_to_original: fallback,
        order: 0,
    }
}

proptest! {
    #[test]
    fn wildcard_literal_patterns_match_themselves(text in literal_text()) {
        prop_assert!(wildcard_match(&text, &text));
    }

    #[test]
    fn wildcard_star_matches_everything(text in any_text()) {
        prop_assert!(wildcard_match("*", &text));
    }

    #[test]
    fn wildcard_question_matches_exactly_one_character(c in any::<char>()) {
        prop_assert!(wildcard_match("?", &c.to_string()));
        prop_assert!(!wildcard_match("??", &c.to_string()));
    }

    #[test]
    fn empty_pattern_matches_only_empty_input(text in any_text()) {
        prop_assert_eq!(wildcard_match("", &text), text.is_empty());
    }

    #[test]
    fn rewrite_returns_the_original_when_disabled(url in any_text()) {
        let disabled = UrlRewriteSettings {
            enabled: false,
            rules: vec![rule(MatchType::Wildcard, "*", &["https://mirror.example"], true, true)],
        };
        prop_assert_eq!(rewrite_url(&url, &disabled), vec![url.clone()]);

        let no_rules = UrlRewriteSettings {
            enabled: true,
            rules: Vec::new(),
        };
        prop_assert_eq!(rewrite_url(&url, &no_rules), vec![url]);
    }

    #[test]
    fn rewrite_candidates_are_nonempty_unique_and_keep_the_fallback(url in any_text()) {
        let settings = UrlRewriteSettings {
            enabled: true,
            rules: vec![rule(
                MatchType::Wildcard,
                "*",
                &["https://mirror1.example", "https://mirror2.example"],
                true,
                true,
            )],
        };
        let candidates = rewrite_url(&url, &settings);
        prop_assert!(!candidates.is_empty());
        let unique: HashSet<&String> = candidates.iter().collect();
        prop_assert_eq!(unique.len(), candidates.len());
        prop_assert!(candidates.iter().any(|candidate| candidate == &url));
    }

    #[test]
    fn regex_rules_never_panic(pattern in any_text(), url in any_text()) {
        let settings = UrlRewriteSettings {
            enabled: true,
            rules: vec![rule(MatchType::Regex, &pattern, &["https://mirror.example"], true, false)],
        };
        let candidates = rewrite_url(&url, &settings);
        prop_assert!(!candidates.is_empty());
    }

    #[test]
    fn host_rules_never_panic(pattern in any_text(), url in any_text()) {
        let settings = UrlRewriteSettings {
            enabled: true,
            rules: vec![rule(MatchType::Host, &pattern, &["https://mirror.example"], false, false)],
        };
        let candidates = rewrite_url(&url, &settings);
        prop_assert!(!candidates.is_empty());
    }
}
