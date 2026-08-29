use std::collections::{HashMap, HashSet};

use limedl_core::cdn::speed_test::SpeedTestResult;
use limedl_core::types::{
    AdaptiveProfile, AppSettings, BackgroundOpacityPreset, BtAntiLeechAction, BtChokingAlgorithm,
    BtEncryptionMode, BtFileStatus, BtPeerInfo, BtPieceInfo, BtPreallocateMode,
    BtSeedChokingAlgorithm, BtTrackerInfo, ChecksumMode, ChunkSizeStrategy, CloseBehavior,
    ColorMode, DiskType, DoubleClickOnCompleted, DoubleClickOnUncompleted, DownloadProgress,
    DownloadState, DownloadSummary, LogLevel, MatchType, ProxyMode, ReplacementMode, RewriteTarget,
    SchedulerMode, TaskKind, ThemeColor, UrlRewriteRule, UrlRewriteSettings,
};
use slint::{Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};

use crate::i18n::{self, Language};
use crate::{
    CdnCandidateItem, InspectorInfo, LabsFormData, PeerItem, SettingsFormData, TaskItem,
    TorrentFileItem, TrackerItem, UrlRewriteRuleItem, UrlRewriteTargetItem,
};

/// Human-readable byte formatting.
pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;

    if bytes >= TIB {
        format!("{:.2} TB", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.2} GB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Format download speed in bytes/second.
pub fn format_speed(speed: Option<f64>) -> String {
    match speed {
        Some(s) if s > 0.0 => format!("{}/s", format_bytes(s as u64)),
        _ => String::new(),
    }
}

/// Format ETA seconds.
pub fn format_eta(eta: Option<u64>, lang: Language) -> String {
    i18n::format_eta(eta, lang)
}

/// Supported sort fields for task list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortField {
    #[default]
    Created = 0,
    Size = 1,
    Speed = 2,
    Progress = 3,
    Name = 4,
}

impl From<i32> for SortField {
    fn from(val: i32) -> Self {
        match val {
            1 => SortField::Size,
            2 => SortField::Speed,
            3 => SortField::Progress,
            4 => SortField::Name,
            _ => SortField::Created,
        }
    }
}

/// Convert a `DownloadSummary` into a Slint `TaskItem`.
pub fn summary_to_task_item(summary: &DownloadSummary, selected: bool, lang: Language) -> TaskItem {
    let (state_code, can_pause, can_resume, is_completed, is_failed) = match summary.state {
        DownloadState::Downloading => ("downloading", true, false, false, false),
        DownloadState::Paused => ("paused", false, true, false, false),
        DownloadState::Completed => ("completed", false, false, true, false),
        DownloadState::Failed => ("failed", false, true, false, true),
        DownloadState::Canceled => ("failed", false, true, false, true),
        DownloadState::Queued => ("queued", true, false, false, false),
        DownloadState::Retrying => ("downloading", true, false, false, false),
        DownloadState::Verifying => ("verifying", false, false, false, false),
    };
    let state_label = i18n::format_state_label(&summary.state, lang);

    let progress = match summary.total_bytes {
        Some(total) if total > 0 => {
            (summary.downloaded_bytes as f64 / total as f64).clamp(0.0, 1.0) as f32
        }
        _ => {
            if is_completed {
                1.0
            } else {
                0.0
            }
        }
    };

    let size_text = match summary.total_bytes {
        Some(total) => format!("{} / {}", format_bytes(summary.downloaded_bytes), format_bytes(total)),
        None => format_bytes(summary.downloaded_bytes),
    };

    let kind_str = match summary.kind {
        TaskKind::Http => "http",
        TaskKind::Bt => "bt",
    };

    TaskItem {
        id: SharedString::from(&summary.id),
        kind: SharedString::from(kind_str),
        file_name: SharedString::from(&summary.file_name),
        url: SharedString::from(&summary.url),
        state_code: SharedString::from(state_code),
        state_label: SharedString::from(state_label),
        progress,
        speed_text: SharedString::from(format_speed(summary.speed_bytes_per_second)),
        size_text: SharedString::from(size_text),
        eta_text: SharedString::from(format_eta(summary.eta_seconds, lang)),
        can_pause,
        can_resume,
        is_completed,
        is_failed,
        selected,
    }
}

/// Convert a `DownloadSummary` into a Slint `InspectorInfo`.
pub fn summary_to_inspector_info(summary: &DownloadSummary, lang: Language) -> InspectorInfo {
    let state_label = i18n::format_state_label(&summary.state, lang);

    let progress = match summary.total_bytes {
        Some(total) if total > 0 => {
            (summary.downloaded_bytes as f64 / total as f64).clamp(0.0, 1.0) as f32
        }
        _ => {
            if matches!(summary.state, DownloadState::Completed) {
                1.0
            } else {
                0.0
            }
        }
    };

    let kind_str = match summary.kind {
        TaskKind::Http => "http",
        TaskKind::Bt => "bt",
    };

    let total_size_text = summary
        .total_bytes
        .map(format_bytes)
        .unwrap_or_else(|| i18n::format_unknown(lang).to_string());
    let downloaded_size_text = format_bytes(summary.downloaded_bytes);
    let uploaded_size_text = summary.uploaded_bytes.map(format_bytes).unwrap_or_default();

    let threads_text = i18n::format_threads_text(
        Some(match summary.thread_mode {
            limedl_core::types::ThreadMode::Adaptive => "Adaptive",
            limedl_core::types::ThreadMode::Fixed => "Fixed",
        }),
        summary.allocated_thread_count.unwrap_or(1),
        lang,
    );

    let seed_leech_text = i18n::format_seed_leech(summary.seed_count, summary.leech_count, lang);

    InspectorInfo {
        id: SharedString::from(&summary.id),
        kind: SharedString::from(kind_str),
        file_name: SharedString::from(&summary.file_name),
        url: SharedString::from(&summary.url),
        destination_path: SharedString::from(&summary.destination_path),
        state_label: SharedString::from(state_label),
        speed_text: SharedString::from(format_speed(summary.speed_bytes_per_second)),
        upload_speed_text: SharedString::from(format_speed(summary.upload_speed_bytes_per_second)),
        total_size_text: SharedString::from(total_size_text),
        downloaded_size_text: SharedString::from(downloaded_size_text),
        uploaded_size_text: SharedString::from(uploaded_size_text),
        eta_text: SharedString::from(format_eta(summary.eta_seconds, lang)),
        progress,
        connection_count: summary.connection_count as i32,
        threads_text: SharedString::from(threads_text),
        info_hash_text: SharedString::from(summary.info_hash.clone().unwrap_or_default()),
        seed_leech_text: SharedString::from(seed_leech_text),
        error_text: SharedString::from(summary.error.clone().unwrap_or_default()),
    }
}

/// Convert `BtPeerInfo` to Slint `PeerItem`.
pub fn peer_info_to_item(peer: &BtPeerInfo) -> PeerItem {
    PeerItem {
        address: SharedString::from(&peer.address),
        client: SharedString::from(&peer.client),
        flags: SharedString::from(&peer.flags),
        download_speed: SharedString::from(format_speed(Some(peer.download_speed))),
        upload_speed: SharedString::from(format_speed(Some(peer.upload_speed))),
        progress: peer.progress.clamp(0.0, 1.0) as f32,
    }
}

/// Convert `BtTrackerInfo` to Slint `TrackerItem`.
pub fn tracker_info_to_item(tracker: &BtTrackerInfo) -> TrackerItem {
    TrackerItem {
        url: SharedString::from(&tracker.url),
    }
}

/// Convert `BtFileStatus` to Slint `TorrentFileItem`.
pub fn file_status_to_item(file: &BtFileStatus) -> TorrentFileItem {
    let progress = if file.size > 0 {
        (file.downloaded_bytes as f64 / file.size as f64).clamp(0.0, 1.0) as f32
    } else {
        1.0
    };

    TorrentFileItem {
        index: file.index as i32,
        path: SharedString::from(&file.path),
        size_text: SharedString::from(format_bytes(file.size)),
        downloaded_text: SharedString::from(format_bytes(file.downloaded_bytes)),
        progress,
        included: file.included,
    }
}

