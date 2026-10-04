use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use slint::{Model, SharedString, VecModel};

use crate::{MainWindow, ToastItem};

/// How long a dismissed toast stays in the queue (and therefore in the Slint
/// model) so the card can play its exit animation before the row is dropped.
/// Keep this a little longer than the `animate` duration in
/// `ui/components/toast_stack.slint`, so a slow first frame of the exit
/// animation is not cut off.
pub const TOAST_EXIT_MS: u64 = 240;

/// One pending in-app toast (auto-expires after `duration`).
pub struct ToastEntry {
    pub id: usize,
    pub message: String,
    pub kind: &'static str, // success / error / warning / info
    /// Set while the card plays its exit animation. The row stays in the model
    /// (and in the layout) until the exit timer removes it, so the stack does
    /// not reflow under a fading card.
    pub leaving: bool,
}

pub type ToastQueue = Arc<Mutex<Vec<ToastEntry>>>;

static TOAST_SEQ: AtomicUsize = AtomicUsize::new(1);

/// Push the current queue contents into the Slint property (any thread).
///
/// The model is reconciled **in place** rather than replaced: a fresh
/// `VecModel` would make the repeater destroy and recreate every row, replaying
/// the enter animation of untouched toasts whenever a sibling is dismissed.
pub fn sync_toasts(ui_weak: &slint::Weak<MainWindow>, queue: &ToastQueue) {
    let items: Vec<ToastItem> = queue
        .lock()
        .iter()
        .map(|e| ToastItem {
            id: e.id as i32,
            message: SharedString::from(e.message.as_str()),
            kind: SharedString::from(e.kind),
            leaving: e.leaving,
        })
        .collect();
    let weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = weak.upgrade() {
            apply_toasts(&ui, items);
        }
    });
}

/// Diff `items` into the live `toasts` model, reusing rows whose id survives.
pub(crate) fn apply_toasts(ui: &MainWindow, items: Vec<ToastItem>) {
    let model = ui.get_toasts();
    let Some(model) = model.as_any().downcast_ref::<VecModel<ToastItem>>() else {
        // The property's default value is a `SharedVectorModel`, so the first
        // sync lands here; after that the property holds our `VecModel`.
        ui.set_toasts(Rc::new(VecModel::from(items)).into());
        return;
    };

    let live_ids: HashSet<i32> = items.iter().map(|item| item.id).collect();
    let mut row = 0;
    while row < model.row_count() {
        if live_ids.contains(&model.row_data(row).unwrap().id) {
            row += 1;
        } else {
            model.remove(row);
        }
    }

    for (row, item) in items.into_iter().enumerate() {
        match model.row_data(row) {
            Some(existing) if existing.id == item.id => {
                if existing != item {
                    model.set_row_data(row, item);
                }
            }
            _ => model.insert(row, item),
        }
    }
}

/// Show an in-app toast; auto-dismisses after `duration`. Safe from any thread.
pub fn push_toast(
    ui_weak: &slint::Weak<MainWindow>,
    queue: &ToastQueue,
    message: String,
    kind: &'static str,
    duration: Duration,
) {
    let id = TOAST_SEQ.fetch_add(1, Ordering::Relaxed);
    queue.lock().push(ToastEntry {
        id,
        message,
        kind,
        leaving: false,
    });
    sync_toasts(ui_weak, queue);
    let ui_weak = ui_weak.clone();
    let queue = queue.clone();
    tokio::spawn(async move {
        tokio::time::sleep(duration).await;
        retire_toast(&ui_weak, &queue, id);
    });
}

/// Dismiss a toast immediately (from the UI close button).
pub fn dismiss_toast(ui_weak: &slint::Weak<MainWindow>, queue: &ToastQueue, id: i32) {
    retire_toast(ui_weak, queue, id as usize);
}

/// Start a toast's exit animation, then drop it once the animation has had
/// time to play. Also used by the auto-expire timer, so both dismissal paths
/// animate out. A second call for the same id is a no-op.
fn retire_toast(ui_weak: &slint::Weak<MainWindow>, queue: &ToastQueue, id: usize) {
    let started = {
        let mut entries = queue.lock();
        match entries.iter_mut().find(|e| e.id == id) {
            Some(entry) if !entry.leaving => {
                entry.leaving = true;
                true
            }
            _ => false,
        }
    };
    if !started {
        return;
    }
    sync_toasts(ui_weak, queue);

    let ui_weak = ui_weak.clone();
    let queue = queue.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(TOAST_EXIT_MS)).await;
        queue.lock().retain(|e| e.id != id);
        sync_toasts(&ui_weak, &queue);
    });
}

/// Collapse duplicate download warnings into a single toast.
///
/// Core emits warnings per peer (anti-leech bans), per mirror failover and per
/// disk-space check, so an unfiltered feed would flood the toast stack.
pub struct WarningDedup {
    last_key: String,
    last_at: Option<std::time::Instant>,
}

impl WarningDedup {
    const WINDOW: Duration = Duration::from_secs(5);

    pub fn new() -> Self {
        Self {
            last_key: String::new(),
            last_at: None,
        }
    }

    /// Returns `true` when the warning is new enough to be worth surfacing.
    pub fn should_show(&mut self, id: &str, message: &str) -> bool {
        let key = format!("{id}:{message}");
        let now = std::time::Instant::now();
        let is_duplicate = key == self.last_key
            && self
                .last_at
                .is_some_and(|prev| now.duration_since(prev) < Self::WINDOW);
        self.last_key = key;
        self.last_at = Some(now);
        !is_duplicate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The anti-flood rule, and the reason the queue is not simply a list: one
    /// torrent can emit the same warning per peer within milliseconds.
    #[test]
    fn repeated_warnings_are_collapsed_and_new_ones_are_not() {
        let mut dedup = WarningDedup::new();

        assert!(dedup.should_show("http:1", "peer banned"), "the first warning is shown");
        assert!(
            !dedup.should_show("http:1", "peer banned"),
            "the same warning again within the window is dropped"
        );
        assert!(dedup.should_show("http:1", "disk full"), "a different message is news");
        assert!(dedup.should_show("http:2", "peer banned"), "a different task is news");
        assert!(
            dedup.should_show("http:1", "peer banned"),
            "only the *most recent* key is remembered, so alternating messages all show"
        );
    }
}
