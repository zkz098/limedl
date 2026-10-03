//! UI assembly, shared by `main()` and the in-process UI tests.
//!
//! `main()` owns everything that touches the operating system: single-instance
//! claim, tray icon, logging, window hooks and the event loop.
//! This module owns the part that is pure UI — build the window, seed it from
//! the settings, publish the shared [`AppContext`] and register every callback.
//!
//! The split exists so `src/ui_tests/` can drive the *real* window with the
//! *real* handlers instead of re-implementing the wiring: the tests call
//! [`build_ui`] with the same inputs and a headless core handle, then send
//! pointer events at elements. Everything `build_ui` needs from the OS or from
//! async core lookups is passed in as an already-resolved input
//! ([`UiBootInputs`]), which keeps this function synchronous and free of timers,
//! so a test never has to pump an event loop to see its effect.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;
use slint::{ComponentHandle, SharedString};
use tokio::sync::watch;

use limedl_core::dispatcher::Dispatcher;
use limedl_core::event_bus::EventBus;
use limedl_core::types::{AppSettings, SortDirection};

use crate::bridge::TaskStore;
use crate::context::AppContext;
use crate::i18n::{self, Language};
use crate::update;
use crate::{MainWindow, UpdateState, handlers, ui_sync};

/// Inputs `main()` must resolve before the window exists: the two OS queries
/// (`install_kind`, `base_dir`) and the async core lookups (settings, download
/// directory).
pub struct UiBootInputs {
    pub dispatcher: Arc<Dispatcher>,
    pub event_bus: Arc<EventBus>,
    pub settings: AppSettings,
    pub language: Language,
    /// Resolved download directory: settings → dispatcher → OS default.
    pub default_download_dir: String,
    /// Writable app data root (window geometry, logs, update work dir).
    pub base_dir: PathBuf,
    /// Shutdown handle of the Aria2 RPC server. Created by `main()` *before*
    /// the UI because the server is spawned there, and shared with the settings
    /// handlers that hot-reload it.
    pub rpc_shutdown: Arc<Mutex<Option<watch::Sender<bool>>>>,
    /// Which channel this binary was installed through (drives the update UI).
    pub install_kind: update::InstallKind,
}

/// The live window plus everything the background machinery needs to talk to
/// it. `window` duplicates `ctx.ui` (same handle, no extra component) and is
/// kept separate so callers can destructure without fighting the borrow of
/// `ctx` — `main()` uses it for `show()` / `window()` / the event loop.
pub struct UiState {
    pub window: MainWindow,
    pub ctx: AppContext,
}

/// Resolve the directory the new-task dialog starts in: an explicit setting
/// wins, then the core's own default, then the OS download folder, then the
/// app's state directory.
///
/// Shared with the UI tests on purpose. `main()` resolves the core half
/// asynchronously (`Dispatcher::default_download_dir`) and only calls this when
/// the setting is empty, so the lookup stays off the startup path in the
/// configured case; the tests pass `None` because a minimally wired core has no
/// opinion. Keeping one implementation is what makes the two paths agree.
pub fn default_download_dir(
    settings: &AppSettings,
    core_dir: Option<String>,
    state_dir: &Path,
) -> String {
    if !settings.download.default_download_dir.is_empty() {
        return settings.download.default_download_dir.clone();
    }
    if let Some(dir) = core_dir {
        return dir;
    }
    if let Some(dir) = crate::paths::dirs_download_dir() {
        return dir.to_string_lossy().to_string();
    }
    state_dir.to_string_lossy().to_string()
}

