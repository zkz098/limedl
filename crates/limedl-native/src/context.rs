use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::sync::watch;

use limedl_core::cdn::speed_test::SpeedTestResult;
use limedl_core::dispatcher::Dispatcher;
use limedl_core::event_bus::EventBus;
use limedl_core::types::{AppSettings, TorrentFileEntry, UrlRewriteRule};

use crate::MainWindow;
use crate::bridge::TaskStore;
use crate::toast::ToastQueue;

pub struct AppContext {
    pub ui: MainWindow,
    pub ui_weak: slint::Weak<MainWindow>,
    pub dispatcher: Arc<Dispatcher>,
    pub event_bus: Arc<EventBus>,
    pub store: Arc<Mutex<TaskStore>>,
    pub current_settings: Arc<Mutex<AppSettings>>,
    pub toast_queue: ToastQueue,
    pub rpc_shutdown: Arc<Mutex<Option<watch::Sender<bool>>>>,
    pub new_task_torrent_entries: Arc<Mutex<Vec<TorrentFileEntry>>>,
    pub new_task_torrent_included: Arc<Mutex<Vec<bool>>>,
    pub active_inspector_id: Arc<Mutex<Option<String>>>,
    pub labs_expanded_ids: Arc<Mutex<HashSet<String>>>,
    pub labs_candidates: Arc<Mutex<Vec<SpeedTestResult>>>,
    pub rewrite_rules: Arc<Mutex<Vec<UrlRewriteRule>>>,
    pub sandbox_test_url: Arc<Mutex<String>>,
    pub game_mode_active: Arc<Mutex<bool>>,
    pub is_overclock_mode: Arc<Mutex<bool>>,
    pub tray_speed_limit_active: Arc<AtomicBool>,
    pub base_dir: PathBuf,
}