/// Generate a dynamic piece map bitmap image and summary label from `BtPieceInfo` slice.
pub fn generate_piece_map_image(pieces: &[BtPieceInfo], lang: Language) -> (Image, String) {
    if pieces.is_empty() {
        let buf = SharedPixelBuffer::<Rgba8Pixel>::new(1, 1);
        return (Image::from_rgba8(buf), i18n::format_piece_map_summary(0, 0, 0.0, lang));
    }

    let total = pieces.len();
    let completed = pieces.iter().filter(|p| p.completed).count();
    let percent = if total > 0 {
        (completed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let summary_text = i18n::format_piece_map_summary(completed, total, percent, lang);

    let cols: u32 = if total > 2000 {
        64
    } else if total > 500 {
        48
    } else if total > 100 {
        32
    } else {
        24
    };

    let rows: u32 = (total as u32).div_ceil(cols).max(1);
    let cell_size: u32 = if rows > 40 { 6 } else if rows > 20 { 8 } else { 10 };
    let padding: u32 = 1;

    let width = cols * cell_size;
    let height = rows * cell_size;

    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    let slice = buffer.make_mut_slice();

    // Background color: #0f1114
    let bg_pixel = Rgba8Pixel { r: 15, g: 17, b: 20, a: 255 };
    slice.fill(bg_pixel);

    // Color definitions
    let completed_pixel = Rgba8Pixel { r: 132, g: 204, b: 22, a: 255 }; // #84cc16
    let pending_pixel = Rgba8Pixel { r: 38, g: 42, b: 49, a: 255 };    // #262a31

    for (idx, piece) in pieces.iter().enumerate() {
        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;

        let x_start = col * cell_size;
        let y_start = row * cell_size;
        let x_end = (x_start + cell_size).saturating_sub(padding).min(width);
        let y_end = (y_start + cell_size).saturating_sub(padding).min(height);

        let color = if piece.completed {
            completed_pixel
        } else {
            pending_pixel
        };

        for y in y_start..y_end {
            for x in x_start..x_end {
                let pixel_idx = (y * width + x) as usize;
                if pixel_idx < slice.len() {
                    slice[pixel_idx] = color;
                }
            }
        }
    }

    (Image::from_rgba8(buffer), summary_text)
}

/// Canonical option lists for the settings dialog ComboBoxes (persisted values).
/// The display labels live in `ui/components/settings_dialog.slint` via `@tr` and
/// MUST keep the same order as these arrays. Form structs carry the selected
/// index (see `SettingsFormData` in `ui/types.slint`), never the raw string.
pub(crate) mod combo {
    pub const COLOR_MODES: [&str; 3] = ["system", "light", "dark"];
    pub const THEME_COLORS: [&str; 3] = ["amber", "sky", "lime"];
    pub const OPACITY_PRESETS: [&str; 3] = ["default", "acrylic", "frosted"];
    pub const LANGUAGES: [&str; 2] = ["zh-CN", "en-US"];
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
}

fn proxy_mode_to_str(m: ProxyMode) -> SharedString {
    SharedString::from(match m {
        ProxyMode::Disabled => "disabled",
        ProxyMode::System => "system",
        ProxyMode::Manual => "manual",
    })
}
fn str_to_proxy_mode(s: &str) -> Option<ProxyMode> {
    match s.trim() {
        "disabled" => Some(ProxyMode::Disabled),
        "system" => Some(ProxyMode::System),
        "manual" => Some(ProxyMode::Manual),
        _ => None,
    }
}
fn scheduler_mode_to_str(m: SchedulerMode) -> SharedString {
    SharedString::from(match m {
        SchedulerMode::Traditional => "traditional",
        SchedulerMode::Automatic => "automatic",
    })
}
fn adaptive_profile_to_str(p: AdaptiveProfile) -> SharedString {
    SharedString::from(match p {
        AdaptiveProfile::Conservative => "conservative",
        AdaptiveProfile::Balanced => "balanced",
        AdaptiveProfile::Aggressive => "aggressive",
    })
}
fn chunk_strategy_to_str(s: ChunkSizeStrategy) -> SharedString {
    SharedString::from(match s {
        ChunkSizeStrategy::Adaptive => "adaptive",
        ChunkSizeStrategy::Fixed => "fixed",
    })
}
fn checksum_to_str(c: ChecksumMode) -> SharedString {
    SharedString::from(match c {
        ChecksumMode::Blake3 => "blake3",
        ChecksumMode::Sha256 => "sha256",
        ChecksumMode::Xxh3128 => "xxh3_128",
        ChecksumMode::None => "none",
        ChecksumMode::Sha1 => "sha1",
    })
}
fn str_to_checksum(s: &str) -> Option<ChecksumMode> {
    match s.trim() {
        "blake3" => Some(ChecksumMode::Blake3),
        "sha256" => Some(ChecksumMode::Sha256),
        "xxh3_128" => Some(ChecksumMode::Xxh3128),
        "none" => Some(ChecksumMode::None),
        "sha1" => Some(ChecksumMode::Sha1),
        _ => None,
    }
}
fn color_mode_to_str(c: &ColorMode) -> SharedString {
    SharedString::from(match c {
        ColorMode::System => "system",
        ColorMode::Light => "light",
        ColorMode::Dark => "dark",
    })
}
fn theme_color_to_str(c: &ThemeColor) -> SharedString {
    SharedString::from(match c {
        ThemeColor::Amber => "amber",
        ThemeColor::Sky => "sky",
        ThemeColor::Lime => "lime",
    })
}
fn close_behavior_to_str(c: &CloseBehavior) -> SharedString {
    SharedString::from(match c {
        CloseBehavior::MinimizeToTray => "minimizeToTray",
        CloseBehavior::Exit => "exit",
    })
}
fn log_level_to_str(l: LogLevel) -> SharedString {
    SharedString::from(match l {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    })
}
fn str_to_log_level(s: &str) -> Option<LogLevel> {
    match s.trim() {
        "trace" => Some(LogLevel::Trace),
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warn" => Some(LogLevel::Warn),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}
fn encryption_to_str(m: BtEncryptionMode) -> SharedString {
    SharedString::from(match m {
        BtEncryptionMode::Enabled => "enabled",
        BtEncryptionMode::Disabled => "disabled",
        BtEncryptionMode::Forced => "forced",
    })
}
fn preallocate_to_str(m: BtPreallocateMode) -> SharedString {
    SharedString::from(match m {
        BtPreallocateMode::None => "none",
        BtPreallocateMode::Full => "full",
    })
}
fn background_opacity_to_str(v: &BackgroundOpacityPreset) -> SharedString {
    SharedString::from(match v {
        BackgroundOpacityPreset::Default => "default",
        BackgroundOpacityPreset::Acrylic => "acrylic",
        BackgroundOpacityPreset::Frosted => "frosted",
    })
}
fn str_to_background_opacity(s: &str) -> BackgroundOpacityPreset {
    match s {
        "acrylic" => BackgroundOpacityPreset::Acrylic,
        "frosted" => BackgroundOpacityPreset::Frosted,
        _ => BackgroundOpacityPreset::Default,
    }
}
fn double_click_completed_to_str(v: DoubleClickOnCompleted) -> SharedString {
    SharedString::from(match v {
        DoubleClickOnCompleted::None => "none",
        DoubleClickOnCompleted::OpenFile => "open_file",
        DoubleClickOnCompleted::OpenInExplorer => "open_in_explorer",
        DoubleClickOnCompleted::OpenDownloadDir => "open_download_dir",
    })
}
fn double_click_uncompleted_to_str(v: DoubleClickOnUncompleted) -> SharedString {
    SharedString::from(match v {
        DoubleClickOnUncompleted::None => "none",
        DoubleClickOnUncompleted::TogglePauseResume => "toggle_pause_resume",
    })
}
fn anti_leech_action_to_str(v: BtAntiLeechAction) -> SharedString {
    SharedString::from(match v {
        BtAntiLeechAction::Ban => "ban",
        BtAntiLeechAction::LimitSlots => "limit_slots",
    })
}
fn seed_choking_to_str(v: BtSeedChokingAlgorithm) -> SharedString {
    SharedString::from(match v {
        BtSeedChokingAlgorithm::FastestUpload => "fastest_upload",
        BtSeedChokingAlgorithm::RoundRobin => "round_robin",
        BtSeedChokingAlgorithm::AntiLeech => "anti_leech",
    })
}
fn choking_to_str(v: BtChokingAlgorithm) -> SharedString {
    SharedString::from(match v {
        BtChokingAlgorithm::FixedSlots => "fixed_slots",
        BtChokingAlgorithm::RateBased => "rate_based",
    })
}

/// Convert `AppSettings` and runtime modes to `SettingsFormData`.
pub fn app_settings_to_form(
    settings: &AppSettings,
    game_mode: bool,
    overclock_mode: bool,
    io_status_text: &str,
    disk_type_text: &str,
    lang: Language,
) -> SettingsFormData {
    let speed_limit_kb = if settings.global_speed_limit_bps > 0 {
        (settings.global_speed_limit_bps / 1024).to_string()
    } else {
        "0".to_string()
    };
    let bt_dl_kb = if settings.bt.global_download_rate_limit > 0 {
        (settings.bt.global_download_rate_limit / 1024).to_string()
    } else {
        "0".to_string()
    };
    let bt_ul_kb = if settings.bt.global_upload_rate_limit > 0 {
        (settings.bt.global_upload_rate_limit / 1024).to_string()
    } else {
        "0".to_string()
    };

    let language_src = if settings.appearance.language.is_empty() {
        lang.as_bcp47()
    } else {
        settings.appearance.language.as_str()
    };

    SettingsFormData {
        // 常规下载
        default_download_dir: SharedString::from(&settings.download.default_download_dir),
        max_parallel_tasks: SharedString::from(
            settings.scheduler.traditional.max_parallel_tasks.to_string(),
        ),
        global_speed_limit_kb: SharedString::from(speed_limit_kb),
        download_max_retries: SharedString::from(settings.download.default_max_retries.to_string()),
        download_checksum_idx: combo::idx_of(
            combo::CHECKSUMS,
            &checksum_to_str(settings.download.default_checksum),
        ),
        download_auto_detect_sha256: settings.download.auto_detect_sha256,
        download_user_agent: SharedString::from(&settings.download.default_user_agent),
        // 外观
        appearance_color_mode_idx: combo::idx_of(
            combo::COLOR_MODES,
            &color_mode_to_str(&settings.appearance.color_mode),
        ),
        appearance_theme_color_idx: combo::idx_of(
            combo::THEME_COLORS,
            &theme_color_to_str(&settings.appearance.theme_color),
        ),
        appearance_background_opacity_idx: combo::idx_of(
            combo::OPACITY_PRESETS,
            &background_opacity_to_str(&settings.appearance.background_opacity),
        ),
        appearance_language_idx: combo::idx_of(combo::LANGUAGES, language_src),
        appearance_close_behavior_idx: combo::idx_of(
            combo::CLOSE_BEHAVIORS,
            &close_behavior_to_str(&settings.appearance.close_behavior),
        ),
        appearance_show_detail_info: settings.appearance.show_detail_info,
        autostart: settings.autostart,
        notifications_enabled: settings.notifications.enabled,
        double_click_completed_idx: combo::idx_of(
            combo::DOUBLE_CLICK_COMPLETED,
            &double_click_completed_to_str(settings.double_click.on_completed),
        ),
        double_click_uncompleted_idx: combo::idx_of(
            combo::DOUBLE_CLICK_UNCOMPLETED,
            &double_click_uncompleted_to_str(settings.double_click.on_uncompleted),
        ),
        // 代理
        proxy_mode_idx: combo::idx_of(
            combo::PROXY_MODES,
            &proxy_mode_to_str(settings.proxy.mode),
        ),
        proxy_manual_url: SharedString::from(&settings.proxy.manual_url),
        // 调度
        scheduler_mode_idx: combo::idx_of(
            combo::SCHEDULER_MODES,
            &scheduler_mode_to_str(settings.scheduler.mode),
        ),
        scheduler_max_parallel_threads: SharedString::from(
            settings.scheduler.automatic.max_parallel_threads.to_string(),
        ),
        scheduler_max_threads_per_task: SharedString::from(
            settings.scheduler.automatic.max_threads_per_task.to_string(),
        ),
        scheduler_min_threads_per_task: SharedString::from(
            settings.scheduler.automatic.min_threads_per_task.to_string(),
        ),
        scheduler_adaptive_profile_idx: combo::idx_of(
            combo::ADAPTIVE_PROFILES,
            &adaptive_profile_to_str(settings.scheduler.automatic.adaptive_profile),
        ),
        scheduler_chunk_strategy_idx: combo::idx_of(
            combo::CHUNK_STRATEGIES,
            &chunk_strategy_to_str(settings.scheduler.chunk_size_strategy),
        ),
        scheduler_tail_sprint: settings.scheduler.tail_sprint_enabled,
        scheduler_connection_warmup: settings.scheduler.connection_warmup_enabled,
        // BT 基础 + 进阶
        dht_enabled: settings.bt.dht_enabled,
        listen_port: SharedString::from(
            settings.bt.listen_port.map(|p| p.to_string()).unwrap_or_default(),
        ),
        max_bt_connections: SharedString::from(settings.bt.max_peers_per_torrent.to_string()),
        tracker_url: SharedString::from(&settings.bt.tracker_list_url),
        bt_upnp_enabled: settings.bt.upnp_enabled,
        bt_natpmp_enabled: settings.bt.enable_natpmp,
        bt_ipv6_enabled: settings.bt.enable_ipv6,
        bt_pex_enabled: settings.bt.enable_pex,
        bt_lsd_enabled: settings.bt.enable_lsd,
        bt_utp_enabled: settings.bt.enable_utp,
        bt_encryption_mode_idx: combo::idx_of(
            combo::ENCRYPTION_MODES,
            &encryption_to_str(settings.bt.encryption_mode),
        ),
        bt_preallocate_mode_idx: combo::idx_of(
            combo::PREALLOC_MODES,
            &preallocate_to_str(settings.bt.preallocate_mode),
        ),
        bt_max_downloads: SharedString::from(settings.bt.max_downloads.to_string()),
        bt_max_seeds: SharedString::from(settings.bt.max_seeds.to_string()),
        bt_max_torrents: SharedString::from(settings.bt.max_torrents.to_string()),
        bt_active_limit: SharedString::from(settings.bt.active_limit.to_string()),
        bt_global_download_rate_limit_kb: SharedString::from(bt_dl_kb),
        bt_global_upload_rate_limit_kb: SharedString::from(bt_ul_kb),
        bt_enable_fast_extension: settings.bt.enable_fast_extension,
        bt_enable_holepunch: settings.bt.enable_holepunch,
        bt_enable_web_seed: settings.bt.enable_web_seed,
        bt_enable_super_seeding: settings.bt.enable_super_seeding,
        bt_pause_upload_when_limit: settings.bt.pause_upload_when_limit_reached,
        bt_upload_limit_kb: SharedString::from(if settings.bt.upload_limit_bytes > 0 {
            (settings.bt.upload_limit_bytes / 1024).to_string()
        } else {
            "0".to_string()
        }),
        bt_upload_ratio_limit: SharedString::from({
            let r = settings.bt.upload_ratio_limit;
            if r == 0.0 { "0".to_string() } else { r.to_string() }
        }),
        bt_anti_leech_enabled: settings.bt.anti_leech_enabled,
        bt_anti_leech_action_idx: combo::idx_of(
            combo::ANTI_LEECH_ACTIONS,
            &anti_leech_action_to_str(settings.bt.anti_leech_action),
        ),
        bt_anti_leech_grace_secs: SharedString::from(settings.bt.anti_leech_grace_secs.to_string()),
        bt_anti_leech_ratio: SharedString::from({
            let r = settings.bt.anti_leech_ratio;
            if r == 0.0 { "0".to_string() } else { r.to_string() }
        }),
        bt_anti_leech_ban_secs: SharedString::from(settings.bt.anti_leech_ban_secs.to_string()),
        bt_anti_leech_max_upload_slots: SharedString::from(
            settings.bt.anti_leech_max_upload_slots.to_string(),
        ),
        bt_blocklist_enabled: settings.bt.blocklist_enabled,
        bt_blocklist_path: SharedString::from(&settings.bt.blocklist_path),
        bt_seed_choking_algorithm_idx: combo::idx_of(
            combo::SEED_CHOKING,
            &seed_choking_to_str(settings.bt.seed_choking_algorithm),
        ),
        bt_choking_algorithm_idx: combo::idx_of(
            combo::CHOKING_ALGOS,
            &choking_to_str(settings.bt.choking_algorithm),
        ),
        bt_max_upload_slots_per_torrent: SharedString::from(
            settings.bt.max_upload_slots_per_torrent.to_string(),
        ),
        bt_smart_ban_max_failures: SharedString::from(settings.bt.smart_ban_max_failures.to_string()),
        bt_smart_ban_parole: settings.bt.smart_ban_parole,
        bt_eviction_ban_duration_secs: SharedString::from(
            settings.bt.eviction_ban_duration_secs.to_string(),
        ),
        bt_data_contribution_timeout_secs: SharedString::from(
            settings.bt.data_contribution_timeout_secs.to_string(),
        ),
        // IO 基线
        io_buffer_limit_mb: SharedString::from(settings.io_baseline.buffer_limit_mb.to_string()),
        io_game_mode_buffer_mb: SharedString::from(
            settings.io_baseline.game_mode_buffer_mb.to_string(),
        ),
        io_max_parallel_hdd: SharedString::from(settings.io_baseline.max_parallel_hdd.to_string()),
        io_game_mode_max_parallel: SharedString::from(
            settings.io_baseline.game_mode_max_parallel.to_string(),
        ),
        io_hdd_buffer_enabled: settings.io_baseline.hdd_buffer_enabled,
        io_ssd_write_combine_mb: SharedString::from(
            settings.io_baseline.ssd_write_combine_mb.to_string(),
        ),
        // 日志
        logging_enabled: settings.logging.enabled,
        logging_level_idx: combo::idx_of(
            combo::LOG_LEVELS,
            &log_level_to_str(settings.logging.level),
        ),
        logging_file_path: SharedString::from(&settings.logging.file_path),
        logging_retention_count: SharedString::from(
            settings.logging.retention_count.map(|v| v.to_string()).unwrap_or_default(),
        ),
        logging_retention_days: SharedString::from(
            settings.logging.retention_days.map(|v| v.to_string()).unwrap_or_default(),
        ),
        // Aria2
        aria2_enabled: settings.aria2_rpc.enabled,
        aria2_port: SharedString::from(settings.aria2_rpc.port.to_string()),
        aria2_secret: SharedString::from(settings.aria2_rpc.secret.clone().unwrap_or_default()),
        // 运行态
        game_mode,
        overclock_mode,
        io_status_text: SharedString::from(io_status_text),
        disk_type_text: SharedString::from(disk_type_text),
        max_in_memory_downloads: SharedString::from(
            settings.max_in_memory_downloads.to_string(),
        ),
        app_version: SharedString::from(format!("v{}", env!("CARGO_PKG_VERSION"))),
        engine_version: SharedString::from(format!("limedl-core v{}", env!("CARGO_PKG_VERSION"))),
        arch_info: SharedString::from(format!("{} / {} (Skia)", std::env::consts::ARCH, std::env::consts::OS)),
    }
}

/// Update `AppSettings` from `SettingsFormData`.
/// Returns `Err` with a human-readable message if any field contains
/// non-empty but unparsable content (e.g. "abc" in a numeric field or
/// an invalid proxy URL). Empty strings retain the previous value for
/// numeric fields (mirroring Tauri behaviour) but mandatory string fields
/// like proxy manual URL are validated eagerly so the user gets immediate
/// feedback instead of a silent no-op or a later `normalize_settings` error.
pub fn update_app_settings_from_form(
    settings: &mut AppSettings,
    form: &SettingsFormData,
) -> Result<(), String> {
    // ── 严格校验：非空但无法解析的数值为用户输入错误，必须报错而非静默忽略
    {
        let check_u64 = |raw: &str, label: &str| -> Result<(), String> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(());
            }
            s.parse::<u64>()
                .map(|_| ())
                .map_err(|_| format!("{} 格式错误: '{}' 请输入有效数字", label, s))
        };
        let check_usize = |raw: &str, label: &str| -> Result<(), String> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(());
            }
            s.parse::<usize>()
                .map(|_| ())
                .map_err(|_| format!("{} 格式错误: '{}' 请输入有效整数", label, s))
        };
        let check_u32 = |raw: &str, label: &str| -> Result<(), String> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(());
            }
            s.parse::<u32>()
                .map(|_| ())
                .map_err(|_| format!("{} 格式错误: '{}' 请输入有效整数", label, s))
        };
        let check_u16 = |raw: &str, label: &str| -> Result<(), String> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(());
            }
            s.parse::<u16>()
                .map(|_| ())
                .map_err(|_| format!("{} 格式错误: '{}' 请输入 0-65535 的端口号", label, s))
        };
        let check_f64 = |raw: &str, label: &str| -> Result<(), String> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(());
            }
            s.parse::<f64>()
                .map(|_| ())
                .map_err(|_| format!("{} 格式错误: '{}' 请输入有效数字", label, s))
        };
        // 下载
        check_u32(form.download_max_retries.trim(), "最大重试次数")?;
        check_usize(form.max_parallel_tasks.trim(), "最大并发任务数")?;
        check_u64(form.global_speed_limit_kb.trim(), "全局限速")?;
        // 调度
        check_usize(form.scheduler_max_parallel_threads.trim(), "自动调度-最大并行线程")?;
        check_usize(form.scheduler_max_threads_per_task.trim(), "单任务最大线程")?;
        check_usize(form.scheduler_min_threads_per_task.trim(), "单任务最小线程")?;
        // BT 基础
        check_u16(form.listen_port.trim(), "BT 监听端口")?;
        check_u32(form.max_bt_connections.trim(), "每 Torrent 最大 Peers")?;
        // BT 队列与限速
        check_u32(form.bt_max_downloads.trim(), "BT 最大下载数")?;
        check_u32(form.bt_max_seeds.trim(), "BT 最大做种数")?;
        check_u32(form.bt_max_torrents.trim(), "BT 最大 Torrent 数")?;
        check_u32(form.bt_active_limit.trim(), "BT 活跃限制")?;
        check_u64(form.bt_global_download_rate_limit_kb.trim(), "BT 全局下载限速")?;
        check_u64(form.bt_global_upload_rate_limit_kb.trim(), "BT 全局上传限速")?;
        check_u64(form.bt_upload_limit_kb.trim(), "做种上传限制")?;
        check_f64(form.bt_upload_ratio_limit.trim(), "分享率限制")?;
        check_u64(form.bt_anti_leech_grace_secs.trim(), "反吸血宽限期")?;
        check_f64(form.bt_anti_leech_ratio.trim(), "反吸血分享率阈值")?;
        check_u64(form.bt_anti_leech_ban_secs.trim(), "反吸血封禁时长")?;
        check_u32(form.bt_anti_leech_max_upload_slots.trim(), "反吸血限槽模式槽位")?;
        check_u32(form.bt_max_upload_slots_per_torrent.trim(), "每 Torrent 最大上传槽")?;
        check_u32(form.bt_smart_ban_max_failures.trim(), "智能封禁阈值")?;
        check_u64(form.bt_eviction_ban_duration_secs.trim(), "驱逐封禁时长")?;
        check_u64(form.bt_data_contribution_timeout_secs.trim(), "无贡献超时")?;
        // IO
        check_u64(form.io_buffer_limit_mb.trim(), "IO 缓冲上限")?;
        check_u64(form.io_game_mode_buffer_mb.trim(), "游戏模式缓冲")?;
        check_u32(form.io_max_parallel_hdd.trim(), "HDD 最大并行")?;
        check_u32(form.io_game_mode_max_parallel.trim(), "游戏模式最大并行")?;
        check_u64(form.io_ssd_write_combine_mb.trim(), "SSD 合并缓冲")?;
        // 日志
        if !form.logging_retention_count.trim().is_empty() {
            check_u32(form.logging_retention_count.trim(), "日志保留数量")?;
        }
        if !form.logging_retention_days.trim().is_empty() {
            check_u32(form.logging_retention_days.trim(), "日志保留天数")?;
        }
        // Aria2
        if !form.aria2_port.trim().is_empty() {
            check_u16(form.aria2_port.trim(), "Aria2 端口")?;
        }
        // 高级
        if !form.max_in_memory_downloads.trim().is_empty() {
            check_usize(form.max_in_memory_downloads.trim(), "内存保留记录数")?;
        }
        // 代理：若为 manual 则必须提供合法 URL，留空会在 normalize 阶段报错，这里提前给出更友好的提示
        if combo::value_at(combo::PROXY_MODES, form.proxy_mode_idx) == "manual"
            && form.proxy_manual_url.trim().is_empty()
        {
            return Err("代理模式为 manual 时必须填写代理 URL".to_string());
        }
    }
    // ── 下载 ──
    let dir = form.default_download_dir.trim();
    if !dir.is_empty() {
        settings.download.default_download_dir = dir.to_string();
    }
    if let Ok(v) = form.download_max_retries.trim().parse::<u32>() {
        settings.download.default_max_retries = v.min(100);
    }
    if let Some(c) = str_to_checksum(combo::value_at(combo::CHECKSUMS, form.download_checksum_idx))
    {
        settings.download.default_checksum = c;
    }
    settings.download.auto_detect_sha256 = form.download_auto_detect_sha256;
    // user-agent 允许清空（回退到默认值由后端处理），此处直接写入
    settings.download.default_user_agent = form.download_user_agent.trim().to_string();

    if let Ok(parallel) = form.max_parallel_tasks.trim().parse::<usize>()
        && parallel > 0
    {
        settings.scheduler.traditional.max_parallel_tasks = parallel.min(64);
    }
    if let Ok(limit_kb) = form.global_speed_limit_kb.trim().parse::<u64>() {
        settings.global_speed_limit_bps = limit_kb * 1024;
    }

    // ── 外观 / 启动 ──
    match combo::value_at(combo::COLOR_MODES, form.appearance_color_mode_idx) {
        "light" => settings.appearance.color_mode = ColorMode::Light,
        "dark" => settings.appearance.color_mode = ColorMode::Dark,
        _ => settings.appearance.color_mode = ColorMode::System,
    }
    match combo::value_at(combo::THEME_COLORS, form.appearance_theme_color_idx) {
        "amber" => settings.appearance.theme_color = ThemeColor::Amber,
        "sky" => settings.appearance.theme_color = ThemeColor::Sky,
        _ => settings.appearance.theme_color = ThemeColor::Lime,
    }
    settings.appearance.background_opacity = str_to_background_opacity(combo::value_at(
        combo::OPACITY_PRESETS,
        form.appearance_background_opacity_idx,
    ));
    settings.appearance.language = combo::value_at(combo::LANGUAGES, form.appearance_language_idx)
        .to_string();
    match combo::value_at(combo::CLOSE_BEHAVIORS, form.appearance_close_behavior_idx) {
        "exit" => settings.appearance.close_behavior = CloseBehavior::Exit,
        _ => settings.appearance.close_behavior = CloseBehavior::MinimizeToTray,
    }
    settings.appearance.show_detail_info = form.appearance_show_detail_info;
    settings.autostart = form.autostart;
    settings.notifications.enabled = form.notifications_enabled;
    match combo::value_at(combo::DOUBLE_CLICK_COMPLETED, form.double_click_completed_idx) {
        "open_file" => settings.double_click.on_completed = DoubleClickOnCompleted::OpenFile,
        "open_in_explorer" => {
            settings.double_click.on_completed = DoubleClickOnCompleted::OpenInExplorer
        }
        "open_download_dir" => {
            settings.double_click.on_completed = DoubleClickOnCompleted::OpenDownloadDir
        }
        _ => settings.double_click.on_completed = DoubleClickOnCompleted::None,
    }
    match combo::value_at(combo::DOUBLE_CLICK_UNCOMPLETED, form.double_click_uncompleted_idx) {
        "toggle_pause_resume" => {
            settings.double_click.on_uncompleted = DoubleClickOnUncompleted::TogglePauseResume
        }
        _ => settings.double_click.on_uncompleted = DoubleClickOnUncompleted::None,
    }

    // ── 代理 ──
    if let Some(m) = str_to_proxy_mode(combo::value_at(combo::PROXY_MODES, form.proxy_mode_idx)) {
        settings.proxy.mode = m;
    }
    settings.proxy.manual_url = form.proxy_manual_url.trim().to_string();

    // ── 调度 ──
    if let Some(m) = match combo::value_at(combo::SCHEDULER_MODES, form.scheduler_mode_idx) {
        "traditional" => Some(SchedulerMode::Traditional),
        "automatic" => Some(SchedulerMode::Automatic),
        _ => None,
    } {
        settings.scheduler.mode = m;
    }
    if let Ok(v) = form.scheduler_max_parallel_threads.trim().parse::<usize>()
        && v > 0
    {
        settings.scheduler.automatic.max_parallel_threads = v.min(128);
    }
    if let Ok(v) = form.scheduler_max_threads_per_task.trim().parse::<usize>()
        && v > 0
    {
        settings.scheduler.automatic.max_threads_per_task = v.min(64);
    }
    if let Ok(v) = form.scheduler_min_threads_per_task.trim().parse::<usize>() {
        settings.scheduler.automatic.min_threads_per_task =
            v.min(settings.scheduler.automatic.max_threads_per_task);
    }
    match combo::value_at(combo::ADAPTIVE_PROFILES, form.scheduler_adaptive_profile_idx) {
        "conservative" => {
            settings.scheduler.automatic.adaptive_profile = AdaptiveProfile::Conservative
        }
        "aggressive" => {
            settings.scheduler.automatic.adaptive_profile = AdaptiveProfile::Aggressive
        }
        _ => settings.scheduler.automatic.adaptive_profile = AdaptiveProfile::Balanced,
    }
    match combo::value_at(combo::CHUNK_STRATEGIES, form.scheduler_chunk_strategy_idx) {
        "fixed" => settings.scheduler.chunk_size_strategy = ChunkSizeStrategy::Fixed,
        _ => settings.scheduler.chunk_size_strategy = ChunkSizeStrategy::Adaptive,
    }
    settings.scheduler.tail_sprint_enabled = form.scheduler_tail_sprint;
    settings.scheduler.connection_warmup_enabled = form.scheduler_connection_warmup;

    // ── BT ──
    settings.bt.dht_enabled = form.dht_enabled;
    let lp = form.listen_port.trim();
    if lp.is_empty() {
        settings.bt.listen_port = None;
    } else if let Ok(port) = lp.parse::<u16>() {
        if port == 0 {
            settings.bt.listen_port = None;
        } else {
            settings.bt.listen_port = Some(port);
        }
    }
    if let Ok(conns) = form.max_bt_connections.trim().parse::<u32>()
        && conns > 0
    {
        settings.bt.max_peers_per_torrent = conns.min(4096);
    }
    let tracker_url = form.tracker_url.trim();
    // 允许清空
    settings.bt.tracker_list_url = tracker_url.to_string();
    settings.bt.upnp_enabled = form.bt_upnp_enabled;
    settings.bt.enable_natpmp = form.bt_natpmp_enabled;
    settings.bt.enable_ipv6 = form.bt_ipv6_enabled;
    settings.bt.enable_pex = form.bt_pex_enabled;
    settings.bt.enable_lsd = form.bt_lsd_enabled;
    settings.bt.enable_utp = form.bt_utp_enabled;
    match combo::value_at(combo::ENCRYPTION_MODES, form.bt_encryption_mode_idx) {
        "disabled" => settings.bt.encryption_mode = BtEncryptionMode::Disabled,
        "forced" => settings.bt.encryption_mode = BtEncryptionMode::Forced,
        _ => settings.bt.encryption_mode = BtEncryptionMode::Enabled,
    }
    match combo::value_at(combo::PREALLOC_MODES, form.bt_preallocate_mode_idx) {
        "full" => settings.bt.preallocate_mode = BtPreallocateMode::Full,
        _ => settings.bt.preallocate_mode = BtPreallocateMode::None,
    }
    if let Ok(v) = form.bt_max_downloads.trim().parse::<u32>() {
        settings.bt.max_downloads = v.clamp(1, 1000);
    }
    if let Ok(v) = form.bt_max_seeds.trim().parse::<u32>() {
        settings.bt.max_seeds = v.clamp(0, 1000);
    }
    if let Ok(v) = form.bt_max_torrents.trim().parse::<u32>() {
        settings.bt.max_torrents = v.clamp(1, 10000);
    }
    if let Ok(v) = form.bt_active_limit.trim().parse::<u32>() {
        settings.bt.active_limit = v.clamp(1, 10000);
    }
    if let Ok(v) = form.bt_global_download_rate_limit_kb.trim().parse::<u64>() {
        settings.bt.global_download_rate_limit = v * 1024;
    }
    if let Ok(v) = form.bt_global_upload_rate_limit_kb.trim().parse::<u64>() {
        settings.bt.global_upload_rate_limit = v * 1024;
    }
    settings.bt.enable_fast_extension = form.bt_enable_fast_extension;
    settings.bt.enable_holepunch = form.bt_enable_holepunch;
    settings.bt.enable_web_seed = form.bt_enable_web_seed;
    settings.bt.enable_super_seeding = form.bt_enable_super_seeding;
    settings.bt.pause_upload_when_limit_reached = form.bt_pause_upload_when_limit;
    if let Ok(v) = form.bt_upload_limit_kb.trim().parse::<u64>() {
        settings.bt.upload_limit_bytes = v * 1024;
    }
    if let Ok(v) = form.bt_upload_ratio_limit.trim().parse::<f64>() {
        settings.bt.upload_ratio_limit = v.clamp(0.0, 1000.0);
    }
    settings.bt.anti_leech_enabled = form.bt_anti_leech_enabled;
    match combo::value_at(combo::ANTI_LEECH_ACTIONS, form.bt_anti_leech_action_idx) {
        "limit_slots" => settings.bt.anti_leech_action = BtAntiLeechAction::LimitSlots,
        _ => settings.bt.anti_leech_action = BtAntiLeechAction::Ban,
    }
    if let Ok(v) = form.bt_anti_leech_grace_secs.trim().parse::<u64>() {
        settings.bt.anti_leech_grace_secs = v;
    }
    if let Ok(v) = form.bt_anti_leech_ratio.trim().parse::<f64>() {
        settings.bt.anti_leech_ratio = v.clamp(0.0, 1.0);
    }
    if let Ok(v) = form.bt_anti_leech_ban_secs.trim().parse::<u64>() {
        settings.bt.anti_leech_ban_secs = v;
    }
    if let Ok(v) = form.bt_anti_leech_max_upload_slots.trim().parse::<u32>()
        && v > 0
    {
        settings.bt.anti_leech_max_upload_slots = v.clamp(1, 64);
    }
    settings.bt.blocklist_enabled = form.bt_blocklist_enabled;
    settings.bt.blocklist_path = form.bt_blocklist_path.trim().to_string();
    match combo::value_at(combo::SEED_CHOKING, form.bt_seed_choking_algorithm_idx) {
        "round_robin" => settings.bt.seed_choking_algorithm = BtSeedChokingAlgorithm::RoundRobin,
        "anti_leech" => settings.bt.seed_choking_algorithm = BtSeedChokingAlgorithm::AntiLeech,
        _ => settings.bt.seed_choking_algorithm = BtSeedChokingAlgorithm::FastestUpload,
    }
    match combo::value_at(combo::CHOKING_ALGOS, form.bt_choking_algorithm_idx) {
        "rate_based" => settings.bt.choking_algorithm = BtChokingAlgorithm::RateBased,
        _ => settings.bt.choking_algorithm = BtChokingAlgorithm::FixedSlots,
    }
    if let Ok(v) = form.bt_max_upload_slots_per_torrent.trim().parse::<u32>()
        && v > 0
    {
        settings.bt.max_upload_slots_per_torrent = v.clamp(1, 64);
    }
    if let Ok(v) = form.bt_smart_ban_max_failures.trim().parse::<u32>()
        && v > 0
    {
        settings.bt.smart_ban_max_failures = v.clamp(1, 100);
    }
    settings.bt.smart_ban_parole = form.bt_smart_ban_parole;
    if let Ok(v) = form.bt_eviction_ban_duration_secs.trim().parse::<u64>() {
        settings.bt.eviction_ban_duration_secs = v;
    }
    if let Ok(v) = form.bt_data_contribution_timeout_secs.trim().parse::<u64>() {
        settings.bt.data_contribution_timeout_secs = v;
    }

    // ── IO 基线 ──
    if let Ok(v) = form.io_buffer_limit_mb.trim().parse::<u64>() {
        settings.io_baseline.buffer_limit_mb = v.clamp(64, 32768);
    }
    if let Ok(v) = form.io_game_mode_buffer_mb.trim().parse::<u64>() {
        settings.io_baseline.game_mode_buffer_mb = v.clamp(16, 4096);
    }
    if let Ok(v) = form.io_max_parallel_hdd.trim().parse::<u32>() {
        settings.io_baseline.max_parallel_hdd = v.clamp(1, 16);
    }
    if let Ok(v) = form.io_game_mode_max_parallel.trim().parse::<u32>() {
        settings.io_baseline.game_mode_max_parallel = v.clamp(1, 8);
    }
    settings.io_baseline.hdd_buffer_enabled = form.io_hdd_buffer_enabled;
    if let Ok(v) = form.io_ssd_write_combine_mb.trim().parse::<u64>() {
        settings.io_baseline.ssd_write_combine_mb = v.min(4096);
    }

    // ── 日志 ──
    settings.logging.enabled = form.logging_enabled;
    if let Some(l) = str_to_log_level(combo::value_at(combo::LOG_LEVELS, form.logging_level_idx)) {
        settings.logging.level = l;
    }
    settings.logging.file_path = form.logging_file_path.trim().to_string();
    let rc = form.logging_retention_count.trim();
    if rc.is_empty() {
        settings.logging.retention_count = None;
    } else if let Ok(v) = rc.parse::<u32>() {
        settings.logging.retention_count = Some(v.min(10000));
    }
    let rd = form.logging_retention_days.trim();
    if rd.is_empty() {
        settings.logging.retention_days = None;
    } else if let Ok(v) = rd.parse::<u32>() {
        settings.logging.retention_days = Some(v.min(36500));
    }

    // ── Aria2 ──
    settings.aria2_rpc.enabled = form.aria2_enabled;
    if let Ok(v) = form.aria2_port.trim().parse::<u16>()
        && v != 0
    {
        settings.aria2_rpc.port = v;
    }
    let sec = form.aria2_secret.trim();
    if sec.is_empty() {
        settings.aria2_rpc.secret = None;
    } else {
        settings.aria2_rpc.secret = Some(sec.to_string());
    }

    // ── 高级 ──
    if let Ok(v) = form.max_in_memory_downloads.trim().parse::<usize>()
        && v > 0
    {
        settings.max_in_memory_downloads = v.clamp(10, 10000);
    }

    Ok(())
}

