//! Scheduler and rebalance logic — extracted from manager.rs
//! (Phase 5 of the manager.rs split).
//!
//! Contains the background scheduler loop and adaptive AIMD thread rebalancing.
//!
//! `Scheduler` is an independent actor type.  All its methods receive a
//! `&DownloadManager` or `Arc<DownloadManager>` parameter to access shared
//! state, avoiding any ownership cycle with `DownloadManager` (which holds
//! `Arc<Scheduler>`).

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use foldhash::HashMap;
use reqwest::Url;

use crate::{
    aimd::{self, AimdState, Direction},
    database::Database,
    download::{
        DEFAULT_FIXED_THREADS, MAX_TRADITIONAL_THREADS, ManagedDownload, log_background_error,
        sync_snapshot_with_manifest,
    },
    error::Result,
    manager::DownloadManager,
    manifest::Manifest,
    now_ms,
    persistence::persist_manifest_snapshots_batch,
    types::{AdaptiveProfile, AppSettings, DownloadState, SchedulerMode, ThreadMode},
};

const SCHEDULER_TICK: Duration = Duration::from_secs(2);

/// Maximum concurrent connections (threads) allowed to a single hostname.
const MAX_CONNECTIONS_PER_HOST: usize = 6;

/// Zero-sized actor type for scheduler and rebalance logic.
///
/// All methods receive `&DownloadManager` to access shared state.
/// `DownloadManager` holds `Arc<Scheduler>` for delegation.
pub struct Scheduler;

