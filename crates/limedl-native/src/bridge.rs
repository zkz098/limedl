use std::collections::HashMap;

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

/// Convert a `DownloadSummary` into a Slint `TaskItem`.
pub fn summary_to_task_item(summary: &DownloadSummary) -> TaskItem {
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

/// State store managing the task collection and filtering.
pub struct TaskStore {
    tasks: HashMap<String, DownloadSummary>,
    current_category: i32,
}

impl TaskStore {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            current_category: 0,
        }
    }

    pub fn set_category(&mut self, cat: i32) {
        self.current_category = cat;
    }

    #[allow(dead_code)]
    pub fn category(&self) -> i32 {
        self.current_category
    }

    pub fn insert_or_update(&mut self, summary: DownloadSummary) {
        self.tasks.insert(summary.id.clone(), summary);
    }

    pub fn remove(&mut self, id: &str) {
        self.tasks.remove(id);
    }

    pub fn replace_all(&mut self, list: Vec<DownloadSummary>) {
        self.tasks.clear();
        for item in list {
            self.tasks.insert(item.id.clone(), item);
        }
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

    /// Return filtered task list for the current category.
    pub fn filtered_items(&self) -> Vec<TaskItem> {
        let mut list: Vec<&DownloadSummary> = self.tasks.values().collect();
        // Sort by created_at descending
        list.sort_by_key(|b| std::cmp::Reverse(b.created_at_ms));

        list.into_iter()
            .filter(|task| match self.current_category {
                1 => matches!(task.state, DownloadState::Downloading | DownloadState::Retrying | DownloadState::Verifying),
                2 => matches!(task.state, DownloadState::Paused | DownloadState::Queued),
                3 => matches!(task.state, DownloadState::Completed),
                4 => matches!(task.state, DownloadState::Failed | DownloadState::Canceled),
                _ => true, // 0: All
            })
            .map(summary_to_task_item)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use limedl_core::types::ThreadMode;

    fn sample_summary(id: &str, state: DownloadState, downloaded: u64, total: Option<u64>) -> DownloadSummary {
        DownloadSummary {
            id: id.to_string(),
            kind: TaskKind::Http,
            state,
            url: format!("https://example.com/{id}.bin"),
            file_name: format!("{id}.bin"),
            destination_path: format!("/downloads/{id}.bin"),
            total_bytes: total,
            downloaded_bytes: downloaded,
            connection_count: 4,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: Some(4),
            adaptive_profile: None,
            thread_note: None,
            speed_bytes_per_second: Some(1024.0 * 1024.0 * 5.0),
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
            created_at_ms: 1000,
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
    fn test_task_store_crud_and_counts() {
        let mut store = TaskStore::new();
        assert_eq!(store.counts(), (0, 0, 0, 0, 0));

        let t1 = sample_summary("t1", DownloadState::Downloading, 500, Some(1000));
        let t2 = sample_summary("t2", DownloadState::Paused, 200, Some(1000));
        let t3 = sample_summary("t3", DownloadState::Completed, 1000, Some(1000));

        store.insert_or_update(t1);
        store.insert_or_update(t2);
        store.insert_or_update(t3);

        assert_eq!(store.counts(), (3, 1, 1, 1, 0));
        assert!(store.total_speed() > 0.0);

        // Filter: All
        assert_eq!(store.filtered_items().len(), 3);

        // Filter: Downloading
        store.set_category(1);
        assert_eq!(store.filtered_items().len(), 1);
        assert_eq!(store.filtered_items()[0].id.as_str(), "t1");

        // Filter: Paused
        store.set_category(2);
        assert_eq!(store.filtered_items().len(), 1);
        assert_eq!(store.filtered_items()[0].id.as_str(), "t2");

        // Filter: Completed
        store.set_category(3);
        assert_eq!(store.filtered_items().len(), 1);
        assert_eq!(store.filtered_items()[0].id.as_str(), "t3");

        // Remove t1
        store.remove("t1");
        assert_eq!(store.counts(), (2, 0, 1, 1, 0));
    }
}
