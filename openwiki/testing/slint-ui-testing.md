---
type: testing
title: Slint UI Testing
description: The two-layer approach to testing limedl's desktop UI — the in-process L1 fixture that drives the real MainWindow (fixture selection, ids as contracts, recording backend, accessibility and geometry assertions) and the L2 MCP server for interactive inspection.
tags: [testing, slint, ui, fixtures, mcp]
sources:
  - id: openwiki-source-8037e2358a2c4f9b2c722a11
    resource: repo://AGENTS.md
  - id: openwiki-source-a57d5901fe2a65b6bf66c789
    resource: repo://crates/limedl-native/build.rs
  - id: openwiki-source-66f7aa200e72bcf46a9b1342
    resource: repo://crates/limedl-native/src/ui_tests/async_contracts/dialogs.rs
  - id: openwiki-source-609f1f96cd2144c685b84607
    resource: repo://crates/limedl-native/src/ui_tests/async_contracts/mod.rs
  - id: openwiki-source-19236607c3fd169ba329ef60
    resource: repo://crates/limedl-native/src/ui_tests/layout.rs
  - id: openwiki-source-d4153d1b0168cae501e8c53a
    resource: repo://crates/limedl-native/src/ui_tests/mod.rs
  - id: openwiki-source-cb1d26941bd1e33067df4677
    resource: repo://crates/limedl-native/src/ui_tests/recording.rs
  - id: openwiki-source-4c401762f2250ca52aba8325
    resource: repo://crates/limedl-native/src/ui_tests/toast.rs
  - id: openwiki-source-031df146f1a4c52e00c3f83b
    resource: repo://crates/limedl-native/src/ui_tests/updater.rs
  - id: openwiki-source-f1911c421777843811200186
    resource: repo://docs/manual-smoke-testing.md
