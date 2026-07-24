#![allow(dead_code)]

use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::cdn::ip_ranges::{CdnIpCache, get_ip_ranges};
use crate::cdn::resolver::build_accelerated_client;
use crate::cdn::speed_test::{
    CdnTestPhase, DefaultNodeResult, SpeedTestConfig, SpeedTestResult, measure_default_node,
    run_speed_test,
};
use crate::types::AppSettings;

/// Lifecycle state of the CDN accelerator.
#[derive(Debug, Clone, PartialEq)]
pub enum AccelState {
    /// No active test, no accelerated client.
    Idle,
    /// Speed test is in progress (background task running).
    Testing,
    /// Test completed successfully — an accelerated client is available.
    Ready,
    /// Test failed with the given error message.
    Error(String),
}

/// Sentinel value for phase_atomic meaning "no active phase".
const PHASE_NONE: u8 = 0xFF;

/// State machine that manages the CDN acceleration lifecycle.
///
/// Callers wrap this in an `Arc<CdnAccelerator>` and pass it to methods that
/// spawn background tasks.
pub struct CdnAccelerator {
    state: RwLock<AccelState>,
    active_ip: RwLock<Option<IpAddr>>,
    active_speed_mbps: RwLock<Option<f64>>,
    cancel_token: RwLock<Option<CancellationToken>>,
    accelerated_client: RwLock<Option<reqwest::Client>>,
    /// Atomic phase indicator: 0=FetchingRanges, 1=Screening,
    /// 2=MeasuringThroughput, 0xFF=None. Written from sync progress
    /// callbacks without spawning.
    phase_atomic: AtomicU8,
    phase_progress_current: AtomicU64,
    phase_progress_total: AtomicU64,
    all_candidates: RwLock<Vec<SpeedTestResult>>,
    default_node: RwLock<Option<DefaultNodeResult>>,
    /// Shared IP range cache — populated by `start_test()` and available
    /// for `is_cloudflare_domain()` from `resolve_client()`.
    ip_cache: RwLock<Option<CdnIpCache>>,
}

impl Default for CdnAccelerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CdnAccelerator {
    /// Create a new accelerator in [`AccelState::Idle`].
    pub fn new() -> Self {
        Self {
            state: RwLock::new(AccelState::Idle),
            active_ip: RwLock::new(None),
            active_speed_mbps: RwLock::new(None),
            cancel_token: RwLock::new(None),
            accelerated_client: RwLock::new(None),
            phase_atomic: AtomicU8::new(PHASE_NONE),
            phase_progress_current: AtomicU64::new(0),
            phase_progress_total: AtomicU64::new(0),
            all_candidates: RwLock::new(Vec::new()),
            default_node: RwLock::new(None),
            ip_cache: RwLock::new(None),
        }
    }

