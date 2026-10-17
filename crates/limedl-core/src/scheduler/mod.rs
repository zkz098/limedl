//! Scheduler and rebalance logic — extracted from manager.rs
//! (Phase 5 of the manager.rs split).
//!
//! Contains the background scheduler loop and adaptive AIMD thread rebalancing.
//!
//! `Scheduler` is an independent actor type.  All its methods receive a
//! `&dyn ManagerState` or `Arc<dyn ManagerState>` parameter to access shared
//! state, so this module does not depend on `manager` — which owns
//! `Arc<Scheduler>` and implements that trait.

use std::{
    path::Path,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use foldhash::HashMap;
use parking_lot::Mutex;
use reqwest::Url;

use crate::{
    aimd::{self, AimdState, Direction},
    database::Database,
    download::{
        DEFAULT_FIXED_THREADS, MAX_TRADITIONAL_THREADS, ManagedDownload, log_background_error,
        sync_snapshot_with_manifest,
    },
    error::Result,
    manager_state::ManagerState,
    manifest::Manifest,
    now_ms,
    persistence::persist_manifest_snapshots_batch,
    types::{
        AdaptiveProfile, AppSettings, DiskType, DownloadState, QueueSortStrategy, SchedulerMode,
        ThreadMode,
    },
};

const SCHEDULER_TICK: Duration = Duration::from_secs(2);

/// Maximum concurrent connections (threads) allowed to a single hostname.
const MAX_CONNECTIONS_PER_HOST: usize = 16;

/// Rebalance passes a task may spend waiting for a slot before automatic mode
/// rotates it in by preempting the lowest-ranked active task.
///
/// At the default 2 s tick this is roughly 30 s; event-driven rebalances can
/// make it sooner.  The bound is what keeps the active set stable: without it
/// the allocator would either starve the tail forever or (as it used to) churn
/// the active set every tick.
const AUTOMATIC_ROTATION_PASSES: u32 = 15;

/// Scheduler state that must survive between rebalance passes.
///
/// Automatic mode used to be stateless and re-derive the active set from the
/// *remaining* bytes of every task.  Because a running task's remaining bytes
/// only ever shrink while a queued task's stay frozen, the ranking fed back on
/// itself and the active set flipped every tick.  The allocator now keys on
/// static task properties and remembers how long each task has waited.
#[derive(Debug, Default)]
struct AutomaticRebalanceState {
    /// Consecutive rebalance passes each eligible task has spent with no
    /// threads.  Drives which task the next rotation promotes.
    waiting_passes: HashMap<String, u32>,
    /// Passes since the last rotation, capped below [`AUTOMATIC_ROTATION_PASSES`]
    /// when there is nothing to rotate (every task fits).
    passes_since_rotation: u32,
}

/// Actor type for scheduler and rebalance logic.
///
/// Methods that need shared state receive `&dyn ManagerState`; `DownloadManager`
/// implements that trait and holds `Arc<Scheduler>` for delegation.
pub struct Scheduler {
    /// Persisted across passes so allocations stay sticky and starvation is
    /// detectable; see [`AutomaticRebalanceState`].
    automatic: Mutex<AutomaticRebalanceState>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            automatic: Mutex::new(AutomaticRebalanceState::default()),
        }
    }

    /// Start the background scheduler loop (750ms tick or rebalance_notify).
    /// Consumes `self: Arc<Self>` to keep the scheduler alive, and
    /// `dm: Arc<dyn ManagerState>` for the spawned task.
    pub(crate) fn start_scheduler_loop(self: Arc<Self>, dm: Arc<dyn ManagerState>) {
        tokio::spawn(async move {
            let mut consecutive_rebalance_failures: u32 = 0;
            loop {
                let tick = {
                    let settings = dm.settings_service().get().await;
                    if settings.scheduler.expert_mode {
                        Duration::from_millis(settings.scheduler.expert.scheduler_tick_ms)
                    } else {
                        SCHEDULER_TICK
                    }
                };

                tokio::select! {
                    _ = tokio::time::sleep(tick) => {}
                    _ = dm.controls().rebalance_notify.notified() => {}
                    _ = dm.controls().shutdown_token.cancelled() => {
                        tracing::info!("Scheduler loop shutting down");
                        break;
                    }
                }

                self.apply_speed_limit_schedule(dm.as_ref()).await;

                if let Err(error) = self.update_adaptive_targets(dm.as_ref()).await {
                    log_background_error("update adaptive targets", &error);
                }
                match self.rebalance_allocations(dm.as_ref()).await {
                    Ok(()) => {
                        consecutive_rebalance_failures = 0;
                    }
                    Err(error) => {
                        consecutive_rebalance_failures += 1;
                        if consecutive_rebalance_failures >= 3 {
                            tracing::error!(
                                "rebalance_allocations failed {consecutive_rebalance_failures} consecutive times: {error:#}"
                            );
                        } else {
                            log_background_error("rebalance allocations", &error);
                        }
                    }
                }
            }
        });
    }

    /// Update adaptive (AIMD) thread targets for all active downloads.
    ///
    /// Deliberately **not** gated on `settings.proxy.mode`: the tuner used to
    /// bail out whenever a proxy was configured, a leftover from the removed
    /// network-learning feature it was coupled to (it also silently disabled
    /// overclock mode, which lives behind the same guard). Proxied transfers
    /// must adapt like any other transfer.
    ///
    /// Every target change is mirrored into the task snapshot, so the value the
    /// API exposes (the WebUI "target thread count") never lags behind the
    /// manifest.
    pub(crate) async fn update_adaptive_targets(&self, dm: &dyn ManagerState) -> Result<()> {
        let settings = dm.settings_service().get().await;
        if settings.scheduler.mode != SchedulerMode::Automatic {
            return Ok(());
        }

        let adaptive_cap = settings.scheduler.automatic.max_threads_per_task.max(1);
        let min_threads = settings.scheduler.automatic.min_threads_per_task.max(1);

        let downloads = dm.downloads().read().await;

        // ── Overclock mode: pin all adaptive tasks at max threads ──────────
        if dm.concurrency().overclock_mode() {
            pin_overclock_targets(downloads.values(), adaptive_cap);
            return Ok(());
        }

        for managed in downloads.values() {
            update_one_adaptive(managed, &settings, adaptive_cap, min_threads);
        }

        Ok(())
    }

    /// Rebalance thread allocations across all active downloads.
    pub(crate) async fn rebalance_allocations(&self, dm: &dyn ManagerState) -> Result<()> {
        let settings = dm.settings_service().get().await;
        let max_parallel_hdd = dm.buffer_pool().effective_max_parallel() as usize;

        // Phase 1: collect all Arc references under the read lock, then drop it
        let (mut entries, all_downloads) = {
            let guard = dm.downloads().read().await;
            let entries: Vec<_> = guard.values().cloned().collect();
            let all_downloads = entries.clone();
            (entries, all_downloads)
        };

        let mut dir_cache: HashMap<String, DiskType> = HashMap::default();
        let mut disk_types: HashMap<String, DiskType> = HashMap::default();
        for managed in &entries {
            let (id, dir) = {
                let core = managed.lock_core();
                (core.manifest.id.clone(), core.manifest.destination_dir.clone())
            };
            let dt = match dir_cache.get(&dir) {
                Some(&dt) => dt,
                None => {
                    let dt = dm.resolve_disk_type(Path::new(&dir)).await;
                    dir_cache.insert(dir, dt);
                    dt
                }
            };
            disk_types.insert(id, dt);
        }

        match settings.scheduler.mode {
            SchedulerMode::Traditional => {
                rebalance_traditional(&mut entries, &settings, &disk_types, max_parallel_hdd)
            }
            SchedulerMode::Automatic => {
                // Parking-lot guard is scoped so it is never held across an await.
                let mut state = self.automatic.lock();
                rebalance_automatic(
                    &entries,
                    &all_downloads,
                    &settings,
                    &mut state,
                    &disk_types,
                    max_parallel_hdd,
                );
            }
        }

        let downloading_count = all_downloads
            .iter()
            .filter(|m| m.lock_core().manifest.state == DownloadState::Downloading)
            .count();
        dm.concurrency()
            .active_http_count
            .store(downloading_count, Ordering::Release);

        persist_rebalanced(dm.db(), &all_downloads).await;
        Ok(())
    }

    /// Check the speed limit schedule and apply the matching limit (or revert to global default).
    async fn apply_speed_limit_schedule(&self, dm: &dyn ManagerState) {
        let settings = dm.settings_service().get().await;
        let schedule = &settings.speed_limit_schedule;
        if schedule.is_empty() {
            return;
        }

        // Get current local hour using the `time` crate
        let current_hour = match time::OffsetDateTime::now_local() {
            Ok(now) => now.hour(),
            Err(e) => {
                tracing::warn!("Failed to determine local time, falling back to global speed limit: {e}");
                dm.rate_limiter().set_rate(settings.global_speed_limit_bps);
                return;
            }
        };

        // Find matching slot
        let limit = schedule.iter().find_map(|slot| {
            if slot.start_hour <= slot.end_hour {
                // Normal range: e.g. 8-18
                if current_hour >= slot.start_hour && current_hour < slot.end_hour {
                    Some(slot.limit_bps)
                } else {
                    None
                }
            } else {
                // Wraps midnight: e.g. 22-6
                if current_hour >= slot.start_hour || current_hour < slot.end_hour {
                    Some(slot.limit_bps)
                } else {
                    None
                }
            }
        });

        match limit {
            Some(bps) => dm.rate_limiter().set_rate(bps),
            None => {
                // Re-read settings to get global_speed_limit_bps (schedule may have changed)
                let settings = dm.settings_service().get().await;
                dm.rate_limiter().set_rate(settings.global_speed_limit_bps);
            }
        }
    }
}

