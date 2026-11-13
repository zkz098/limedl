---
id: workflows/scheduler-and-concurrency
title: Scheduler, AIMD and Concurrency Control
status: verified
generated_at: 2026-11-13T06:38:14Z
commit: c28b25a7355ded0aa81f07dfee7c66c53537eaf3
evidence:
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 105
  - 151
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 735d1504158bec11c7648bcb73ea611a120d63d1
- path: crates/limedl-engine/src/download/shared.rs
  lines:
  - 16
  - 66
  blob: 4fd41e05b1cb50e99f9aad5e048f9a31ab2fba7f
  snippet_sha1: bf4e4c162589e3d32b034db701fec3f8b4bf3929
- path: crates/limedl-engine/src/aimd/mod.rs
  lines:
  - 11
  - 66
  blob: 8e194d080f3e28b29f50aff0f4908200a49d1290
  snippet_sha1: 8a6f0325eee4e8b4867d499b6c77e750d52e4bef
- path: crates/limedl-engine/src/aimd/mod.rs
  lines:
  - 68
  - 93
  blob: 8e194d080f3e28b29f50aff0f4908200a49d1290
  snippet_sha1: 67f027bdca9ab1976af8e639861f36b85edc1e2a
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 391
  - 463
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 60053da6d7ff7d10d9a34db763fe900f981ce924
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 153
  - 186
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 9853964d6a5c92da0c880681f3fd1afe7579ff89
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 484
  - 534
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 3bb9f50ddfc9c425c2da872ebba3c4123cbb45e3
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 649
  - 690
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 240b51161aeaff9c6d4c8518c6feb8df814c783a
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 536
  - 579
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: c6114a4b9df38fb97912ce3ae4e9670e775302b6
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 581
  - 624
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 250be67df95c5821d7142d0043b30a26621c3ce6
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 740
  - 809
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 4461a3b3a8f24d0aa4787aef553fd846e5e90a7f
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 620
  - 646
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 9011a3a2b4753dccaddd8f88233b26daedce753b
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 789
  - 806
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 88ee6959be6ab6752035e6589eca1acacf382f66
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 343
  - 386
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: ed5038fb4ebe438b40688607953f430c2b5865c8
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 188
  - 258
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 067e75ea27d50c5f99e5dd83c815da34ca726658
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1335
  - 1358
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 0d41de7ab7343213eab8be7432c3a79e9260d54a
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 886
  - 943
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 8804b7c2b07b7e0b68f9c068b3388c2761b69206
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1003
  - 1058
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 80b737bbbda6cc2ca4640c9a4591f8f66da52087
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1060
  - 1083
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: a8d474c6183550606f3f01997ddd984d646b830f
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1249
  - 1333
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 914f75a15211fca391ecbfcf1ef415578b797e76
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 982
  - 1000
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: dcfb3fbb954a0b291b3694f25112653a898a4c7f
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 44
  - 47
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 1b5f0f96818f90fbd971b9a28cfe2f27796d3ea5
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 839
  - 884
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 495b2658bd61b1e8aab8d6b82938ab05addd7452
- path: crates/limedl-disk/src/services/concurrency.rs
  lines:
  - 59
  - 94
  blob: f847cee78c143c31348accac0b9891d808ececa2
  snippet_sha1: 2c13dc034d317dc9f7dfc38e2008708d780991b8
- path: crates/limedl-types/src/slot_guard.rs
  lines:
  - 8
  - 24
  blob: 1f3d029a9bba5aaa7fea5c72273d284a0f3483dd
  snippet_sha1: 615d81ca02b19a11dc86c8d16fde3b277a8faa26
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 328
  - 332
  blob: 2f774d1c7b11e7af6fb302a7063501181b4f8a0b
  snippet_sha1: 1e5e36759cd8d805ac860709ad2804550606b9d7
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 240
  - 246
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 3e305b1885ff68840a0ebd1da4c3070c596dd68d
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 67
  - 75
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: 3d10b42629f34222716feee9ce314093a2cb8f1c
- path: crates/limedl-bt/src/lifecycle.rs
  lines:
  - 42
  - 58
  blob: 23e02d1a5f118ba05f750ebb8c2d535723fcd3b0
  snippet_sha1: 3dde36f82d7a5b395b4566a5fa1768e4ce510fa0
