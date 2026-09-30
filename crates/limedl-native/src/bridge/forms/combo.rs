//! Canonical option lists for the settings dialog ComboBoxes.

pub const COLOR_MODES: [&str; 3] = ["system", "light", "dark"];
pub const THEME_COLORS: [&str; 3] = ["amber", "sky", "lime"];
pub const OPACITY_PRESETS: [&str; 3] = ["default", "acrylic", "frosted"];
pub const LANGUAGES: [&str; 3] = ["zh-CN", "zh-TW", "en-US"];
pub const CLOSE_BEHAVIORS: [&str; 2] = ["minimizeToTray", "exit"];
pub const DOUBLE_CLICK_COMPLETED: [&str; 4] =
    ["none", "open_file", "open_in_explorer", "open_download_dir"];
pub const DOUBLE_CLICK_UNCOMPLETED: [&str; 2] = ["none", "toggle_pause_resume"];
pub const CHECKSUMS: [&str; 5] = ["blake3", "sha256", "xxh3_128", "none", "sha1"];
pub const PROXY_MODES: [&str; 3] = ["disabled", "system", "manual"];
pub const SCHEDULER_MODES: [&str; 2] = ["automatic", "traditional"];
pub const ADAPTIVE_PROFILES: [&str; 3] = ["conservative", "balanced", "aggressive"];
pub const CHUNK_STRATEGIES: [&str; 2] = ["adaptive", "fixed"];
pub const ENCRYPTION_MODES: [&str; 3] = ["enabled", "disabled", "forced"];
pub const PREALLOC_MODES: [&str; 2] = ["none", "full"];
pub const ANTI_LEECH_ACTIONS: [&str; 2] = ["ban", "limit_slots"];
pub const SEED_CHOKING: [&str; 3] = ["fastest_upload", "round_robin", "anti_leech"];
pub const CHOKING_ALGOS: [&str; 2] = ["fixed_slots", "rate_based"];
pub const LOG_LEVELS: [&str; 5] = ["trace", "debug", "info", "warn", "error"];

/// Index of `value` in `list`; `0` when missing (Slint ComboBox default).
pub fn idx_of(list: impl AsRef<[&'static str]>, value: &str) -> i32 {
    list.as_ref()
        .iter()
        .position(|v| *v == value)
        .map_or(0, |i| i as i32)
}

/// Canonical value at `index`; `list[0]` when out of range.
pub fn value_at(list: impl AsRef<[&'static str]>, index: i32) -> &'static str {
    let list = list.as_ref();
    list.get(index.max(0) as usize).copied().unwrap_or(list[0])
}