// ── Scheduler helper functions ───────────────────────────────────────────────

/// Extract the hostname from a download's final_url for per-host connection limiting.
fn hostname_from_manifest(manifest: &Manifest) -> Option<String> {
    Url::parse(&manifest.final_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
}

/// Computes the effective thread allocation cap for a given manifest and settings.
fn effective_allocation_cap(manifest: &Manifest, settings: &AppSettings) -> usize {
    if !manifest.supports_ranges {
        return 1;
    }

    match settings.scheduler.mode {
        SchedulerMode::Traditional => manifest
            .requested_thread_count
            .or(manifest.desired_thread_count)
            .unwrap_or(DEFAULT_FIXED_THREADS)
            .clamp(1, MAX_TRADITIONAL_THREADS),
        SchedulerMode::Automatic => {
            let desired = match manifest.thread_mode {
                ThreadMode::Fixed => manifest.requested_thread_count.unwrap_or(1),
                ThreadMode::Adaptive => manifest.desired_thread_count.unwrap_or(1),
            };
            desired.clamp(1, effective_automatic_task_cap(settings))
        }
    }
}

fn effective_automatic_task_cap(settings: &AppSettings) -> usize {
    settings.scheduler.automatic.max_threads_per_task.max(1)
}

/// Track thread-count direction changes for oscillation detection.
/// An oscillation is detected when the direction flips ≥3 consecutive times.
fn track_direction(aimd: &mut AimdState, dir: Direction) {
    match aimd.last_direction {
        Some(prev) if prev != dir => {
            aimd.oscillation_count = aimd.oscillation_count.saturating_add(1);
        }
        Some(_) => {
            aimd.oscillation_count = 0;
        }
        None => {
            // First direction — not a flip, just record it
        }
    }
    aimd.last_direction = Some(dir);
}

/// Check for oscillation (≥3 consecutive direction flips) and apply hysteresis lock.
fn check_oscillation(
    aimd: &mut AimdState,
    manifest: &mut crate::manifest::Manifest,
    current: usize,
    min_threads: usize,
    now: Instant,
    thresholds: &AdaptiveThresholds,
) {
    if aimd.oscillation_count >= thresholds.oscillation_threshold {
        let lock_target = if thresholds.is_expert {
            ((current as f64) * thresholds.decrease_factor).ceil() as usize
        } else {
            ((current as f64) * 0.7).ceil() as usize
        };
        manifest.desired_thread_count = Some(lock_target.max(min_threads));
        manifest.updated_at_ms = now_ms();
        aimd.hysteresis_lock_until = Some(now + thresholds.cooldown * thresholds.hysteresis_multiplier);
        aimd.consecutive_good_samples = 0;
        aimd.consecutive_bad_samples = 0;
        aimd.oscillation_count = 0;
        aimd.stable_cycles = 0;
        aimd.is_probing_up = false;
        aimd.probe_pre_throughput = None;
    }
}

// ── AIMD helpers ─────────────────────────────────────────────────────────────

/// AIMD thresholds derived from the adaptive profile.
struct AdaptiveThresholds {
    profile: AdaptiveProfile,
    degrade: f64,
    increase: f64,
    severe_drop: f64,
    samples_needed: u32,
    cooldown: Duration,
    probe_stable_cycles: u32,
    decrease_factor: f64,
    step_ratio: f64,
    oscillation_threshold: u32,
    hysteresis_multiplier: u32,
    is_expert: bool,
}

impl AdaptiveThresholds {
    fn from_settings(settings: &AppSettings, profile: AdaptiveProfile) -> Self {
        if settings.scheduler.expert_mode {
            let exp = &settings.scheduler.expert;
            let degrade = if exp.aimd_degrade_threshold > 0.50 {
                (1.0 - exp.aimd_degrade_threshold).max(0.01)
            } else {
                exp.aimd_degrade_threshold
            };
            let increase = if exp.aimd_increase_threshold >= 1.0 {
                exp.aimd_increase_threshold - 1.0
            } else if exp.aimd_increase_threshold > 0.50 {
                (1.0 - exp.aimd_increase_threshold).max(0.01)
            } else {
                exp.aimd_increase_threshold
            };
            Self {
                profile,
                degrade,
                increase,
                severe_drop: exp.aimd_severe_drop_threshold,
                samples_needed: exp.aimd_samples_needed,
                cooldown: Duration::from_secs(exp.aimd_cooldown_secs),
                probe_stable_cycles: exp.aimd_probe_stable_cycles,
                decrease_factor: exp.aimd_decrease_factor,
                step_ratio: exp.aimd_step_ratio,
                oscillation_threshold: exp.aimd_oscillation_threshold,
                hysteresis_multiplier: exp.aimd_hysteresis_multiplier,
                is_expert: true,
            }
        } else {
            let (degrade, increase, samples_needed, probe_stable_cycles) = match profile {
                AdaptiveProfile::Conservative => (0.18, 0.08, 2, 3),
                AdaptiveProfile::Balanced => (0.16, 0.04, 1, 2),
                AdaptiveProfile::Aggressive => (0.20, 0.03, 1, 2),
            };
            let (decrease_factor, step_ratio) = match profile {
                AdaptiveProfile::Conservative => (0.7, 0.0),
                AdaptiveProfile::Balanced => (0.5, 0.25),
                AdaptiveProfile::Aggressive => (0.5, 0.3333333333333333),
            };
            Self {
                profile,
                degrade,
                increase,
                severe_drop: 0.50,
                samples_needed,
                cooldown: aimd::cooldown_for_profile(profile),
                probe_stable_cycles,
                decrease_factor,
                step_ratio,
                oscillation_threshold: 3,
                hysteresis_multiplier: 4,
                is_expert: false,
            }
        }
    }
}

/// Overclock mode: pin every adaptive, range-capable active task at `cap`.
fn pin_overclock_targets<'a>(downloads: impl Iterator<Item = &'a Arc<ManagedDownload>>, cap: usize) {
    for managed in downloads {
        let mut core = managed.lock_core();
        let manifest = &mut core.manifest;
        if manifest.thread_mode == ThreadMode::Adaptive
            && manifest.state == DownloadState::Downloading
            && manifest.supports_ranges
        {
            manifest.desired_thread_count = Some(cap);
            manifest.updated_at_ms = now_ms();
            sync_snapshot_with_manifest(&mut core);
        }
    }
}

