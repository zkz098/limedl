---
type: desktop
title: Native Desktop UI (Slint)
description: How the limedl Slint desktop client is assembled and driven — the main/ui_boot split, AppContext, the pure bridge mapping layer, handler plumbing, the EventBus subscriber, i18n rules, and platform integration for tray, autostart, single instance, power and window geometry.
tags: [desktop, slint, ui, event-stream, i18n, platform]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T01:52:48.489Z
sources:
  - id: openwiki-source-e5a81e0b5d28c08eec080c86
    resource: repo://crates/limedl-native/src/autostart.rs
  - id: openwiki-source-2ba14b7f8d43883e76ffefa7
    resource: repo://crates/limedl-native/src/bridge/mod.rs
  - id: openwiki-source-ec243741e58f29f41733a43b
    resource: repo://crates/limedl-native/src/context.rs
  - id: openwiki-source-62650ee8fbcfd9713b6c1fa1
    resource: repo://crates/limedl-native/src/event_stream/bus.rs
  - id: openwiki-source-57b792082fe069747cfd4458
    resource: repo://crates/limedl-native/src/handlers/common.rs
  - id: openwiki-source-fd6be2a9af9417e7143ee8cb
    resource: repo://crates/limedl-native/src/handlers/mod.rs
  - id: openwiki-source-e3df85aea81b1db0ad56647b
    resource: repo://crates/limedl-native/src/i18n/mod.rs
  - id: openwiki-source-c41cc3787c7358f1d82dd1aa
    resource: repo://crates/limedl-native/src/i18n/tests.rs
  - id: openwiki-source-fb048d5fb7a13a8f9b8fad76
    resource: repo://crates/limedl-native/src/main.rs
  - id: openwiki-source-4e7bfc4372b706c2df6eec37
    resource: repo://crates/limedl-native/src/platform_win.rs
  - id: openwiki-source-9fcf6c9f238bbeab9e68750f
    resource: repo://crates/limedl-native/src/power.rs
  - id: openwiki-source-2eeefaa5bac4b0e28bfe7d6e
    resource: repo://crates/limedl-native/src/settings_sync.rs
  - id: openwiki-source-3d2f0bb3886d015eae78f8e2
    resource: repo://crates/limedl-native/src/single_instance.rs
  - id: openwiki-source-5073fe8c9810f134863f36c3
    resource: repo://crates/limedl-native/src/toast.rs
  - id: openwiki-source-0881c19aaf90e245d1cac357
    resource: repo://crates/limedl-native/src/tray.rs
  - id: openwiki-source-0f94f42c28d80adb527ec8d0
    resource: repo://crates/limedl-native/src/ui_boot.rs
  - id: openwiki-source-dc8eda3d2c4e2e45f618f8dc
    resource: repo://crates/limedl-native/src/ui_sync.rs
  - id: openwiki-source-90185777dff572d79a3b452d
    resource: repo://crates/limedl-native/ui/theme.slint
generated: { by: "pi", at: "2026-10-04T01:52:48.489Z" }
---

# Native Desktop UI (Slint)

`crates/limedl-native` is the only shipped desktop client and the only UI over
`limedl-core`. It is a single Slint process: the engine is linked in directly, so
there is no IPC, webview or serialization boundary between UI and engine.

## The main / ui_boot split

`main()` owns everything that touches the operating system, and `ui_boot::build_ui`
owns the pure UI assembly:

- `main()` parses the CLI contract `limedl-native [--hidden] [<url|magnet|path|limedl://…>]`,
  claims the single-instance slot, resolves the data directory, calls
  `bootstrap()`, initializes logging, syncs autostart, optionally starts the
  Aria2 RPC server, decides the language, resolves the default download
  directory, then calls `build_ui`, installs the tray, restores window geometry,
  registers platform hooks, starts the background listeners and the event loop.
- `build_ui(UiBootInputs)` receives everything that needs the OS or an async
  lookup as **already-resolved inputs** (settings, language, default directory,
  base directory, RPC shutdown handle, install kind). It is deliberately **not
  async and free of timers**: it builds the real `MainWindow`, applies
  translation and appearance, constructs `AppContext` and calls
  `handlers::register_all`. That is exactly the seam that lets the in-process UI
  tests drive the same window and callbacks headlessly.

Evidence: `repo://crates/limedl-native/src/main.rs#L31-L103`,
`repo://crates/limedl-native/src/ui_boot.rs#L8-L16`,
`repo://crates/limedl-native/src/ui_boot.rs#L62-L224`.

## AppContext

`AppContext` is the shared handle bag passed to every handler. It holds the
`MainWindow` and its weak handle, the `Dispatcher`, the `EventBus`, the
`Arc<Mutex<TaskStore>>`, the toast queue, the RPC shutdown sender, scratch state
for the new-task torrent file list, the active inspector id, Labs expansion and
candidates, rewrite rules, sandbox test URL, game/overclock flags, the tray speed
limit flag and the base directory.

