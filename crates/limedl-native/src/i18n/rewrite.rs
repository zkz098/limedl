//! URL rewrite rule names and presets.

use super::*;

/// Default name of a freshly added custom URL rewrite rule.
pub fn new_rewrite_rule_name(lang: Language) -> &'static str {
    match lang {
        Language::ZhCn => "新建自定义规则",
        Language::ZhTw => "新建自訂規則",
        Language::EnUs => "New Custom Rule",
    }
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
    match lang {
        Language::ZhCn => RewritePresetNames {
            github: "GitHub 镜像代理",
            huggingface: "Hugging Face 镜像",
            civitai: "Civitai 镜像",
        },
        Language::ZhTw => RewritePresetNames {
            github: "GitHub 鏡像代理",
            huggingface: "Hugging Face 鏡像",
            civitai: "Civitai 鏡像",
        },
        Language::EnUs => RewritePresetNames {
            github: "GitHub Mirror Proxy",
            huggingface: "Hugging Face Mirror",
            civitai: "Civitai Mirror",
        },
    }
}