/// Run one AIMD decision step for a single download.
///
/// Takes the `core` lock first and the `aimd` lock second, matching every other
/// scheduler path — do not reverse the order.
fn update_one_adaptive(
    managed: &Arc<ManagedDownload>,
    settings: &AppSettings,
    adaptive_cap: usize,
    min_threads: usize,
) {
    let mut core = managed.lock_core();
    let manifest = &mut core.manifest;
    if manifest.thread_mode != ThreadMode::Adaptive
        || manifest.state != DownloadState::Downloading
        || !manifest.supports_ranges
    {
        return;
    }

    let mut aimd = managed.lock_aimd();
    let now = Instant::now();
    let throughput = aimd
        .sample_throughput(manifest.downloaded_bytes, now)
        .unwrap_or_else(|| {
            manifest
                .allocated_thread_count
                .unwrap_or(0)
                .saturating_mul(1) as f64
        });

    let current = manifest.desired_thread_count.unwrap_or(1).max(1);
    let allocated = manifest.allocated_thread_count.unwrap_or(0);
    let profile = manifest
        .adaptive_profile_snapshot
        .unwrap_or(settings.scheduler.automatic.adaptive_profile);

    if aimd_in_cooldown(&mut aimd, now) {
        return;
    }

    // ── Hysteresis lock: suspend AIMD decisions during oscillation recovery ──
    if hysteresis_skips_decision(&mut aimd, now, throughput) {
        return;
    }

    // ── Settling period: suspend decisions right after thread changes ──
    if aimd_in_settling(&mut aimd, now, throughput) {
        return;
    }

    let thresholds = AdaptiveThresholds::from_settings(settings, profile);

    // ── Probing verification: evaluate outcome of a previous upward probe ──
    if aimd.is_probing_up {
        if allocated != current {
            // Wait until the allocation catches up with the target
            aimd.record_sample(throughput);
            return;
        }

        let pre_throughput = aimd.probe_pre_throughput.unwrap_or(0.0);
        let pre_threads = aimd.probe_pre_threads;
        let improved = pre_throughput > 0.0
            && throughput >= pre_throughput * (1.0 + thresholds.increase);
        aimd.is_probing_up = false;
        aimd.probe_pre_throughput = None;

        if improved {
            // Probe succeeded: retain higher concurrency
            aimd.last_throughput = Some(throughput);
            aimd.stable_cycles = 0;
            aimd.consecutive_bad_samples = 0;
            aimd.recent_penalty = false;
            aimd.record_sample(throughput);
            track_direction(&mut aimd, Direction::Up);
            check_oscillation(
                &mut aimd,
                manifest,
                current,
                min_threads,
                now,
                &thresholds,
            );
            sync_snapshot_with_manifest(&mut core);
            return;
        }

        // Probe did not yield throughput gain: rollback to pre-probe threads
        let rollback_target = pre_threads.max(min_threads);
        manifest.desired_thread_count = Some(rollback_target);
        manifest.updated_at_ms = now_ms();
        aimd.cooldown_until = Some(now + thresholds.cooldown * 2);
        aimd.settling_until = Some(now + thresholds.cooldown);
        aimd.last_throughput = Some(pre_throughput.max(throughput));
        aimd.stable_cycles = 0;
        aimd.consecutive_bad_samples = 0;
        aimd.recent_penalty = false;
        aimd.record_sample(throughput);
        track_direction(&mut aimd, Direction::Down);
        check_oscillation(
            &mut aimd,
            manifest,
            current,
            min_threads,
            now,
            &thresholds,
        );
        sync_snapshot_with_manifest(&mut core);
        return;
    }

    let throughput_drop = aimd
        .last_throughput
        .is_some_and(|last| last > 0.0 && throughput < last * (1.0 - thresholds.degrade));

    let is_severe_drop = aimd
        .last_throughput
        .is_some_and(|last| last > 0.0 && throughput < last * thresholds.severe_drop);

    let needed_bad_samples = if thresholds.is_expert {
        thresholds.samples_needed
    } else {
        2
    };

    let degradation_confirmed = if throughput_drop {
        aimd.consecutive_bad_samples = aimd.consecutive_bad_samples.saturating_add(1);
        aimd.stable_cycles = 0;
        is_severe_drop || aimd.consecutive_bad_samples >= needed_bad_samples || aimd.recent_penalty
    } else {
        aimd.consecutive_bad_samples = 0;
        false
    };

    let should_decrease = current > 1
        && match profile {
            AdaptiveProfile::Conservative => aimd.recent_penalty || degradation_confirmed,
            AdaptiveProfile::Balanced | AdaptiveProfile::Aggressive => degradation_confirmed,
        };

    if should_decrease {
        apply_decrease(
            &mut aimd,
            manifest,
            current,
            min_threads,
            now,
            throughput,
            &thresholds,
        );
        // `check_oscillation` may override the target with the hysteresis
        // lock value, so publish it after it ran.
        sync_snapshot_with_manifest(&mut core);
        return;
    }

    let changed_up = maybe_increase(
        &mut aimd,
        manifest,
        adaptive_cap,
        throughput,
        &thresholds,
        now,
    );

    if !changed_up {
        aimd.last_throughput = Some(throughput);
    }
    aimd.recent_penalty = false;
    aimd.record_sample(throughput);

    // ── Oscillation tracking (AI = Up) ──
    if changed_up {
        track_direction(&mut aimd, Direction::Up);
        check_oscillation(
            &mut aimd,
            manifest,
            current,
            min_threads,
            now,
            &thresholds,
        );
        sync_snapshot_with_manifest(&mut core);
    }
}