/// Format detected disk types into readable summary text.
pub fn format_disk_types_map(disks: &HashMap<String, DiskType>) -> String {
    if disks.is_empty() {
        return "未检测到磁盘信息".to_string();
    }

    let mut parts = Vec::new();
    for (path, disk_type) in disks {
        let type_name = match disk_type {
            DiskType::Ssd => "SSD 固态硬盘",
            DiskType::Hdd => "HDD 机械硬盘",
        };
        parts.push(format!("{path} ({type_name})"));
    }
    parts.join(" | ")
}

/// Format buffer pool / IO status JSON payload.
pub fn format_io_status_json(val: &serde_json::Value) -> String {
    let allocated = val.get("allocatedBytes").and_then(|v| v.as_u64()).unwrap_or(0);
    let capacity = val.get("capacityBytes").and_then(|v| v.as_u64()).unwrap_or(1024 * 1024 * 1024);
    let active_buffers = val.get("activeBuffers").and_then(|v| v.as_u64()).unwrap_or(0);

    format!(
        "已用缓存: {} / 上限: {} (活跃缓冲槽: {} 个)",
        format_bytes(allocated),
        format_bytes(capacity),
        active_buffers
    )
}

/// State store managing task collections, filtering, search, sorting, and multi-selection.
pub struct TaskStore {
    tasks: HashMap<String, DownloadSummary>,
    current_category: i32,
    search_query: String,
    sort_field: SortField,
    sort_asc: bool,
    selected_ids: HashSet<String>,
    language: Language,
}

