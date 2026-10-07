---
type: workflow
title: Scheduler, AIMD and Concurrency Control
description: limedl's background scheduler loop and adaptive thread allocation — per-task thread-mode resolution, the AIMD throughput tuner, traditional vs automatic rebalancing, per-host connection caps, and the slot guards that bound HTTP and BT concurrency.
tags: [scheduler, aimd, concurrency, threads, rebalancing]
sources:
  - id: openwiki-source-0087afceb8965d051cbaa6fa
    resource: repo://crates/limedl-core/src/aimd/mod.rs
  - id: openwiki-source-6e8cafe6d766137aab11d68d
    resource: repo://crates/limedl-core/src/download/shared.rs
  - id: openwiki-source-351431881f59a7b65e1583b0
    resource: repo://crates/limedl-core/src/scheduler/mod.rs
  - id: openwiki-source-76cf0b396abab954f820977a
    resource: repo://crates/limedl-core/src/services/concurrency.rs
  - id: openwiki-source-a6c9307e4c12f6dae4a8f919
    resource: repo://crates/limedl-core/src/slot_guard.rs
  - id: openwiki-source-7724302ca1a6f04e7163013e
    resource: repo://crates/limedl-core/src/speed_tracker.rs
  - id: openwiki-source-098d28438aacd15b419786dc
    resource: repo://crates/limedl-core/src/task_lifecycle/mod.rs
generated: { by: "pi", at: "2026-10-07T03:53:23.435Z" }
verified:
  - by: openwiki/0.7.1
    at: 2026-10-07T03:53:23.435Z
---

# Scheduler, AIMD and Concurrency Control

`Scheduler` is a zero-sized actor that `DownloadManager` holds as `Arc<Scheduler>`.
Its background loop decides how many connections each active download gets, and
the AIMD tuner adapts those targets to measured throughput.

## The scheduler loop

`start_scheduler_loop` spawns a task that selects on a `SCHEDULER_TICK` of **2 s**,
the `rebalance_notify` wakeup, or the shutdown token. Each iteration:

1. applies the speed-limit schedule for the current local hour;
2. calls `update_adaptive_targets`;
3. calls `rebalance_allocations`.

Consecutive `rebalance_allocations` failures are counted and escalated to an
error log after three.

Evidence: `repo://crates/limedl-core/src/scheduler/mod.rs#L34-L85`.

## Thread-mode resolution

`resolve_thread_settings(settings, request, supports_parallel)` decides a task's
mode before the first run:

- no range support → `Fixed` with 1 thread;
- **Traditional** mode → `Fixed` at the requested (or default) count clamped to
  1..=`MAX_TRADITIONAL_THREADS` (32);
- **Automatic + Adaptive** → the profile's `initial_desired_threads` as the
  desired target, with `None` requested;
- **Automatic + Fixed** → `Fixed` at the requested count clamped to
  `max_threads_per_task`.

`supports_parallelism` requires range support and a total size of at least
`chunk_size * 2`.

Evidence: `repo://crates/limedl-core/src/download/shared.rs#L20-L68`.

## AIMD

`AimdState` tracks the last sample bytes/time, last throughput, cooldown,
consecutive good/bad samples, a `recent_penalty` flag, cumulative/peak
throughput, oscillation state (`last_direction`, `oscillation_count`,
`hysteresis_lock_until`), and the probe/settling state added by the
proactive-probing fix (`settling_until`, `stable_cycles`, `is_probing_up`,
`probe_pre_throughput`, `probe_pre_threads`). `sample_throughput` computes
bytes-per-second since the last sample; `record_sample` ignores
non-positive/non-finite values.

`aimd/mod.rs` exposes three pure profile helpers:

| Profile | `initial_desired_threads` (fraction of cap) | `reduce_threads` factor | `cooldown_for_profile` |
| --- | --- | --- | --- |
| Conservative | 0.5 | 0.7 | 4 s |
| Balanced | 0.75 | 0.5 | 3 s |
| Aggressive | 1.0 | 0.5 | 2 s |

Decreases never go below `min_threads`. The scheduler adds its own thresholds per
profile (`AdaptiveThresholds::for_profile`): `degrade` / `increase` fractions,
`consecutive_good_samples` needed, and `probe_stable_cycles` before a proactive
probe (Conservative 3, Balanced/Aggressive 2).

Evidence: `repo://crates/limedl-core/src/aimd/mod.rs#L6-L95`,
`repo://crates/limedl-core/src/scheduler/mod.rs#L282-L300`.

### One adaptive decision

`update_adaptive_targets` only runs in `SchedulerMode::Automatic`. It is
deliberately **not gated on `settings.proxy.mode`**: the tuner used to bail out
whenever a proxy was configured (a leftover from the removed network-learning
feature, which also silently disabled overclock mode). Proxied transfers adapt
like any other. Overclock mode pins all adaptive downloading tasks at the cap;
otherwise each task goes through `update_one_adaptive`.

`update_one_adaptive` takes the **`core` lock first and the `aimd` lock second** —
the same order as every other scheduler path, and it must not be reversed. It
only acts on adaptive tasks that are `Downloading` with range support, then:

- returns early during a penalty cooldown (clearing `recent_penalty`), the
  hysteresis lock, or the post-change settling window, while still recording
  throughput;
- if a proactive probe is in flight, waits for the allocation to catch up and
  then either keeps the raised target or rolls it back (see below);
- decreases only when degradation is confirmed (a severe drop, two consecutive
  bad samples, or `recent_penalty`; Conservative treats the penalty flag as
  sufficient);