/// While a penalty cooldown is active, clear the penalty flag and skip the
/// decision; returns `true` when the caller must return early.
fn aimd_in_cooldown(aimd: &mut AimdState, now: Instant) -> bool {
    if let Some(cooldown_until) = aimd.cooldown_until
        && now < cooldown_until
    {
        aimd.recent_penalty = false;
        return true;
    }
    false
}

/// During the hysteresis lock, keep recording throughput and skip decisions;
/// once it expires, clear the lock. Returns `true` when the caller must return.
fn hysteresis_skips_decision(aimd: &mut AimdState, now: Instant, throughput: f64) -> bool {
    if let Some(lock_until) = aimd.hysteresis_lock_until {
        if now < lock_until {
            aimd.last_throughput = Some(throughput);
            aimd.record_sample(throughput);
            return true;
        }
        aimd.hysteresis_lock_until = None;
        aimd.oscillation_count = 0;
    }
    false
}

/// While a settling period is active, record throughput samples and skip decisions;
/// once it expires, establish a fresh throughput baseline.
fn aimd_in_settling(aimd: &mut AimdState, now: Instant, throughput: f64) -> bool {
    if let Some(settling_until) = aimd.settling_until {
        if now < settling_until {
            aimd.record_sample(throughput);
            return true;
        }
        aimd.settling_until = None;
        if aimd.last_throughput.is_none() && throughput > 0.0 {
            aimd.last_throughput = Some(throughput);
        }
    }
    false
}