impl TaskStore {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::with_language(Language::default())
    }

    pub fn with_language(lang: Language) -> Self {
        Self {
            tasks: HashMap::new(),
            current_category: 0,
            search_query: String::new(),
            sort_field: SortField::Created,
            sort_asc: false,
            selected_ids: HashSet::new(),
            language: lang,
        }
    }

    pub fn set_language(&mut self, lang: Language) {
        self.language = lang;
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn set_category(&mut self, cat: i32) {
        self.current_category = cat;
    }

    #[allow(dead_code)]
    pub fn category(&self) -> i32 {
        self.current_category
    }

    pub fn get_summary(&self, id: &str) -> Option<DownloadSummary> {
        self.tasks.get(id).cloned()
    }

    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query.trim().to_lowercase();
    }

    pub fn set_sort_field(&mut self, field: SortField) {
        self.sort_field = field;
    }

    pub fn sort_field(&self) -> i32 {
        self.sort_field as i32
    }

    pub fn toggle_sort_order(&mut self) -> bool {
        self.sort_asc = !self.sort_asc;
        self.sort_asc
    }

    pub fn sort_asc(&self) -> bool {
        self.sort_asc
    }

    pub fn toggle_select(&mut self, id: &str) {
        if self.selected_ids.contains(id) {
            self.selected_ids.remove(id);
        } else {
            self.selected_ids.insert(id.to_string());
        }
    }

    pub fn select_all(&mut self) {
        let ids: Vec<String> = self
            .filtered_items_internal()
            .into_iter()
            .map(|item| item.id.clone())
            .collect();
        for id in ids {
            self.selected_ids.insert(id);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_ids.clear();
    }

    pub fn selected_count(&self) -> usize {
        self.selected_ids.len()
    }

    pub fn selected_ids(&self) -> Vec<String> {
        self.selected_ids.iter().cloned().collect()
    }

    pub fn insert_or_update(&mut self, summary: DownloadSummary) {
        self.tasks.insert(summary.id.clone(), summary);
    }

    pub fn remove(&mut self, id: &str) {
        self.tasks.remove(id);
        self.selected_ids.remove(id);
    }

    pub fn replace_all(&mut self, list: Vec<DownloadSummary>) {
        self.tasks.clear();
        for item in list {
            self.tasks.insert(item.id.clone(), item);
        }
        self.selected_ids.retain(|id| self.tasks.contains_key(id));
    }

    pub fn update_progress(&mut self, progress: &DownloadProgress) {
        if let Some(summary) = self.tasks.get_mut(&progress.id) {
            summary.state = progress.state;
            summary.downloaded_bytes = progress.downloaded_bytes;
            if progress.total_bytes.is_some() {
                summary.total_bytes = progress.total_bytes;
            }
            summary.speed_bytes_per_second = progress.speed_bytes_per_second;
            summary.eta_seconds = progress.eta_seconds;
        }
    }

    /// Calculate counts for each category.
    pub fn counts(&self) -> (usize, usize, usize, usize, usize) {
        let mut all = 0;
        let mut downloading = 0;
        let mut paused = 0;
        let mut completed = 0;
        let mut failed = 0;

        for task in self.tasks.values() {
            all += 1;
            match task.state {
                DownloadState::Downloading | DownloadState::Retrying | DownloadState::Verifying => {
                    downloading += 1;
                }
                DownloadState::Paused | DownloadState::Queued => {
                    paused += 1;
                }
                DownloadState::Completed => {
                    completed += 1;
                }
                DownloadState::Failed | DownloadState::Canceled => {
                    failed += 1;
                }
            }
        }

        (all, downloading, paused, completed, failed)
    }

    /// Calculate total speed across all active downloads.
    pub fn total_speed(&self) -> f64 {
        self.tasks
            .values()
            .filter_map(|t| {
                if matches!(t.state, DownloadState::Downloading) {
                    t.speed_bytes_per_second
                } else {
                    None
                }
            })
            .sum()
    }

    fn filtered_items_internal(&self) -> Vec<&DownloadSummary> {
        let query = &self.search_query;
        let mut list: Vec<&DownloadSummary> = self
            .tasks
            .values()
            .filter(|task| {
                // Category filter
                let cat_match = match self.current_category {
                    1 => matches!(
                        task.state,
                        DownloadState::Downloading
                            | DownloadState::Retrying
                            | DownloadState::Verifying
                    ),
                    2 => matches!(task.state, DownloadState::Paused | DownloadState::Queued),
                    3 => matches!(task.state, DownloadState::Completed),
                    4 => matches!(task.state, DownloadState::Failed | DownloadState::Canceled),
                    _ => true, // 0: All
                };
                if !cat_match {
                    return false;
                }

                // Search filter
                if !query.is_empty() {
                    let name_match = task.file_name.to_lowercase().contains(query);
                    let url_match = task.url.to_lowercase().contains(query);
                    if !name_match && !url_match {
                        return false;
                    }
                }

                true
            })
            .collect();

        // Sort items
        list.sort_by(|a, b| {
            let ordering = match self.sort_field {
                SortField::Created => a.created_at_ms.cmp(&b.created_at_ms),
                SortField::Size => a.total_bytes.unwrap_or(0).cmp(&b.total_bytes.unwrap_or(0)),
                SortField::Speed => {
                    let sa = a.speed_bytes_per_second.unwrap_or(0.0);
                    let sb = b.speed_bytes_per_second.unwrap_or(0.0);
                    sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
                }
                SortField::Progress => {
                    let pa = match a.total_bytes {
                        Some(t) if t > 0 => a.downloaded_bytes as f64 / t as f64,
                        _ => {
                            if matches!(a.state, DownloadState::Completed) {
                                1.0
                            } else {
                                0.0
                            }
                        }
                    };
                    let pb = match b.total_bytes {
                        Some(t) if t > 0 => b.downloaded_bytes as f64 / t as f64,
                        _ => {
                            if matches!(b.state, DownloadState::Completed) {
                                1.0
                            } else {
                                0.0
                            }
                        }
                    };
                    pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
                }
                SortField::Name => a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()),
            };

            if self.sort_asc {
                ordering
            } else {
                ordering.reverse()
            }
        });

        list
    }

    /// Return filtered and sorted task items for Slint view.
    pub fn filtered_items(&self) -> Vec<TaskItem> {
        self.filtered_items_internal()
            .into_iter()
            .map(|summary| {
                let is_selected = self.selected_ids.contains(&summary.id);
                summary_to_task_item(summary, is_selected, self.language)
            })
            .collect()
    }
}

