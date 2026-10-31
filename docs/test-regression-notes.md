# Test regression notes

Institutional memory for the test suite: the regressions it has already caught,
and the test areas whose intent is easy to break. Test layout and conventions are
in [`openwiki/testing/testing-strategy.md`](../openwiki/testing/testing-strategy.md)
and [`openwiki/testing/slint-ui-testing.md`](../openwiki/testing/slint-ui-testing.md).

## Real bugs the UI tests caught

When adding UI test cases, aim at these directions:

- The `FocusScope` holding `handle_key_escape` had a bare
  `if … { reject }` for "a dialog is open, swallow shortcuts", and the value was
  dropped — so `Ctrl+A` / `Space` / `Delete` acted on the list **behind** the
  modal (`Delete` deleted the selection). It is now part of an `else if` chain.
- Clicking a table column header called `set_sort_field`, which did not reset the
  direction — switching columns could land on descending, contradicting the
  documented "new column sorts ascending". It now uses `apply_sort(field, true)`.
- At `min-width` (1000 px) the toolbar overflowed the right edge and `ta_new_task`
  was clipped by ~18 px. Fixed by raising `min-width` to 1100 px; `layout.rs` pins
  it under English labels too.
- The toolbar's BT runtime pills are only turned on by the runtime poller
  (`bt_status_visible`), which the in-process fixture never set — so the 1100 px
  `min-width` was validated against a toolbar ~107 px narrower than the one users
  get. With BT connected, a small or HiDPI maximized window (work area < ~1160
  logical px) clipped the right-hand actions, `New Task` included, with no
  horizontal scroll to reach them. Fixed by collapsing the pills below
  `bt_pills_min_width` (1280 px) in `appwindow.slint`; `layout.rs` now drives
  `bt_status_visible` and pins the breakpoint in both locales.
- On low-resolution or scaled displays (e.g. 720p/768p at 125% DPI scale where a
  maximized window has work area ~1024x540), Windows forces the window to the
  monitor work area, bypassing `min-width: 1100px` and `min-height: 660px`. The
  toolbar secondary buttons ("Pause All", "Resume All", "Clear Completed")
  pushed `New Task` off-screen, and the left sidebar cut off the "Settings" and
  "Labs" buttons at the bottom. Fixed by collapsing secondary toolbar button text
  below 1200 px (or below 1360 px when BT status pills are visible) while retaining
  tooltips, and giving the left sidebar responsive vertical spacing plus a
  `ScrollView` so Settings and Labs always remain reachable. `layout.rs` pins the
  1024x540 / 1024x576 work area in both zh-CN and en-US.
- The new-task dialog modal had a fixed height (600 px when
  `preview_state == "ready"` or in batch mode), while low-resolution or scaled
  displays (work area height ~540 px) forced the modal outside the window,
  clipping the footer ("Start Download" / "Add Batch Tasks") and header. Fixed by
  capping the modal height to `min(parent.height - 32px, ...)`, adding
  `min-height: 0px` to its `ScrollView`, and applying compact spacing when height
  is below 600 px. `layout.rs::dialogs_fit_on_small_work_areas` pins the dialog
  in both locales across single, batch, and torrent preview states.
- The task inspector's Tab 3 (Piece Map) hardcoded its image height to 380 px
  without a `ScrollView`, overflowing the 440 px modal panel on small work areas.
  Fixed by wrapping Tab 3 in a `ScrollView` and applying adaptive image height
  below 600 px.
- Restoring saved window geometry from a high-resolution display (e.g. 4K) on a
  smaller monitor could restore oversized dimensions exceeding the screen. Fixed
  by clamping restored geometry to the target monitor's work area in
  `platform_win::clamp_geometry_to_work_area`.
- The table view's inline action column was clipped out of the visible area at
  default and minimum widths (the table has no horizontal scroll). Fixed by
  pinning the action column to the right edge (`root.width - 138px`), letting data
  columns clip behind it, and shrinking the file column `min-width` from 260 px to
  140 px so the default column set shows a few columns at minimum width.
