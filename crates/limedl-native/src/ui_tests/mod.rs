//! In-process UI tests: they build the **real** `MainWindow` through
//! [`crate::ui_boot::build_ui`] and drive it with Slint's testing backend.
//!
//! `bridge/` unit tests cover the mapping and the state machine; nothing covered
//! the wiring in between, so a renamed `.slint` id, a callback bound to the wrong
//! handler or a dialog reading a property nobody sets was only found by hand.
//! These tests click the actual controls and assert the state that follows.
//!
//! Ground rules for adding a test here:
//!
//! * **No event loop.** `init_no_event_loop()` installs a mock platform: timers
//!   never fire and `slint::invoke_from_event_loop` delivers nothing. Assert on
//!   state a callback changes *synchronously*. Work that finishes on a background
//!   task is deliberately out of scope for this layer — that is the L2/MCP and
//!   engine-test territory described in `.opencode/guides/testing-guide.md`.
//! * **The platform is per thread.** Slint's context is a thread-local, so
//!   `init_no_event_loop()` must run once *per thread*, never once per process: a
//!   shared `Once` would leave every other test thread without a platform
//!   ("platform not initialized" in `MainWindow::new()`).
//! * **Click the real control.** `click("Component::id")` sends pointer
//!   press/release into the hit-testing path, which is what catches a wiring
//!   mistake. Direct `invoke_*` calls are the fallback for controls that live
//!   inside a `for`/`if` where a single id matches many elements.
//! * **Invisible means absent.** Element queries skip subtrees that are not
//!   visible, and every dialog here is `visible: is_open || opacity > 0.01`.
//!   `has()` therefore doubles as an "is this dialog actually on screen" check.
//! * **`tokio::spawn` must not panic.** Handlers fire and forget (clipboard
//!   prefill, sort persistence, toast expiry). The fixture enters a
//!   *current-thread* tokio runtime so the spawn is legal and then never drives
//!   it, so those tasks stay un-polled and cannot race the assertions.

use std::sync::Arc;

use i_slint_backend_testing::{ElementHandle, ElementQuery};
use parking_lot::Mutex;
use slint::Model;
use slint::platform::PointerEventButton;
use tempfile::TempDir;

use limedl_core::BackendRegistry;
use limedl_core::dispatcher::Dispatcher;
use limedl_core::event_bus::EventBus;
use limedl_core::types::{
    AppSettings, DownloadState, DownloadSummary, Priority, TaskKind, ThreadMode,
};

use crate::context::AppContext;
use crate::i18n::Language;
use crate::ui_boot::{self, UiBootInputs, UiState};
use crate::update::InstallKind;
use crate::{MainWindow, ui_sync};

mod list;
mod shell;

