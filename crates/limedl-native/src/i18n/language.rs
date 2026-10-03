//! Language enum and translation-catalog switching.

/// Supported languages in the limedl native desktop client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    ZhCn,
    ZhTw,
    EnUs,
}

impl Language {
    /// Parse from language code (e.g., "zh", "zh-CN", "zh_CN", "en", "en-US", "system").
    pub fn from_code(code: &str) -> Self {
        let code_trimmed = code.trim().to_lowercase();
        if code_trimmed.starts_with("zh") {
            if code_trimmed.contains("tw")
                || code_trimmed.contains("hk")
                || code_trimmed.contains("mo")
                || code_trimmed.contains("hant")
            {
                Language::ZhTw
            } else {
                Language::ZhCn
            }
        } else if code_trimmed.starts_with("en") {
            Language::EnUs
        } else {
            Language::detect_system()
        }
    }

    /// Detect system locale automatically via `sys_locale`.
    pub fn detect_system() -> Self {
        if let Some(locale) = sys_locale::get_locale() {
            let loc = locale.to_lowercase();
            if loc.starts_with("zh") {
                if loc.contains("tw")
                    || loc.contains("hk")
                    || loc.contains("mo")
                    || loc.contains("hant")
                {
                    return Language::ZhTw;
                }
                return Language::ZhCn;
            }
        }
        Language::EnUs
    }

    /// Bundled translation locale code used by Slint (matches directory in lang/).
    pub fn as_code(&self) -> &'static str {
        match self {
            Language::ZhCn => "zh_CN",
            Language::ZhTw => "zh_TW",
            Language::EnUs => "en",
        }
    }

    /// Standard BCP-47 tag for serialization in settings.
    pub fn as_bcp47(&self) -> &'static str {
        match self {
            Language::ZhCn => "zh-CN",
            Language::ZhTw => "zh-TW",
            Language::EnUs => "en-US",
        }
    }
}

/// Activate bundled translation catalog in Slint runtime.
/// Must be called after the first Slint component has been created.
pub fn apply_translation(lang: Language) {
    if let Err(e) = slint::select_bundled_translation(lang.as_code()) {
        tracing::warn!(
            "Failed to select Slint translation '{}': {e}",
            lang.as_code()
        );
    }
}
