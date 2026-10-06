//! Enum <-> persisted-string conversions for the settings form.

use limedl_core::types::{
    AdaptiveProfile, Aria2AuthMode, BackgroundOpacityPreset, BtAntiLeechAction, BtChokingAlgorithm,
    BtEncryptionMode, BtPreallocateMode, BtSeedChokingAlgorithm, ChecksumMode, ChunkSizeStrategy,
    CloseBehavior, ColorMode, DoubleClickOnCompleted, DoubleClickOnUncompleted, LogLevel, ProxyMode,
    SchedulerMode, ThemeColor,
};
use slint::SharedString;

pub(crate) fn aria2_auth_mode_to_str(m: Aria2AuthMode) -> SharedString {
    SharedString::from(match m {
        Aria2AuthMode::Single => "single",
        Aria2AuthMode::PerClient => "per_client",
    })
}
pub(crate) fn str_to_aria2_auth_mode(s: &str) -> Option<Aria2AuthMode> {
    match s.trim() {
        "single" => Some(Aria2AuthMode::Single),
        "per_client" => Some(Aria2AuthMode::PerClient),
        _ => None,
    }
}

pub(crate) fn proxy_mode_to_str(m: ProxyMode) -> SharedString {
    SharedString::from(match m {
        ProxyMode::Disabled => "disabled",
        ProxyMode::System => "system",
        ProxyMode::Manual => "manual",
    })
}
pub(crate) fn str_to_proxy_mode(s: &str) -> Option<ProxyMode> {
    match s.trim() {
        "disabled" => Some(ProxyMode::Disabled),
        "system" => Some(ProxyMode::System),
        "manual" => Some(ProxyMode::Manual),
        _ => None,
    }
}
pub(crate) fn scheduler_mode_to_str(m: SchedulerMode) -> SharedString {
    SharedString::from(match m {
        SchedulerMode::Traditional => "traditional",
        SchedulerMode::Automatic => "automatic",
    })
}
pub(crate) fn adaptive_profile_to_str(p: AdaptiveProfile) -> SharedString {
    SharedString::from(match p {
        AdaptiveProfile::Conservative => "conservative",
        AdaptiveProfile::Balanced => "balanced",
        AdaptiveProfile::Aggressive => "aggressive",
    })
}
pub(crate) fn chunk_strategy_to_str(s: ChunkSizeStrategy) -> SharedString {
    SharedString::from(match s {
        ChunkSizeStrategy::Adaptive => "adaptive",
        ChunkSizeStrategy::Fixed => "fixed",
    })
}
pub(crate) fn checksum_to_str(c: ChecksumMode) -> SharedString {
    SharedString::from(match c {
        ChecksumMode::Blake3 => "blake3",
        ChecksumMode::Sha256 => "sha256",
        ChecksumMode::Sha512 => "sha512",
        ChecksumMode::None => "none",
    })
}
pub(crate) fn str_to_checksum(s: &str) -> Option<ChecksumMode> {
    match s.trim() {
        "blake3" => Some(ChecksumMode::Blake3),
        "sha256" => Some(ChecksumMode::Sha256),
        "sha512" => Some(ChecksumMode::Sha512),
        "none" => Some(ChecksumMode::None),
        _ => None,
    }
}
pub(crate) fn color_mode_to_str(c: &ColorMode) -> SharedString {
    SharedString::from(match c {
        ColorMode::System => "system",
        ColorMode::Light => "light",
        ColorMode::Dark => "dark",
    })
}
pub(crate) fn theme_color_to_str(c: &ThemeColor) -> SharedString {
    SharedString::from(match c {
        ThemeColor::Amber => "amber",
        ThemeColor::Sky => "sky",
        ThemeColor::Lime => "lime",
        ThemeColor::Violet => "violet",
        ThemeColor::Monochrome => "monochrome",
    })
}
pub(crate) fn close_behavior_to_str(c: &CloseBehavior) -> SharedString {
    SharedString::from(match c {
        CloseBehavior::MinimizeToTray => "minimizeToTray",
        CloseBehavior::Exit => "exit",
    })
}
pub(crate) fn log_level_to_str(l: LogLevel) -> SharedString {
    SharedString::from(match l {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    })
}
pub(crate) fn str_to_log_level(s: &str) -> Option<LogLevel> {
    match s.trim() {
        "trace" => Some(LogLevel::Trace),
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warn" => Some(LogLevel::Warn),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}
pub(crate) fn encryption_to_str(m: BtEncryptionMode) -> SharedString {
    SharedString::from(match m {
        BtEncryptionMode::Enabled => "enabled",
        BtEncryptionMode::Disabled => "disabled",
        BtEncryptionMode::Forced => "forced",
    })
}
pub(crate) fn preallocate_to_str(m: BtPreallocateMode) -> SharedString {
    SharedString::from(match m {
        BtPreallocateMode::None => "none",
        BtPreallocateMode::Full => "full",
    })
}
pub(crate) fn background_opacity_to_str(v: &BackgroundOpacityPreset) -> SharedString {
    SharedString::from(match v {
        BackgroundOpacityPreset::Default => "default",
        BackgroundOpacityPreset::Acrylic => "acrylic",
        BackgroundOpacityPreset::Frosted => "frosted",
    })
}
pub(crate) fn str_to_background_opacity(s: &str) -> BackgroundOpacityPreset {
    match s {
        "acrylic" => BackgroundOpacityPreset::Acrylic,
        "frosted" => BackgroundOpacityPreset::Frosted,
        _ => BackgroundOpacityPreset::Default,
    }
}
pub(crate) fn double_click_completed_to_str(v: DoubleClickOnCompleted) -> SharedString {
    SharedString::from(match v {
        DoubleClickOnCompleted::None => "none",
        DoubleClickOnCompleted::OpenFile => "open_file",
        DoubleClickOnCompleted::OpenInExplorer => "open_in_explorer",
        DoubleClickOnCompleted::OpenDownloadDir => "open_download_dir",
    })
}
pub(crate) fn double_click_uncompleted_to_str(v: DoubleClickOnUncompleted) -> SharedString {
    SharedString::from(match v {
        DoubleClickOnUncompleted::None => "none",
        DoubleClickOnUncompleted::TogglePauseResume => "toggle_pause_resume",
    })
}
pub(crate) fn anti_leech_action_to_str(v: BtAntiLeechAction) -> SharedString {
    SharedString::from(match v {
        BtAntiLeechAction::Ban => "ban",
        BtAntiLeechAction::LimitSlots => "limit_slots",
    })
}
pub(crate) fn seed_choking_to_str(v: BtSeedChokingAlgorithm) -> SharedString {
    SharedString::from(match v {
        BtSeedChokingAlgorithm::FastestUpload => "fastest_upload",
        BtSeedChokingAlgorithm::RoundRobin => "round_robin",
        BtSeedChokingAlgorithm::AntiLeech => "anti_leech",
    })
}
pub(crate) fn choking_to_str(v: BtChokingAlgorithm) -> SharedString {
    SharedString::from(match v {
        BtChokingAlgorithm::FixedSlots => "fixed_slots",
        BtChokingAlgorithm::RateBased => "rate_based",
    })
}