// ── Labs Bridge Helpers ─────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn app_settings_to_labs_form(
    settings: &AppSettings,
    is_testing: bool,
    phase_label: &str,
    progress_percent: f32,
    progress_label: &str,
    speed_improvement: Option<&str>,
    latency_improvement: Option<&str>,
    default_node_text: Option<&str>,
    ranges_text: &str,
    show_advanced: bool,
    test_url: &str,
    matched_rule: &str,
    candidates: &[String],
    lang: Language,
) -> LabsFormData {
    let cdn = &settings.cdn_acceleration;
    let (status_type, status_label) = if is_testing {
        ("testing", match lang { Language::ZhCn => "测速中", Language::EnUs => "Testing" })
    } else if cdn.last_error.is_some() {
        ("error", match lang { Language::ZhCn => "测速失败", Language::EnUs => "Failed" })
    } else if cdn.active_ip.is_some() {
        ("ready", match lang { Language::ZhCn => "准备就绪", Language::EnUs => "Ready" })
    } else {
        ("idle", match lang { Language::ZhCn => "未配置", Language::EnUs => "Not Configured" })
    };

    let active_speed_text = cdn
        .active_speed_mbps
        .map(|s| format!("{s:.2} MB/s"))
        .unwrap_or_default();

    let last_test_time = cdn
        .last_test_at_ms
        .map(|ts| {
            let secs = (ts / 1000) as i64;
            if let Ok(dt) = time::OffsetDateTime::from_unix_timestamp(secs) {
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                    dt.year(),
                    dt.month() as u8,
                    dt.day(),
                    dt.hour(),
                    dt.minute(),
                    dt.second()
                )
            } else {
                format!("{secs}")
            }
        })
        .unwrap_or_default();

    LabsFormData {
        cdn_enabled: cdn.enabled,
        cdn_provider: SharedString::from(if cdn.provider.is_empty() { "cloudflare" } else { &cdn.provider }),
        cdn_custom_test_url: SharedString::from(cdn.custom_test_url.as_deref().unwrap_or_default()),
        cdn_custom_cidrs: SharedString::from(cdn.custom_cidrs.as_deref().unwrap_or_default()),
        cdn_status_type: SharedString::from(status_type),
        cdn_status_label: SharedString::from(status_label),
        cdn_is_testing: is_testing,
        cdn_phase_label: SharedString::from(phase_label),
        cdn_progress_percent: progress_percent,
        cdn_progress_label: SharedString::from(progress_label),
        cdn_active_ip: SharedString::from(cdn.active_ip.as_deref().unwrap_or_default()),
        cdn_active_speed_text: SharedString::from(active_speed_text),
        cdn_last_test_time: SharedString::from(last_test_time),
        cdn_speed_improvement_text: SharedString::from(speed_improvement.unwrap_or_default()),
        cdn_latency_improvement_text: SharedString::from(latency_improvement.unwrap_or_default()),
        cdn_default_node_text: SharedString::from(default_node_text.unwrap_or_default()),
        cdn_last_error: SharedString::from(cdn.last_error.as_deref().unwrap_or_default()),
        cdn_show_advanced: show_advanced,
        cdn_manual_ip: SharedString::default(),
        cdn_manual_ip_error: SharedString::default(),
        cdn_ranges_text: SharedString::from(ranges_text),

        url_rewrite_enabled: settings.url_rewrite.enabled,
        url_rewrite_test_url: SharedString::from(test_url),
        url_rewrite_test_matched_rule: SharedString::from(matched_rule),
        url_rewrite_test_candidates_count: candidates.len() as i32,
        url_rewrite_test_result_1: SharedString::from(candidates.first().cloned().unwrap_or_default()),
        url_rewrite_test_result_2: SharedString::from(candidates.get(1).cloned().unwrap_or_default()),
        url_rewrite_test_result_3: SharedString::from(candidates.get(2).cloned().unwrap_or_default()),
    }
}

