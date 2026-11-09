---
id: workflows/scheduler-and-concurrency
title: Scheduler, AIMD and Concurrency Control
status: draft
generated_at: 2026-11-09T11:42:32Z
commit: e50d853ddda31dd2196341bd48fb7ca25d84e40c
evidence:
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 101
  - 149
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: d368050658b3c3476242b7dbf1e5c00a11fb2c49
  note: drift-projected L105-L151@2159ecd2 -> L101-L149 (similarity 0.98)
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
  - 389
  - 461
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 2d71eadb9120ef82dd13f5d379c8ca70348c4f29
  note: drift-projected L391-L463@2159ecd2 -> L389-L461 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 151
  - 184
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 0230980ac61b5103487aec5136655108d1d94bb2
  note: drift-projected L153-L186@2159ecd2 -> L151-L184 (similarity 0.97)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 481
  - 531
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 30482bd9575af34cacf82dcc0e50446e04ab8f6b
  note: drift-projected L484-L534@2159ecd2 -> L481-L531 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 640
  - 681
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: df75b0a657b79355ad607dc0dffcd876b99fa617
  note: drift-projected L649-L690@2159ecd2 -> L640-L681 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 533
  - 574
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 5f561df8d9464f2448536ece79677d5ab645fde2
  note: drift-projected L536-L579@2159ecd2 -> L533-L574 (similarity 0.95)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 576
  - 616
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: fac7e88f90cc605d985865e552b5b93e2bc6e9e6
  note: drift-projected L581-L624@2159ecd2 -> L576-L616 (similarity 0.93)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 731
  - 800
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 329a9edef4dd1c2efe42bda1e8f9ce8b7af642d5
  note: drift-projected L740-L809@2159ecd2 -> L731-L800 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 614
  - 637
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: aa55a3af7f81cedbfd8d758d2c5e0f965945675c
  note: drift-projected L620-L646@2159ecd2 -> L614-L637 (similarity 0.85)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 780
  - 797
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 70f49b9f5c4793e9049b12bfae02ed0a51b54fb7
  note: drift-projected L789-L806@2159ecd2 -> L780-L797 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 341
  - 384
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: d3c31e20ca2a249b608952662b9013516ab06c95
  note: drift-projected L343-L386@2159ecd2 -> L341-L384 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 186
  - 256
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 700e714b66b47700f6fb04b552e96a2624419431
  note: drift-projected L188-L258@2159ecd2 -> L186-L256 (similarity 0.99)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1323
  - 1346
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 273489d1f74b16d332b56dc8aad96987f507dade
  note: drift-projected L1335-L1358@2159ecd2 -> L1323-L1346 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 877
  - 932
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 463ecc7c341da0f711416ea8496d3b3a29667821
  note: drift-projected L886-L943@2159ecd2 -> L877-L932 (similarity 0.97)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 991
  - 1046
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 5bfbafbe5175a7bb87a74cf1d612d1008e1eb14c
  note: drift-projected L1003-L1058@2159ecd2 -> L991-L1046 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1048
  - 1071
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 30a495f772e1a615b3fdd94785d1a7289719a501
  note: drift-projected L1060-L1083@2159ecd2 -> L1048-L1071 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 1237
  - 1321
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: e7fea1bf68600ec9aea52a8113d3e3d8b889da6e
  note: drift-projected L1249-L1333@2159ecd2 -> L1237-L1321 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 971
  - 988
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: e72c57e47e0e31bdc00f9a9394499be6610dcc81
  note: drift-projected L982-L1000@2159ecd2 -> L971-L988 (similarity 0.95)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 42
  - 45
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 14cb8dc3f818cee9da16ff709dd8e78c7bbee538
  note: drift-projected L44-L47@2159ecd2 -> L42-L45 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 830
  - 875
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 9eac1eab49810218986a7717d0a927de86e2387a
  note: drift-projected L839-L884@2159ecd2 -> L830-L875 (similarity 1.00)
- path: crates/limedl-disk/src/services/concurrency.rs
  lines:
  - 59
  - 94
  blob: bb145d2ddf019912e6213e47df15b4be5656dacc
  snippet_sha1: fb53244d6833ced92cd4d5b2356ba18d7d61e669
  note: content lost at HEAD; re-pinned at recorded range (draft)
- path: crates/limedl-types/src/slot_guard.rs
  lines:
  - 10
  - 46
  blob: f5e14f83880cd91e937ea5b85b44af7af538e362
  snippet_sha1: 9ff3991e7ee0a95dff5e0a30aef5a6ffaeb0442c
  note: drift-projected L8-L24@788daafd -> L10-L46 (similarity 1.00)
- path: crates/limedl-core/src/dispatcher.rs
  lines:
  - 338
  - 342
  blob: 2f774d1c7b11e7af6fb302a7063501181b4f8a0b
  snippet_sha1: 9f509af798c4cc3e819e12461de8ce22e3d8351b
  note: drift-projected L328-L332@0cc22fba -> L338-L342 (similarity 1.00)
- path: crates/limedl-engine/src/scheduler/mod.rs
  lines:
  - 238
  - 244
  blob: c44ff5d6f8a044959dfc3b61b1faae6408b3af55
  snippet_sha1: 2178501b22668c25dfe043fc422856804eddb0b1
  note: drift-projected L240-L246@2159ecd2 -> L238-L244 (similarity 1.00)
- path: crates/limedl-core/src/bootstrap.rs
  lines:
  - 70
  - 78
  blob: 50795894cd09930a7c13f6e67352c4a0e4648894
  snippet_sha1: 6e424828b0db299ae82300163b370baeb24041b8
  note: drift-projected L67-L75@e7734589 -> L70-L78 (similarity 1.00)
- path: crates/limedl-bt/src/lifecycle.rs
  lines:
  - 42
  - 58
  blob: a59f3869c649c0d6b1d159cb89aa567e254cbc62
  snippet_sha1: 852690e4a18c79bb7e12e062b7148e1f779aa96d
  note: content lost at HEAD; re-pinned at recorded range (draft)
- path: crates/limedl-engine/src/speed_tracker.rs
  lines:
  - 12
  - 135
  blob: 67a2ce06c47b488ac8682fc2391fa89d844a8a3f
  snippet_sha1: 3c001a0fc2aa7ba4f9bd2ff55251ba80ebc5e6fb
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 416
  - 456
  blob: f79123457a4d696179ee96b3725fabc9a64fe7a0
  snippet_sha1: a54f824018c0f3625f624f90a84d761320787248
  note: drift-projected L445-L499@e2b0f898 -> L416-L456 (similarity 0.73)
- path: crates/limedl-engine/src/task_lifecycle/mod.rs
  lines:
  - 348
  - 357
  blob: f79123457a4d696179ee96b3725fabc9a64fe7a0
  snippet_sha1: 7d677ccba27fa5ec326650224a54e7bb53c3e5f6
  note: drift-projected L368-L379@e2b0f898 -> L348-L357 (similarity 0.83)
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
