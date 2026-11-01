# Test regression notes

Institutional memory for the test suite: the regressions it has already caught,
and the test areas whose intent is easy to break. Test layout and conventions are
in [`.wiki/pages/testing/testing-strategy.md`](../.wiki/pages/testing/testing-strategy.md)
and [`.wiki/pages/testing/slint-ui-testing.md`](../.wiki/pages/testing/slint-ui-testing.md).

## UI regressions

The UI regressions the in-process suite caught are documented with citations in
[`.wiki/pages/testing/slint-ui-testing.md`](../.wiki/pages/testing/slint-ui-testing.md#regressions-these-tests-pin):
the Escape chain, dialog-swallowed shortcuts, ascending column sorts, the
1100px/low-resolution toolbar and modal geometry, the piece-map tab, the wizard
language cards, window-geometry clamping and the pinned table action column.
When adding a UI test case, start from those directions.

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

`crates/limedl-bt/src/tests/` starts a real librqbit session via `make_backend()`
(DHT/UPnP/IPv6/uTP off, listen port 0); `make_network_backend()` (DHT+uTP on)
exists only for the `#[ignore]`d peer-discovery tests. Two implementation-coupled
points:

- `handle_event()` and `emit_progress_for_all_torrents()` (`alerts.rs`) are kept
  `pub` for testability; assertions go through `event_bus.subscribe()` +
  `rx.try_recv()`. Branches that only log and publish no event must assert that
  the receiver stays empty, so a future change cannot quietly turn a log into an
  event.
- An offline session cannot manufacture real wire peers, so anti-leech is covered
  in two pieces instead: the `peer_is_leecher` predicate over faked
  `LeecherPeerInfo` structs, and the bookkeeping sweeps (expired-ban cleanup,
  slot caps untouched without peers, loop respawn replaces the previous task).
  Upload policy covers the throttle path end to end
  (`upload_policy_unpauses_when_limits_are_cleared` drives the real
  `session.pause`). Changing those branches beyond these shapes cannot rely on
  the existing tests to catch a regression.

## Crash-consistency / startup hardening changes (2026-10)

The durable-progress rework (see
[`.wiki/pages/systems/persistence-and-recovery.md`](../.wiki/pages/systems/persistence-and-recovery.md))
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