impl Scheduler {
    /// Start the background scheduler loop (750ms tick or rebalance_notify).
    /// Consumes `self: Arc<Self>` to keep the scheduler alive, and
    /// `dm: Arc<DownloadManager>` for the spawned task.
    pub fn start_scheduler_loop(self: Arc<Self>, dm: Arc<DownloadManager>) {
        tokio::spawn(async move {
            let mut consecutive_rebalance_failures: u32 = 0;
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(SCHEDULER_TICK) => {}
                    _ = dm.controls.rebalance_notify.notified() => {}
                    _ = dm.controls.shutdown_token.cancelled() => {
                        tracing::info!("Scheduler loop shutting down");
                        break;
                    }
                }

                self.apply_speed_limit_schedule(&dm).await;

                if let Err(error) = self.update_adaptive_targets(&dm).await {
                    log_background_error("update adaptive targets", &error);
                }
                match self.rebalance_allocations(&dm).await {
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
    pub async fn update_adaptive_targets(&self, dm: &DownloadManager) -> Result<()> {
        let settings = dm.settings_service.get().await;
        if settings.scheduler.mode != SchedulerMode::Automatic {
            return Ok(());
        }

        let adaptive_cap = settings.scheduler.automatic.max_threads_per_task.max(1);
        let min_threads = settings.scheduler.automatic.min_threads_per_task.max(1);

        let downloads = dm.downloads.read().await;

        // ── Overclock mode: pin all adaptive tasks at max threads ──────────
        if dm.concurrency.overclock_mode() {
            pin_overclock_targets(downloads.values(), adaptive_cap);
            return Ok(());
        }

        for managed in downloads.values() {
            update_one_adaptive(managed, &settings, adaptive_cap, min_threads);
        }

        Ok(())
    }

    /// Rebalance thread allocations across all active downloads.
    pub async fn rebalance_allocations(&self, dm: &DownloadManager) -> Result<()> {
        let settings = dm.settings_service.get().await;

        // Phase 1: collect all Arc references under the read lock, then drop it
        let (mut entries, all_downloads) = {
            let guard = dm.downloads.read().await;
            let entries: Vec<_> = guard.values().cloned().collect();
            let all_downloads = entries.clone();
            (entries, all_downloads)
        };

        match settings.scheduler.mode {
            SchedulerMode::Traditional => rebalance_traditional(&mut entries, &settings),
            SchedulerMode::Automatic => rebalance_automatic(&entries, &all_downloads, &settings),
        }

        persist_rebalanced(&dm.db, &all_downloads).await;
        Ok(())
    }

    /// Check the speed limit schedule and apply the matching limit (or revert to global default).
    async fn apply_speed_limit_schedule(&self, dm: &DownloadManager) {
        let settings = dm.settings_service.get().await;
        let schedule = &settings.speed_limit_schedule;
        if schedule.is_empty() {
            return;
        }

        // Get current local hour using the `time` crate
        let current_hour = match time::OffsetDateTime::now_local() {
            Ok(now) => now.hour(),
            Err(e) => {
                tracing::warn!("Failed to determine local time, falling back to global speed limit: {e}");
                dm.rate_limiter.set_rate(settings.global_speed_limit_bps);
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
            Some(bps) => dm.rate_limiter.set_rate(bps),
            None => {
                // Re-read settings to get global_speed_limit_bps (schedule may have changed)
                let settings = dm.settings_service.get().await;
                dm.rate_limiter.set_rate(settings.global_speed_limit_bps);
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

/// Returns the number of bytes remaining to download.
fn remaining_bytes(manifest: &Manifest) -> u64 {
    manifest
        .total_bytes
        .unwrap_or(manifest.downloaded_bytes)
        .saturating_sub(manifest.downloaded_bytes)
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
    cooldown: &Duration,
) {
    const OSCILLATION_THRESHOLD: u32 = 3;
    if aimd.oscillation_count >= OSCILLATION_THRESHOLD {
        let lock_target = ((current as f64) * 0.7).ceil() as usize;
        manifest.desired_thread_count = Some(lock_target.max(min_threads));
        manifest.updated_at_ms = now_ms();
        aimd.hysteresis_lock_until = Some(now + *cooldown * 4);
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
    samples_needed: u32,
    cooldown: Duration,
    probe_stable_cycles: u32,
}

impl AdaptiveThresholds {
    fn for_profile(profile: AdaptiveProfile) -> Self {
        let (degrade, increase, samples_needed, probe_stable_cycles) = match profile {
            AdaptiveProfile::Conservative => (0.18, 0.08, 2, 3),
            AdaptiveProfile::Balanced => (0.16, 0.04, 1, 2),
            AdaptiveProfile::Aggressive => (0.20, 0.03, 1, 2),
        };
        Self {
            profile,
            degrade,
            increase,
            samples_needed,
            cooldown: aimd::cooldown_for_profile(profile),
            probe_stable_cycles,
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

    let thresholds = AdaptiveThresholds::for_profile(profile);

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
                &thresholds.cooldown,
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
            &thresholds.cooldown,
        );
        sync_snapshot_with_manifest(&mut core);
        return;
    }

    let throughput_drop = aimd
        .last_throughput
        .is_some_and(|last| last > 0.0 && throughput < last * (1.0 - thresholds.degrade));

    let is_severe_drop = aimd
        .last_throughput
        .is_some_and(|last| last > 0.0 && throughput < last * 0.50);

    let degradation_confirmed = if throughput_drop {
        aimd.consecutive_bad_samples = aimd.consecutive_bad_samples.saturating_add(1);
        aimd.stable_cycles = 0;
        is_severe_drop || aimd.consecutive_bad_samples >= 2 || aimd.recent_penalty
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
            &thresholds.cooldown,
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
    manifest.desired_thread_count =
        Some(aimd::reduce_threads(current, thresholds.profile, min_threads));
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
    check_oscillation(aimd, manifest, current, min_threads, now, &thresholds.cooldown);
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
            let step = match thresholds.profile {
                AdaptiveProfile::Conservative => 1usize,
                AdaptiveProfile::Balanced => (current / 4).max(1),
                AdaptiveProfile::Aggressive => (current / 3).max(1),
            };
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
        let step = match thresholds.profile {
            AdaptiveProfile::Conservative => 1usize,
            AdaptiveProfile::Balanced => (current / 4).max(1),
            AdaptiveProfile::Aggressive => (current / 3).max(1),
        };
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
#[derive(Default)]
struct HostCapTracker {
    used: HashMap<String, usize>,
}

impl HostCapTracker {
    /// Cap `want` by the host's remaining connection budget and record the grant.
    fn grant(&mut self, manifest: &Manifest, want: usize) -> usize {
        let Some(host) = hostname_from_manifest(manifest) else {
            return want;
        };
        let used = self.used.get(&host).copied().unwrap_or(0);
        let remaining = MAX_CONNECTIONS_PER_HOST.saturating_sub(used);
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

/// Traditional mode: priority/age order, at most `max_parallel_tasks` running.
fn rebalance_traditional(entries: &mut [Arc<ManagedDownload>], settings: &AppSettings) {
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
    let mut host_caps = HostCapTracker::default();
    for managed in entries {
        let mut core = managed.lock_core();
        let manifest = &mut core.manifest;
        if is_terminal_state(manifest.state) {
            manifest.allocated_thread_count = Some(0);
            manifest.connection_count = 0;
            sync_snapshot_with_manifest(&mut core);
            continue;
        }

        if running < settings.scheduler.traditional.max_parallel_tasks {
            let allocation = effective_allocation_cap(manifest, settings).max(1);
            let allocation = host_caps.grant(manifest, allocation);

            if allocation > 0 {
                set_downloading(manifest, allocation);
                running = running.saturating_add(1);
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

/// Automatic mode: sort candidates, then split `max_parallel_threads` over them.
fn rebalance_automatic(
    entries: &[Arc<ManagedDownload>],
    all_downloads: &[Arc<ManagedDownload>],
    settings: &AppSettings,
) {
    let candidates = sort_automatic_candidates(entries);

    let (mut allocations, remaining_budget, mut host_caps) = allocate_initial_minimums(
        &candidates,
        settings,
        settings.scheduler.automatic.max_parallel_threads,
    );
    distribute_remaining_budget(
        &candidates,
        settings,
        &mut allocations,
        remaining_budget,
        &mut host_caps,
    );

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

/// Active candidates ordered by priority (desc) then remaining bytes (desc).
fn sort_automatic_candidates(entries: &[Arc<ManagedDownload>]) -> Vec<Arc<ManagedDownload>> {
    let mut keyed: Vec<(u8, u64, &Arc<ManagedDownload>)> = entries
        .iter()
        .filter(|managed| !is_terminal_state(managed.lock_core().manifest.state))
        .map(|managed| {
            let (remaining, priority) = {
                let core = managed.lock_core();
                (remaining_bytes(&core.manifest), core.manifest.priority as u8)
            };
            (priority, remaining, managed)
        })
        .collect();
    // Priority descending first, then remaining bytes descending.
    keyed.sort_by(|(pa, ra, _), (pb, rb, _)| pb.cmp(pa).then_with(|| rb.cmp(ra)));
    keyed
        .into_iter()
        .map(|(_, _, managed)| Arc::clone(managed))
        .collect()
}

/// First pass: give every candidate its minimum share, capped per task and host.
fn allocate_initial_minimums(
    candidates: &[Arc<ManagedDownload>],
    settings: &AppSettings,
    budget: usize,
) -> (HashMap<String, usize>, usize, HostCapTracker) {
    let min_per_task = settings.scheduler.automatic.min_threads_per_task.max(1);
    let mut allocations: HashMap<String, usize> = HashMap::default();
    let mut host_caps = HostCapTracker::default();
    let mut remaining_budget = budget;

    for managed in candidates {
        let core = managed.lock_core();
        if remaining_budget == 0 {
            allocations.insert(core.manifest.id.clone(), 0);
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
        allocations.insert(core.manifest.id.clone(), start);
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
            let cap = effective_allocation_cap(&core.manifest, settings);

            let host = hostname_from_manifest(&core.manifest);
            let host_at_cap = host
                .as_ref()
                .is_some_and(|h| host_caps.used_by(h) >= MAX_CONNECTIONS_PER_HOST);

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
