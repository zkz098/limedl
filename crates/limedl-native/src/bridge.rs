use std::collections::{HashMap, HashSet};

use limedl_core::types::{DownloadProgress, DownloadState, DownloadSummary, TaskKind};
use slint::SharedString;

use crate::TaskItem;

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

/// Apply a high-frequency `DownloadProgress` update onto an existing `TaskItem`.
#[allow(dead_code)]
pub fn apply_progress(item: &mut TaskItem, progress: &DownloadProgress) {
    let (state_code, state_label, can_pause, can_resume, is_completed, is_failed) = match progress.state {
        DownloadState::Downloading => ("downloading", "下载中", true, false, false, false),
        DownloadState::Paused => ("paused", "已暂停", false, true, false, false),
        DownloadState::Completed => ("completed", "已完成", false, false, true, false),
        DownloadState::Failed => ("failed", "失败", false, true, false, true),
        DownloadState::Canceled => ("failed", "已取消", false, true, false, true),
        DownloadState::Queued => ("queued", "排队中", true, false, false, false),
        DownloadState::Retrying => ("downloading", "重试中", true, false, false, false),
        DownloadState::Verifying => ("verifying", "校验中", false, false, false, false),
    };

    let prog_val = match progress.total_bytes {
        Some(total) if total > 0 => {
            (progress.downloaded_bytes as f64 / total as f64).clamp(0.0, 1.0) as f32
        }
        _ => {
            if is_completed {
                1.0
            } else {
                item.progress
            }
        }
    };

    let size_text = match progress.total_bytes {
        Some(total) => format!("{} / {}", format_bytes(progress.downloaded_bytes), format_bytes(total)),
        None => format_bytes(progress.downloaded_bytes),
    };

    item.state_code = SharedString::from(state_code);
    item.state_label = SharedString::from(state_label);
    item.progress = prog_val;
    item.speed_text = SharedString::from(format_speed(progress.speed_bytes_per_second));
    item.size_text = SharedString::from(size_text);
    item.eta_text = SharedString::from(format_eta(progress.eta_seconds));
    item.can_pause = can_pause;
    item.can_resume = can_resume;
    item.is_completed = is_completed;
    item.is_failed = is_failed;
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

        store.set_search_query(String::new());
        assert_eq!(store.filtered_items().len(), 3);
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
        assert!(store.filtered_items()[0].selected || store.filtered_items()[1].selected);

        store.select_all();
        assert_eq!(store.selected_count(), 2);

        store.clear_selection();
        assert_eq!(store.selected_count(), 0);
    }
}