generated: { by: "pi", at: "2026-10-04T11:42:48.469Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T11:42:48.469Z
---

# Slint UI Testing

The desktop UI is tested at two layers, chosen by what regression you want to
catch: **L1** in-process tests that run in the normal `cargo nextest` gate, and
**L2** the MCP server for interactive inspection of a running app.

## L1: in-process tests

`crates/limedl-native/src/ui_tests/` builds the **real** `MainWindow` through
`crate::ui_boot::build_ui` and drives it with Slint's testing backend: element
queries by `.slint` id, simulated clicks, key presses, no window and no display.
`bridge/` unit tests cover pure mapping and state machines; these tests cover the
wiring between them — a callback bound to the wrong handler, a dialog reading a
property nobody sets, or the blast radius of a destructive button.

Evidence: `repo://crates/limedl-native/src/ui_tests/mod.rs#L1-L30`.

### The platform is per thread; the event loop is per process

Two testing-backend flavors exist and are incompatible by construction:

- `init_no_event_loop()` installs a mock platform with mock time and **no**
  event-loop proxy. It is installed once **per thread** (`PLATFORM_READY`), so
  many tests share a process; queued `invoke_from_event_loop` callbacks are
  dropped.
- `init_integration_test_with_mock_time()` adds the event-loop proxy that
  `pump_until` drains. That proxy lives in a **global** `OnceCell`, so exactly
  one test per process may install it; `EVENT_LOOP_PLATFORM_TAKEN` panics on a
  second attempt.

Evidence: `repo://crates/limedl-native/src/ui_tests/mod.rs#L88-L122`.

### Fixture selection

| Fixture | Use |
| --- | --- |
| `with_ui` | default settings, synchronous body |
| `with_settings` | a window seeded from specific `AppSettings` |
| `with_language(Language::EnUs, …)` | the same window with the English catalog (wider labels) |
| `with_ui_async` + `new_window` | contracts that settle on spawned work/timers |

`with_ui`/`with_settings`/`with_language` enter a current-thread tokio runtime
without ever driving it: `tokio::spawn` is legal (handlers fire and forget) but
the task is not polled, so assertions must be synchronous — timers do not fire
and `invoke_from_event_loop` delivers nothing. `with_language` applies the
catalog through `slint::select_bundled_translation`, so `@tr` strings really
switch; that selection is process-global, and a test that flips it must restore
it because `cargo test` runs siblings in one process (nextest does not).

Evidence: `repo://crates/limedl-native/src/ui_tests/mod.rs#L125-L180`.

`with_ui_async` is the only fixture that drives the runtime and Slint's queue.
Because the event-loop proxy is process-global, **every** pump-based scenario
lives in the single `event_loop_contracts` test in `async_contracts/mod.rs`; each
scenario is a `pub(super) async fn` grouped by surface (`bus`, `selection`,
`hotkeys`, `rows`, `new_task`, `toolbar`, `dialogs`) and called from that one
`#[test]`. Each scenario builds its own window with `new_window()` to stay
independent, and they advance a shared mocked clock in 20 ms steps.

Evidence: `repo://crates/limedl-native/src/ui_tests/async_contracts/mod.rs#L1-L50`,
`repo://crates/limedl-native/src/ui_tests/mod.rs#L181-L206`.

### TestUi API

`TestUi` wraps the window, the `AppContext` and the recording core. The core
interaction API is `find`, `find_all`, `has`, `click`, `click_nth`,
`right_click_nth`, `shift_click_nth`, `type_text`, `press_keys`, `search`, `seed`
and `visible_rows`.

`shift_click_nth` is the real range gesture: a pointer event carries no modifiers
of its own, so the core fills `PointerEvent.modifiers` from the modifier keys the
window currently sees held down. The helper presses Shift, clicks, then releases
Shift.

Evidence: `repo://crates/limedl-native/src/ui_tests/mod.rs#L294-L490`.

### Toast assertions

The toast helpers read the Rust-side queue rather than the rendered rows:
`toasts()` returns `(kind, message)` for the pending entries,
`assert_toast(kind, text)` requires exactly one, `toast_ids()` names them for
`dismiss_toast(id)`, which invokes the same callback the close button calls.
Dismissal is a two-stage retire: the entry is marked `leaving` so it stays in
the model for the exit animation, and is dropped only after the animation
window. `toasts()` and `toast_ids()` therefore filter `leaving` out — otherwise
a dismissed toast would still count as pending. `ui_tests/toast.rs` asserts both
halves: the entry is retired from the pending set and it remains in the queue
during the animation. `sync_toasts` reconciles the model in place for the same
reason, so only genuinely new rows play the enter animation. The reconcile test
also queries `ToastStack::toast_close` to force the stack to lay out, so a
binding loop between the card's height and its layout slot fails the test.

Evidence: `repo://crates/limedl-native/src/ui_tests/mod.rs#L584-L632`,
`repo://crates/limedl-native/src/ui_tests/toast.rs#L22-L135`.

### Window size and geometry

`window_logical_size()` reads the **root element**, not
`Window::size()/scale_factor()`. The testing backend stores whatever size it was
given as physical while reporting the host's scale factor, so
`physical / scale_factor()` is wrong on HiDPI machines. `set_window_size` takes
logical pixels.

Geometry assertions are `assert_inside_window`, `assert_min_size`,
`assert_no_overlap`, and `bounds`; `assert_inside_window` dumps `id_tree()` on
failure. The testing window defaults to 800x600, so a full-page assertion sets a
larger size first.

Evidence: `repo://crates/limedl-native/src/ui_tests/mod.rs#L434-L585`.

**Clipped means absent**: element queries skip subtrees that are not visible, and
dialogs are `visible: is_open || opacity > 0.01`, so `has()` doubles as an "is
this dialog actually on screen" check. A control below the fold of a scrolling
tab must be scrolled into view first — `scroll_danger_zone_into_view` dispatches
`WindowEvent::PointerScrolled` and pumps, because the Flickable animates.

Evidence: `repo://crates/limedl-native/src/ui_tests/mod.rs#L20-L28`,
`repo://crates/limedl-native/src/ui_tests/async_contracts/dialogs.rs#L425-L445`.

### The recording backend: assert the blast radius

`RecordingBackend` implements `DownloadBackend` and records every call the UI
makes instead of downloading. It is registered for `TaskKind::Http`, and seeded
tasks must use valid UUIDs (`http_task(n, …)`) because `TaskAction` parses wire
ids and silently skips unparseable ones.

- Mutating calls return `NotFound` on purpose, so the *failure* path is reachable
  (the row is dropped optimistically, the backend rejects, the list must
  resynchronize) and no `Updated` event is published behind the test's back.
- `start` is the exception: it returns `Ok` with a synthetic id because the
  success path clears the URL and closes the dialog.
- `starts()` returns the `StartCall` payloads (url, destination dir, file name,
  checksum, expected checksum, selected file indices) — the only way to assert
  that what the form collected really became a `StartDownloadRequest`.
- `settings_pushes()` counts `UpdateSettings` calls, shared by the settings, labs
  and wizard save paths.

Evidence: `repo://crates/limedl-native/src/ui_tests/recording.rs#L1-L130`.

### Accessibility assertions

`Text` announces its `text` as its accessible label, and the shared
`PrimaryButton`/`SecondaryButton`/`DangerButton` declare
`accessible-role`/label/enabled. Tests assert what the user actually sees with
`ElementHandle::accessible_label()` and `accessible_enabled()` — for example the
updater's phase-to-copy matrix and the enabled state of the check button.

Evidence: `repo://crates/limedl-native/src/ui_tests/updater.rs#L25-L80`.

### Pumping

`pump_once` advances the mocked clock by 20 ms, yields the tokio runtime so
spawned tasks run, then queues a `quit_event_loop` future and runs Slint's event
loop to drain queued callbacks and due timers. `pump_until` repeats this against
a 5 s wall-clock deadline and waits 1 ms of real time between rounds with
`tokio::time::sleep`, then fails loudly with the recorded engine calls if the
predicate never holds, so a contract that does not settle fails instead of
hanging CI. The real-time yield matters because the mocked clock makes a round
almost instant while the engine call being waited for can be blocked on
`spawn_blocking` work (`SettingsService` persists through `tokio::fs`); a counted
loop could otherwise exhaust every round before such a task was ever polled — the
settings-save contract did exactly that on a loaded coverage runner. The wait is
an async timer on the real-time current-thread runtime rather than a blocking
`std::thread::sleep`, so the runtime also polls those spawned completions while
the wait is in progress.

Evidence: `repo://crates/limedl-native/src/ui_tests/mod.rs#L641-L693`.

### Element ids are test contracts

Element lookup needs the debug metadata Slint's compiler embeds on request.
`build.rs` turns it on automatically when `PROFILE=debug` (so `cargo run` and
`cargo test` need no setup) and leaves an explicit `SLINT_EMIT_DEBUG_INFO=1`
alone, which is what lets a `--release` binary be driven by the MCP server.
Without the metadata every id lookup silently returns nothing and prints a
warning listing the ids that exist. Adding a test usually means adding an `id:`
to the `.slint` element, and renaming one is a breaking change for the test that
clicks it.

Evidence: `repo://crates/limedl-native/build.rs#L56-L74`.

### Known limitations

The testing crate's `internal` feature cannot compile from crates.io (its test
font path only exists in the Slint workspace), so there are no in-process
screenshots; pixels stay on the L2/MCP path. The table is horizontally clipped
rather than scrollable, so table assertions are written at the declared minimum
window (1100x660) and the action column stays pinned to the right edge.