/// Apply a decrease decision and run the Down-side oscillation tracking.
fn apply_decrease(
    aimd: &mut AimdState,
    manifest: &mut Manifest,
    current: usize,
    min_threads: usize,
    now: Instant,
    throughput: f64,
    thresholds: &AdaptiveThresholds,
) {
    let reduced = if thresholds.is_expert {
        ((current as f64) * thresholds.decrease_factor).ceil() as usize
    } else {
        aimd::reduce_threads(current, thresholds.profile, min_threads)
    };
    manifest.desired_thread_count = Some(reduced.max(min_threads.max(1)));
    manifest.updated_at_ms = now_ms();
    aimd.cooldown_until = Some(now + thresholds.cooldown);
    aimd.settling_until = Some(now + thresholds.cooldown);
    aimd.last_throughput = None;
    aimd.consecutive_good_samples = 0;
    aimd.consecutive_bad_samples = 0;
    aimd.stable_cycles = 0;
    aimd.is_probing_up = false;
    aimd.probe_pre_throughput = None;
    aimd.recent_penalty = false;
    aimd.record_sample(throughput);
    // ── Oscillation tracking (MD = Down) ──
    track_direction(aimd, Direction::Down);
    check_oscillation(aimd, manifest, current, min_threads, now, thresholds);
}

fn calculate_increase_step(current: usize, thresholds: &AdaptiveThresholds) -> usize {
    if thresholds.is_expert {
        if thresholds.step_ratio > 0.0 {
            ((current as f64) * thresholds.step_ratio).ceil() as usize
        } else {
            1usize
        }
    } else {
        match thresholds.profile {
            AdaptiveProfile::Conservative => 1usize,
            AdaptiveProfile::Balanced => (current / 4).max(1),
            AdaptiveProfile::Aggressive => (current / 3).max(1),
        }
    }
}

/// Apply the up-side AIMD step when allocation caught up with the target,
/// or initiate proactive probing when transfer is stable.
///
/// Returns `true` when the target was raised (drives the Up oscillation track).
fn maybe_increase(
    aimd: &mut AimdState,
    manifest: &mut Manifest,
    adaptive_cap: usize,
    throughput: f64,
    thresholds: &AdaptiveThresholds,
    now: Instant,
) -> bool {
    let current = manifest.desired_thread_count.unwrap_or(1).max(1);
    let allocated = manifest.allocated_thread_count.unwrap_or(0);

    if allocated != current {
        return false;
    }

    if current >= adaptive_cap {
        return false;
    }

    // Path 1: Throughput improved significantly over last throughput
    let throughput_improved = match aimd.last_throughput {
        Some(last) if last > 0.0 => throughput >= last * (1.0 + thresholds.increase),
        _ => false,
    };

    if throughput_improved {
        aimd.consecutive_good_samples = aimd.consecutive_good_samples.saturating_add(1);
        aimd.consecutive_bad_samples = 0;
        if aimd.consecutive_good_samples >= thresholds.samples_needed {
            let step = calculate_increase_step(current, thresholds);
            let next = (current + step).min(adaptive_cap.max(1));
            if next > current {
                manifest.desired_thread_count = Some(next);
                manifest.updated_at_ms = now_ms();
                aimd.settling_until = Some(now + thresholds.cooldown);
                aimd.last_throughput = None;
                aimd.consecutive_good_samples = 0;
                aimd.stable_cycles = 0;
                return true;
            }
        }
    } else {
        aimd.consecutive_good_samples = 0;
    }

    // Path 2: Anti-deadlock proactive probing when steady
    aimd.stable_cycles = aimd.stable_cycles.saturating_add(1);
    if aimd.stable_cycles >= thresholds.probe_stable_cycles {
        let step = calculate_increase_step(current, thresholds);
        let next = (current + step).min(adaptive_cap.max(1));
        if next > current {
            manifest.desired_thread_count = Some(next);
            manifest.updated_at_ms = now_ms();
            aimd.is_probing_up = true;
            aimd.probe_pre_throughput = Some(throughput);
            aimd.probe_pre_threads = current;
            aimd.settling_until = Some(now + thresholds.cooldown);
            aimd.last_throughput = None;
            aimd.stable_cycles = 0;
            aimd.consecutive_good_samples = 0;
            return true;
        }
    }

    false
}

// ── Rebalance helpers ────────────────────────────────────────────────────────

/// States where a rebalance pass must not hand out connections.
fn is_terminal_state(state: DownloadState) -> bool {
    matches!(
        state,
        DownloadState::Paused
            | DownloadState::Completed
            | DownloadState::Failed
            | DownloadState::Canceled
            | DownloadState::Verifying
    )
}

/// Put a task into the queued state (no allocation).
fn set_queued(manifest: &mut Manifest) {
    manifest.allocated_thread_count = Some(0);
    manifest.connection_count = 0;
    manifest.state = DownloadState::Queued;
}

/// Mark a task as actively downloading with `allocation` connections.
fn set_downloading(manifest: &mut Manifest, allocation: usize) {
    manifest.allocated_thread_count = Some(allocation);
    manifest.connection_count = allocation;
    manifest.state = DownloadState::Downloading;
}

/// Per-host connection accounting for one rebalance pass.
struct HostCapTracker {
    used: HashMap<String, usize>,
    max_per_host: usize,
}

impl HostCapTracker {
    fn new(max_per_host: usize) -> Self {
        Self {
            used: HashMap::default(),
            max_per_host,
        }
    }

    /// Cap `want` by the host's remaining connection budget and record the grant.
    fn grant(&mut self, manifest: &Manifest, want: usize) -> usize {
        let Some(host) = hostname_from_manifest(manifest) else {
            return want;
        };
        let used = self.used.get(&host).copied().unwrap_or(0);
        let remaining = self.max_per_host.saturating_sub(used);
        let capped = want.min(remaining);
        if capped > 0 {
            self.record(&host, capped);
        }
        capped
    }

    /// Record `count` additional connections for `host`.
    fn record(&mut self, host: &str, count: usize) {
        *self.used.entry(host.to_string()).or_insert(0) += count;
    }