    /// Kick off a CDN speed test in a background task.
    ///
    /// The method returns immediately after setting the state to
    /// [`AccelState::Testing`] and spawning the work.  If a test is already
    /// running the call is a no-op.
    ///
    /// When the background test finishes, the best candidate IP is applied via
    /// [`apply_ip`](Self::apply_ip) and the state moves to
    /// [`AccelState::Ready`].  On error the state moves to
    /// [`AccelState::Error`].
    pub async fn start_test(self: &Arc<Self>, settings: AppSettings) -> anyhow::Result<()> {
        {
            let mut state = self.state.write().await;
            if *state == AccelState::Testing {
                return Ok(()); // already running — idempotent
            }
            *state = AccelState::Testing;
        }

        // Reset stored results from any previous run.
        *self.active_ip.write().await = None;
        *self.active_speed_mbps.write().await = None;

        let token = CancellationToken::new();
        *self.cancel_token.write().await = Some(token.clone());

        let this = Arc::clone(self);

        tokio::spawn(async move {
            // ── Phase: FetchingRanges ────────────────────────────
            tracing::info!("cdn test: phase=FetchingRanges");
            this.phase_atomic.store(CdnTestPhase::FetchingRanges as u8, Ordering::Release);
            this.phase_progress_current.store(0, Ordering::Release);
            this.phase_progress_total.store(0, Ordering::Release);

            let ip_cache = Arc::new(tokio::sync::Mutex::new(CdnIpCache {
                ipv4_addrs: Vec::new(),
                ipv6_addrs: Vec::new(),
                ipv4_cidrs: Vec::new(),
                ipv6_cidrs: Vec::new(),
                fetched_at: Instant::now(),
                from_fallback: true,
            }));
            let range_data = get_ip_ranges(ip_cache, token.child_token()).await;

            // Check cancellation before starting the heavy work.
            if token.is_cancelled() {
                tracing::info!("cdn test: cancelled during FetchingRanges");
                *this.state.write().await = AccelState::Idle;
                this.phase_atomic.store(PHASE_NONE, Ordering::Release);
                this.phase_progress_current.store(0, Ordering::Release);
                this.phase_progress_total.store(0, Ordering::Release);
                return;
            }

            // Store the fetched cache for use by is_cloudflare_domain().
            *this.ip_cache.write().await = Some(range_data.clone());

            let ips = range_data.all_addrs();
            tracing::info!(
                "cdn test: got {} IPs (v4={} v6={} fallback={})",
                ips.len(),
                range_data.ipv4_addrs.len(),
                range_data.ipv6_addrs.len(),
                range_data.from_fallback,
            );

            if ips.is_empty() {
                tracing::error!("cdn test: no Cloudflare IPs available");
                *this.state.write().await = AccelState::Error("no Cloudflare IPs available".into());
                this.phase_atomic.store(PHASE_NONE, Ordering::Release);
                this.phase_progress_current.store(0, Ordering::Release);
                this.phase_progress_total.store(0, Ordering::Release);
                return;
            }

            // ── Phase: Screening → MeasuringThroughput ──────────
            tracing::info!(
                "cdn test: phase=Screening → MeasuringThroughput, {} IPs",
                ips.len()
            );
            let config = SpeedTestConfig::default();
            let acc_ref = Arc::clone(&this);
            let progress_cb: crate::cdn::speed_test::ProgressFn =
                Box::new(move |phase, current, total| {
                    // Direct atomic stores — no tokio::spawn needed since
                    // the callback is already called from a sync context.
                    acc_ref
                        .phase_atomic
                        .store(phase as u8, Ordering::Release);
                    acc_ref
                        .phase_progress_current
                        .store(current, Ordering::Release);
                    acc_ref
                        .phase_progress_total
                        .store(total, Ordering::Release);
                });

            let (results, default_node) = tokio::select! {
                _ = token.cancelled() => {
                    tracing::info!("cdn test: cancelled during speed test");
                    *this.state.write().await = AccelState::Idle;
                    this.phase_atomic.store(PHASE_NONE, Ordering::Release);
                    this.phase_progress_current.store(0, Ordering::Release);
                    this.phase_progress_total.store(0, Ordering::Release);
                    return;
                }
                r = async {
                    tokio::join!(
                        run_speed_test(&ips, &config, &settings, Some(progress_cb)),
                        measure_default_node(&settings),
                    )
                } => r,
            };

            if token.is_cancelled() {
                tracing::info!("cdn test: cancelled after speed test completed");
                *this.state.write().await = AccelState::Idle;
                this.phase_atomic.store(PHASE_NONE, Ordering::Release);
                this.phase_progress_current.store(0, Ordering::Release);
                this.phase_progress_total.store(0, Ordering::Release);
                return;
            }

            *this.all_candidates.write().await = results.clone();
            *this.default_node.write().await = Some(default_node.clone());

            let best = results.into_iter().next();

            match best {
                Some(result) => {
                    let speed = result.throughput_mbps.unwrap_or(0.0);

                    let should_fallback = if let Some(dn_speed) = default_node.throughput_mbps {
                        let best_worse = result.throughput_mbps.is_none_or(|s| s < dn_speed);
                        if best_worse {
                            tracing::info!(
                                "cdn test: fallback — best candidate {speed:.2} MB/s < default {dn_speed:.2} MB/s, keeping default routing",
                            );
                        }
                        best_worse
                    } else {
                        false
                    };

                    if should_fallback {
                        let dn_speed = default_node.throughput_mbps.unwrap_or(0.0);
                        *this.state.write().await = AccelState::Error(format!(
                            "best candidate {speed:.2} MB/s is not faster than default node {dn_speed:.2} MB/s — keeping default routing"
                        ));
                    } else {
                        if result.throughput_mbps.is_some() {
                            tracing::info!(
                                "cdn test: best candidate ip={} speed={speed:.2} MB/s latency={:.1}ms (selected by throughput)",
                                result.ip,
                                result.tcp_latency_ms,
                            );
                        } else {
                            tracing::info!(
                                "cdn test: best candidate ip={} latency={:.1}ms (throughput test failed for all, selected by latency fallback)",
                                result.ip,
                                result.tcp_latency_ms,
                            );
                        }
                        match this.apply_ip(result.ip, speed, &settings).await {
                            Ok(()) => {
                                tracing::info!("cdn test: state=Ready, accelerated client built");
                            }
                            Err(e) => {
                                tracing::error!(
                                    "cdn test: failed to build accelerated client: {e}"
                                );
                                *this.state.write().await = AccelState::Error(format!(
                                    "failed to build accelerated client: {e}"
                                ));
                            }
                        }
                    }
                }
                None => {
                    tracing::error!(
                        "cdn test: no candidates at all — screening produced zero reachable IPs"
                    );
                    *this.state.write().await =
                        AccelState::Error("no reachable CDN IPs — all candidates failed".into());
                }
            }

            this.phase_atomic.store(PHASE_NONE, Ordering::Release);
            this.phase_progress_current.store(0, Ordering::Release);
            this.phase_progress_total.store(0, Ordering::Release);
        });

        Ok(())
    }

