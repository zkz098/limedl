use limedl_core::types::{
    BtFileStatus, BtPeerInfo, BtTrackerInfo, DownloadState, DownloadSummary,
    TaskKind, TorrentFileEntry,
};
use slint::SharedString;

use crate::i18n::{self, Language};
use crate::{
    InspectorInfo, NewTaskTorrentFileItem, PeerItem, TaskItem, TorrentFileItem, TrackerItem,
};
use super::format::{format_bytes, format_eta, format_speed};

/// Supported sort fields for task list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortField {
    #[default]
    Created = 0,
    Size = 1,
    Speed = 2,
    Progress = 3,
    Name = 4,
    State = 5,
}

impl From<i32> for SortField {
    fn from(val: i32) -> Self {
        match val {
            1 => SortField::Size,
            2 => SortField::Speed,
            3 => SortField::Progress,
            4 => SortField::Name,
            5 => SortField::State,
            _ => SortField::Created,
        }
    }
}

/// Map the persisted `AppearanceSettings::sort_key` onto the list's sort field.
pub fn sort_key_to_field(key: limedl_core::types::SortKey) -> i32 {
    use limedl_core::types::SortKey;
    match key {
        SortKey::Name => SortField::Name as i32,
        SortKey::Size => SortField::Size as i32,
        SortKey::Progress => SortField::Progress as i32,
        SortKey::Speed => SortField::Speed as i32,
        SortKey::State => SortField::State as i32,
        SortKey::AddedAt => SortField::Created as i32,
    }
}

/// Inverse of [`sort_key_to_field`] — used when persisting a user sort change.
pub fn field_to_sort_key(field: i32) -> limedl_core::types::SortKey {
    use limedl_core::types::SortKey;
    match SortField::from(field) {
        SortField::Name => SortKey::Name,
        SortField::Size => SortKey::Size,
        SortField::Progress => SortKey::Progress,
        SortField::Speed => SortKey::Speed,
        SortField::State => SortKey::State,
        SortField::Created => SortKey::AddedAt,
    }
}

/// Ordered list of table column keys the native UI can show/hide. Mirrors the
/// web client's `VALID_COLUMN_KEYS` (minus the fields the native list does not
/// render) so a settings file stays portable between both editions.
pub const COLUMN_KEYS: [&str; 10] = [
    "file",
    "size",
    "downloaded",
    "status",
    "progress",
    "speed",
    "priority",
    "uploadSpeed",
    "seeds",
    "eta",
];

/// Column keys shown when `appearance.visible_columns` is empty/unknown.
pub const DEFAULT_VISIBLE_COLUMNS: [&str; 8] = [
    "file",
    "size",
    "downloaded",
    "status",
    "progress",
    "speed",
    "priority",
    "eta",
];

/// Whether a column key is visible for the given settings list.
pub fn column_is_visible(visible_columns: &[String], key: &str) -> bool {
    if visible_columns.is_empty() {
        return DEFAULT_VISIBLE_COLUMNS.contains(&key);
    }
    visible_columns.iter().any(|c| c == key)
}

/// Sort rank for the "state" sort key: active work first, then paused/queued,
/// finished, and failures last.
pub(crate) fn state_rank(task: &DownloadSummary) -> u8 {
    match task.state {
        DownloadState::Downloading | DownloadState::Retrying | DownloadState::Verifying => 0,
        DownloadState::Queued => 1,
        DownloadState::Paused => 2,
        DownloadState::Completed => 3,
        DownloadState::Failed | DownloadState::Canceled => 4,
    }
}

/// Classify a file into an icon category based on its extension.
pub fn detect_file_category(filename: &str) -> &'static str {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "rmvb" | "ts" | "m4v" => "video",
        "mp3" | "flac" | "wav" | "aac" | "ogg" | "m4a" | "wma" | "opus" | "ape" => "audio",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "iso" | "dmg" | "img" | "vhd" => "archive",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "epub" | "csv" => "document",
        "exe" | "msi" | "apk" | "deb" | "rpm" | "appimage" | "pkg" => "installer",
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "bmp" | "ico" | "psd" | "tiff" => "image",
        _ => "default",
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
        downloaded_text: SharedString::from(format_bytes(summary.downloaded_bytes)),
        upload_speed_text: SharedString::from(format_speed(
            summary.upload_speed_bytes_per_second,
        )),
        seeds_text: SharedString::from(
            summary
                .seed_count
                .map(|v| v.to_string())
                .unwrap_or_else(|| i18n::format_unknown(lang).to_string()),
        ),
        priority_code: SharedString::from(priority_code(summary.priority)),
        priority_label: SharedString::from(i18n::format_priority_label(summary.priority, lang)),
        can_pause,
        can_resume,
        is_completed,
        is_failed,
        selected,
        file_type: SharedString::from(detect_file_category(&summary.file_name)),
    }
}

/// Stable wire code for a download priority (`high` / `normal` / `low`).
pub fn priority_code(priority: limedl_core::types::Priority) -> &'static str {
    use limedl_core::types::Priority;
    match priority {
        Priority::High => "high",
        Priority::Normal => "normal",
        Priority::Low => "low",
    }
}

/// Parse a priority wire code back into the core enum (unknown → Normal).
pub fn str_to_priority(code: &str) -> limedl_core::types::Priority {
    use limedl_core::types::Priority;
    match code.trim().to_ascii_lowercase().as_str() {
        "high" => Priority::High,
        "low" => Priority::Low,
        _ => Priority::Normal,
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
    let sanitized_client: String = peer
        .client
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let trimmed_client = sanitized_client.trim();

    PeerItem {
        address: SharedString::from(&peer.address),
        client: SharedString::from(trimmed_client),
        flags: SharedString::from(peer.flags.trim()),
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

/// Convert a `TorrentFileEntry` (torrent preview) into a new-task dialog file
/// row for pre-download file selection.
pub fn torrent_entry_to_item(entry: &TorrentFileEntry, included: bool) -> NewTaskTorrentFileItem {
    NewTaskTorrentFileItem {
        index: entry.index as i32,
        path: SharedString::from(entry.path.trim_start_matches('/')),
        size_text: SharedString::from(format_bytes(entry.size)),
        included,
    }
}