    /// Connections already granted to `host`.
    fn used_by(&self, host: &str) -> usize {
        self.used.get(host).copied().unwrap_or(0)
    }
}

fn effective_max_connections_per_host(settings: &AppSettings) -> usize {
    if settings.scheduler.expert_mode {
        settings.scheduler.expert.max_connections_per_host
    } else {
        MAX_CONNECTIONS_PER_HOST
    }
}

/// Traditional mode: priority/age order, at most `max_parallel_tasks` running,
/// with at most `max_parallel_hdd` tasks concurrently accessing mechanical drives.
fn rebalance_traditional(
    entries: &mut [Arc<ManagedDownload>],
    settings: &AppSettings,
    disk_types: &HashMap<String, DiskType>,
    max_parallel_hdd: usize,
) {
    // Pre-snapshot the sort keys so the comparator does not take each
    // per-download lock twice per comparison.
    entries.sort_by_cached_key(|managed| {
        let core = managed.lock_core();
        (
            std::cmp::Reverse(core.manifest.priority as u8),
            core.manifest.created_at_ms,
        )
    });

    let mut running = 0usize;
    let mut running_hdd = 0usize;
    let mut host_caps = HostCapTracker::new(effective_max_connections_per_host(settings));
    for managed in entries {
        let mut core = managed.lock_core();
        let manifest = &mut core.manifest;
        if is_terminal_state(manifest.state) {
            manifest.allocated_thread_count = Some(0);
            manifest.connection_count = 0;
            sync_snapshot_with_manifest(&mut core);
            continue;
        }

        let is_hdd = disk_types
            .get(&manifest.id)
            .copied()
            .unwrap_or(DiskType::Ssd)
            == DiskType::Hdd;
        let hdd_allowed = !is_hdd || running_hdd < max_parallel_hdd;

        if running < settings.scheduler.traditional.max_parallel_tasks && hdd_allowed {
            let allocation = effective_allocation_cap(manifest, settings).max(1);
            let allocation = host_caps.grant(manifest, allocation);

            if allocation > 0 {
                set_downloading(manifest, allocation);
                running = running.saturating_add(1);
                if is_hdd {
                    running_hdd = running_hdd.saturating_add(1);
                }
            } else {
                set_queued(manifest);
            }
        } else {
            set_queued(manifest);
        }
        manifest.updated_at_ms = now_ms();
        sync_snapshot_with_manifest(&mut core);
    }
}

/// Automatic mode: order candidates by static task properties, then split
/// `max_parallel_threads` over them, respecting the media quota (`max_parallel_hdd`).
///
/// The ordering is deliberately **monotonic** (priority, total size, creation
/// time) and the allocation is **sticky**: a task that already holds threads
/// keeps them across passes.  A long-waiting task is only rotated in after
/// [`AUTOMATIC_ROTATION_PASSES`] passes, which bounds churn while preventing
/// starvation.  See [`AutomaticRebalanceState`] for why the old
/// remaining-bytes ordering churned.
fn rebalance_automatic(
    entries: &[Arc<ManagedDownload>],
    all_downloads: &[Arc<ManagedDownload>],
    settings: &AppSettings,
    state: &mut AutomaticRebalanceState,
    disk_types: &HashMap<String, DiskType>,
    max_parallel_hdd: usize,
) {
    let mut candidates = sort_automatic_candidates(entries, settings);
    order_and_rotate_candidates(&mut candidates, state, disk_types, max_parallel_hdd);

    let (mut allocations, remaining_budget, mut host_caps) = allocate_initial_minimums(
        &candidates,
        settings,
        settings.scheduler.automatic.max_parallel_threads,
        disk_types,
        max_parallel_hdd,
    );
    distribute_remaining_budget(
        &candidates,
        settings,
        &mut allocations,
        remaining_budget,
        &mut host_caps,
    );

    update_waiting_passes(&candidates, &allocations, state);

    for managed in all_downloads {
        let mut core = managed.lock_core();
        let manifest = &mut core.manifest;
        let allocation = allocations.get(&manifest.id).copied().unwrap_or(0);
        if is_terminal_state(manifest.state) {
            manifest.allocated_thread_count = Some(0);
            manifest.connection_count = 0;
        } else if allocation == 0 {
            set_queued(manifest);
        } else {
            manifest.allocated_thread_count = Some(allocation);
            manifest.connection_count = allocation;
            if manifest.state != DownloadState::Retrying {
                manifest.state = DownloadState::Downloading;
            }
        }
        manifest.updated_at_ms = now_ms();
        sync_snapshot_with_manifest(&mut core);
    }
}

/// Active candidates ordered according to the configured QueueSortStrategy.
///
/// Every key is a **static** task property (priority, total size, creation
/// time).  The previous implementation ordered on *remaining* bytes, which
/// changes on every progress update; that made the ordering—and therefore the
/// active set—unstable (see [`AutomaticRebalanceState`]).
fn sort_automatic_candidates(
    entries: &[Arc<ManagedDownload>],
    settings: &AppSettings,
) -> Vec<Arc<ManagedDownload>> {
    let strategy = if settings.scheduler.expert_mode {
        settings.scheduler.expert.queue_sort_strategy
    } else {
        QueueSortStrategy::PriorityFirst
    };

    let mut keyed: Vec<(u8, u64, u64, &Arc<ManagedDownload>)> = entries
        .iter()
        .filter(|managed| !is_terminal_state(managed.lock_core().manifest.state))
        .map(|managed| {
            let (total, priority, created_at_ms) = {
                let core = managed.lock_core();
                (
                    core.manifest.total_bytes.unwrap_or(0),
                    core.manifest.priority as u8,
                    core.manifest.created_at_ms,
                )
            };
            (priority, total, created_at_ms, managed)
        })
        .collect();

    match strategy {
        QueueSortStrategy::PriorityFirst => {
            // Priority descending, then total size descending, then creation order.
            keyed.sort_by(|(pa, ta, ca, _), (pb, tb, cb, _)| {
                pb.cmp(pa).then_with(|| tb.cmp(ta)).then_with(|| ca.cmp(cb))
            });
        }
        QueueSortStrategy::ShortestJobFirst => {
            // Total size ascending (job length), then priority descending, then creation order.
            keyed.sort_by(|(pa, ta, ca, _), (pb, tb, cb, _)| {
                ta.cmp(tb).then_with(|| pb.cmp(pa)).then_with(|| ca.cmp(cb))
            });
        }
        QueueSortStrategy::Fifo => {
            // Strict creation order ascending (FIFO), then priority descending.
            keyed.sort_by(|(pa, _, ca, _), (pb, _, cb, _)| ca.cmp(cb).then_with(|| pb.cmp(pa)));
        }
    }

    keyed
        .into_iter()
        .map(|(_, _, _, managed)| Arc::clone(managed))
        .collect()
}

