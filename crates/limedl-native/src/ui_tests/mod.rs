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
//! * **Click the real control** (`ui.click("Component::id")`): that is what
//!   catches a wiring mistake. Direct `invoke_*` calls are the fallback for
//!   controls inside a `for`/`if` where one id matches many elements, and for
//!   state machines whose value is in the Rust-side index bookkeeping.
//! * **The platform is per thread.** Slint's context is a thread-local, so the
//!   testing backend is installed once *per thread*, never once per process: a
//!   shared `Once` would leave every other test thread without a platform
//!   ("platform not initialized" in `MainWindow::new()`).
//! * **Synchronous by default.** `with_ui` installs mock time and never runs the
//!   event loop, so timers do not fire and `invoke_from_event_loop` delivers
//!   nothing. Assert on what a callback changes *before* it spawns. When the
//!   contract really is asynchronous, use [`with_ui_async`] + [`TestUi::pump_until`].
//! * **Invisible means absent.** Element queries skip subtrees that are not
//!   visible, and every dialog here is `visible: is_open || opacity > 0.01`.
//!   `has()` therefore doubles as an "is this dialog actually on screen" check.
//! * **Assert the blast radius, not just the property.** [`RecordingBackend`]
//!   captures what the UI asked the engine to do, which is the difference
//!   between dropping a row and deleting a file on disk.
//! * **`tokio::spawn` must not panic.** Handlers fire and forget (clipboard
//!   prefill, sort persistence, task actions). `with_ui` enters a current-thread
//!   runtime without driving it, so those tasks stay un-polled; `with_ui_async`
//!   drives it through `block_on` and `pump_until`.

use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

use i_slint_backend_testing::{ElementHandle, ElementQuery};
use parking_lot::Mutex;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalSize, Model};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use limedl_core::types::{
    AppSettings, DownloadState, DownloadSummary, Priority, TaskKind, ThreadMode,
};
use limedl_core::{BackendRegistry, Dispatcher, EventBus};
use limedl_core::services::SettingsService;

use crate::context::AppContext;
use crate::i18n::Language;
use crate::ui_boot::{self, UiBootInputs, UiState};
use crate::update::InstallKind;
use crate::{MainWindow, ui_sync};

use recording::RecordingBackend;

mod async_contracts;
mod inspector;
mod labs;
mod layout;
mod list;
mod new_task;
mod recording;
mod settings;
mod shell;
mod toast;
mod updater;

// Install the testing backend for the calling thread (`init_platform` below).
//
// Two flavors exist and they are incompatible by construction:
//
// * `init_no_event_loop()` is per *thread*: it installs a mock platform with
//   mock time and no event-loop proxy, so several tests can share one process.
//   That is what [`with_ui`]/[`with_settings`] use — and the reason queued
//   callbacks (`invoke_from_event_loop`) are dropped there.
// * `init_integration_test_with_mock_time()` adds the event-loop proxy that
//   [`TestUi::pump_until`] drains. The proxy lives in a **global** `OnceCell`,
//   so this flavor can be installed by exactly one test per process — see
//   [`event_loop_contracts`](crate::ui_tests::async_contracts).
//
// A third option, `TestingBackendOptions { renderer_name }` (real pixels), is not
// available at all: it sits behind the crate's `internal` feature, which cannot
// compile from crates.io — `configure_test_fonts` does
// `include_dir!("$CARGO_MANIFEST_DIR/../../../tests/screenshots/fonts")`, a path
// that only exists inside the Slint workspace. In-process screenshots stay on the
// L2/MCP path.
thread_local! {
    /// The Slint context is itself a thread-local `OnceCell`, so the *mock*
    /// backend has to be installed once per test thread: libtest runs each test
    /// on its own thread and nextest on its own process. A process-wide `Once`
    /// would be wrong — it fires on whichever thread ran first and leaves the
    /// others platform-less ("platform not initialized" in `MainWindow::new()`).
    static PLATFORM_READY: Cell<bool> = const { Cell::new(false) };
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

fn init_platform_with_event_loop() {
    if EVENT_LOOP_PLATFORM_TAKEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        panic!(
            "the testing backend's event loop is process-global and can only be installed once; \
             keep every pump-based scenario inside the single `event_loop_contracts` test \
             (src/ui_tests/async_contracts/)"
        );
    }
    i_slint_backend_testing::init_integration_test_with_mock_time();
}

