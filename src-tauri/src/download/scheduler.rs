//! Scheduler, rebalance, and network-learning logic — extracted from manager.rs
//! (Phase 5 of the manager.rs split).
//!
//! Contains the background scheduler loop, adaptive AIMD thread rebalancing,
//! and network learning logic that records throughput metrics from completed downloads.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Context;

use super::{
    aimd::{self, AimdState},
    error::Result,
    manager::{
        DEFAULT_FIXED_THREADS, DownloadManager, MAX_TRADITIONAL_THREADS, ManagedDownload,
        log_background_error, now_ms, sync_snapshot_with_manifest,
    },
    manifest::Manifest,
    persistence::persist_manifest_snapshot,
    settings::normalize_learning_metrics,
    types::{
        AdaptiveProfile, AppSettings, DeviceLearningMode, DownloadState, NetworkLearningMetrics,
        NetworkLearningSettings, ProxyMode, SchedulerMode, ThreadMode,
    },
};

const SCHEDULER_TICK: Duration = Duration::from_secs(2);

// ── DownloadManager scheduler/rebalance methods ──────────────────────────────

impl DownloadManager {
    pub(crate) fn start_scheduler_loop(self: Arc<Self>) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(SCHEDULER_TICK) => {}
                    _ = self.rebalance_notify.notified() => {}
                }

                if let Err(error) = self.update_adaptive_targets().await {
                    log_background_error("update adaptive targets", &error);
                }
                if let Err(error) = self.rebalance_allocations().await {
                    log_background_error("rebalance allocations", &error);
                }
            }
        });
    }

    pub(crate) async fn update_adaptive_targets(&self) -> Result<()> {
        let settings = self.settings.read().await.clone();
        if settings.scheduler.mode != SchedulerMode::Automatic {
            return Ok(());
        }

        if settings.proxy.mode != ProxyMode::Disabled {
            return Ok(());
        }

        let learning_metrics = active_learning_metrics(&settings.network_learning);
        let adaptive_cap = active_scene_thread_cap(&settings.network_learning)
            .unwrap_or(settings.scheduler.automatic.max_threads_per_task.max(1))
            .min(settings.scheduler.automatic.max_threads_per_task.max(1));
        let min_threads = settings.scheduler.automatic.min_threads_per_task.max(1);

        let downloads = self.downloads.read().await;
        for managed in downloads.values() {
            let mut core = managed.lock_core();
            let manifest = &mut core.manifest;
            if manifest.thread_mode != ThreadMode::Adaptive
                || manifest.state != DownloadState::Downloading
                || !manifest.supports_ranges
            {
                continue;
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

            if let Some(cooldown_until) = aimd.cooldown_until
                && now < cooldown_until
            {
                aimd.recent_penalty = false;
                continue;
            }

            let mut degrade_threshold: f64 = match profile {
                AdaptiveProfile::Conservative => 0.18,
                AdaptiveProfile::Balanced => 0.16,
                AdaptiveProfile::Aggressive => 0.20,
            };
            let mut increase_threshold: f64 = match profile {
                AdaptiveProfile::Conservative => 0.08,
                AdaptiveProfile::Balanced => 0.04,
                AdaptiveProfile::Aggressive => 0.0,
            };
            let mut samples_needed: u32 = match profile {
                AdaptiveProfile::Conservative => 2,
                AdaptiveProfile::Balanced => 1,
                AdaptiveProfile::Aggressive => 1,
            };
            let mut cooldown = aimd::cooldown_for_profile(profile);

            if let Some(metrics) = learning_metrics {
                if metrics.penalty_rate >= 0.2 || metrics.stability_score <= 0.6 {
                    degrade_threshold *= 0.7;
                    increase_threshold += 0.05;
                    samples_needed = samples_needed.saturating_add(1);
                    cooldown += Duration::from_secs(2);
                } else if metrics.stability_score >= 0.88
                    && metrics.penalty_rate <= 0.06
                    && metrics.estimated_bandwidth_bps >= 6.0 * 1024.0 * 1024.0
                {
                    degrade_threshold += 0.03;
                    increase_threshold = (increase_threshold * 0.5).max(0.0);
                    samples_needed = samples_needed.saturating_sub(1).max(1);
                    cooldown = cooldown.saturating_sub(Duration::from_secs(1));
                }
            }

            degrade_threshold = match profile {
                AdaptiveProfile::Conservative => degrade_threshold,
                AdaptiveProfile::Balanced => degrade_threshold.max(0.16),
                AdaptiveProfile::Aggressive => degrade_threshold.max(0.20),
            };

            let throughput_drop = aimd
                .last_throughput
                .is_some_and(|last| last > 0.0 && throughput < last * (1.0 - degrade_threshold));
            let should_decrease = current > 1
                && match profile {
                    AdaptiveProfile::Conservative => aimd.recent_penalty || throughput_drop,
                    AdaptiveProfile::Balanced | AdaptiveProfile::Aggressive => throughput_drop,
                };

            if should_decrease {
                manifest.desired_thread_count =
                    Some(aimd::reduce_threads(current, profile, min_threads));
                manifest.updated_at_ms = now_ms();
                aimd.cooldown_until = Some(now + cooldown);
                aimd.consecutive_good_samples = 0;
                aimd.consecutive_bad_samples = aimd.consecutive_bad_samples.saturating_add(1);
                aimd.recent_penalty = false;
                aimd.record_sample(throughput);
                continue;
            }

            if allocated == current {
                let improved = match aimd.last_throughput {
                    Some(last) if last > 0.0 => throughput >= last * (1.0 + increase_threshold),
                    _ => true,
                };

                if improved {
                    aimd.consecutive_good_samples = aimd.consecutive_good_samples.saturating_add(1);
                    aimd.consecutive_bad_samples = 0;
                    if aimd.consecutive_good_samples >= samples_needed {
                        let next = (current + 1).min(adaptive_cap.max(1));
                        manifest.desired_thread_count = Some(next);
                        manifest.updated_at_ms = now_ms();
                        aimd.consecutive_good_samples = 0;
                    }
                }
            }

            aimd.last_throughput = Some(throughput);
            aimd.recent_penalty = false;
            aimd.record_sample(throughput);
        }

        Ok(())
    }

    pub(crate) async fn learn_from_download(&self, managed: Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        if manifest.thread_mode != ThreadMode::Adaptive {
            return Ok(());
        }

        let settings = self.settings.read().await.clone();
        if settings.scheduler.mode != SchedulerMode::Automatic
            || settings.network_learning.device_mode == DeviceLearningMode::Mobile
            || settings.proxy.mode != ProxyMode::Disabled
        {
            return Ok(());
        }

        let Some(scene) = settings.network_learning.scenes.first() else {
            return Ok(());
        };
        if !scene.learning_enabled {
            return Ok(());
        }

        let sample = {
            let aimd = managed.lock_aimd();
            build_learning_sample(&manifest, &aimd, &settings)?
        };
        let alpha = match settings.network_learning.device_mode {
            DeviceLearningMode::Fixed => 0.45,
            DeviceLearningMode::SemiMobile => 0.22,
            DeviceLearningMode::Mobile => 0.0,
        };

        let scheduler_cap = settings.scheduler.automatic.max_threads_per_task.max(1);
        let now = now_ms();
        let mut next_settings = settings.clone();
        let scene = &mut next_settings.network_learning.scenes[0];
        let next_metrics =
            blend_learning_metrics(scene.learned_metrics.as_ref(), sample, alpha, scheduler_cap);
        scene.learned_metrics = Some(next_metrics.clone());
        scene.updated_at_ms = now;

        let db = self.db.clone();
        let scene_id = scene.id.clone();
        tokio::task::spawn_blocking(move || db.upsert_learning_metrics(&scene_id, &next_metrics))
            .await
            .context("learn metrics persist task panicked")?
            .context("failed to persist learning metrics")?;

        *self.settings.write().await = next_settings;
        Ok(())
    }

    pub(crate) async fn rebalance_allocations(&self) -> Result<()> {
        let settings = self.settings.read().await.clone();

        // Phase 1: collect all Arc references under the read lock, then drop it
        // so that writers (download_start/cancel/remove) are not blocked during
        // the rebalance computation and DB persistence (Phase 2).
        let (mut entries, all_downloads) = {
            let guard = self.downloads.read().await;
            let entries: Vec<_> = guard.values().cloned().collect();
            let all_downloads = entries.clone();
            (entries, all_downloads)
        };
        // Read lock is released — Phase 2 can proceed without blocking writers.

        match settings.scheduler.mode {
            SchedulerMode::Traditional => {
                entries.sort_by_key(|managed| managed.lock_core().manifest.created_at_ms);

                let mut running = 0usize;
                for managed in entries {
                    let mut core = managed.lock_core();
                    let manifest = &mut core.manifest;
                    let terminal = matches!(
                        manifest.state,
                        DownloadState::Paused
                            | DownloadState::Completed
                            | DownloadState::Failed
                            | DownloadState::Canceled
                            | DownloadState::Verifying
                    );
                    if terminal {
                        manifest.allocated_thread_count = Some(0);
                        manifest.connection_count = 0;
                        sync_snapshot_with_manifest(&mut core);
                        continue;
                    }

                    if running < settings.scheduler.traditional.max_parallel_tasks {
                        let allocation = effective_allocation_cap(manifest, &settings).max(1);
                        manifest.allocated_thread_count = Some(allocation);
                        manifest.connection_count = allocation;
                        manifest.state = DownloadState::Downloading;
                        running = running.saturating_add(1);
                    } else {
                        manifest.allocated_thread_count = Some(0);
                        manifest.connection_count = 0;
                        manifest.state = DownloadState::Queued;
                    }
                    manifest.updated_at_ms = now_ms();
                    sync_snapshot_with_manifest(&mut core);
                }
            }
            SchedulerMode::Automatic => {
                let mut candidates = entries
                    .into_iter()
                    .filter(|managed| {
                        let core = managed.lock_core();
                        !matches!(
                            core.manifest.state,
                            DownloadState::Paused
                                | DownloadState::Completed
                                | DownloadState::Failed
                                | DownloadState::Canceled
                                | DownloadState::Verifying
                        )
                    })
                    .collect::<Vec<_>>();

                candidates.sort_by(|left, right| {
                    remaining_bytes(&right.lock_core().manifest)
                        .cmp(&remaining_bytes(&left.lock_core().manifest))
                });

                let mut remaining_budget = settings.scheduler.automatic.max_parallel_threads;
                let min_per_task = settings.scheduler.automatic.min_threads_per_task.max(1);
                let mut allocations: HashMap<String, usize> = HashMap::new();
                for managed in &candidates {
                    let core = managed.lock_core();
                    if remaining_budget == 0 {
                        allocations.insert(core.manifest.id.clone(), 0);
                        continue;
                    }
                    let start = if remaining_budget >= min_per_task {
                        min_per_task
                    } else {
                        remaining_budget
                    };
                    allocations.insert(core.manifest.id.clone(), start);
                    remaining_budget = remaining_budget.saturating_sub(start);
                }

                while remaining_budget > 0 {
                    let mut granted = false;
                    for managed in &candidates {
                        let core = managed.lock_core();
                        let entry = allocations.entry(core.manifest.id.clone()).or_insert(0);
                        let cap = effective_allocation_cap(&core.manifest, &settings);
                        if *entry < cap {
                            *entry += 1;
                            remaining_budget -= 1;
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

                for managed in &all_downloads {
                    let mut core = managed.lock_core();
                    let manifest = &mut core.manifest;
                    let allocation = allocations.get(&manifest.id).copied().unwrap_or(0);
                    if matches!(
                        manifest.state,
                        DownloadState::Paused
                            | DownloadState::Completed
                            | DownloadState::Failed
                            | DownloadState::Canceled
                            | DownloadState::Verifying
                    ) {
                        manifest.allocated_thread_count = Some(0);
                        manifest.connection_count = 0;
                    } else if allocation == 0 {
                        manifest.allocated_thread_count = Some(0);
                        manifest.connection_count = 0;
                        manifest.state = DownloadState::Queued;
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
        }

        let mut first_error = None;
        for managed in &all_downloads {
            if let Err(error) = persist_manifest_snapshot(&self.db, managed).await {
                log_background_error("persist rebalanced manifest", &error);
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error.into());
        }
        Ok(())
    }
}

// ── Scheduler helper functions ───────────────────────────────────────────────

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
    active_scene_thread_cap(&settings.network_learning)
        .unwrap_or(settings.scheduler.automatic.max_threads_per_task.max(1))
        .min(settings.scheduler.automatic.max_threads_per_task.max(1))
        .max(1)
}

/// Returns the current active learning metrics, if any.
pub(crate) fn active_learning_metrics(
    settings: &NetworkLearningSettings,
) -> Option<&NetworkLearningMetrics> {
    if settings.device_mode == DeviceLearningMode::Mobile {
        return None;
    }

    settings
        .scenes
        .first()
        .filter(|scene| scene.learning_enabled)
        .and_then(|scene| scene.learned_metrics.as_ref())
}

/// Returns the recommended thread cap from active learning metrics, if any.
pub(crate) fn active_scene_thread_cap(settings: &NetworkLearningSettings) -> Option<usize> {
    active_learning_metrics(settings).map(|metrics| metrics.recommended_max_threads_per_task_cap)
}

/// Builds a learning sample from completed download data.
fn build_learning_sample(
    manifest: &Manifest,
    aimd: &AimdState,
    settings: &AppSettings,
) -> Result<NetworkLearningMetrics> {
    let profile = manifest
        .adaptive_profile_snapshot
        .unwrap_or(settings.scheduler.automatic.adaptive_profile);
    let scheduler_cap = settings.scheduler.automatic.max_threads_per_task.max(1);
    let throughput = if aimd.throughput_sample_count > 0 {
        (aimd.throughput_sum / f64::from(aimd.throughput_sample_count)).max(0.0_f64)
    } else {
        0.0_f64
    };
    let penalty_rate = if aimd.throughput_sample_count > 0 {
        f64::from(aimd.penalty_count) / f64::from(aimd.throughput_sample_count.max(1))
    } else if aimd.penalty_count > 0 {
        1.0_f64
    } else {
        0.0_f64
    };

    let mut stability_score = (1.0 - penalty_rate * 1.35).clamp(0.0, 1.0);
    stability_score = match manifest.state {
        DownloadState::Completed => (stability_score + 0.08).clamp(0.0, 1.0),
        DownloadState::Failed => (stability_score - 0.3).clamp(0.0, 1.0),
        DownloadState::Paused => (stability_score - 0.08).clamp(0.0, 1.0),
        _ => stability_score,
    };

    let base_threads = recommended_threads_from_bandwidth(throughput, profile);
    let recommended_initial_threads =
        adjust_threads_for_stability(base_threads, stability_score, penalty_rate)
            .clamp(1, scheduler_cap);
    let recommended_max_threads_per_task_cap = derive_recommended_cap(
        recommended_initial_threads,
        stability_score,
        penalty_rate,
        scheduler_cap,
    );

    Ok(NetworkLearningMetrics {
        estimated_bandwidth_bps: throughput,
        stability_score,
        penalty_rate,
        recommended_initial_threads,
        recommended_max_threads_per_task_cap,
        sample_count: 1,
        last_observed_at_ms: now_ms(),
    })
}

/// Blends a new learning sample with previous metrics using exponential moving average.
fn blend_learning_metrics(
    previous: Option<&NetworkLearningMetrics>,
    sample: NetworkLearningMetrics,
    alpha: f64,
    scheduler_cap: usize,
) -> NetworkLearningMetrics {
    let Some(previous) = previous else {
        return normalize_learning_metrics(sample, scheduler_cap);
    };

    let alpha = alpha.clamp(0.05, 0.95);
    let blended_initial = ((previous.recommended_initial_threads as f64) * (1.0 - alpha)
        + (sample.recommended_initial_threads as f64) * alpha)
        .round() as usize;
    let blended_cap = ((previous.recommended_max_threads_per_task_cap as f64) * (1.0 - alpha)
        + (sample.recommended_max_threads_per_task_cap as f64) * alpha)
        .round() as usize;

    normalize_learning_metrics(
        NetworkLearningMetrics {
            estimated_bandwidth_bps: previous.estimated_bandwidth_bps * (1.0 - alpha)
                + sample.estimated_bandwidth_bps * alpha,
            stability_score: previous.stability_score * (1.0 - alpha)
                + sample.stability_score * alpha,
            penalty_rate: previous.penalty_rate * (1.0 - alpha) + sample.penalty_rate * alpha,
            recommended_initial_threads: blended_initial,
            recommended_max_threads_per_task_cap: blended_cap.max(blended_initial),
            sample_count: previous.sample_count.saturating_add(1),
            last_observed_at_ms: sample.last_observed_at_ms,
        },
        scheduler_cap,
    )
}

fn recommended_threads_from_bandwidth(throughput: f64, profile: AdaptiveProfile) -> usize {
    if throughput <= 0.0 {
        return aimd::initial_desired_threads(profile);
    }

    match throughput {
        value if value < 1.0 * 1024.0 * 1024.0 => 1,
        value if value < 4.0 * 1024.0 * 1024.0 => 2,
        value if value < 12.0 * 1024.0 * 1024.0 => 4,
        _ => 6,
    }
}

fn adjust_threads_for_stability(threads: usize, stability_score: f64, penalty_rate: f64) -> usize {
    if penalty_rate >= 0.25 || stability_score <= 0.55 {
        return threads.saturating_sub(1).max(1);
    }
    if penalty_rate <= 0.05 && stability_score >= 0.9 {
        return threads.saturating_add(1);
    }
    threads
}

fn derive_recommended_cap(
    recommended_initial_threads: usize,
    stability_score: f64,
    penalty_rate: f64,
    scheduler_cap: usize,
) -> usize {
    let extra = if penalty_rate <= 0.05 && stability_score >= 0.9 {
        3
    } else if penalty_rate <= 0.12 && stability_score >= 0.75 {
        2
    } else {
        1
    };

    recommended_initial_threads
        .saturating_add(extra)
        .clamp(recommended_initial_threads, scheduler_cap)
}