/// Order candidates for the automatic allocator, keeping the running set
/// sticky and rotating a long-waiting task in only when needed.
///
/// The list is rebuilt from the static key every pass, so the *order* cannot
/// carry stickiness — the manifest's allocation does.  Incumbents (tasks that
/// held threads last pass) are therefore placed ahead of waiting tasks, which
/// keeps the active set stable regardless of how much progress it made.  Two
/// bounded exceptions can move a waiting task forward:
///
/// - **Priority preemption** — a waiting task that strictly outranks the
///   lowest-priority incumbent takes its slot.  Priority is static, so this is
///   event-driven, not a per-tick race.
/// - **Starvation rotation** — after [`AUTOMATIC_ROTATION_PASSES`] passes the
///   longest-waiting task is rotated in, replacing the lowest-ranked active
///   task.  This bounds starvation without reintroducing churn.
fn order_and_rotate_candidates(
    candidates: &mut Vec<Arc<ManagedDownload>>,
    state: &mut AutomaticRebalanceState,
    disk_types: &HashMap<String, DiskType>,
    max_parallel_hdd: usize,
) {
    let (mut active, mut waiting): (Vec<_>, Vec<_>) = candidates
        .drain(..)
        .partition(|managed| prior_allocation(managed) > 0);

    let active_hdd_count = active
        .iter()
        .filter(|m| {
            let id = m.lock_core().manifest.id.clone();
            disk_types.get(&id).copied().unwrap_or(DiskType::Ssd) == DiskType::Hdd
        })
        .count();

    // ── Priority preemption (static key, at most one swap per pass) ──
    if !active.is_empty()
        && !waiting.is_empty()
        && let Some(wait_pos) = waiting.iter().position(|w| {
            let w_id = w.lock_core().manifest.id.clone();
            let w_is_hdd =
                disk_types.get(&w_id).copied().unwrap_or(DiskType::Ssd) == DiskType::Hdd;
            let w_prio = priority_of(w);

            if w_is_hdd && active_hdd_count >= max_parallel_hdd {
                // If HDD quota is already full, an HDD candidate can only preempt an active HDD candidate
                active.iter().any(|a| {
                    let a_id = a.lock_core().manifest.id.clone();
                    disk_types.get(&a_id).copied().unwrap_or(DiskType::Ssd) == DiskType::Hdd
                        && w_prio > priority_of(a)
                })
            } else {
                active.iter().any(|a| w_prio > priority_of(a))
            }
        })
    {
        let w = &waiting[wait_pos];
        let w_id = w.lock_core().manifest.id.clone();
        let w_is_hdd =
            disk_types.get(&w_id).copied().unwrap_or(DiskType::Ssd) == DiskType::Hdd;
        let w_prio = priority_of(w);

        let victim_pos = if w_is_hdd && active_hdd_count >= max_parallel_hdd {
            // Victim must be the lowest-priority active HDD task
            active
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    let a_id = a.lock_core().manifest.id.clone();
                    disk_types.get(&a_id).copied().unwrap_or(DiskType::Ssd) == DiskType::Hdd
                        && w_prio > priority_of(a)
                })
                .min_by_key(|(_, a)| priority_of(a))
                .map(|(pos, _)| pos)
        } else {
            // Victim is the lowest-priority active task overall
            active
                .iter()
                .enumerate()
                .filter(|(_, a)| w_prio > priority_of(a))
                .min_by_key(|(_, a)| priority_of(a))
                .map(|(pos, _)| pos)
        };

        if let Some(v_pos) = victim_pos {
            let promoted = waiting.remove(wait_pos);
            let demoted = active.remove(v_pos);
            active.push(promoted);
            waiting.push(demoted);
        }
    }

    // ── Starvation rotation ──
    if active.is_empty() || waiting.is_empty() {
        // Nothing to rotate: keep the counter from banking a full period while
        // the budget is not contended, which would rotate immediately once it is.
        state.passes_since_rotation = state
            .passes_since_rotation
            .min(AUTOMATIC_ROTATION_PASSES.saturating_sub(1));
    } else {
        state.passes_since_rotation = state.passes_since_rotation.saturating_add(1);
        if state.passes_since_rotation >= AUTOMATIC_ROTATION_PASSES {
            let promote_pos = waiting
                .iter()
                .enumerate()
                .max_by_key(|(_, managed)| {
                    let id = managed.lock_core().manifest.id.clone();
                    state.waiting_passes.get(&id).copied().unwrap_or(0)
                })
                .map(|(pos, _)| pos)
                .expect("waiting is non-empty");

            let promoted_id = waiting[promote_pos].lock_core().manifest.id.clone();
            let promoted_is_hdd = disk_types
                .get(&promoted_id)
                .copied()
                .unwrap_or(DiskType::Ssd)
                == DiskType::Hdd;

            let demote_pos = if promoted_is_hdd && active_hdd_count >= max_parallel_hdd {
                // Demote the lowest-priority / last active HDD candidate
                active
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| {
                        let a_id = a.lock_core().manifest.id.clone();
                        disk_types.get(&a_id).copied().unwrap_or(DiskType::Ssd) == DiskType::Hdd
                    })
                    .min_by_key(|(_, a)| priority_of(a))
                    .map(|(pos, _)| pos)
            } else {
                None
            };

            let (promoted, demoted) = match demote_pos {
                Some(d_pos) => {
                    let promoted = waiting.remove(promote_pos);
                    let demoted = active.remove(d_pos);
                    (promoted, demoted)
                }
                None => {
                    let promoted = waiting.remove(promote_pos);
                    let demoted = active.pop().expect("active is non-empty");
                    (promoted, demoted)
                }
            };

            active.push(promoted);
            waiting.push(demoted);
            state.passes_since_rotation = 0;
        }
    }

    active.append(&mut waiting);
    *candidates = active;
}

