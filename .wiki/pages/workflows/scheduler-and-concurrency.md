---
id: workflows/scheduler-and-concurrency
title: Scheduler, AIMD and Concurrency Control
status: verified
generated_at: 2026-10-31T15:00:16Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence:
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 105
  - 151
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 0d2ae3a5557e844dd7c76f008c4483480a649491
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
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 2d71eadb9120ef82dd13f5d379c8ca70348c4f29
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 153
  - 186
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 98a87ea5c8250e9ce823ea6ac6d3cb3ab975bdb2
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 484
  - 534
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 30482bd9575af34cacf82dcc0e50446e04ab8f6b
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 649
  - 690
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: df75b0a657b79355ad607dc0dffcd876b99fa617
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 536
  - 579
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 2c8915b5f1fc99a4cf640ebd7d41c1a0ed0c1bdc
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 581
  - 624
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: f1877762ed1bdb44994aad65204dd5b5d698594f
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 740
  - 809
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 329a9edef4dd1c2efe42bda1e8f9ce8b7af642d5
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 620
  - 646
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: f0dfb28a2fb6f48295e0e88d34d5d3ee9c90eb47
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 789
  - 806
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 70f49b9f5c4793e9049b12bfae02ed0a51b54fb7
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 343
  - 386
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: d3c31e20ca2a249b608952662b9013516ab06c95
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 188
  - 258
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: fe5849508725b202e0ba6da069723a66ba5438c2
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1335
  - 1358
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 273489d1f74b16d332b56dc8aad96987f507dade
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 886
  - 943
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 411ecbc765b50e7507f47171d859db72532ea5ed
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1003
  - 1058
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 5bfbafbe5175a7bb87a74cf1d612d1008e1eb14c
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1060
  - 1083
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 30a495f772e1a615b3fdd94785d1a7289719a501
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1249
  - 1333
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: e7fea1bf68600ec9aea52a8113d3e3d8b889da6e
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 982
  - 1000
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: dfc8d2f95524237afb33a8d4e4880d6282e21aa6
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 44
  - 47
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 14cb8dc3f818cee9da16ff709dd8e78c7bbee538
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 839
  - 884
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 9eac1eab49810218986a7717d0a927de86e2387a
- path: crates/limedl-disk/src/services/concurrency.rs
  lines:
  - 59
  - 94
  blob: 9cdcba1d1f38e23764b032a905ed68848684941a
  snippet_sha1: 56c053483a747d04aafe14238c7c4d9f0c976942
- path: crates/limedl-types/src/slot_guard.rs
  lines:
  - 8
  - 24
  blob: 788daafd54fd398eb2f312ba926f687762047d30
  snippet_sha1: 2696fea499bf5e09e45d051b22e590564ce33170
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 328
  - 332
  blob: 0cc22fbae5b57aef68d19cca9bbb86f8e9516d76
  snippet_sha1: 9f509af798c4cc3e819e12461de8ce22e3d8351b
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 240
  - 246
  blob: 2159ecd2cb59b38b7c09ee74df77cade248b14ac
  snippet_sha1: 2178501b22668c25dfe043fc422856804eddb0b1
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 67
  - 75
  blob: e7734589a56cfbec112230cb3856dab3b3ef1e19
  snippet_sha1: 6e424828b0db299ae82300163b370baeb24041b8
- path: crates/limedl-bt/src/lifecycle.rs
  lines:
  - 42
  - 58
  blob: e33acea93c70b3f4e2b23736c21aa3c8c0953ec2
  snippet_sha1: 9d413931ad68937535f6a799309a8e3e25ee8cf6
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
  blob: e2b0f898b2e1fea578b56a582f8dd6d4165d682a
  snippet_sha1: 11b7537b592f925ba91ca01479cf4c7745816ce1
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 368
  - 379
  blob: e2b0f898b2e1fea578b56a582f8dd6d4165d682a
  snippet_sha1: e04be973a9e3af5eed16e110415f3ae13cfc80b5
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
