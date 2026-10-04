---
type: desktop
title: Native Desktop UI (Slint)
description: How the limedl Slint desktop client is assembled and driven — the main/ui_boot split, AppContext, the pure bridge mapping layer, handler plumbing, the EventBus subscriber, i18n rules, and platform integration for tray, autostart, single instance, power and window geometry.
tags: [desktop, slint, ui, event-stream, i18n, platform]
sources:
  - id: openwiki-source-e5a81e0b5d28c08eec080c86
    resource: repo://crates/limedl-native/src/autostart.rs
  - id: openwiki-source-2ba14b7f8d43883e76ffefa7
    resource: repo://crates/limedl-native/src/bridge/mod.rs
  - id: openwiki-source-7acf26e7ec8ef8633b599976
    resource: repo://crates/limedl-native/src/bridge/models.rs
  - id: openwiki-source-ec243741e58f29f41733a43b
    resource: repo://crates/limedl-native/src/context.rs
  - id: openwiki-source-464561a96b1e4d21d0fee313
    resource: repo://crates/limedl-native/src/crash.rs
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
  - id: openwiki-source-21d94ca69186f557c4be155a
    resource: repo://crates/limedl-native/ui/components/labs_dialog.slint
  - id: openwiki-source-c8ea6494d2edd0f743860be5
    resource: repo://crates/limedl-native/ui/components/settings/tab_about.slint
  - id: openwiki-source-271291a9954de641cdaeace8
    resource: repo://crates/limedl-native/ui/components/task_card.slint
  - id: openwiki-source-6fc9c568593230054ba6dc94
    resource: repo://crates/limedl-native/ui/components/task_table.slint
  - id: openwiki-source-8dffa5722cdb3e6e3fb0c023
    resource: repo://crates/limedl-native/ui/components/toast_stack.slint
  - id: openwiki-source-90185777dff572d79a3b452d
    resource: repo://crates/limedl-native/ui/theme.slint