/// See [`init_platform_with_event_loop`].
static EVENT_LOOP_PLATFORM_TAKEN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Build a window with default settings and run `body` (the common case).
pub(crate) fn with_ui(body: impl FnOnce(&mut TestUi)) {
    with_settings(AppSettings::default(), body);
}

/// An absolute directory path that is valid on this platform.
///
/// The override editor rejects relative keys, and `D:\downloads` is relative on
/// Unix — a test that hardcodes one passes on Windows and fails on macOS and
/// Linux, where the suite also runs.
pub(crate) fn absolute_dir(name: &str) -> String {
    std::env::temp_dir().join(name).to_string_lossy().to_string()
}

/// Build a window whose startup state comes from `settings` — used to assert
/// that persisted preferences actually reach the UI. The UI language stays at the
/// fixture's zh-CN; see [`with_language`].
pub(crate) fn with_settings(settings: AppSettings, body: impl FnOnce(&mut TestUi)) {
    with_language_settings(settings, Language::ZhCn, body);
}

/// Build a window with `language` as the UI language.
///
/// `build_ui` applies it through `slint::select_bundled_translation`, i.e. the
/// `.slint` `@tr` strings really switch — which is the only way to exercise the
/// layouts with the wider English labels (`layout.rs`). That selection is
/// **process-global**: every window created after this one re-applies its own
/// language, and a test that flips it mid-flight must put it back, because
/// `cargo test` runs sibling tests in one process (nextest does not).
pub(crate) fn with_language(language: Language, body: impl FnOnce(&mut TestUi)) {
    with_language_settings(AppSettings::default(), language, body);
}

fn with_language_settings(
    settings: AppSettings,
    language: Language,
    body: impl FnOnce(&mut TestUi),
) {
    init_platform();
    // Current-thread runtime: `enter()` makes `tokio::spawn` legal without ever
    // polling the task, so handlers may fire and forget and the assertions stay
    // deterministic. `with_ui_async` is the variant that actually drives them.
    let runtime = current_thread_runtime();
    {
        let _entered = runtime.enter();
        let mut ui = TestUi::new(settings, language);
        body(&mut ui);
    }
    drop(runtime);
}

/// Like [`with_ui`], but the body may `.await` and therefore call
/// [`TestUi::pump_until`] to complete spawned work or timers.
///
/// The event loop is process-global, so **only one test may use this** (see the
/// module docs): `event_loop_contracts` is that test, and it builds a pristine
/// window per scenario with [`new_window`].
pub(crate) fn with_ui_async(body: impl AsyncFnOnce(&mut TestUi)) {
    init_platform_with_event_loop();
    let runtime = current_thread_runtime();
    runtime.block_on(async {
        // No `enter()`: `block_on` already provides the runtime context, and
        // entering here would forbid `pump_until` from driving it.
        let mut ui = TestUi::new(AppSettings::default(), Language::ZhCn);
        body(&mut ui).await;
    });
}

/// Build another window on the platform this thread already installed.
///
/// The pump-based scenarios share one event loop (one test per process), so they
/// each ask for their own window to stay independent. The language is the
/// fixture's default, which also restores it for everything that runs afterwards.
pub(crate) fn new_window() -> TestUi {
    TestUi::new(AppSettings::default(), Language::ZhCn)
}

fn current_thread_runtime() -> Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread tokio runtime")
}

/// A box in logical window coordinates, as returned by [`TestUi::bounds`].
pub(crate) type LogicalBox = (f32, f32, f32, f32);

/// A live main window, the context the handlers were registered with, and the
/// recording core they talk to. The window is dropped when the closure returns,
/// which is also what unregisters its item tree from the (thread-local) Slint
/// context.
pub(crate) struct TestUi {
    pub window: MainWindow,
    pub ctx: AppContext,
    pub core: Arc<RecordingBackend>,
    /// `LocalTime`/`SystemTime`-free clock for the assertions: see [`TestUi::pump_once`].
    _data_dir: TempDir,
}