pub fn update_app_settings_from_labs_form(settings: &mut AppSettings, form: &LabsFormData) {
    settings.cdn_acceleration.enabled = form.cdn_enabled;
    settings.cdn_acceleration.provider = form.cdn_provider.trim().to_string();
    let test_url = form.cdn_custom_test_url.trim().to_string();
    settings.cdn_acceleration.custom_test_url = if test_url.is_empty() { None } else { Some(test_url) };
    let cidrs = form.cdn_custom_cidrs.trim().to_string();
    settings.cdn_acceleration.custom_cidrs = if cidrs.is_empty() { None } else { Some(cidrs) };
    settings.url_rewrite.enabled = form.url_rewrite_enabled;
}

pub fn cdn_candidates_to_slint(
    candidates: &[SpeedTestResult],
    active_ip: &str,
) -> ModelRc<CdnCandidateItem> {
    let items: Vec<CdnCandidateItem> = candidates
        .iter()
        .map(|c| {
            let ip_str = c.ip.to_string();
            let is_active = !ip_str.is_empty() && ip_str == active_ip;
            let latency_text = format!("{:.1} ms", c.tcp_latency_ms);
            let throughput_text = match c.throughput_mbps {
                Some(tp) if tp > 0.0 => format!("{:.2} MB/s", tp),
                _ => "-".to_string(),
            };
            let is_failed = c.error.is_some();

            CdnCandidateItem {
                ip: SharedString::from(ip_str),
                latency_text: SharedString::from(latency_text),
                throughput_text: SharedString::from(throughput_text),
                throughput_mbps: c.throughput_mbps.unwrap_or(0.0) as f32,
                is_active,
                is_failed,
            }
        })
        .collect();

    ModelRc::new(VecModel::from(items))
}