    /// Cancel any running speed test and set state back to [`AccelState::Idle`].
    ///
    /// If no test is active this is a no-op.  Uses [`RwLock::try_write`] so it
    /// is safe to call from within an async runtime (e.g. a `#[tokio::test]` or
    /// Tauri command).  If the lock cannot be acquired immediately the
    /// background task will still observe the cancellation and clean up.
    pub fn cancel_test(&self) {
        if let Ok(mut guard) = self.cancel_token.try_write()
            && let Some(token) = guard.take()
        {
            token.cancel();
        }
        if let Ok(mut guard) = self.state.try_write() {
            *guard = AccelState::Idle;
        }
    }

    /// Return a clone of the current accelerator state.
    pub async fn status(&self) -> AccelState {
        self.state.read().await.clone()
    }

    /// Build an accelerated `reqwest::Client` for the given IP, store it, and
    /// move the state to [`AccelState::Ready`].
    pub async fn apply_ip(
        &self,
        ip: IpAddr,
        speed_mbps: f64,
        settings: &AppSettings,
    ) -> anyhow::Result<()> {
        let client = build_accelerated_client("speed.cloudflare.com", ip, settings)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        *self.accelerated_client.write().await = Some(client);
        *self.active_ip.write().await = Some(ip);
        *self.active_speed_mbps.write().await = Some(speed_mbps);
        *self.state.write().await = AccelState::Ready;

        Ok(())
    }

    /// Restore the accelerator state from persisted settings.
    ///
    /// Called at startup to re-apply the previously selected CDN IP so that
    /// downloads can use acceleration immediately without re-running the speed test.
    pub async fn init_from_settings(self: &Arc<Self>, settings: &AppSettings) {
        let cdn = &settings.cdn_acceleration;
        if !cdn.enabled {
            return;
        }
        let Some(ip_str) = cdn.active_ip.as_deref() else {
            return;
        };
        let Ok(ip) = ip_str.parse::<IpAddr>() else {
            tracing::warn!("cdn init: invalid persisted IP '{ip_str}', clearing");
            return;
        };
        let speed = cdn.active_speed_mbps.unwrap_or(0.0);
        match self.apply_ip(ip, speed, settings).await {
            Ok(()) => {
                tracing::info!(
                    "cdn init: restored active IP {ip} at {speed:.2} MB/s from settings"
                );
            }
            Err(e) => {
                tracing::warn!("cdn init: failed to restore active IP {ip}: {e}");
            }
        }
    }

    /// Reset the accelerator to [`AccelState::Idle`], dropping all stored data
    /// including the accelerated client.
    pub async fn clear(&self) {
        *self.state.write().await = AccelState::Idle;
        *self.active_ip.write().await = None;
        *self.active_speed_mbps.write().await = None;
        *self.cancel_token.write().await = None;
        *self.accelerated_client.write().await = None;
        self.phase_atomic.store(PHASE_NONE, Ordering::Release);
        self.phase_progress_current.store(0, Ordering::Release);
        self.phase_progress_total.store(0, Ordering::Release);
        *self.all_candidates.write().await = Vec::new();
        *self.default_node.write().await = None;
    }

    /// Return a clone of the accelerated client if the state is
    /// [`AccelState::Ready`], otherwise `None`.
    pub async fn get_client(&self) -> Option<reqwest::Client> {
        self.accelerated_client.read().await.clone()
    }

    /// Return the active accelerated IP, or None if not set.
    pub async fn active_ip(&self) -> Option<IpAddr> {
        *self.active_ip.read().await
    }

    /// Return the measured throughput in MB/s, or None if not set.
    pub async fn active_speed_mbps(&self) -> Option<f64> {
        *self.active_speed_mbps.read().await
    }

    /// Return the current test phase, or None if no test is active.
    pub async fn phase(&self) -> Option<CdnTestPhase> {
        let p = self.phase_atomic.load(Ordering::Acquire);
        match p {
            0 => Some(CdnTestPhase::FetchingRanges),
            1 => Some(CdnTestPhase::Screening),
            2 => Some(CdnTestPhase::MeasuringThroughput),
            _ => None,
        }
    }

