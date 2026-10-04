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
- The new-task dialog modal had a fixed height (600 px when
  `preview_state == "ready"`), while an expanded torrent file list is taller.
  Fixed by putting the dialog body in a `ScrollView` so the footer
  ("Start Download") always stays in the modal.
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
