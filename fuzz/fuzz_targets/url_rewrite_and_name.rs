#![no_main]

use libfuzzer_sys::fuzz_target;
use limedl_types::types::{
    MatchType, ReplacementMode, RewriteTarget, UrlRewriteRule, UrlRewriteSettings,
};
use limedl_types::url_rewrite::{rewrite_url, wildcard_match};

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    let _ = limedl_engine::download::initial_file_name_from_url(input);

    // `pattern\0text` lets one input drive both sides of the wildcard matcher;
    // without a separator the input is both pattern and text.
    let (pattern, text) = input.split_once('\0').unwrap_or((input, input));
    let _ = wildcard_match(pattern, text);

    for match_type in [
        MatchType::Host,
        MatchType::Prefix,
        MatchType::Regex,
        MatchType::Wildcard,
    ] {
        let rule = UrlRewriteRule {
            id: "fuzz".into(),
            name: "fuzz".into(),
            enabled: true,
            match_type,
            pattern: pattern.to_string(),
            replacement_mode: ReplacementMode::Template,
            targets: vec![RewriteTarget {
                url_template: "https://mirror.example/{url}".into(),
                enabled: true,
                order: 0,
            }],
            encode_url: true,
            fallback_to_original: true,
            order: 0,
        };
        let settings = UrlRewriteSettings {
            enabled: true,
            rules: vec![rule],
        };
        let _ = rewrite_url(text, &settings);
    }
});
