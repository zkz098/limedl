use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use slint::{SharedString, VecModel};

use crate::{MainWindow, ToastItem};

/// One pending in-app toast (auto-expires after `duration`).
pub struct ToastEntry {
    pub id: usize,
    pub message: String,
    pub kind: &'static str, // success / error / warning / info
}

pub type ToastQueue = Arc<Mutex<Vec<ToastEntry>>>;

static TOAST_SEQ: AtomicUsize = AtomicUsize::new(1);

/// Push the current queue contents into the Slint property (any thread).
pub fn sync_toasts(ui_weak: &slint::Weak<MainWindow>, queue: &ToastQueue) {
    let items: Vec<ToastItem> = queue
        .lock()
        .iter()
        .map(|e| ToastItem {
            id: e.id as i32,
            message: SharedString::from(e.message.as_str()),
            kind: SharedString::from(e.kind),
        })
        .collect();
    let weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_toasts(Rc::new(VecModel::from(items)).into());
        }
    });
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
    });
    sync_toasts(ui_weak, queue);
    let ui_weak = ui_weak.clone();
    let queue = queue.clone();
    tokio::spawn(async move {
        tokio::time::sleep(duration).await;
        queue.lock().retain(|e| e.id != id);
        sync_toasts(&ui_weak, &queue);
    });
}

/// Dismiss a toast immediately (from the UI close button).
pub fn dismiss_toast(ui_weak: &slint::Weak<MainWindow>, queue: &ToastQueue, id: i32) {
    queue.lock().retain(|e| e.id != id as usize);
    sync_toasts(ui_weak, queue);
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