/// The allocation the previous pass wrote for a task (0 when it never ran).
fn prior_allocation(managed: &Arc<ManagedDownload>) -> usize {
    managed
        .lock_core()
        .manifest
        .allocated_thread_count
        .unwrap_or(0)
}

/// Task priority as an ordered integer (higher wins).
fn priority_of(managed: &Arc<ManagedDownload>) -> u8 {
    managed.lock_core().manifest.priority as u8
}

/// Refresh per-task starvation counters from the allocation just computed.
///
/// Only waiting candidates are tracked, and the map is rebuilt each pass so
/// removed or newly active tasks cannot leak stale counters.
fn update_waiting_passes(
    candidates: &[Arc<ManagedDownload>],
    allocations: &HashMap<String, usize>,
    state: &mut AutomaticRebalanceState,
) {
    let mut waiting_passes: HashMap<String, u32> = HashMap::default();
    for managed in candidates {
        let id = managed.lock_core().manifest.id.clone();
        if allocations.get(&id).copied().unwrap_or(0) > 0 {
            continue;
        }
        let passes = state.waiting_passes.get(&id).copied().unwrap_or(0);
        waiting_passes.insert(id, passes.saturating_add(1));
    }
    state.waiting_passes = waiting_passes;
}

/// First pass: give every candidate its minimum share, capped per task, host, and media quota.
fn allocate_initial_minimums(
    candidates: &[Arc<ManagedDownload>],
    settings: &AppSettings,
    budget: usize,
    disk_types: &HashMap<String, DiskType>,
    max_parallel_hdd: usize,
) -> (HashMap<String, usize>, usize, HostCapTracker) {
    let min_per_task = settings.scheduler.automatic.min_threads_per_task.max(1);
    let mut allocations: HashMap<String, usize> = HashMap::default();
    let mut host_caps = HostCapTracker::new(effective_max_connections_per_host(settings));
    let mut remaining_budget = budget;
    let mut running_hdd = 0usize;

    for managed in candidates {
        let core = managed.lock_core();
        let id = core.manifest.id.clone();
        let is_hdd = disk_types.get(&id).copied().unwrap_or(DiskType::Ssd) == DiskType::Hdd;

        if remaining_budget == 0 || (is_hdd && running_hdd >= max_parallel_hdd) {
            allocations.insert(id, 0);
            continue;
        }

        let cap = effective_allocation_cap(&core.manifest, settings);
        let start = if remaining_budget >= min_per_task {
            min_per_task
        } else {
            remaining_budget
        }
        .min(cap);
        let start = host_caps.grant(&core.manifest, start);
        if start > 0 && is_hdd {
            running_hdd += 1;
        }
        allocations.insert(id, start);
        remaining_budget = remaining_budget.saturating_sub(start);
    }

    (allocations, remaining_budget, host_caps)
}

/// Second pass: hand the remaining budget out one thread at a time, round-robin.
fn distribute_remaining_budget(
    candidates: &[Arc<ManagedDownload>],
    settings: &AppSettings,
    allocations: &mut HashMap<String, usize>,
    mut remaining_budget: usize,
    host_caps: &mut HostCapTracker,
) {
    while remaining_budget > 0 {
        let mut granted = false;
        for managed in candidates {
            let core = managed.lock_core();
            let entry = allocations.entry(core.manifest.id.clone()).or_insert(0);
            // Only distribute remaining budget to candidates that already received
            // an initial allocation (> 0). Candidates that were capped at 0 (e.g. because
            // host cap or HDD quota was reached) must remain at 0.
            if *entry == 0 {
                continue;
            }
            let cap = effective_allocation_cap(&core.manifest, settings);

            let host = hostname_from_manifest(&core.manifest);
            let host_at_cap = host
                .as_ref()
                .is_some_and(|h| host_caps.used_by(h) >= host_caps.max_per_host);

            if *entry < cap && !host_at_cap {
                *entry += 1;
                remaining_budget -= 1;
                if let Some(ref h) = host {
                    host_caps.record(h, 1);
                }
                granted = true;
                if remaining_budget == 0 {
                    break;
                }
            }
        }

        if !granted {
            break;
        }
    }
}

/// Fail-soft batch persist of every non-terminal download's rebalanced manifest.
async fn persist_rebalanced(db: &Arc<Database>, all_downloads: &[Arc<ManagedDownload>]) {
    let active_list: Vec<_> = all_downloads
        .iter()
        .filter(|managed| {
            let state = managed.lock_core().manifest.state;
            state != DownloadState::Completed
                && state != DownloadState::Failed
                && state != DownloadState::Canceled
        })
        .cloned()
        .collect();

    if !active_list.is_empty()
        && let Err(error) = persist_manifest_snapshots_batch(db, &active_list).await
    {
        log_background_error("persist rebalanced manifest batch", &error);
        tracing::warn!(
            "batch persist failed for {} active download(s): {}",
            active_list.len(),
            error
        );
    }
}

#[cfg(test)]
mod tests;