- The setup wizard's step 2 had three `LanguageCard`s in one row at `width: 50%`
  each (150 % plus gaps), so the `en-US` card overflowed the modal at any width and
  the English option was unclickable. Fixed by giving the three cards
  `horizontal-stretch: 1`.

## Logging rotation / retention tests

`logging.rs`'s `#[cfg(test)] mod tests` only tests the filesystem layer and never
touches the process-global `LOGGER_CONTROL` (`init_logging` can only succeed
once), so it can run safely in the same process as other lib tests:

- `rotate_startup_logs_shifts_without_losing_content` — right-shift rotation must
  rename from the **highest number down**, asserting the content mapping
  file-by-file; ascending order would first overwrite `.1` into `.2` and silently
  drop the oldest log.
- `perform_startup_rotation_skips_while_lock_is_held` — uses `File::try_lock` to
  simulate a second instance holding the lock and asserts rotation is skipped,
  then runs once released (the safety net for desktop + NAS on one machine).
- `cleanup_by_count_keeps_exactly_the_limit` (boundary `num > count`, `Some(0)`
  clears) and `cleanup_by_age_removes_only_old_files` (uses `File::set_modified`
  to fake a 10-day-old mtime; the current log participates too).
- `dynamic_file_writer_honors_enabled_and_survives_open_failure` — disabled writes
  are dropped, enabled writes create the parent directory, and an open failure
  (path is a directory) degrades to a sink instead of panicking (failures go to
  stderr only — that is the logging module's contract).

## BT backend test notes

`bt_backend/tests/` starts a real irontide session via `make_backend()` (DHT/LSD/
UPnP/PEX/uTP off, port 0). Two implementation-coupled points:

- `handle_alert()` and `emit_progress_for_all_torrents()` were extracted as
  `pub(super)` functions for testability; alert branches that only log and publish
  no event must also have an assertion (`drain_events()` is empty), so a future
  change cannot quietly turn a log into an event.
- An offline session cannot manufacture real peers, so anti-leech's
  "ban leecher / cap upload slots" and upload policy's "pause by limit" branches
  are **not** covered (`get_peer_info` is empty and the loop returns early).
  Changing those branches cannot rely on the existing tests to catch a regression.

## Crash-consistency / startup hardening changes (2026-10)

The durable-progress rework (see
[`troubleshooting.md`](troubleshooting.md#durable-vs-received-progress-crash-consistency))
landed a set of changes whose failure modes are only visible from the outside, so
the notes below are worth keeping:

- **A re-lock inside an already-held core guard is a self-deadlock**, and it
  presents as an unrelated hang. The first version of the finalize durability check
  called `ensure_fully_durable(&managed)`, which called `managed.lock_core()` from
  *inside* the block that already held the guard (`parking_lot::Mutex` is not
  reentrant). `aria2_add_uri_multiple_uris_use_mirror_fallback` and four other
  `aria2_rpc::e2e_tests` handled it as a 60–180 s timeout, because every tokio
  worker ended up parked on the futex: the test's own `rpc_call` could never be
  polled. The fix is `ensure_core_fully_durable(&core)`, which takes the guard it
  was given. **Do not call `lock_core()` from a function that is passed a guard.**
- **Two `database::tests` fixtures used `end` as an exclusive bound** (`start: 0,
  end: 500` with `downloaded: 500, completed: true`, i.e. a 501-byte chunk). They
  passed only because `completed` used to be stored verbatim; once the row derives
  `completed` from the durable count the fixtures had to become self-consistent.
  `ChunkManifest::end` is the **last byte**, not one past it.
- **`record_progress_on_managed` must not set `chunk.dirty`.** The dirty flag is
  what schedules a row write, and a row may only record durable bytes. The old
  assertion in `manager_tests::progress::record_progress_normal_update` pinned the
  opposite behaviour and was inverted on purpose.
- **The retry loop needs a budget, not just per-request retries**: the single-stream
  and chunked loops both re-request from the new offset when a server ends the body
  early, and each request had a fresh retry counter, so a server that always ends
  the response immediately spun forever. `RequestBudget` (`http_executor`) bounds
  the requests per unit of work and is unit-tested directly
  (`request_budget_*` in `http_executor::tests`).

## AIMD scheduler anti-deadlock & cascade notes (2026-10)

The adaptive HTTP thread scheduling rework solved the 1-thread deadlock and compounding decrease death spiral:

- **Proactive probing breaks 1-thread deadlock**: Single connections in a stable bandwidth environment do not spontaneously accelerate, meaning passive AI (`throughput >= last * (1.0 + increase)`) never fired. The scheduler proactively probes upwards after `probe_stable_cycles` consecutive stable cycles. If the probe yields no throughput improvement, it rolls back to the pre-probe thread count and enters cooldown (`aimd_probe_rollback_when_no_gain`, `aimd_probe_success_retains_increase`).
- **Settling period & baseline reset eliminate compounding decrease cascades**: Decreasing threads reduces immediate throughput by design. If `last_throughput` retained the pre-decrease high value, the next evaluation cycle misinterpreted the drop as fresh network congestion and halved threads again repeatedly down to 1. `apply_decrease` now clears `last_throughput` and sets `settling_until` to establish a clean baseline after connection allocation settles (`aimd_no_cascade_after_decrease`).
- **Worker pool sizing in chunked executor**: `http_executor/chunked.rs` previously capped workers at `target.min((chunk_count / 2).max(1))`. With 2 or 3 chunks, worker concurrency was artificially clamped to 1, starving the scheduler's target. The worker limit is now `target.min(chunk_count)`.

## Settings form completeness (`src/bridge/forms/fields.rs`, 2026-10)

Adding a setting used to mean editing up to nine places by hand — the
`AppSettings` struct and its serde defaults, `normalize_settings`, `to_form.rs`,
`from_form.rs`, the `SettingsFormData` declaration in `ui/types.slint`, the
settings tab, `aria2_global_options` and the validation labels — with nothing to
catch a forgotten one. The table in `fields.rs` is now the single declaration of
every `SettingsFormData` field, and two tests turn it into a guarantee:

- `table_covers_every_settings_form_field` parses
  `export struct SettingsFormData` out of `ui/types.slint` and fails when the
  table and the declaration disagree in either direction.
- `every_settings_leaf_round_trips_through_the_form` builds a form whose
  round-trip fields all hold a sentinel, pushes it through
  `update_app_settings_from_form` and back through `app_settings_to_form`, and
  fails on (a) a field that does not come back with exactly the sentinel value,
  or (b) an `AppSettings` JSON leaf that no form field moved. The second half is
  why `NOT_ON_THE_FORM` exists: every leaf without a dialog field must be listed
  there with a reason.

Part (a) is exact equality, not "the field moved". The sentinels are chosen to
survive the clamps and the rounding (`"5"` for `io_game_mode_max_parallel` is the
one case where a wider value would be silently clamped to 8), so the test also
fails on a clamp that swallows a value or a `{:.2}`-style reformat that drifts —
which a difference-based assertion would not notice. Only the forced-on `file`
column is excluded, and the table marks it `display` for that reason.

## Translation catalogs (`src/i18n/`, 2026-10)

The Slint strings (`@tr`) and the Rust strings (`i18n::format_*`) used to live in
two places: `lang/*/LC_MESSAGES/limedl-native.po` for the UI and a hand-written
three-language `match lang` in every `i18n/*.rs` function. A message needed in
both places existed twice and had already diverged — the About tab's
"v{} downloaded and verified. Restart to apply." said `重启后生效` in the catalog
and `重启应用后生效` in the toast. Both sides now read the `.po` catalogs;
`src/i18n/catalog.rs` is generated from them by `cargo xtask i18n generate`, and
`i18n::tr`/`tr_fmt` look up the same msgids the Slint compiler bundles.

The tests that keep it honest are listed in
[`i18n.md`](i18n.md#what-the-tests-enforce). The scan in
`every_rust_message_exists_in_the_catalogs` is deliberately textual and skips
comments and string literals: a doc comment that mentions `tr("…")` as an example
(and the `@tr("` needle inside the Slint scan) would otherwise be collected as a
key.
