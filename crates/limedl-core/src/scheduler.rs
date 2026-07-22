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
    aimd,
    error::Result,
    manager::{
        DEFAULT_FIXED_THREADS, DownloadManager, MAX_TRADITIONAL_THREADS, log_background_error,
        sync_snapshot_with_manifest,
    },
    manifest::Manifest,
    now_ms,
    persistence::persist_manifest_snapshot,
    types::{AdaptiveProfile, AppSettings, DownloadState, ProxyMode, SchedulerMode, ThreadMode},
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
    /// Start the background scheduler loop (2s tick or rebalance_notify).
    /// Consumes `self: Arc<Self>` to keep the scheduler alive, and
    /// `dm: Arc<DownloadManager>` for the spawned task.
    pub fn start_scheduler_loop(self: Arc<Self>, dm: Arc<DownloadManager>) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(SCHEDULER_TICK) => {}
                    _ = dm.controls.rebalance_notify.notified() => {}
                    _ = dm.controls.shutdown_token.cancelled() => {
                        tracing::info!("Scheduler loop shutting down");
                        break;
                    }
                }

                if let Err(error) = self.update_adaptive_targets(&dm).await {
                    log_background_error("update adaptive targets", &error);
                }
                if let Err(error) = self.rebalance_allocations(&dm).await {
                    log_background_error("rebalance allocations", &error);
                }
            }
        });
    }

    /// Update adaptive (AIMD) thread targets for all active downloads.
    pub async fn update_adaptive_targets(&self, dm: &DownloadManager) -> Result<()> {
        let settings = dm.settings.read().await.clone();
        if settings.scheduler.mode != SchedulerMode::Automatic {
            return Ok(());
        }

        if settings.proxy.mode != ProxyMode::Disabled {
            return Ok(());
        }

        let adaptive_cap = settings.scheduler.automatic.max_threads_per_task.max(1);
        let min_threads = settings.scheduler.automatic.min_threads_per_task.max(1);

        let downloads = dm.downloads.read().await;

        // ── Overclock mode: pin all adaptive tasks at max threads ──────────
        if dm.overclock_mode() {
            for managed in downloads.values() {
                let mut core = managed.lock_core();
                let manifest = &mut core.manifest;
                if manifest.thread_mode != ThreadMode::Adaptive
                    || manifest.state != DownloadState::Downloading
                    || !manifest.supports_ranges
                {
                    continue;
                }
                if manifest.desired_thread_count != Some(adaptive_cap) {
                    manifest.desired_thread_count = Some(adaptive_cap);
                    manifest.updated_at_ms = now_ms();
                    sync_snapshot_with_manifest(&mut core);
                }
            }
            return Ok(());
        }

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
            let increase_threshold: f64 = match profile {
                AdaptiveProfile::Conservative => 0.08,
                AdaptiveProfile::Balanced => 0.04,
                AdaptiveProfile::Aggressive => 0.03,
            };
            let samples_needed: u32 = match profile {
                AdaptiveProfile::Conservative => 2,
                AdaptiveProfile::Balanced => 1,
                AdaptiveProfile::Aggressive => 1,
            };
            let cooldown = aimd::cooldown_for_profile(profile);

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

    /// Rebalance thread allocations across all active downloads.
    pub async fn rebalance_allocations(&self, dm: &DownloadManager) -> Result<()> {
        let settings = dm.settings.read().await.clone();

        // Phase 1: collect all Arc references under the read lock, then drop it
        let (mut entries, all_downloads) = {
            let guard = dm.downloads.read().await;
            let entries: Vec<_> = guard.values().cloned().collect();
            let all_downloads = entries.clone();
            (entries, all_downloads)
        };

        match settings.scheduler.mode {
            SchedulerMode::Traditional => {
                entries.sort_by_key(|managed| managed.lock_core().manifest.created_at_ms);

                let mut running = 0usize;
                let mut host_threads: HashMap<String, usize> = HashMap::default();
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
                        // Apply per-host connection cap
                        let allocation = if let Some(host) = hostname_from_manifest(manifest) {
                            let used = host_threads.get(&host).copied().unwrap_or(0);
                            let remaining = MAX_CONNECTIONS_PER_HOST.saturating_sub(used);
                            let capped = allocation.min(remaining);
                            if capped > 0 {
                                host_threads.insert(host, used + capped);
                            }
                            capped
                        } else {
                            allocation
                        };

                        if allocation > 0 {
                            manifest.allocated_thread_count = Some(allocation);
                            manifest.connection_count = allocation;
                            manifest.state = DownloadState::Downloading;
                            running = running.saturating_add(1);
                        } else {
                            manifest.allocated_thread_count = Some(0);
                            manifest.connection_count = 0;
                            manifest.state = DownloadState::Queued;
                        }
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
                let candidates = entries
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

                // Pre-snapshot remaining bytes to avoid repeated lock
                // acquisitions during sort (each sort_by callback would
                // lock both candidates otherwise). Snapshot inside a
                // block so the MutexGuard is dropped before `m` moves.
                let mut with_remaining: Vec<(u64, _)> = candidates
                    .into_iter()
                    .map(|m| {
                        let remaining = {
                            let core = m.lock_core();
                            remaining_bytes(&core.manifest)
                        };
                        (remaining, m)
                    })
                    .collect();
                with_remaining.sort_by_key(|(r, _)| *r);
                // Reverse to get descending order (largest remaining first),
                // matching the original right.cmp(left) ordering.
                let candidates = with_remaining
                    .into_iter()
                    .rev()
                    .map(|(_, m)| m)
                    .collect::<Vec<_>>();

                let mut remaining_budget = settings.scheduler.automatic.max_parallel_threads;
                let min_per_task = settings.scheduler.automatic.min_threads_per_task.max(1);
                let mut allocations: HashMap<String, usize> = HashMap::default();
                let mut host_threads: HashMap<String, usize> = HashMap::default();

                // Initial minimum allocation with per-host cap
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
                    // Apply per-host connection cap
                    let start = if let Some(host) = hostname_from_manifest(&core.manifest) {
                        let used = host_threads.get(&host).copied().unwrap_or(0);
                        let remaining = MAX_CONNECTIONS_PER_HOST.saturating_sub(used);
                        let capped = start.min(remaining);
                        if capped > 0 {
                            host_threads.insert(host, used + capped);
                        }
                        capped
                    } else {
                        start
                    };
                    allocations.insert(core.manifest.id.clone(), start);
                    remaining_budget = remaining_budget.saturating_sub(start);
                }

                // Round-robin additional allocation with per-host cap
                while remaining_budget > 0 {
                    let mut granted = false;
                    for managed in &candidates {
                        let core = managed.lock_core();
                        let entry = allocations.entry(core.manifest.id.clone()).or_insert(0);
                        let cap = effective_allocation_cap(&core.manifest, &settings);

                        // Check per-host cap
                        let host = hostname_from_manifest(&core.manifest);
                        let host_at_cap = host.as_ref().is_some_and(|h| {
                            host_threads.get(h).copied().unwrap_or(0) >= MAX_CONNECTIONS_PER_HOST
                        });

                        if *entry < cap && !host_at_cap {
                            *entry += 1;
                            remaining_budget -= 1;
                            if let Some(ref h) = host {
                                *host_threads.entry(h.clone()).or_insert(0) += 1;
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
            if let Err(error) = persist_manifest_snapshot(&dm.db, managed).await {
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

#[cfg(test)]
mod tests {
    use crate::manifest::Manifest;
    use crate::manager::{DEFAULT_FIXED_THREADS, MAX_TRADITIONAL_THREADS};
    use crate::types::{
        AppSettings, AutomaticSchedulerSettings, DownloadState, SchedulerMode,
        SchedulerSettings, ThreadMode, TraditionalSchedulerSettings,
    };

    // ── helpers ───────────────────────────────────────────────────────────

    fn manifest(url: &str) -> Manifest {
        Manifest {
            id: String::new(),
            url: url.to_string(),
            final_url: url.to_string(),
            user_agent: String::new(),
            destination_dir: String::new(),
            file_name: String::new(),
            file_name_locked: false,
            destination_path: String::new(),
            temp_path: String::new(),
            total_bytes: None,
            downloaded_bytes: 0,
            supports_ranges: true,
            chunk_size: 4194304,
            connection_count: 0,
            thread_mode: ThreadMode::Adaptive,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile_snapshot: None,
            thread_note: None,
            etag: None,
            last_modified: None,
            state: DownloadState::Queued,
            cdn_accelerated: false,
            checksum_mode: crate::types::ChecksumMode::None,
            checksum: None,
            expected_checksum: None,
            error: None,
            created_at_ms: 0,
            updated_at_ms: 0,
            chunks: vec![],
            mirror_url: None,
            mirror_urls: Vec::new(),
            current_mirror_index: 0,
        }
    }

    fn settings_traditional() -> AppSettings {
        AppSettings {
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Traditional,
                traditional: TraditionalSchedulerSettings {
                    max_parallel_tasks: 3,
                },
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        }
    }

    fn settings_automatic() -> AppSettings {
        AppSettings {
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Automatic,
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        }
    }

    // ── effective_allocation_cap ──────────────────────────────────────────

    #[test]
    fn cap_no_ranges_returns_one() {
        let mut m = manifest("https://example.com/file.bin");
        m.supports_ranges = false;
        assert_eq!(super::effective_allocation_cap(&m, &settings_traditional()), 1);
        assert_eq!(super::effective_allocation_cap(&m, &settings_automatic()), 1);
    }

    #[test]
    fn cap_traditional_uses_requested_threads() {
        let mut m = manifest("https://example.com/file.bin");
        m.requested_thread_count = Some(12);
        assert_eq!(super::effective_allocation_cap(&m, &settings_traditional()), 12);
    }

    #[test]
    fn cap_traditional_falls_back_to_desired() {
        let mut m = manifest("https://example.com/file.bin");
        m.requested_thread_count = None;
        m.desired_thread_count = Some(6);
        assert_eq!(super::effective_allocation_cap(&m, &settings_traditional()), 6);
    }

    #[test]
    fn cap_traditional_default_when_none_set() {
        let m = manifest("https://example.com/file.bin");
        assert_eq!(
            super::effective_allocation_cap(&m, &settings_traditional()),
            DEFAULT_FIXED_THREADS
        );
    }

    #[test]
    fn cap_traditional_clamps_to_max() {
        let mut m = manifest("https://example.com/file.bin");
        m.requested_thread_count = Some(99);
        assert_eq!(
            super::effective_allocation_cap(&m, &settings_traditional()),
            MAX_TRADITIONAL_THREADS
        );
    }

    #[test]
    fn cap_traditional_clamps_to_min() {
        let mut m = manifest("https://example.com/file.bin");
        m.requested_thread_count = Some(0);
        assert_eq!(super::effective_allocation_cap(&m, &settings_traditional()), 1);
    }

    #[test]
    fn cap_automatic_fixed_uses_requested() {
        let mut m = manifest("https://example.com/file.bin");
        m.thread_mode = ThreadMode::Fixed;
        m.requested_thread_count = Some(5);
        let s = settings_automatic();
        assert_eq!(super::effective_allocation_cap(&m, &s), 5);
    }

    #[test]
    fn cap_automatic_fixed_defaults_to_one() {
        let mut m = manifest("https://example.com/file.bin");
        m.thread_mode = ThreadMode::Fixed;
        m.requested_thread_count = None;
        let s = settings_automatic();
        assert_eq!(super::effective_allocation_cap(&m, &s), 1);
    }

    #[test]
    fn cap_automatic_adaptive_uses_desired() {
        let mut m = manifest("https://example.com/file.bin");
        m.thread_mode = ThreadMode::Adaptive;
        m.desired_thread_count = Some(4);
        let s = settings_automatic();
        assert_eq!(super::effective_allocation_cap(&m, &s), 4);
    }

    #[test]
    fn cap_automatic_adaptive_defaults_to_one() {
        let mut m = manifest("https://example.com/file.bin");
        m.thread_mode = ThreadMode::Adaptive;
        m.desired_thread_count = None;
        let s = settings_automatic();
        assert_eq!(super::effective_allocation_cap(&m, &s), 1);
    }

    #[test]
    fn cap_automatic_clamps_to_effective_task_cap() {
        let mut m = manifest("https://example.com/file.bin");
        m.thread_mode = ThreadMode::Adaptive;
        m.desired_thread_count = Some(99);
        let s = AppSettings {
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Automatic,
                automatic: AutomaticSchedulerSettings {
                    max_threads_per_task: 3,
                    ..AutomaticSchedulerSettings::default()
                },
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        };
        assert_eq!(super::effective_allocation_cap(&m, &s), 3);
    }

    // ── hostname_from_manifest ────────────────────────────────────────────

    #[test]
    fn hostname_valid_url_returns_host() {
        let m = manifest("https://cdn.example.com/path/file.zip");
        assert_eq!(super::hostname_from_manifest(&m), Some("cdn.example.com".to_string()));
    }

    #[test]
    fn hostname_domain_with_port() {
        let m = manifest("http://localhost:8080/download/file.iso");
        assert_eq!(super::hostname_from_manifest(&m), Some("localhost".to_string()));
    }

    #[test]
    fn hostname_no_host_returns_none() {
        let m = manifest("file:///C:/path/to/file.txt");
        // file:// URIs may have no host_str
        assert_eq!(super::hostname_from_manifest(&m), None);
    }

    #[test]
    fn hostname_invalid_url_does_not_panic() {
        // Completely unparseable
        let m = manifest("\0invalid url\t\n");
        let result = super::hostname_from_manifest(&m);
        assert!(result.is_none());
    }

    #[test]
    fn hostname_empty_string_does_not_panic() {
        let m = manifest("");
        let result = super::hostname_from_manifest(&m);
        assert!(result.is_none());
    }

    // ── remaining_bytes ───────────────────────────────────────────────────

    #[test]
    fn remaining_normal_case() {
        let mut m = manifest("https://example.com/file.bin");
        m.total_bytes = Some(1000);
        m.downloaded_bytes = 300;
        assert_eq!(super::remaining_bytes(&m), 700);
    }

    #[test]
    fn remaining_no_total_bytes_returns_zero() {
        let mut m = manifest("https://example.com/file.bin");
        m.total_bytes = None;
        m.downloaded_bytes = 500;
        assert_eq!(super::remaining_bytes(&m), 0);
    }

    #[test]
    fn remaining_total_less_than_downloaded_returns_zero() {
        let mut m = manifest("https://example.com/file.bin");
        m.total_bytes = Some(100);
        m.downloaded_bytes = 500;
        assert_eq!(super::remaining_bytes(&m), 0);
    }

    #[test]
    fn remaining_equal_values_returns_zero() {
        let mut m = manifest("https://example.com/file.bin");
        m.total_bytes = Some(500);
        m.downloaded_bytes = 500;
        assert_eq!(super::remaining_bytes(&m), 0);
    }

    #[test]
    fn remaining_zero_total_and_downloaded() {
        let mut m = manifest("https://example.com/file.bin");
        m.total_bytes = Some(0);
        m.downloaded_bytes = 0;
        assert_eq!(super::remaining_bytes(&m), 0);
    }

    // ── effective_automatic_task_cap ──────────────────────────────────────

    #[test]
    fn task_cap_normal() {
        let s = AppSettings {
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Automatic,
                automatic: AutomaticSchedulerSettings {
                    max_threads_per_task: 8,
                    ..AutomaticSchedulerSettings::default()
                },
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        };
        assert_eq!(super::effective_automatic_task_cap(&s), 8);
    }

    #[test]
    fn task_cap_zero_clamps_to_one() {
        let s = AppSettings {
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Automatic,
                automatic: AutomaticSchedulerSettings {
                    max_threads_per_task: 0,
                    ..AutomaticSchedulerSettings::default()
                },
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        };
        assert_eq!(super::effective_automatic_task_cap(&s), 1);
    }

    #[test]
    fn task_cap_default_is_at_least_one() {
        let s = AppSettings {
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Automatic,
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        };
        // Default max_threads_per_task is 8, so cap is 8
        assert_eq!(super::effective_automatic_task_cap(&s), 8);
    }
}