thread_local! {
    /// Slint's global context is a thread-local `OnceCell`, so the testing
    /// backend has to be installed once per test *thread*: libtest runs each
    /// test on its own thread and nextest on its own process, and either way a
    /// second call on the same thread would panic with "platform already
    /// initialized". A process-wide `Once` is the wrong tool — it would fire on
    /// whichever thread ran first and leave the others platform-less.
    static PLATFORM_READY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn init_platform() {
    PLATFORM_READY.with(|ready| {
        if ready.get() {
            return;
        }
        i_slint_backend_testing::init_no_event_loop();
        ready.set(true);
    });
}

/// Build a window with default settings. This is the common case.
pub(crate) fn with_ui(body: impl FnOnce(&mut TestUi)) {
    with_settings(AppSettings::default(), body);
}

/// Build a window whose startup state comes from `settings` — used to assert
/// that persisted preferences actually reach the UI.
pub(crate) fn with_settings(settings: AppSettings, body: impl FnOnce(&mut TestUi)) {
    init_platform();

    // Current-thread runtime: `enter()` makes `tokio::spawn` legal without ever
    // polling the spawned task (see the module docs).
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread tokio runtime");
    {
        let _entered = runtime.enter();
        let mut ui = TestUi::new(settings);
        body(&mut ui);
    }
    drop(runtime);
}

/// A live main window plus the context the handlers were registered with. The
/// window is dropped when the closure returns, which is also what unregisters
/// its item tree from the shared (thread-local) Slint context.
pub(crate) struct TestUi {
    pub window: MainWindow,
    pub ctx: AppContext,
    /// Held only to keep the directory alive (dropping it deletes the temp dir
    /// that the download/state paths were resolved into). Never read: a UI
    /// assertion has no business poking at the data root.
    _data_dir: TempDir,
}

impl TestUi {
    fn new(settings: AppSettings) -> Self {
        let data_dir = tempfile::tempdir().expect("temp data dir");
        let base_dir = data_dir.path().to_path_buf();
        // Mirrors `main()`: the state directory is `<root>/downloads` and the
        // default download directory is resolved by the same shared rule.
        let state_dir = base_dir.join("downloads");
        std::fs::create_dir_all(&state_dir).expect("create state dir");

        // The minimal core handle: no SQLite, no scheduler, no BT session. The
        // UI tests assert on UI state, and every `Dispatcher` accessor they can
        // reach degrades gracefully when a service is absent (`list()` on an
        // empty registry yields no rows, `get_io_status()` returns Err and the
        // settings dialog formats the "not ready" text).
        let event_bus = Arc::new(EventBus::new(64));
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(BackendRegistry::new()),
            event_bus.clone(),
        ));

        let default_download_dir = ui_boot::default_download_dir(&settings, None, &state_dir);

        let state = ui_boot::build_ui(UiBootInputs {
            dispatcher,
            event_bus,
            settings,
            language: Language::ZhCn,
            default_download_dir,
            base_dir,
            rpc_shutdown: Arc::new(Mutex::new(None)),
            // The update UI only mirrors this; asking the OS here would make the
            // test depend on how the test binary happens to be installed.
            install_kind: InstallKind::Portable,
        })
        .expect("ui_boot::build_ui");
        let UiState { window, ctx } = state;

        Self {
            window,
            ctx,
            _data_dir: data_dir,
        }
    }

    /// Look up an element by its qualified `Component::id`.
    ///
    /// Panics with the ids that are actually present when the lookup fails: the
    /// usual cause is a missing `id:` in the `.slint` file or a subtree that
    /// renders only when a condition holds, and the list makes that obvious.
    pub fn find(&self, id: &str) -> ElementHandle {
        if let Some(element) = ElementHandle::find_by_element_id(&self.window, id).next() {
            return element;
        }
        panic!("no element with id {id:?}; visible ids: {:?}", self.ids());
    }

    /// Every element id currently in the tree, sorted and deduplicated.
    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = ElementQuery::from_root(&self.window)
            .match_descendants()
            .match_predicate(|element| element.id().is_some())
            .find_all()
            .into_iter()
            .filter_map(|element| element.id().map(|id| id.to_string()))
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Whether `Component::id` is part of the tree right now. Because element
    /// queries prune invisible subtrees, this is also the "is it on screen"
    /// check for dialogs and conditional panels.
    pub fn has(&self, id: &str) -> bool {
        ElementHandle::find_by_element_id(&self.window, id)
            .next()
            .is_some()
    }

    /// Left-click the middle of `Component::id`: pointer moves to the element
    /// centre, presses and releases, exactly like a user tap.
    pub fn click(&self, id: &str) {
        self.find(id).mock_single_click(PointerEventButton::Left);
    }

    /// Type a query into the search box the way `SearchInput` does: the widget
    /// owns `text <=> root.search_query` and emits `text_changed(<new text>)`,
    /// so a user keystroke moves both the property and the callback. Doing only
    /// one of the two would test a state the app never reaches — and the empty
    /// state's conditional create button reads the property, not the store.
    pub fn search(&self, query: &str) {
        self.window.set_search_query(query.into());
        self.window.invoke_search_changed(query.into());
    }

    /// Replace the task list the way `main()`'s startup load does, and push it
    /// through the same `refresh_ui` the handlers use.
    pub fn seed(&self, tasks: Vec<DownloadSummary>) {
        let mut store = self.ctx.store.lock();
        store.replace_all(tasks);
        ui_sync::refresh_ui(&self.window, &store);
    }

    /// Number of rows the list currently shows.
    pub fn visible_rows(&self) -> usize {
        self.window.get_tasks().row_count()
    }
}

/// A `DownloadSummary` fixture for the list tests.
///
/// Written as a full struct literal (matching `limedl-core`'s
/// `backend_registry` tests) rather than a serde round-trip: adding a field to
/// the summary then breaks the build here, where it is a one-line fix, instead
/// of silently producing a default that the test does not intend.
pub(crate) fn task(
    id: &str,
    file_name: &str,
    state: DownloadState,
    downloaded: u64,
    total: u64,
) -> DownloadSummary {
    DownloadSummary {
        id: id.to_string(),
        kind: TaskKind::Http,
        state,
        url: format!("https://example.invalid/{file_name}"),
        file_name: file_name.to_string(),
        destination_path: format!("/downloads/{file_name}"),
        total_bytes: Some(total),
        downloaded_bytes: downloaded,
        connection_count: 1,
        thread_mode: ThreadMode::Fixed,
        requested_thread_count: None,
        desired_thread_count: None,
        allocated_thread_count: None,
        adaptive_profile: None,
        thread_note: None,
        speed_bytes_per_second: None,
        eta_seconds: None,
        uploaded_bytes: None,
        upload_speed_bytes_per_second: None,
        peer_count: None,
        upload_status: None,
        info_hash: None,
        expected_checksum: None,
        error: None,
        cdn_accelerated: false,
        cdn_node_ip: None,
        created_at_ms: 1,
        priority: Priority::Normal,
        seed_count: None,
        leech_count: None,
        download_limit_bps: None,
        upload_limit_bps: None,
        chunks: vec![],
        mirror_url: None,
    }
}