generated: { by: "pi", at: "2026-10-04T07:31:34.204Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T07:31:34.204Z
---

# Native Desktop UI (Slint)

`crates/limedl-native` is the only shipped desktop client and the only UI over
`limedl-core`. It is a single Slint process: the engine is linked in directly, so
there is no IPC, webview or serialization boundary between UI and engine.

## The main / ui_boot split

`main()` is a thin wrapper: it awaits `run()` and, on error, hands the error to
`crash::report_startup_failure` (message box + crash log) before exiting non-zero.
Without that, a failure during startup made the window never appear with nothing
written anywhere — the process has no console (`windows_subsystem = "windows"`).

`run()` owns everything that touches the operating system, and
`ui_boot::build_ui` owns the pure UI assembly:

- `run()` parses the CLI contract `limedl-native [--hidden] [<url|magnet|path|limedl://…>]`,
  resolves the data directory, claims the single-instance slot, installs the
  panic hook, initializes logging with the **defaults**, calls `bootstrap()`, then
  re-applies logging with the **configured** settings, syncs autostart, optionally
  starts the Aria2 RPC server, decides the language, resolves the default download
  directory, then calls `build_ui`, installs the tray, restores window geometry,
  registers platform hooks, starts the background listeners, watches
  SIGTERM/SIGINT and finally runs the event loop.
- Logging comes up before the engine on purpose: a bootstrap, settings or
  migration failure is exactly the class of failure that most needs a log, and the
  second `init_logging` call only re-applies level/enabled/path.
- `build_ui(UiBootInputs)` receives everything that needs the OS or an async
  lookup as **already-resolved inputs** (settings, language, default directory,
  base directory, RPC shutdown handle, install kind). It is deliberately **not
  async and free of timers**: it builds the real `MainWindow`, applies
  translation and appearance, constructs `AppContext` and calls
  `handlers::register_all`. That is exactly the seam that lets the in-process UI
  tests drive the same window and callbacks headlessly.

Evidence: `repo://crates/limedl-native/src/main.rs#L54-L130`,
`repo://crates/limedl-native/src/ui_boot.rs#L8-L16`,
`repo://crates/limedl-native/src/ui_boot.rs#L62-L224`.

## Crash reporting

`crash.rs` is the only place that can leave evidence of a panic, because the
release profile builds with `panic = "abort"` (the hook still runs on the way to
`abort()`):

- `install_panic_hook(state_dir)` registers a hook that appends timestamp,
  thread, `file:line`, payload and `Backtrace::force_capture()` to
  `<state_dir>/logs/crash.log`, and echoes the same text to stderr.
- The log rotates to `crash.log.1` past `CRASH_LOG_MAX_BYTES` (1 MiB), and the
  hook is deliberately allocation-light: a panic hook that itself panics would
  abort without a report.
- `report_startup_failure` writes the same kind of report and then shows a
  localized `rfd` message box (the language comes from the OS locale, since
  settings may not have loaded).
- Any `catch_unwind` in the tree is therefore a development/test-only safety net,
  not release error recovery.

Evidence: `repo://crates/limedl-native/src/crash.rs#L1-L120`.

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
`repo://crates/limedl-native/src/toast.rs#L147-L175`.

Toasts themselves are a shared `Vec<ToastEntry>` behind a mutex. `push_toast`
appends an entry with `leaving: false`, syncs the Slint model and spawns an
auto-dismiss task for the given duration; both that task and the close-button
`dismiss_toast` route through `retire_toast`, which sets `leaving` and syncs
again, then drops the entry only after `TOAST_EXIT_MS` (240 ms). The extra state
is what lets the card play its exit animation: the row stays in the model — and
in the stack layout — until the animation has had time to run, so retiring one
toast does not reflow the others mid-slide.

`sync_toasts` reconciles the live model in place (`apply_toasts`) instead of
replacing it: a fresh `VecModel` would make the repeater destroy and recreate
every row, replaying the enter animation of untouched toasts whenever a sibling
was dismissed. Rows whose id survives are updated via `set_row_data`; only new
ids are inserted. The Slint card (`toast_stack.slint`) starts off-screen and
flips an `entered` flag from a one-shot `Timer`, so the property animation has
an initial value to animate from, then slides back out when `toast.leaving`
flips. Rust's `TOAST_EXIT_MS` is deliberately a little longer than the `.slint`
200 ms animation so a slow first frame is not cut off.

Evidence: `repo://crates/limedl-native/src/toast.rs#L12-L145`,
`repo://crates/limedl-native/ui/components/toast_stack.slint#L34-L62`.

## Download progress rendering

`summary_to_task_item` derives `TaskItem::is_indeterminate`: a `Queued` task with
zero downloaded bytes, or a `Downloading` task with zero downloaded bytes and an
unknown or zero total, is indeterminate; every other state — including a
`Downloading` task whose total is known — is determinate.

The two row components render the flag differently:

- **Determinate** bars (`task_card.slint`, `task_table.slint`) animate their
  width over 250 ms (`animate width { duration: 250ms; easing: ease-out; }`), and
  the table bar adds a soft white highlight at the leading edge while the task is
  actively downloading between 2% and 99%. The update-download bar in
  `tab_about.slint` and the CDN-speedtest bar in `labs_dialog.slint` reuse the
  same width animation.
- **Indeterminate** bars replace the fill with a 35%-wide highlight beam that
  sweeps the track on a 1500 ms cycle (`mod(animation-tick(), 1500ms) / 1500ms`)
  and paints it with a `@linear-gradient` between the accent and white, so a task
  that has not reported any bytes still shows that it is alive.

Evidence: `repo://crates/limedl-native/src/bridge/models.rs#L194-L203`,
`repo://crates/limedl-native/ui/components/task_card.slint#L250-L298`,
`repo://crates/limedl-native/ui/components/task_table.slint#L501-L547`,
`repo://crates/limedl-native/ui/components/settings/tab_about.slint#L143-L152`,
`repo://crates/limedl-native/ui/components/labs_dialog.slint#L270-L278`.

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

`InstanceClaim::claim(base_dir)` resolves to one of two strategies, and the
primary handle must stay alive for the whole process — dropping it releases the
claim. The handle is held, not read: `ClaimState::Primary` carries a
`PrimaryHandle` that on Windows wraps a raw mutex `HANDLE`, and the field is
deliberately exempted from `dead_code` because keeping the value alive (never
calling `CloseHandle`) is the entire point.

- **Windows**: a session-local named mutex (`Local\limedl-native-single-instance`).
  Ownership is read from `GetLastError() == ERROR_ALREADY_EXISTS` after
  `CreateMutexW`; secondary launches focus the existing window with
  `FindWindowW` + `ShowWindow(SW_RESTORE)` and forward the payload via
  `WM_COPYDATA`.
- **macOS / Linux**: an exclusive advisory lock on `<data-dir>/instance.lock`
  (`File::try_lock`, released by the kernel on process death, so a crash cannot
  leave a stale owner) plus a Unix domain socket that the primary publishes and
  secondary launches connect to. The socket path falls back to a short hashed
  name under the temp directory when the data directory is deeper than the
  `sun_path` budget (100 bytes).

`notify_primary` now **returns whether the request reached a primary**, and a
short retry window absorbs the "double-clicked while the first launch is still
starting" race. A `false` return makes `main` report the failure instead of
exiting quietly. This replaced a fixed loopback TCP port (45997): any unrelated
program that happened to own that port looked like a running limedl, the launch
was classified secondary, the notification connect failed silently, and the user
got neither a window nor an error.

Evidence: `repo://crates/limedl-native/src/single_instance.rs#L1-L45`,
`repo://crates/limedl-native/src/single_instance.rs#L88-L165`,
`repo://crates/limedl-native/src/single_instance.rs#L256-L330`.

### Autostart

`autostart.rs` is a thin dispatcher over per-OS modules: only the `#[cfg]`
selected backend (`windows_impl`, `linux_impl`, `macos_impl`, or a no-op
fallback) is compiled, and the shared helpers (`current_exe_string`,
`registration_is_current`) are used by all of them. Every OS registration
appends `--hidden`, so login starts go to the tray. Windows portable/NSIS uses
the HKCU `Run` key; MSIX uses the `windows.startupTask` manifest extension
because registry `Run` is virtualized in MSIX; Linux writes an XDG `.desktop`;
macOS writes a LaunchAgent. The `expected_command` helper — which quotes a
spaced executable path and appends `--hidden` — is shared by Windows and Linux
only (`#[cfg(any(windows, target_os = "linux"))]`); macOS builds its plist
`ProgramArguments` array from `current_exe_string` directly. On Unix,
`registered_for_current_exe` and `sync_from_settings` re-register when the
executable moved (portable extraction to a new folder, debug vs release) or lost
the hidden flag.

Evidence: `repo://crates/limedl-native/src/autostart.rs#L11-L42`,
`repo://crates/limedl-native/src/autostart.rs#L225-L266`,
`repo://crates/limedl-native/src/autostart.rs#L380-L394`.

### Tray and power

The tray menu (show, pause all, resume all, speed-limit check, game mode, open
download dir, quit) is built with `muda` and updated on the UI thread by
`update_tray_menu_and_tooltip`. On Linux the tray is a D-Bus StatusNotifierItem
(ksni). **Building the icon is best-effort**: a missing StatusNotifier host
(GNOME without the AppIndicator extension, a bare window manager) makes `build()`
fail, and `main` logs `tray_init_failure_message` and keeps going instead of
aborting the launch before the window is shown — the window is the primary UI,
the tray is an extra.

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

Related pages: [Workspace and System Architecture](../architecture/overview.md),
[Settings and Configuration](../systems/settings-and-configuration.md),
[Self-Update and Distribution Channels](self-update-and-distribution.md),
[Slint UI Testing](../testing/slint-ui-testing.md).