- path: crates/limedl-engine/src/speed_tracker.rs
  lines:
  - 12
  - 135
  blob: 67a2ce06c47b488ac8682fc2391fa89d844a8a3f
  snippet_sha1: 3c001a0fc2aa7ba4f9bd2ff55251ba80ebc5e6fb
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 445
  - 499
  blob: 06fa226e016db7406aab96ecc589b08052421836
  snippet_sha1: 89f9c5963c4247b3e2c8086258ddf9093f54ff80
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 368
  - 379
  blob: 06fa226e016db7406aab96ecc589b08052421836
  snippet_sha1: 283686dd091e2febb91bf437a303f8891c21cad4
type: workflow
---
# Scheduler, AIMD and Concurrency Control

`Scheduler` is a zero-sized actor that `DownloadManager` holds as
`Arc<Scheduler>`; since the engine split it lives in the `limedl-engine` crate.
Its background loop decides how many connections each active download gets, and
the AIMD tuner adapts those targets to measured throughput.

## The scheduler loop

`start_scheduler_loop` spawns a task that selects on a tick of **2 s**
(`SCHEDULER_TICK`, or the expert-configured `scheduler_tick_ms`), backing off to
`IDLE_SCHEDULER_TICK` (30 s) when no task is active, or the `rebalance_notify`
wakeup, or the shutdown token. Each iteration:

1. applies the speed-limit schedule for the current local hour;
2. calls `update_adaptive_targets`;
3. calls `rebalance_allocations`.

Consecutive `rebalance_allocations` failures are counted and escalated to an
error log after three
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L105-L151]].

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
`chunk_size * 2`
[[cite:crates/limedl-engine/src/download/shared.rs:L16-L66]].

## AIMD

`AimdState` tracks the last sample bytes/time, last throughput, cooldown,
consecutive good/bad samples, a `recent_penalty` flag, cumulative/peak
throughput, oscillation state (`last_direction`, `oscillation_count`,
`hysteresis_lock_until`), and the probe/settling state added by the
proactive-probing fix (`settling_until`, `stable_cycles`, `is_probing_up`,
`probe_pre_throughput`, `probe_pre_threads`). `sample_throughput` computes
bytes-per-second since the last sample; `record_sample` ignores
non-positive/non-finite values
[[cite:crates/limedl-engine/src/aimd/mod.rs:L11-L66]].

`aimd/mod.rs` exposes three pure profile helpers:

| Profile | `initial_desired_threads` (fraction of cap) | `reduce_threads` factor | `cooldown_for_profile` |
| --- | --- | --- | --- |
| Conservative | 0.5 | 0.7 | 4 s |
| Balanced | 0.75 | 0.5 | 3 s |
| Aggressive | 1.0 | 0.5 | 2 s |

Decreases never go below `min_threads`
[[cite:crates/limedl-engine/src/aimd/mod.rs:L68-L93]]. The scheduler derives its
own thresholds per decision via `AdaptiveThresholds::from_settings`: `degrade` /
`increase` fractions, `severe_drop`, `samples_needed`, `probe_stable_cycles`
(Conservative 3, Balanced/Aggressive 2), `decrease_factor` / `step_ratio`,
oscillation threshold 3 and hysteresis multiplier 4 — every one of which expert
mode can override field by field
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L391-L463]].

### One adaptive decision

`update_adaptive_targets` only runs in `SchedulerMode::Automatic`. It is
deliberately **not gated on `settings.proxy.mode`**: the tuner used to bail out
whenever a proxy was configured (a leftover from the removed network-learning
feature, which also silently disabled overclock mode). Proxied transfers adapt
like any other. Overclock mode pins all adaptive downloading tasks at the cap;
otherwise each task goes through `update_one_adaptive`
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L153-L186]].

`update_one_adaptive` takes the **`core` lock first and the `aimd` lock second** —
the same order as every other scheduler path, and it must not be reversed. It
only acts on adaptive tasks that are `Downloading` with range support, then
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L484-L534]]:

- returns early during a penalty cooldown (clearing `recent_penalty`), the
  hysteresis lock, or the post-change settling window, while still recording
  throughput [[cite:crates/limedl-engine/src/scheduler/mod.rs:L649-L690]];
- if a proactive probe is in flight, waits for the allocation to catch up and
  then either keeps the raised target or rolls it back (see below)
  [[cite:crates/limedl-engine/src/scheduler/mod.rs:L536-L579]];
- decreases only when degradation is confirmed (a severe drop, two consecutive
  bad samples, or `recent_penalty`; Conservative treats the penalty flag as
  sufficient) [[cite:crates/limedl-engine/src/scheduler/mod.rs:L581-L624]];
- otherwise attempts an increase: `maybe_increase` requires the allocation to
  have caught up with the target and the task to be below the cap, then either
  improved throughput with enough consecutive good samples (step +1
  Conservative, `current/4` Balanced, `current/3` Aggressive) or a
  stable-transfer proactive probe
  [[cite:crates/limedl-engine/src/scheduler/mod.rs:L740-L809]].

Every change to `desired_thread_count` calls `sync_snapshot_with_manifest` before
returning, so the exposed "target thread count" never lags the manifest
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L620-L646]].

### Proactive probing and the cascading-decrease fix

The tuner used to be able to deadlock at a low thread count: a single small
throughput wobble could trigger a decrease, and once the target bottomed out
there was no path back up. Two changes break that
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L581-L624]]:

- **Decrease confirmation.** A decrease is applied only once degradation is
  confirmed — a severe drop (below `severe_drop`, by default 50 % of the last
  sample), two consecutive bad samples, or `recent_penalty` (Conservative
  additionally treats the penalty flag as sufficient). A single noisy sample no
  longer cascades into repeated decreases.
- **Proactive probing.** When throughput is steady for `probe_stable_cycles`,
  `maybe_increase` raises the target *without* waiting for a throughput
  improvement and records `is_probing_up`, `probe_pre_throughput` and
  `probe_pre_threads`
  [[cite:crates/limedl-engine/src/scheduler/mod.rs:L789-L806]]. The next
  decision (after settling, once allocation caught up) keeps the higher
  concurrency if throughput improved by at least the profile's `increase`
  fraction; otherwise it rolls back to `probe_pre_threads`, applies a 2×
  cooldown and a settling window, and rebases the throughput baseline. This
  turns "no change" into a measured experiment instead of a permanent stall
  [[cite:crates/limedl-engine/src/scheduler/mod.rs:L536-L579]].

`settling_until` suspends decisions right after any thread change (recording
samples), and `hysteresis_lock_until` suspends them for `cooldown × 4` after
oscillation (≥3 direction flips), where the target is clamped to 70 % of the
current value (the expert `decrease_factor` in expert mode). All three gates
record throughput while they skip decisions
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L343-L386]].

## Rebalancing

`rebalance_allocations` collects the `Arc<ManagedDownload>` entries under the
`downloads` read lock, drops the lock, resolves each download's disk type per
destination directory (cached per directory), dispatches by mode, publishes the
number of `Downloading` tasks into `concurrency.active_http_count`, records
whether any task is still active (which selects the tick interval), and persists
the rebalanced manifests in a fail-soft batch
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L188-L258]],
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L1335-L1358]].

### Traditional

`rebalance_traditional` sorts by priority descending then creation time ascending
(using `sort_by_cached_key` so the comparator does not take per-download locks
twice per comparison). It walks the list keeping at most
`max_parallel_tasks` running and at most `max_parallel_hdd` tasks on mechanical
drives; each running task gets its effective allocation (via
`effective_allocation_cap`) capped by the per-host budget, and everything else
is queued (allocation 0)
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L886-L943]].

### Automatic

