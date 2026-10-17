//! URL rewrite rule names and presets.

use super::{Language, tr};

/// Default name of a freshly added custom URL rewrite rule.
pub fn new_rewrite_rule_name(lang: Language) -> &'static str {
    tr("New Custom Rule", lang)
}

/// Localized display names for the built-in URL rewrite presets.
pub struct RewritePresetNames {
    pub github: &'static str,
    pub huggingface: &'static str,
    pub civitai: &'static str,
}

/// Built-in URL rewrite preset names localized (a preset-created rule keeps
/// this name until the user renames it).
pub fn get_rewrite_preset_names(lang: Language) -> RewritePresetNames {
    RewritePresetNames {
        github: tr("GitHub Mirror Proxy", lang),
        huggingface: tr("Hugging Face Mirror", lang),
        civitai: tr("Civitai Mirror", lang),
    }
}