pub fn match_type_to_str(m: MatchType) -> &'static str {
    match m {
        MatchType::Host => "host",
        MatchType::Prefix => "prefix",
        MatchType::Regex => "regex",
        MatchType::Wildcard => "wildcard",
    }
}

pub fn str_to_match_type(s: &str) -> MatchType {
    match s {
        "prefix" => MatchType::Prefix,
        "regex" => MatchType::Regex,
        "wildcard" => MatchType::Wildcard,
        _ => MatchType::Host,
    }
}

pub fn replacement_mode_to_str(m: ReplacementMode) -> &'static str {
    match m {
        ReplacementMode::PrefixProxy => "prefix_proxy",
        ReplacementMode::Template => "template",
    }
}

pub fn str_to_replacement_mode(s: &str) -> ReplacementMode {
    match s {
        "template" => ReplacementMode::Template,
        _ => ReplacementMode::PrefixProxy,
    }
}

pub fn url_rewrite_rules_to_slint(
    rules: &[UrlRewriteRule],
    expanded_ids: &HashSet<String>,
) -> ModelRc<UrlRewriteRuleItem> {
    let items: Vec<UrlRewriteRuleItem> = rules
        .iter()
        .map(|r| {
            let is_expanded = expanded_ids.contains(&r.id);
            let target_items: Vec<UrlRewriteTargetItem> = r
                .targets
                .iter()
                .map(|t| UrlRewriteTargetItem {
                    url_template: SharedString::from(&t.url_template),
                    enabled: t.enabled,
                    order: t.order as i32,
                })
                .collect();

            UrlRewriteRuleItem {
                id: SharedString::from(&r.id),
                name: SharedString::from(&r.name),
                enabled: r.enabled,
                match_type: SharedString::from(match_type_to_str(r.match_type)),
                pattern: SharedString::from(&r.pattern),
                replacement_mode: SharedString::from(replacement_mode_to_str(r.replacement_mode)),
                encode_url: r.encode_url,
                fallback_to_original: r.fallback_to_original,
                order: r.order as i32,
                targets: ModelRc::new(VecModel::from(target_items)),
                is_expanded,
            }
        })
        .collect();

    ModelRc::new(VecModel::from(items))
}

#[allow(dead_code)]
pub fn slint_to_url_rewrite_rules(models: &[UrlRewriteRuleItem]) -> Vec<UrlRewriteRule> {
    models
        .iter()
        .map(|m| {
            let mut targets = Vec::new();
            for i in 0..m.targets.row_count() {
                if let Some(t) = m.targets.row_data(i) {
                    targets.push(RewriteTarget {
                        url_template: t.url_template.to_string(),
                        enabled: t.enabled,
                        order: t.order as u32,
                    });
                }
            }

            UrlRewriteRule {
                id: m.id.to_string(),
                name: m.name.to_string(),
                enabled: m.enabled,
                match_type: str_to_match_type(m.match_type.as_str()),
                pattern: m.pattern.to_string(),
                replacement_mode: str_to_replacement_mode(m.replacement_mode.as_str()),
                encode_url: m.encode_url,
                fallback_to_original: m.fallback_to_original,
                order: m.order as u32,
                targets,
            }
        })
        .collect()
}

pub fn evaluate_url_rewrite(rules: &[UrlRewriteRule], test_url: &str) -> (String, Vec<String>) {
    let trimmed = test_url.trim();
    if trimmed.is_empty() {
        return (String::new(), Vec::new());
    }

    let mut matched_rule_name = String::new();
    let mut enabled_rules: Vec<&UrlRewriteRule> = rules.iter().filter(|r| r.enabled).collect();
    enabled_rules.sort_by_key(|r| r.order);

    for rule in &enabled_rules {
        if limedl_core::url_rewrite::matches_rule(trimmed, rule) {
            matched_rule_name = rule.name.clone();
            break;
        }
    }

    let settings = UrlRewriteSettings {
        enabled: true,
        rules: rules.to_vec(),
    };
    let candidates = limedl_core::url_rewrite::rewrite_url(trimmed, &settings);

    (matched_rule_name, candidates)
}