`AppContext::settings()` is explicitly a *read convenience*, not a cache: it reads
through the dispatcher's `get_settings_blocking()`, and writers must use
`Dispatcher::save_settings_with` so a concurrent update is not clobbered. The
desktop once kept a shadow settings copy with nine write-back points; that was
removed.

Evidence: `repo://crates/limedl-native/src/context.rs#L14-L51`.

## The bridge layer is pure mapping

`src/bridge/` converts between engine types and Slint models without side effects,
which is why it can be unit-tested without a window. It re-exports domain-focused
modules: `models` (`DownloadSummary → TaskItem`/`InspectorInfo`),
`forms` (`AppSettings ↔ SettingsFormData`, split into `combo`, `enums`,
`to_form`, `from_form`, `speed_limit`, `disk_override`, `aria2_clients`),
`task_store` (filter/sort/multi-select), `format`, `clipboard`, `labs`,
`piece_map` and `setup_wizard`.

Evidence: `repo://crates/limedl-native/src/bridge/mod.rs#L1-L20`.

List-style editors follow one contract: their **editing state lives in the UI
model** and Rust parses it only at Save (`read_schedule_rows`,
`read_disk_override_rows`, `read_aria2_client_rows`). Inline edits go through
`set_row_data` so the in-progress input does not lose its cursor; adding or
removing rows rebuilds the model. The Aria2 client editor additionally generates
and hashes tokens on Add/Regenerate and only ever persists `token_hash`.

Evidence: `repo://crates/limedl-native/src/ui_sync.rs#L70-L121`.

## Handler registration and shared plumbing

`handlers::register_all` calls one `register(ctx)` per subsystem: `task`,
`new_task`, `inspector`, `settings`, `setup_wizard`, `labs`, `updater`, `window`.
Each module's `register` only wires callbacks; callback bodies longer than a few
lines are extracted into named functions, and flows shared across callbacks
(batch delete, torrent preview, checksum probing, CDN apply) exist once.

`handlers/common.rs` owns the boilerplate: `with_ui` (upgrade a weak window),
`read_ui` (read a value before spawning), `mutate_store` (mutate + refresh under
the store lock), `refresh_from_anywhere`, `reload_tasks` (re-read from the
dispatcher), `take_selection`, `refresh_after_removal`, `drop_locally`, the
`TaskAction` enum (`Pause`/`Resume`/`Remove`/`Purge`/`OpenInExplorer`) and the
`spawn_action`/`spawn_batch_action`/`spawn_for_state` helpers. New handlers use
these instead of writing `as_weak()`/`upgrade()` blocks.

Evidence: `repo://crates/limedl-native/src/handlers/mod.rs#L1-L22`,
`repo://crates/limedl-native/src/handlers/common.rs#L1-L232`.

## The DownloadEvent subscriber

`event_stream::start_event_bus_listener` spawns one task that matches every
`DownloadEvent` variant explicitly (no catch-all): `Updated`, `Progress`,
`CdnProgress`, `CdnComplete`, `Warning` and `Aria2Notification`. The Aria2
variant is intentionally ignored by the desktop, because the RPC server pushes
those straight to its own clients.

Behavior worth knowing:

- **Removal detection**: `Updated` means "the row changed", including removal.
  `on_updated` asks `dispatcher.status(task_id)`; if the backend no longer knows
  the task, the row is dropped and the inspector closes.
- **Lagged recovery**: on `RecvError::Lagged`, the task calls `Dispatcher::list()`
  and replaces the whole store. Partial replay is impossible, so a snapshot is
  the only correct recovery. `Closed` ends the loop.
- **Notifications**: terminal `Completed`/`Failed` states produce an OS
  notification (when `notifications.enabled`) plus a toast.
- **Warning dedup**: core emits one warning per peer/mirror/disk check, so
  `WarningDedup` collapses the *same* `id:message` within a 5 s window.

Evidence: `repo://crates/limedl-native/src/event_stream/bus.rs#L40-L343`,
`repo://crates/limedl-native/src/toast.rs#L66-L111`.

Toasts themselves are a shared `Vec<ToastEntry>` behind a mutex; `push_toast`
appends, syncs to the Slint model and spawns an auto-dismiss task for the given
duration, and `dismiss_toast` removes one immediately.

Evidence: `repo://crates/limedl-native/src/toast.rs#L12-L62`.

## Settings save side effects live in one module

`settings_sync::SettingsSync` is shared by the settings dialog and the setup
wizard. It provides `commit` (tray limit flag), `sync_autostart` (only when the
flag changed), `sync_aria2_rpc` (stop the old server, restart when enabled) and
`push_ui`, which marshals to the UI thread and applies language, tray menu,
appearance, default directory, task list, optional inspector refresh and dialog
closing. `push_ui` also resets `reset_confirm` on close — the About tab's
two-stage factory-reset gate is disarmed on every close path so Escape or Cancel
cannot leave the app one click from wiping the data directory.