    /// Return the current phase progress as (current, total).
    pub async fn phase_progress(&self) -> (u64, u64) {
        let current = self.phase_progress_current.load(Ordering::Acquire);
        let total = self.phase_progress_total.load(Ordering::Acquire);
        (current, total)
    }

    /// Return all candidates from the most recent speed test.
    pub async fn candidates(&self) -> Vec<SpeedTestResult> {
        self.all_candidates.read().await.clone()
    }

    pub async fn default_node(&self) -> Option<DefaultNodeResult> {
        self.default_node.read().await.clone()
    }

    /// Return a clone of the IP range cache, or None if no test has run yet.
    ///
    /// Used by [`crate::cdn::resolver::is_cloudflare_domain`] to check
    /// Cloudflare CIDR membership with live (rather than static) data.
    pub async fn ip_cache(&self) -> Option<CdnIpCache> {
        self.ip_cache.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    /// Smoke: verify initial state and basic lifecycle via `apply_ip` / `clear`.
    #[tokio::test]
    async fn test_lifecycle() {
        let acc = Arc::new(CdnAccelerator::new());

        // ── Idle ─────────────────────────────────────────────────
        assert_eq!(acc.status().await, AccelState::Idle);
        assert!(acc.get_client().await.is_none());
        assert!(acc.phase().await.is_none());
        assert_eq!(acc.phase_progress().await, (0, 0));
        assert!(acc.candidates().await.is_empty());
        assert!(acc.ip_cache().await.is_none());

        // ── Apply IP → Ready ─────────────────────────────────────
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let settings = AppSettings::default();
        acc.apply_ip(ip, 100.0, &settings).await.unwrap();

        assert_eq!(acc.status().await, AccelState::Ready);
        assert_eq!(*acc.active_ip.read().await, Some(ip));
        assert_eq!(*acc.active_speed_mbps.read().await, Some(100.0));
        assert!(acc.get_client().await.is_some());

        // ── Clear → Idle ─────────────────────────────────────────
        acc.clear().await;
        assert_eq!(acc.status().await, AccelState::Idle);
        assert!(acc.get_client().await.is_none());
        assert!(acc.active_ip.read().await.is_none());
        assert!(acc.phase().await.is_none());
        assert_eq!(acc.phase_progress().await, (0, 0));
        assert!(acc.candidates().await.is_empty());
    }

    /// Verify that `cancel_test` moves the state to Idle and the background
    /// task respects the cancellation token.
    #[tokio::test]
    async fn test_cancel() {
        let acc = Arc::new(CdnAccelerator::new());
        let settings = AppSettings::default();

        acc.start_test(settings).await.unwrap();
        assert_eq!(acc.status().await, AccelState::Testing);

        acc.cancel_test();
        assert_eq!(acc.status().await, AccelState::Idle);

        // Double-cancel is safe.
        acc.cancel_test();
        assert_eq!(acc.status().await, AccelState::Idle);
    }

    /// After `clear()` the accelerated client must be dropped (clone returns
    /// `None`).
    #[tokio::test]
    async fn test_clear_drops_client() {
        let acc = Arc::new(CdnAccelerator::new());
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let settings = AppSettings::default();

        acc.apply_ip(ip, 50.0, &settings).await.unwrap();
        let client = acc.get_client().await;
        assert!(client.is_some(), "client must exist after apply_ip");

        acc.clear().await;

        assert_eq!(acc.status().await, AccelState::Idle);
        assert!(
            acc.get_client().await.is_none(),
            "client must be None after clear"
        );
        assert!(acc.active_ip.read().await.is_none());
        assert!(acc.active_speed_mbps.read().await.is_none());
        assert!(acc.phase().await.is_none());
        assert_eq!(acc.phase_progress().await, (0, 0));
        assert!(acc.candidates().await.is_empty());
    }

    /// `candidates()` returns empty vec initially and after clear.
    #[tokio::test]
    async fn test_candidates_storage() {
        let acc = Arc::new(CdnAccelerator::new());

        // Initially empty.
        assert!(acc.candidates().await.is_empty());

        // Simulate storing a result by writing directly.
        let result = SpeedTestResult {
            ip: IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
            tcp_latency_ms: 5.0,
            throughput_mbps: Some(100.0),
            error: None,
        };
        *acc.all_candidates.write().await = vec![result.clone()];

        let stored = acc.candidates().await;
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].ip, IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)));
        assert_eq!(stored[0].tcp_latency_ms, 5.0);
        assert_eq!(stored[0].throughput_mbps, Some(100.0));

        // Clear resets candidates.
        acc.clear().await;
        assert!(acc.candidates().await.is_empty());
    }
}
