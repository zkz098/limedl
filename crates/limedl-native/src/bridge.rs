use std::collections::{HashMap, HashSet};

use limedl_core::types::{
    BtFileStatus, BtPeerInfo, BtPieceInfo, BtTrackerInfo, DownloadProgress, DownloadState,
    DownloadSummary, TaskKind,
};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer, SharedString};

use crate::{InspectorInfo, PeerItem, TaskItem, TorrentFileItem, TrackerItem};

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
pub fn format_eta(eta: Option<u64>) -> String {
    match eta {
        Some(s) if s > 0 => {
            if s >= 3600 {
                let h = s / 3600;
                let m = (s % 3600) / 60;
                format!("剩余 {h}小时{m}分")
            } else if s >= 60 {
                let m = s / 60;
                let sec = s % 60;
                format!("剩余 {m}分{sec}秒")
            } else {
                format!("剩余 {s}秒")
            }
        }
        _ => String::new(),
    }
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
pub fn summary_to_task_item(summary: &DownloadSummary, selected: bool) -> TaskItem {
    let (state_code, state_label, can_pause, can_resume, is_completed, is_failed) = match summary.state {
        DownloadState::Downloading => ("downloading", "下载中", true, false, false, false),
        DownloadState::Paused => ("paused", "已暂停", false, true, false, false),
        DownloadState::Completed => ("completed", "已完成", false, false, true, false),
        DownloadState::Failed => ("failed", "失败", false, true, false, true),
        DownloadState::Canceled => ("failed", "已取消", false, true, false, true),
        DownloadState::Queued => ("queued", "排队中", true, false, false, false),
        DownloadState::Retrying => ("downloading", "重试中", true, false, false, false),
        DownloadState::Verifying => ("verifying", "校验中", false, false, false, false),
    };

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
        eta_text: SharedString::from(format_eta(summary.eta_seconds)),
        can_pause,
        can_resume,
        is_completed,
        is_failed,
        selected,
    }
}

/// Convert a `DownloadSummary` into a Slint `InspectorInfo`.
pub fn summary_to_inspector_info(summary: &DownloadSummary) -> InspectorInfo {
    let state_label = match summary.state {
        DownloadState::Downloading => "下载中",
        DownloadState::Paused => "已暂停",
        DownloadState::Completed => "已完成",
        DownloadState::Failed => "失败",
        DownloadState::Canceled => "已取消",
        DownloadState::Queued => "排队中",
        DownloadState::Retrying => "重试中",
        DownloadState::Verifying => "校验中",
    };

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
        .unwrap_or_else(|| "未知".to_string());
    let downloaded_size_text = format_bytes(summary.downloaded_bytes);
    let uploaded_size_text = summary.uploaded_bytes.map(format_bytes).unwrap_or_default();

    let threads_text = format!(
        "{:?} (已分配: {} 线程)",
        summary.thread_mode,
        summary.allocated_thread_count.unwrap_or(1)
    );

    let seed_leech_text = match (summary.seed_count, summary.leech_count) {
        (Some(s), Some(l)) => format!("做种: {s} | 下载: {l}"),
        _ => String::new(),
    };

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
        eta_text: SharedString::from(format_eta(summary.eta_seconds)),
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
pub fn generate_piece_map_image(pieces: &[BtPieceInfo]) -> (Image, String) {
    if pieces.is_empty() {
        let buf = SharedPixelBuffer::<Rgba8Pixel>::new(1, 1);
        return (Image::from_rgba8(buf), "暂无分片数据".to_string());
    }

    let total = pieces.len();
    let completed = pieces.iter().filter(|p| p.completed).count();
    let percent = if total > 0 {
        (completed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let summary_text = format!("{completed} / {total} 分片 ({percent:.1}%)");

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

/// State store managing task collections, filtering, search, sorting, and multi-selection.
pub struct TaskStore {
    tasks: HashMap<String, DownloadSummary>,
    current_category: i32,
    search_query: String,
    sort_field: SortField,
    sort_asc: bool,
    selected_ids: HashSet<String>,
}

impl TaskStore {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            current_category: 0,
            search_query: String::new(),
            sort_field: SortField::Created,
            sort_asc: false,
            selected_ids: HashSet::new(),
        }
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
                summary_to_task_item(summary, is_selected)
            })
            .collect()
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
    fn test_format_speed() {
        assert_eq!(format_speed(Some(1024.0 * 1024.0 * 2.5)), "2.50 MB/s");
        assert_eq!(format_speed(None), "");
        assert_eq!(format_speed(Some(0.0)), "");
    }

    #[test]
    fn test_format_eta() {
        assert_eq!(format_eta(Some(30)), "剩余 30秒");
        assert_eq!(format_eta(Some(125)), "剩余 2分5秒");
        assert_eq!(format_eta(Some(3665)), "剩余 1小时1分");
        assert_eq!(format_eta(None), "");
    }

    #[test]
    fn test_piece_map_generation() {
        let pieces = vec![
            BtPieceInfo { index: 0, completed: true },
            BtPieceInfo { index: 1, completed: false },
            BtPieceInfo { index: 2, completed: true },
            BtPieceInfo { index: 3, completed: true },
        ];

        let (_img, text) = generate_piece_map_image(&pieces);
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

        let info = summary_to_inspector_info(&summary);
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
}