- otherwise attempts an increase: `maybe_increase` requires the allocation to
  have caught up with the target, then either improved throughput with enough
  consecutive good samples (step +1 Conservative, `current/4` Balanced,
  `current/3` Aggressive) or a stable-transfer proactive probe.

Every change to `desired_thread_count` calls `sync_snapshot_with_manifest` before
returning, so the exposed "target thread count" never lags the manifest.

Evidence: `repo://crates/limedl-core/src/scheduler/mod.rs#L97-L120`,
`repo://crates/limedl-core/src/scheduler/mod.rs#L324-L490`.

### Proactive probing and the cascading-decrease fix

The tuner used to be able to deadlock at a low thread count: a single small
throughput wobble could trigger a decrease, and once the target bottomed out
there was no path back up. Two changes break that:

- **Decrease confirmation.** A decrease is applied only once degradation is
  confirmed — a severe drop (below 50 % of the last sample), two consecutive bad
  samples, or `recent_penalty` (Conservative additionally treats the penalty flag
  as sufficient). A single noisy sample no longer cascades into repeated
  decreases.
- **Proactive probing.** When throughput is steady for `probe_stable_cycles`,
  `maybe_increase` raises the target *without* waiting for a throughput
  improvement and records `is_probing_up`, `probe_pre_throughput` and
  `probe_pre_threads`. The next decision (after settling, once allocation caught
  up) keeps the higher concurrency if throughput improved by at least the
  profile's `increase` fraction; otherwise it rolls back to `probe_pre_threads`,
  applies a 2× cooldown and a settling window, and rebases the throughput
  baseline. This turns "no change" into a measured experiment instead of a
  permanent stall.

`settling_until` suspends decisions right after any thread change (recording
samples), and `hysteresis_lock_until` suspends them for four cooldowns after
oscillation (≥3 direction flips), where the target is clamped to 70 % of the
current value. All three gates record throughput while they skip decisions.

Evidence: `repo://crates/limedl-core/src/scheduler/mod.rs#L324-L650`.

## Rebalancing

`rebalance_allocations` collects the `Arc<ManagedDownload>` entries under the
`downloads` read lock, drops the lock, then dispatches by mode and persists the
rebalanced manifests in a batch.

Evidence: `repo://crates/limedl-core/src/scheduler/mod.rs#L122-L141`.

### Traditional

`rebalance_traditional` sorts by priority descending then creation time ascending
(using `sort_by_cached_key` so the comparator does not take per-download locks
twice per comparison). It walks the list keeping at most
`max_parallel_tasks` running; each running task gets its effective allocation
capped by the per-host budget, and everything else is queued (allocation 0).

Evidence: `repo://crates/limedl-core/src/scheduler/mod.rs#L572-L610`.

### Automatic

`rebalance_automatic` sorts candidates by priority descending then **remaining
bytes descending** (large files first), then splits `max_parallel_threads` in two
passes:

1. give every candidate its `min_threads_per_task` (or whatever budget remains),
   capped per task and per host;
2. hand out the remaining budget one thread at a time round-robin, respecting the
   per-task cap and the host cap.

Terminal tasks get 0, zero-allocation tasks are queued, and allocated tasks
become `Downloading` unless they are already `Retrying`.

Evidence: `repo://crates/limedl-core/src/scheduler/mod.rs#L614-L745`.

### Per-host cap

`HostCapTracker` caps total connections per hostname at
`MAX_CONNECTIONS_PER_HOST` (6). `grant` caps the request by the host's remaining
budget and records the grant; `used_by` lets the automatic distributor skip a
host that is already at its cap. Both rebalance branches use the same tracker, so
the per-host limit is not duplicated per branch.

Evidence: `repo://crates/limedl-core/src/scheduler/mod.rs#L39-L40`,
`repo://crates/limedl-core/src/scheduler/mod.rs#L541-L566`.

## Concurrency slots

Separately from per-host connection caps, `ConcurrencyManager` bounds how many
downloads run at once. `try_acquire_http`/`try_acquire_bt` are CAS loops over
`AtomicUsize` counters; on success they return a `DownloadSlotGuard` whose `Drop`
decrements the counter. `update_limits` changes the maxima and `rebalance_notify`
wakes the scheduler. The same manager is shared by `DownloadManager` and
`LazyBtBackend`, so `max_concurrent_bt` bounds both.

Evidence: `repo://crates/limedl-core/src/services/concurrency.rs#L8-L94`,
`repo://crates/limedl-core/src/slot_guard.rs#L8-L24`.

## SpeedTracker: real-time speed

`SpeedTracker` keeps a 2 s sliding window as 10 buckets of 200 ms each, with no
heap allocation. `record_bytes` restarts the window on the first sample or after a
gap of at least the whole window (a stall); `current_speed` sums bytes whose
bucket timestamp falls inside `[now - 2 s, now]` and divides by the effective
elapsed time clamped to 200 ms..2 s. It returns `None` when uninitialized or when
no bytes landed in the window, so a stalled transfer decays to "no speed".

`build_snapshot` prefers the tracker over the lifetime average
(`downloaded / (now - created_at)`), which used to make paused/queued or
long-running tasks show severely understated speeds. The tracker is reset on
pause/cancel/restart (`reset_progress`). ETA is derived from the same speed.

Evidence: `repo://crates/limedl-core/src/speed_tracker.rs#L13-L130`,
`repo://crates/limedl-core/src/task_lifecycle/mod.rs#L478-L500`.

Related pages: [HTTP Download Lifecycle](http-download-lifecycle.md),
[Settings and Configuration](../systems/settings-and-configuration.md),
[BitTorrent Backend](bit-torrent-backend.md).
