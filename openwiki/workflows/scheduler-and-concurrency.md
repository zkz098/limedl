---
type: workflow
title: Scheduler, AIMD and Concurrency Control
description: limedl's background scheduler loop and adaptive thread allocation — per-task thread-mode resolution, the AIMD throughput tuner, traditional vs automatic rebalancing, per-host connection caps, and the slot guards that bound HTTP and BT concurrency.
tags: [scheduler, aimd, concurrency, threads, rebalancing]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T01:52:48.489Z
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
generated: { by: "pi", at: "2026-10-04T01:52:48.489Z" }
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
throughput, and oscillation state (`last_direction`, `oscillation_count`,
`hysteresis_lock_until`). `sample_throughput` computes bytes-per-second since the
last sample; `record_sample` ignores non-positive/non-finite values.

Profile helpers:

| Profile | Initial threads (fraction of cap) | Decrease factor | Cooldown |
| --- | --- | --- | --- |
| Conservative | 0.5 | 0.7 | 4 s |
| Balanced | 0.75 | 0.5 | 3 s |
| Aggressive | 1.0 | 0.5 | 2 s |

Decreases never go below `min_threads`.

Evidence: `repo://crates/limedl-core/src/aimd/mod.rs#L6-L90`.

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

- returns early during a penalty cooldown (clearing `recent_penalty`) or while
  the hysteresis lock is active (continuing to record throughput);
- decreases when throughput dropped by the profile's degrade threshold (plus
  `recent_penalty` for Conservative);
- otherwise attempts an increase: `maybe_increase` requires the allocation to
  have caught up with the target, improved throughput, and enough consecutive
  good samples; the step is +2 (Conservative), `current/4` (Balanced) or
  `current/3` (Aggressive), capped at `max_threads_per_task`.

Every change to `desired_thread_count` calls `sync_snapshot_with_manifest` before
returning, so the exposed "target thread count" never lags the manifest.

Evidence: `repo://crates/limedl-core/src/scheduler/mod.rs#L87-L120`,
`repo://crates/limedl-core/src/scheduler/mod.rs#L317-L410`,
`repo://crates/limedl-core/src/scheduler/mod.rs#L432-L486`.

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

<!-- openwiki: broken internal link [/openwiki/workflows/http-download-lifecycle.md] link "/openwiki/workflows/http-download-lifecycle.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
Related pages: [HTTP Download Lifecycle](/openwiki/workflows/http-download-lifecycle.md),
<!-- openwiki: broken internal link [/openwiki/systems/settings-and-configuration.md] link "/openwiki/systems/settings-and-configuration.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[Settings and Configuration](/openwiki/systems/settings-and-configuration.md),
<!-- openwiki: broken internal link [/openwiki/workflows/bit-torrent-backend.md] link "/openwiki/workflows/bit-torrent-backend.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
[BitTorrent Backend](/openwiki/workflows/bit-torrent-backend.md).