`rebalance_automatic` is deliberately **monotonic and sticky**: candidates are
first ordered by static keys — by default priority descending, then **total size**
descending, then creation order (expert mode can switch the
`QueueSortStrategy` to shortest-job-first or FIFO) — replacing the old ordering
on *remaining* bytes, which changed every progress tick and destabilised the
active set
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L1003-L1058]]. Incumbents that
held threads last pass are placed ahead of waiting tasks; a waiting task is
promoted only by strict priority preemption or starvation rotation after
`AUTOMATIC_ROTATION_PASSES` (15) passes
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L1060-L1083]]. The budget
`max_parallel_threads` is then split in two passes:

1. give every candidate its `min_threads_per_task` (or whatever budget remains),
   capped per task and per host, and respecting the HDD quota;
2. hand out the remaining budget one thread at a time round-robin to candidates
   that already got an initial allocation, respecting the per-task cap and the
   host cap [[cite:crates/limedl-engine/src/scheduler/mod.rs:L1249-L1333]].

Terminal tasks get 0, zero-allocation tasks are queued, and allocated tasks
become `Downloading` unless they are already `Retrying`
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L982-L1000]].

### Per-host cap

`HostCapTracker` caps total connections per hostname at
`MAX_CONNECTIONS_PER_HOST` (16; the expert setting
`max_connections_per_host` overrides it in expert mode). `grant` caps the
request by the host's remaining budget and records the grant; `used_by` lets the
automatic distributor skip a host that is already at its cap. Both rebalance
branches use the same tracker, so the per-host limit is not duplicated per
branch [[cite:crates/limedl-engine/src/scheduler/mod.rs:L44-L47]],
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L839-L884]].

## Concurrency slots

Separately from per-host connection caps, `ConcurrencyManager` (now in
`limedl-disk`) bounds how many downloads run at once. `try_acquire_http` /
`try_acquire_bt` are CAS loops over `AtomicUsize` counters; on success they
return a `DownloadSlotGuard` whose `Drop` decrements the counter
[[cite:crates/limedl-disk/src/services/concurrency.rs:L59-L94]],
[[cite:crates/limedl-types/src/slot_guard.rs:L8-L24]]. `update_limits` changes
the maxima and `rebalance_notify` wakes the scheduler
[[cite:crates/limedl-core/src/dispatcher.rs:L328-L332]]. Note that the HTTP path
no longer acquires a slot per task: the scheduler rewrites
`active_http_count` from the number of `Downloading` tasks on every rebalance
[[cite:crates/limedl-engine/src/scheduler/mod.rs:L240-L246]]. BT enforcement is
live: `LazyBtBackend` shares the same `active_bt_count` /
`max_concurrent_bt` arcs handed over at bootstrap and runs the same CAS loop in
its own `try_acquire_bt_slot`, so `max_concurrent_bt` bounds both
[[cite:crates/limedl-core/src/bootstrap.rs:L67-L75]],
[[cite:crates/limedl-bt/src/lifecycle.rs:L42-L58]].

## SpeedTracker: real-time speed

`SpeedTracker` (in `limedl-engine`) keeps a 2 s sliding window as 10 buckets of
200 ms each, with no heap allocation. `record_bytes` restarts the window on the
first sample or after a gap of at least the whole window (a stall);
`current_speed` sums bytes whose bucket timestamp falls inside
`[now - 2 s, now]` and divides by the effective elapsed time clamped to
200 ms..2 s. It returns `None` when uninitialized or when no bytes landed in the
window, so a stalled transfer decays to "no speed"
[[cite:crates/limedl-engine/src/speed_tracker.rs:L12-L135]].

`build_snapshot` reports no speed in terminal states; otherwise it prefers the
tracker once initialized, falling back to the AIMD last throughput or the
lifetime average (`downloaded / (now - created_at)`) — the lifetime average
alone used to make paused/queued or long-running tasks show severely understated
speeds. ETA is derived from the same speed
[[cite:crates/limedl-engine/src/task_lifecycle/mod.rs:L445-L499]]. The tracker
is reset on progress resets (`reset_progress`)
[[cite:crates/limedl-engine/src/task_lifecycle/mod.rs:L368-L379]].

Related pages: [[wiki:workflows/http-download-lifecycle]],
[[wiki:systems/settings-and-configuration]],
[[wiki:workflows/bit-torrent-backend]].