Evidence: `repo://crates/limedl-native/src/settings_sync.rs#L1-L184`.

## Internationalization

Two tracks, and both must be kept in sync:

- `.slint` user-visible strings use `@tr(...)`; catalogs live in
  `lang/{en,zh_CN,zh_TW}/LC_MESSAGES/`.
- Rust-side dynamic text must use `i18n::format_*` helpers (split by domain:
  `task`, `dialogs`, `toast`, `tray`, `validation`, `cdn`, `rewrite`, `schedule`,
  `disk`), never hardcoded CJK, or English builds leak Chinese.

A test (`test_all_slint_tr_strings_in_po_catalogs`) fails when a new `@tr`
string is missing from a catalog.

Evidence: `repo://crates/limedl-native/src/i18n/mod.rs#L1-L16`,
`repo://crates/limedl-native/src/i18n/tests.rs#L246-L260`.

## Platform integration

### Single instance

`InstanceClaim::claim()` uses a session-local named mutex on Windows (secondary
launches find the window by title and forward the payload via `WM_COPYDATA`) and
a loopback TCP listener on port 45997 elsewhere (secondary launches send `show`
or `open:<payload>`). The primary handle must stay alive for the whole process —
dropping it would release the claim.

Evidence: `repo://crates/limedl-native/src/single_instance.rs#L1-L181`.

### Autostart

Every OS registration appends `--hidden`, so login starts go to the tray. Windows
portable/NSIS uses the HKCU `Run` key; MSIX uses the `windows.startupTask`
manifest extension because registry `Run` is virtualized in MSIX; Linux writes an
XDG `.desktop`; macOS writes a LaunchAgent. `registration_is_current` re-registers
when the executable moved (portable extraction to a new folder, debug vs release).

Evidence: `repo://crates/limedl-native/src/autostart.rs#L1-L42`.

### Tray and power

The tray menu (show, pause all, resume all, speed-limit check, game mode, open
download dir, quit) is built with `muda` and updated on the UI thread by
`update_tray_menu_and_tooltip`. On Linux the tray is a D-Bus StatusNotifierItem
(ksni), so a missing host leaves the icon waiting rather than crashing; a failed
D-Bus connection is fatal and produces a readable explanation.

`PowerGuard` uses `SetThreadExecutionState` on Windows to keep the system awake
while downloads run and is a no-op elsewhere.

Evidence: `repo://crates/limedl-native/src/tray.rs#L1-L102`,
`repo://crates/limedl-native/src/power.rs#L1-L113`.

### Window geometry

Geometry is saved on exit (Windows additionally on `WM_EXITSIZEMOVE`/`WM_SIZE`
via the window subclass). Restoration differs by platform:

- **Windows** reads `GetWindowPlacement` so a maximized window still saves its
  restorable size, and uses `MonitorFromRect` to reject rectangles that are no
  longer on any monitor (centering instead). A maximized `ShowWindow` is never
  issued on a hidden window.
- **macOS/Linux** use Slint's `Window::position/size/is_maximized` and a
  `MAX_PLAUSIBLE_COORD` (±20000 px) heuristic instead of real monitor geometry.

Because the OS window only exists after the event loop starts,
`schedule_window_placement_restore` retries every 250 ms until
`apply_restored_window_placement` reports success. This timer is also what
restores geometry on a `--hidden` tray start.

Evidence: `repo://crates/limedl-native/src/platform_win.rs#L553-L675`,
`repo://crates/limedl-native/src/ui_sync.rs#L250-L302`.

## Theming

`ui/theme.slint` is generated from a source mapping and exposes a `Theme` global
with `mode` (from `appearance.color_mode`) and `accent` (from
`appearance.theme_color`). Components must use `Theme.c<hex>` tokens instead of
hardcoded hex; `apply_appearance` translates settings into the `Theme` global and
the OS window theme.

Evidence: `repo://crates/limedl-native/ui/theme.slint#L1-L26`,
`repo://crates/limedl-native/src/ui_sync.rs#L304-L319`.

<!-- openwiki: broken internal link [/openwiki/architecture/overview.md] link "/openwiki/architecture/overview.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Related pages: [Workspace and System Architecture](/openwiki/architecture/overview.md),
<!-- openwiki: broken internal link [/openwiki/systems/settings-and-configuration.md] link "/openwiki/systems/settings-and-configuration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Settings and Configuration](/openwiki/systems/settings-and-configuration.md),
<!-- openwiki: broken internal link [/openwiki/desktop/self-update-and-distribution.md] link "/openwiki/desktop/self-update-and-distribution.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Self-Update and Distribution Channels](/openwiki/desktop/self-update-and-distribution.md),
<!-- openwiki: broken internal link [/openwiki/testing/slint-ui-testing.md] link "/openwiki/testing/slint-ui-testing.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Slint UI Testing](/openwiki/testing/slint-ui-testing.md).
