use limedl_core::types::{DownloadProgress, DownloadState, DownloadSummary};
use crate::i18n::Language;
use crate::TaskItem;
use super::models::{SortField, state_rank, summary_to_task_item};

/// State store managing task collections, filtering, search, sorting, and multi-selection.
pub struct TaskStore {
    tasks: foldhash::HashMap<String, DownloadSummary>,
    current_category: i32,
    search_query: String,
    sort_field: SortField,
    sort_asc: bool,
    selected_ids: foldhash::HashSet<String>,
    last_selected_id: Option<String>,
    language: Language,
}

impl TaskStore {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::with_language(Language::default())
    }

    pub fn with_language(lang: Language) -> Self {
        Self {
            tasks: foldhash::HashMap::default(),
            current_category: 0,
            search_query: String::new(),
            sort_field: SortField::Created,
            sort_asc: false,
            selected_ids: foldhash::HashSet::default(),
            last_selected_id: None,
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

    /// Overwrite the sort field/direction (used to apply persisted settings).
    pub fn apply_sort(&mut self, field: i32, asc: bool) {
        self.sort_field = SortField::from(field);
        self.sort_asc = asc;
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
            if self.last_selected_id.as_deref() == Some(id) {
                self.last_selected_id = None;
            }
        } else {
            self.selected_ids.insert(id.to_string());
            self.last_selected_id = Some(id.to_string());
        }
    }

    pub fn select_range(&mut self, target_id: &str) {
        let (start, end) = {
            let items = self.filtered_items_internal();
            let target_idx = match items.iter().position(|it| it.id == target_id) {
                Some(idx) => idx,
                None => return,
            };

            let anchor_idx = self
                .last_selected_id
                .as_ref()
                .and_then(|anchor_id| items.iter().position(|it| &it.id == anchor_id))
                .unwrap_or(target_idx);

            if anchor_idx <= target_idx {
                (anchor_idx, target_idx)
            } else {
                (target_idx, anchor_idx)
            }
        };

        let ids_to_add: Vec<String> = self
            .filtered_items_internal()
            [start..=end]
            .iter()
            .map(|it| it.id.clone())
            .collect();

        for id in ids_to_add {
            self.selected_ids.insert(id);
        }

        self.last_selected_id = Some(target_id.to_string());
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
        self.last_selected_id = None;
    }

    pub fn selected_count(&self) -> usize {
        self.selected_ids.len()
    }

    pub fn selected_ids(&self) -> Vec<String> {
        self.selected_ids.iter().cloned().collect()
    }

    pub fn completed_ids(&self) -> Vec<String> {
        self.tasks
            .values()
            .filter(|t| matches!(t.state, DownloadState::Completed))
            .map(|t| t.id.clone())
            .collect()
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
                SortField::State => state_rank(a).cmp(&state_rank(b)),
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