Evidence: `repo://crates/limedl-native/src/ui_tests/mod.rs#L30-L48`,
`repo://crates/limedl-native/src/ui_tests/layout.rs#L40-L50`.

## L2: the MCP server

The MCP server exposes element tree, `take_screenshot`, click, drag, type and key
events over MCP Streamable HTTP at `http://127.0.0.1:8080/mcp` (JSON-RPC). It is
for agents and manual inspection, not CI. Operational prerequisites:

- Set `SLINT_EMIT_DEBUG_INFO=1` (for release builds, at **build** time) and
  `SLINT_MCP_PORT=8080`, and pass `--features slint/mcp` on the command line only
  — never add it to a `[features]` table, because it pulls in `prost`/`protox`
  and would land in release builds.
- Quit any running limedl first: the single-instance guard makes a second process
  notify the primary and exit immediately, so the port never opens.
- Do **not** pass `--hidden`: the server starts from the "first window shown"
  hook, which never fires on a hidden launch.
- Isolate with `LIMEDL_DATA_DIR` and use `SLINT_BACKEND=headless` where there is
  no display. The server binds `127.0.0.1`, has no authentication and validates
  `Origin`, so it is a local development tool.

Evidence: `repo://docs/manual-smoke-testing.md#L1-L50`,
`repo://AGENTS.md#L35`.

Related pages: [Native Desktop UI (Slint)](../desktop/native-ui-architecture.md),
[Testing Strategy](testing-strategy.md),
[Build, Tooling, CI and Release Operations](../operations/build-release-and-ci.md).