pub fn create_url_rewrite_preset(preset_key: &str) -> Option<UrlRewriteRule> {
    match preset_key {
        "github" => Some(UrlRewriteRule {
            id: format!("preset-gh-{}", uuid::Uuid::new_v4().simple()),
            name: "GitHub 镜像代理".to_string(),
            enabled: true,
            match_type: MatchType::Host,
            pattern: "*.github.com".to_string(),
            replacement_mode: ReplacementMode::PrefixProxy,
            encode_url: true,
            fallback_to_original: true,
            order: 0,
            targets: vec![
                RewriteTarget {
                    url_template: "https://ghproxy.net".to_string(),
                    enabled: true,
                    order: 0,
                },
                RewriteTarget {
                    url_template: "https://mirror.ghproxy.cc".to_string(),
                    enabled: true,
                    order: 1,
                },
            ],
        }),
        "huggingface" => Some(UrlRewriteRule {
            id: format!("preset-hf-{}", uuid::Uuid::new_v4().simple()),
            name: "Hugging Face 镜像".to_string(),
            enabled: true,
            match_type: MatchType::Regex,
            pattern: r"^https://huggingface\.co/(.*)$".to_string(),
            replacement_mode: ReplacementMode::Template,
            encode_url: false,
            fallback_to_original: true,
            order: 1,
            targets: vec![
                RewriteTarget {
                    url_template: "https://hf-mirror.com/$1".to_string(),
                    enabled: true,
                    order: 0,
                },
            ],
        }),
        "civitai" => Some(UrlRewriteRule {
            id: format!("preset-civitai-{}", uuid::Uuid::new_v4().simple()),
            name: "Civitai 镜像".to_string(),
            enabled: true,
            match_type: MatchType::Host,
            pattern: "*.civitai.com".to_string(),
            replacement_mode: ReplacementMode::PrefixProxy,
            encode_url: true,
            fallback_to_original: true,
            order: 2,
            targets: vec![
                RewriteTarget {
                    url_template: "https://civitai.work".to_string(),
                    enabled: true,
                    order: 0,
                },
            ],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use limedl_core::types::ThreadMode;

    fn sample_summary(
        id: &str,
        name: &str,
        state: DownloadState,
        downloaded: u64,
        total: Option<u64>,
        speed: f64,
        created: u64,
    ) -> DownloadSummary {
        DownloadSummary {
            id: id.to_string(),
            kind: TaskKind::Http,
            state,
            url: format!("https://example.com/{name}"),
            file_name: name.to_string(),
            destination_path: format!("/downloads/{name}"),
            total_bytes: total,
            downloaded_bytes: downloaded,
            connection_count: 4,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: Some(4),
            adaptive_profile: None,
            thread_note: None,
            speed_bytes_per_second: Some(speed),
            eta_seconds: Some(120),
            uploaded_bytes: None,
            upload_speed_bytes_per_second: None,
            peer_count: None,
            upload_status: None,
            info_hash: None,
            expected_checksum: None,
            error: None,
            cdn_accelerated: false,
            cdn_node_ip: None,
            created_at_ms: created,
            priority: limedl_core::types::Priority::Normal,
            seed_count: None,
            leech_count: None,
            download_limit_bps: None,
            upload_limit_bps: None,
            chunks: Vec::new(),
            mirror_url: None,
        }
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024 * 5), "5.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 2), "2.00 GB");
    }

    #[test]
    fn test_combo_idx_and_value_roundtrip() {
        let cases: [(&[&str], &str); 18] = [
            (&combo::COLOR_MODES, "dark"),
            (&combo::THEME_COLORS, "sky"),
            (&combo::OPACITY_PRESETS, "frosted"),
            (&combo::LANGUAGES, "en-US"),
            (&combo::CLOSE_BEHAVIORS, "minimizeToTray"),
            (&combo::DOUBLE_CLICK_COMPLETED, "open_in_explorer"),
            (&combo::DOUBLE_CLICK_UNCOMPLETED, "toggle_pause_resume"),
            (&combo::CHECKSUMS, "xxh3_128"),
            (&combo::PROXY_MODES, "manual"),
            (&combo::SCHEDULER_MODES, "traditional"),
            (&combo::ADAPTIVE_PROFILES, "balanced"),
            (&combo::CHUNK_STRATEGIES, "fixed"),
            (&combo::ENCRYPTION_MODES, "forced"),
            (&combo::PREALLOC_MODES, "full"),
            (&combo::ANTI_LEECH_ACTIONS, "limit_slots"),
            (&combo::SEED_CHOKING, "round_robin"),
            (&combo::CHOKING_ALGOS, "rate_based"),
            (&combo::LOG_LEVELS, "warn"),
        ];
        for (list, value) in cases {
            let idx = combo::idx_of(list, value);
            assert!(idx > 0 || list.first() == Some(&value), "idx for {value}");
            assert_eq!(combo::value_at(list, idx), value, "roundtrip {value}");
        }
        // Out-of-range / unknown values fall back safely.
        assert_eq!(combo::idx_of(combo::COLOR_MODES, "nope"), 0);
        assert_eq!(combo::value_at(combo::COLOR_MODES, 99), "system");
        assert_eq!(combo::value_at(combo::COLOR_MODES, -3), "system");
    }

    #[test]
    fn test_format_speed() {
        assert_eq!(format_speed(Some(1024.0 * 1024.0 * 2.5)), "2.50 MB/s");
        assert_eq!(format_speed(None), "");
        assert_eq!(format_speed(Some(0.0)), "");
    }

    #[test]
    fn test_format_eta() {
        assert_eq!(format_eta(Some(30), Language::ZhCn), "剩余 30秒");
        assert_eq!(format_eta(Some(125), Language::ZhCn), "剩余 2分5秒");
        assert_eq!(format_eta(Some(3665), Language::ZhCn), "剩余 1小时1分");
        assert_eq!(format_eta(None, Language::ZhCn), "");
    }

    #[test]
    fn test_piece_map_generation() {
        let pieces = vec![
            BtPieceInfo { index: 0, completed: true },
            BtPieceInfo { index: 1, completed: false },
            BtPieceInfo { index: 2, completed: true },
            BtPieceInfo { index: 3, completed: true },
        ];

        let (_img, text) = generate_piece_map_image(&pieces, Language::ZhCn);
        assert!(text.contains("3 / 4"));
        assert!(text.contains("75.0%"));
    }

    #[test]
    fn test_inspector_conversion() {
        let summary = sample_summary(
            "bt:abc",
            "ubuntu.torrent",
            DownloadState::Downloading,
            500,
            Some(1000),
            500.0,
            12345,
        );

        let info = summary_to_inspector_info(&summary, Language::ZhCn);
        assert_eq!(info.id.as_str(), "bt:abc");
        assert_eq!(info.file_name.as_str(), "ubuntu.torrent");
        assert_eq!(info.state_label.as_str(), "下载中");
        assert_eq!(info.progress, 0.5);
    }

    #[test]
    fn test_peer_and_file_conversions() {
        let peer = BtPeerInfo {
            address: "1.2.3.4:6881".to_string(),
            client: "qBittorrent/5.0.0".to_string(),
            flags: "uI".to_string(),
            download_speed: 1024.0 * 1024.0 * 1.5,
            upload_speed: 1024.0 * 500.0,
            progress: 0.85,
        };
        let p_item = peer_info_to_item(&peer);
        assert_eq!(p_item.address.as_str(), "1.2.3.4:6881");
        assert_eq!(p_item.client.as_str(), "qBittorrent/5.0.0");
        assert_eq!(p_item.download_speed.as_str(), "1.50 MB/s");
        assert_eq!(p_item.progress, 0.85);

        let file = BtFileStatus {
            index: 0,
            path: "movie/video.mp4".to_string(),
            size: 1024 * 1024 * 100,
            downloaded_bytes: 1024 * 1024 * 50,
            included: true,
        };
        let f_item = file_status_to_item(&file);
        assert_eq!(f_item.index, 0);
        assert_eq!(f_item.path.as_str(), "movie/video.mp4");
        assert_eq!(f_item.size_text.as_str(), "100.00 MB");
        assert_eq!(f_item.downloaded_text.as_str(), "50.00 MB");
        assert_eq!(f_item.progress, 0.5);
    }

    #[test]
    fn test_settings_conversion() {
        let mut settings = AppSettings::default();
        settings.download.default_download_dir = "/custom/downloads".to_string();
        settings.scheduler.traditional.max_parallel_tasks = 5;
        settings.global_speed_limit_bps = 1024 * 500;
        settings.bt.dht_enabled = true;
        settings.bt.listen_port = Some(6882);

        let form = app_settings_to_form(&settings, true, false, "IO OK", "D: SSD", Language::ZhCn);
        assert_eq!(form.default_download_dir.as_str(), "/custom/downloads");
        assert_eq!(form.max_parallel_tasks.as_str(), "5");
        assert_eq!(form.global_speed_limit_kb.as_str(), "500");
        assert!(form.dht_enabled);
        assert_eq!(form.listen_port.as_str(), "6882");
        assert!(form.game_mode);
        assert!(!form.overclock_mode);

        let mut updated = AppSettings::default();
        update_app_settings_from_form(&mut updated, &form).expect("valid form should update");
        assert_eq!(updated.download.default_download_dir, "/custom/downloads");
        assert_eq!(updated.scheduler.traditional.max_parallel_tasks, 5);
        assert_eq!(updated.global_speed_limit_bps, 1024 * 500);
        assert_eq!(updated.bt.listen_port, Some(6882));
    }

    #[test]
    fn test_search_and_sorting() {
        let mut store = TaskStore::new();
        let t1 = sample_summary(
            "t1",
            "ubuntu-24.04.iso",
            DownloadState::Downloading,
            500,
            Some(1000),
            5000.0,
            100,
        );
        let t2 = sample_summary(
            "t2",
            "archlinux.iso",
            DownloadState::Paused,
            200,
            Some(2000),
            1000.0,
            200,
        );
        let t3 = sample_summary(
            "t3",
            "fedora-workstation.iso",
            DownloadState::Completed,
            1500,
            Some(1500),
            0.0,
            300,
        );

        store.insert_or_update(t1);
        store.insert_or_update(t2);
        store.insert_or_update(t3);

        // Default: Sort by Created DESC
        let items = store.filtered_items();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].id.as_str(), "t3");
        assert_eq!(items[1].id.as_str(), "t2");
        assert_eq!(items[2].id.as_str(), "t1");

        // Sort by Size DESC (t2: 2000, t3: 1500, t1: 1000)
        store.set_sort_field(SortField::Size);
        let items = store.filtered_items();
        assert_eq!(items[0].id.as_str(), "t2");
        assert_eq!(items[1].id.as_str(), "t3");
        assert_eq!(items[2].id.as_str(), "t1");

        // Sort by Size ASC
        store.toggle_sort_order();
        let items = store.filtered_items();
        assert_eq!(items[0].id.as_str(), "t1");
        assert_eq!(items[1].id.as_str(), "t3");
        assert_eq!(items[2].id.as_str(), "t2");

        // Search Filter
        store.set_search_query("arch".to_string());
        let items = store.filtered_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id.as_str(), "t2");
    }

    #[test]
    fn test_multi_selection() {
        let mut store = TaskStore::new();
        let t1 = sample_summary(
            "t1",
            "file1.zip",
            DownloadState::Downloading,
            100,
            Some(200),
            10.0,
            10,
        );
        let t2 = sample_summary(
            "t2",
            "file2.zip",
            DownloadState::Downloading,
            100,
            Some(200),
            10.0,
            20,
        );

        store.insert_or_update(t1);
        store.insert_or_update(t2);

        assert_eq!(store.selected_count(), 0);

        store.toggle_select("t1");
        assert_eq!(store.selected_count(), 1);

        store.select_all();
        assert_eq!(store.selected_count(), 2);

        store.clear_selection();
        assert_eq!(store.selected_count(), 0);
    }

    #[test]
    fn test_disk_types_and_io_status() {
        let mut disks = HashMap::new();
        disks.insert("C:\\".to_string(), DiskType::Ssd);
        disks.insert("D:\\".to_string(), DiskType::Hdd);

        let disks_text = format_disk_types_map(&disks);
        assert!(disks_text.contains("SSD"));
        assert!(disks_text.contains("HDD"));

        let empty_disks = HashMap::new();
        assert_eq!(format_disk_types_map(&empty_disks), "未检测到磁盘信息");

        let io_val = serde_json::json!({
            "allocatedBytes": 1024 * 1024 * 64,
            "capacityBytes": 1024 * 1024 * 1024,
            "activeBuffers": 2
        });
        let io_text = format_io_status_json(&io_val);
        assert!(io_text.contains("64.00 MB"));
        assert!(io_text.contains("1.00 GB"));
        assert!(io_text.contains("2 个"));
    }

    #[test]
    fn test_labs_form_and_url_rewrite() {
        let mut settings = AppSettings::default();
        settings.cdn_acceleration.enabled = true;
        settings.cdn_acceleration.active_ip = Some("104.16.0.1".to_string());
        settings.cdn_acceleration.active_speed_mbps = Some(45.2);

        let form = app_settings_to_labs_form(
            &settings,
            false,
            "就绪",
            100.0,
            "测速完成",
            Some("+25%"),
            Some("-15ms"),
            Some("104.16.0.2"),
            "104.16.0.0/12",
            false,
            "https://raw.github.com/user/repo/master/README.md",
            "GitHub 镜像",
            &["https://ghproxy.net/https://raw.github.com/user/repo/master/README.md".to_string()],
            Language::ZhCn,
        );

        assert!(form.cdn_enabled);
        assert_eq!(form.cdn_status_type.as_str(), "ready");
        assert_eq!(form.cdn_active_ip.as_str(), "104.16.0.1");
        assert_eq!(form.url_rewrite_test_matched_rule.as_str(), "GitHub 镜像");

        let gh_rule = create_url_rewrite_preset("github").expect("gh preset");
        assert_eq!(gh_rule.name, "GitHub 镜像代理");
        assert_eq!(gh_rule.targets.len(), 2);

        let (matched_rule, candidates) = evaluate_url_rewrite(
            &[gh_rule],
            "https://raw.github.com/user/repo/master/README.md",
        );
        assert_eq!(matched_rule, "GitHub 镜像代理");
        assert!(candidates.len() >= 2);
    }
}