impl TestUi {
    fn new(mut settings: AppSettings, language: Language) -> Self {
        let data_dir = tempfile::tempdir().expect("temp data dir");
        let base_dir = data_dir.path().to_path_buf();
        // Mirrors `main()`: the state directory is `<root>/downloads` and the
        // default download directory is resolved by the same shared rule.
        let state_dir = base_dir.join("downloads");
        std::fs::create_dir_all(&state_dir).expect("create state dir");

        // OS notifications must not fire during headless tests; the real app
        // reads this from the persisted settings.
        settings.notifications.enabled = false;
        let settings_path = base_dir.join("settings.json");
        std::fs::write(
            &settings_path,
            serde_json::to_vec_pretty(&settings).expect("settings serialize"),
        )
        .expect("write settings.json");
        let settings_service =
            Arc::new(SettingsService::new(settings_path).expect("settings service"));

        // A real (if minimal) core: the recording backend answers `list()` with
        // the fixtures a test seeded and records every task action, so the UI
        // tests can assert on what the UI *asked for* instead of on a mock of
        // its own. The settings service is real too, so the UI reads and writes
        // through the same single source of truth as production. Everything
        // else stays absent on purpose (no SQLite, no scheduler, no BT session)
        // and every accessor the UI reaches degrades gracefully: `get_io_status()`
        // returns Err and the settings dialog formats the "not ready" text.
        let core = RecordingBackend::new();
        let mut registry = BackendRegistry::new();
        registry.register_arc(TaskKind::Http, core.clone());
        let event_bus = Arc::new(EventBus::new(64));
        let dispatcher = Arc::new(Dispatcher::with_settings_service(
            Arc::new(registry),
            event_bus.clone(),
            settings_service,
        ));

        let default_download_dir = ui_boot::default_download_dir(&settings, None, &state_dir);

        let state = ui_boot::build_ui(UiBootInputs {
            dispatcher,
            event_bus,
            settings,
            language,
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
            core,
            _data_dir: data_dir,
        }
    }

    // ── element queries ────────────────────────────────────────────────

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

    /// Every element with an id that is currently in the tree, sorted.
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

    /// Every instance of `id`, in model order — the way to address a control
    /// that lives inside a `for` loop (task rows, rewrite rule rows, …).
    pub fn find_all(&self, id: &str) -> Vec<ElementHandle> {
        ElementHandle::find_by_element_id(&self.window, id).collect()
    }

    /// Whether `Component::id` is part of the tree right now. Because element
    /// queries prune invisible subtrees, this is also the "is it on screen"
    /// check for dialogs and conditional panels.
    pub fn has(&self, id: &str) -> bool {
        ElementHandle::find_by_element_id(&self.window, id)
            .next()
            .is_some()
    }

    // ── interaction ────────────────────────────────────────────────────

    /// Left-click the middle of `Component::id`: pointer moves to the element
    /// centre, presses and releases, exactly like a user tap.
    pub fn click(&self, id: &str) {
        self.find(id).mock_single_click(PointerEventButton::Left);
    }

    /// Right-click the middle of `Component::id`. This is what the task list and
    /// the cards listen for to open their context menu.
    pub fn right_click(&self, id: &str) {
        self.find(id).mock_single_click(PointerEventButton::Right);
    }

    /// Click the `n`-th instance of `id` (0-based, model order).
    pub fn click_nth(&self, id: &str, n: usize) {
        let all = self.find_all(id);
        assert!(
            n < all.len(),
            "id {id:?} has {} instances, wanted #{n}",
            all.len()
        );
        all[n].mock_single_click(PointerEventButton::Left);
    }

    /// Right-click the `n`-th instance of `id` (0-based, model order).
    pub fn right_click_nth(&self, id: &str, n: usize) {
        let all = self.find_all(id);
        assert!(
            n < all.len(),
            "id {id:?} has {} instances, wanted #{n}",
            all.len()
        );
        all[n].mock_single_click(PointerEventButton::Right);
    }

    /// Shift-click the `n`-th instance of `id` (0-based, model order) — the
    /// range gesture.
    ///
    /// A pointer event carries no modifiers of its own: the core fills
    /// `PointerEvent.modifiers` from the modifier keys the window currently sees
    /// held down. Pressing Shift around a plain click is therefore exactly what a
    /// real shift-click delivers (and what `press_keys` already relies on for
    /// Ctrl+A), while `mock_single_click` cannot express it.
    pub fn shift_click_nth(&self, id: &str, n: usize) {
        self.send_key(Key::Shift.into(), true);
        self.click_nth(id, n);
        self.send_key(Key::Shift.into(), false);
    }

    /// Type `text` into whatever currently has focus. Click the field first —
    /// a key event with no focus goes nowhere, silently.
    ///
    /// Slint's own helper for this lives in the testing crate's `internal`
    /// module, which cannot compile outside the Slint workspace (see the note on
    /// [`init_platform`]), so the two lines it needs are inlined here. Uppercase
    /// keys are wrapped in Shift, exactly like the helper does, because a text
    /// input only inserts a character when the modifiers say so.
    pub fn type_text(&self, text: &str) {
        for character in text.chars() {
            if character.is_ascii_uppercase() {
                self.send_key(Key::Shift.into(), true);
            }
            self.send_key(character, true);
            self.send_key(character, false);
            if character.is_ascii_uppercase() {
                self.send_key(Key::Shift.into(), false);
            }
        }
    }

    /// Press and release a key combination, e.g. `&[Key::Control.into(), 'n']`.
    /// Modifiers are separate key events, exactly as a real keyboard delivers
    /// them, and they nest back out in reverse order.
    pub fn press_keys(&self, keys: &[char]) {
        for key in keys {
            self.send_key(*key, true);
        }
        for key in keys.iter().rev() {
            self.send_key(*key, false);
        }
    }

    fn send_key(&self, key: char, pressed: bool) {
        let text: slint::SharedString = key.into();
        self.window.window().dispatch_event(if pressed {
            WindowEvent::KeyPressed { text }
        } else {
            WindowEvent::KeyReleased { text }
        });
    }

    // ── window state ───────────────────────────────────────────────────

    /// Window size in the logical pixels that `absolute_position()`/`size()` use.
    ///
    /// Read from the root window element instead of `Window::size()`: the testing
    /// backend stores whatever it was given as *physical* while reporting the
    /// host's scale factor, so `physical / scale` is wrong on a HiDPI machine
    /// (and `Window::set_size` cannot fix that — it converts with scale 1.0).
    /// The root element is laid out in the same logical space the assertions use.
    ///
    /// `find_by_element_id` cannot reach it (`query_descendants` starts below the
    /// root and the root's id is empty), hence the bare query.
    pub fn window_logical_size(&self) -> (f32, f32) {
        let root = ElementQuery::from_root(&self.window)
            .find_first()
            .expect("the window has a root element");
        let size = root.size();
        (size.width, size.height)
    }

    /// Resize the window (logical pixels) — used to test the layout at
    /// `min-width`/`min-height`, where dialogs are most likely to overflow.
    pub fn set_window_size(&self, width: f32, height: f32) {
        self.window
            .window()
            .set_size(LogicalSize::new(width, height));
    }
    /// Override the device-pixel ratio, i.e. what a HiDPI screen does.
    pub fn set_scale_factor(&self, factor: f32) {
        self.window
            .window()
            .dispatch_event(WindowEvent::ScaleFactorChanged {
                scale_factor: factor,
            });
    }

    // ── list fixtures ──────────────────────────────────────────────────

    /// Replace the task list the way `main()`'s startup load does, and push it
    /// through the same `refresh_ui` the handlers use.
    pub fn seed(&self, tasks: Vec<DownloadSummary>) {
        self.core.set_tasks(tasks.clone());
        let mut store = self.ctx.store.lock();
        store.replace_all(tasks);
        ui_sync::refresh_ui(&self.window, &store);
    }

    /// Number of rows the list currently shows.
    pub fn visible_rows(&self) -> usize {
        self.window.get_tasks().row_count()
    }

    /// Type a query into the search box the way a user does: click the field to
    /// focus it, then send the keystrokes.
    pub fn search(&self, query: &str) {
        self.click("SearchInput::ta");
        if !query.is_empty() {
            self.type_text(query);
            return;
        }
        // Backspace over whatever is there. The search field is the only
        // focusable text input holding this text, so the count is unambiguous.
        let backspaces: Vec<char> =
            std::iter::repeat_n(Key::Backspace.into(), self.window.get_search_query().len())
                .collect();
        self.press_keys(&backspaces);
    }

    // ── assertions: geometry ───────────────────────────────────────────

    /// `(x, y, width, height)` in logical window coordinates.
    pub fn bounds(&self, id: &str) -> LogicalBox {
        let element = self.find(id);
        let position = element.absolute_position();
        let size = element.size();
        (position.x, position.y, size.width, size.height)
    }

    /// Assert that `id` is laid out inside the window and is not degenerate.
    ///
    /// Dialogs are centred with `min(parent.width - 100px, …)` and their content
    /// is a sequence of layouts, so "the footer is below the bottom edge" is a
    /// realistic regression — English labels are wider than the Chinese ones the
    /// layout was tuned for, and the settings tab row already overflowed once.
    pub fn assert_inside_window(&self, id: &str) {
        let (x, y, width, height) = self.bounds(id);
        let (window_width, window_height) = self.window_logical_size();
        assert!(
            width > 0.0 && height > 0.0,
            "{id} has a degenerate size {width}x{height}\nvisible elements:\n{}",
            self.id_tree()
        );
        assert!(
            x >= -0.5
                && y >= -0.5
                && x + width <= window_width + 0.5
                && y + height <= window_height + 0.5,
            "{id} is at ({x}, {y}) {width}x{height}, outside the {window_width}x{window_height} window\n\
             visible elements:\n{}",
            self.id_tree()
        );
    }

    /// Assert a minimum size for a set of controls (a tap target that shrank to
    /// zero is a hit-testing bug, not a cosmetic one).
    pub fn assert_min_size(&self, ids: &[&str], minimum: f32) {
        for id in ids {
            let (_, _, width, height) = self.bounds(id);
            assert!(
                width >= minimum && height >= minimum,
                "{id} is {width}x{height}, smaller than the {minimum}px minimum"
            );
        }
    }

    /// Assert that the listed elements (siblings, or at least not nested) do not
    /// overlap. Useful for toolbars, dialog headers and action bars.
    pub fn assert_no_overlap(&self, ids: &[&str]) {
        let boxes: Vec<(&str, LogicalBox)> = ids.iter().map(|id| (*id, self.bounds(id))).collect();
        for (index, (first_id, first)) in boxes.iter().enumerate() {
            for (second_id, second) in boxes.iter().skip(index + 1) {
                let overlap_x =
                    (first.0 + first.2).min(second.0 + second.2) - first.0.max(second.0);
                let overlap_y =
                    (first.1 + first.3).min(second.1 + second.3) - first.1.max(second.1);
                assert!(
                    overlap_x <= 0.5 || overlap_y <= 0.5,
                    "{first_id} {first:?} overlaps {second_id} {second:?}"
                );
            }
        }
    }

    /// A stable description of every *identified* element: one `id  type  x,y wxh`
    /// line per element, sorted. Diffs of this string catch structural
    /// regressions (a control that stopped being instantiated, a swapped order)
    /// without pinning pixel positions.
    pub fn id_tree(&self) -> String {
        let mut lines: Vec<String> = ElementQuery::from_root(&self.window)
            .match_descendants()
            .match_predicate(|element| element.id().is_some())
            .find_all()
            .into_iter()
            .map(|element| {
                let position = element.absolute_position();
                let size = element.size();
                format!(
                    "{}  {}  {},{:.0} {:.0}x{:.0}",
                    element.id().unwrap_or_default(),
                    element.type_name().unwrap_or_default(),
                    position.x.round(),
                    position.y.round(),
                    size.width.round(),
                    size.height.round()
                )
            })
            .collect();
        lines.sort();
        lines.join("\n")
    }

    // ── assertions: toasts ─────────────────────────────────────────────

    /// The pending toasts as `(kind, message)`. Toasts that are playing their
    /// exit animation (`leaving`) are already retired and must not count.
    pub fn toasts(&self) -> Vec<(String, String)> {
        self.ctx
            .toast_queue
            .lock()
            .iter()
            .filter(|entry| !entry.leaving)
            .map(|entry| (entry.kind.to_string(), entry.message.clone()))
            .collect()
    }

    /// Assert that exactly one toast is pending, of `kind`, whose message
    /// contains `text`. Returns nothing: the assertion *is* the point, and a
    /// wrong-kind toast should fail with the whole queue visible.
    pub fn assert_toast(&self, kind: &str, text: &str) {
        let toasts = self.toasts();
        assert_eq!(
            toasts.len(),
            1,
            "expected exactly one toast, got {toasts:?} (looking for {kind}/{text:?})"
        );
        let (actual_kind, message) = &toasts[0];
        assert_eq!(actual_kind, kind, "toast kind mismatch: {toasts:?}");
        assert!(
            message.contains(text),
            "toast {message:?} does not contain {text:?}"
        );
    }

    /// Remove a toast through the same handler the close button calls.
    pub fn dismiss_toast(&self, id: i32) {
        self.window.invoke_dismiss_toast(id);
    }

    /// Ids of the pending toasts (for [`Self::dismiss_toast`]). Leaving toasts
    /// are excluded: their close button is fading out with them.
    pub fn toast_ids(&self) -> Vec<i32> {
        self.ctx
            .toast_queue
            .lock()
            .iter()
            .filter(|entry| !entry.leaving)
            .map(|entry| entry.id as i32)
            .collect()
    }

    // ── asynchronous settling ──────────────────────────────────────────

    /// Run spawned work and Slint's queue until `done` holds, or fail loudly.
    ///
    /// One round advances the mocked clock (making due timers fire), lets the
    /// tokio runtime poll the tasks the handlers spawned, and then drains
    /// Slint's queue by running the event loop until a freshly queued future
    /// quits it. Bounded on purpose: a contract that never settles fails the
    /// test instead of hanging CI.
    pub async fn pump_until(&self, what: &str, mut done: impl FnMut() -> bool) {
        // Bounded on purpose: a contract that never settles fails the test
        // instead of hanging CI. The bound is wall-clock rather than a round
        // count because a pump round only advances the *mocked* clock and drains
        // Slint's queue, while the engine call it waits for can include real
        // blocking work (`SettingsService` persists through `tokio::fs`, i.e.
        // `spawn_blocking`). A counted loop can exhaust every round before such a
        // task is ever polled — that is how the settings-save contract flaked on
        // a loaded coverage runner.
        const SETTLE_TIMEOUT: Duration = Duration::from_secs(5);
        let deadline = std::time::Instant::now() + SETTLE_TIMEOUT;
        loop {
            if done() {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "{what} did not settle within {SETTLE_TIMEOUT:?}; engine calls so far: {:?}",
                self.core.calls()
            );
            self.pump_once().await;
            // Give the blocking pool a real slice of time before the next
            // (instant) mock-time step, so a `spawn_blocking` completion has
            // landed by the time the runtime is polled again.
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// [`Self::pump_until`] with a fixed number of rounds, for contracts whose
    /// completion is "no further change" rather than a predicate.
    pub async fn pump(&self, rounds: usize) {
        for _ in 0..rounds {
            self.pump_once().await;
        }
    }

    async fn pump_once(&self) {
        // 1. Advance the mocked clock so timers become due.
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20)); // 2. Let the tokio runtime poll what the handlers spawned; their
        //    `invoke_from_event_loop` callbacks land in Slint's queue.
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        // 3. Drain Slint's queue (queued callbacks and now-due timers) and come
        //    back: the loop exits when the future we just queued runs.
        slint::spawn_local(async {
            let _ = slint::quit_event_loop();
        })
        .expect("the testing backend provides an event loop proxy");
        let _ = slint::run_event_loop();
    }
}

/// Wire id of a synthetic HTTP task, e.g. `http:00000000-0000-4000-8000-000000000001`.
///
/// The ids have to be *valid* uuids, not just opaque strings: `TaskAction`
/// parses them with `TaskId::from_wire_string` and silently skips what it cannot
/// parse, so a fixture like `http:one` would make every action test pass while
/// nothing was ever routed to a backend.
pub(crate) fn http_wire(n: u8) -> String {
    format!("http:00000000-0000-4000-8000-{n:012}")
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

/// A task whose id can actually be routed to a backend: `n` picks the uuid.
///
/// `created_at_ms` *decreases* with `n` on purpose: the list defaults to
/// newest-first, so task 1 ends up in row 0 and the row indices the tests use
/// match the numbers they seeded. (Seeding three tasks with one shared timestamp
/// would leave their order to the sort's tie-breaking instead.)
pub(crate) fn http_task(
    n: u8,
    file_name: &str,
    state: DownloadState,
    downloaded: u64,
    total: u64,
) -> DownloadSummary {
    let mut summary = task(&http_wire(n), file_name, state, downloaded, total);
    summary.created_at_ms = (1_000 - u64::from(n)) * 1_000;
    summary
}