/// Build the main window, apply the initial settings and register all UI
/// callbacks.
///
/// Deliberately not `async` and free of timers: window geometry restore and
/// theme sync belong to `main()` because they query the OS or need the event
/// loop to do anything at all. A timer registered here
/// would be invisible to the tests (the testing backend mocks time and never
/// fires it), which is exactly the kind of silent no-op the seam avoids.
pub fn build_ui(inputs: UiBootInputs) -> anyhow::Result<UiState> {
    let UiBootInputs {
        dispatcher,
        event_bus,
        settings,
        language,
        default_download_dir,
        base_dir,
        rpc_shutdown,
        install_kind,
    } = inputs;

    let store = Arc::new(Mutex::new(TaskStore::with_language(language)));
    {
        let mut s = store.lock();
        s.apply_sort(
            crate::bridge::sort_key_to_field(settings.appearance.sort_key),
            matches!(settings.appearance.sort_direction, SortDirection::Asc),
        );
    }

    let window = MainWindow::new()?;
    i18n::apply_translation(language);
    let ui_weak = window.as_weak();

    window.set_update_state(UpdateState {
        phase: "idle".into(),
        latest_version: "".into(),
        notes: "".into(),
        progress_percent: 0.0,
        progress_label: "".into(),
        error_text: "".into(),
        install_kind: match install_kind {
            update::InstallKind::Store => "store".into(),
            update::InstallKind::Installer => "installer".into(),
            update::InstallKind::Portable => "portable".into(),
        },
    });

    ui_sync::apply_appearance(
        &window,
        settings.appearance.color_mode.clone(),
        settings.appearance.theme_color.clone(),
    );
    window.set_default_download_dir(SharedString::from(&default_download_dir));
    window.set_new_task_dir(SharedString::from(&default_download_dir));
    ui_sync::apply_view_preferences(&window, &settings);

    let toast_queue = Arc::new(Mutex::new(Vec::new()));
    let game_mode_active = Arc::new(Mutex::new(dispatcher.game_mode()));
    let is_overclock_mode = Arc::new(Mutex::new(dispatcher.get_overclock_mode()));
    let tray_speed_limit_active = Arc::new(AtomicBool::new(settings.global_speed_limit_bps > 0));

    let ctx = AppContext {
        ui: window.clone_strong(),
        ui_weak: ui_weak.clone(),
        dispatcher,
        event_bus,
        store,
        toast_queue,
        rpc_shutdown,
        new_task_torrent_entries: Arc::new(Mutex::new(Vec::new())),
        new_task_torrent_included: Arc::new(Mutex::new(Vec::new())),
        active_inspector_id: Arc::new(Mutex::new(None)),
        labs_expanded_ids: Arc::new(Mutex::new(HashSet::new())),
        labs_candidates: Arc::new(Mutex::new(Vec::new())),
        rewrite_rules: Arc::new(Mutex::new(settings.url_rewrite.rules.clone())),
        sandbox_test_url: Arc::new(Mutex::new(
            "https://raw.github.com/user/repo/master/README.md".to_string(),
        )),
        game_mode_active,
        is_overclock_mode,
        tray_speed_limit_active,
        base_dir,
    };

    handlers::register_all(&ctx);

    Ok(UiState { window, ctx })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with_dir(dir: &str) -> AppSettings {
        let mut settings = AppSettings::default();
        settings.download.default_download_dir = dir.to_string();
        settings
    }

    #[test]
    fn an_explicit_setting_beats_every_fallback() {
        assert_eq!(
            default_download_dir(
                &settings_with_dir("chosen"),
                Some("core".to_string()),
                Path::new("state")
            ),
            "chosen"
        );
    }

    #[test]
    fn the_core_directory_beats_the_os_download_folder() {
        assert_eq!(
            default_download_dir(
                &settings_with_dir(""),
                Some("core".to_string()),
                Path::new("state")
            ),
            "core"
        );
    }

    #[test]
    fn the_result_is_never_empty() {
        // `paths::dirs_download_dir()` resolves on some hosts and not others, so
        // only the invariant is pinned here: an empty string would open the
        // new-task dialog without any destination at all.
        let resolved = default_download_dir(&settings_with_dir(""), None, Path::new("state"));
        assert!(!resolved.is_empty());
        if crate::paths::dirs_download_dir().is_none() {
            assert_eq!(resolved, "state", "the state directory is the last resort");
        }
    }
}
