//! `CdnService` — unified CDN accelerator service for Tauri and NAS modes.
//!
//! Wraps [`CdnAccelerator`] and provides a stable API consumed by both
//! `commands_cdn.rs` (Tauri desktop) and `rpc.rs` (NAS WebSocket server).
//! This replaces direct `state.cdn_accelerator` access with a single
//! service abstraction.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::cdn::accelerator::{AccelState, CdnAccelerator};
use crate::cdn::ip_ranges::CdnIpCache;
use crate::cdn::speed_test::{CdnTestPhase, DefaultNodeResult, SpeedTestResult};
use crate::event_bus::{DownloadEvent, EventBus};
use crate::types::AppSettings;

/// Outcome of a completed CDN speed test (monitored via [`CdnService::monitor_test`]).
#[derive(Debug, Clone)]
pub struct CdnTestOutcome {
    pub state: AccelState,
    pub active_ip: Option<IpAddr>,
    pub active_speed_mbps: Option<f64>,
    pub candidates: Vec<SpeedTestResult>,
    pub default_node: Option<DefaultNodeResult>,
}

/// Unified service that wraps [`CdnAccelerator`] and provides a consistent API
/// for both Tauri desktop and NAS WebSocket frontends.
///
/// All methods delegate to the inner [`CdnAccelerator`] directly. The
/// `monitor_test` method additionally publishes progress/completion events
/// to an [`EventBus`] so that frontend adapters can relay them to the UI.
pub struct CdnService {
    accelerator: Arc<CdnAccelerator>,
}

impl Default for CdnService {
    fn default() -> Self {
        Self::new()
    }
}

impl CdnService {
    /// Create a new service with a fresh [`CdnAccelerator`].
    pub fn new() -> Self {
        Self {
            accelerator: Arc::new(CdnAccelerator::new()),
        }
    }

    /// Wrap an existing [`CdnAccelerator`].
    ///
    /// Useful when the accelerator must be shared with [`DownloadManager`]
    /// (via `set_cdn_accelerator`) and also used as a service.
    pub fn from_accelerator(accelerator: Arc<CdnAccelerator>) -> Self {
        Self { accelerator }
    }

    /// Return a reference to the inner accelerator (for `DownloadManager` etc.).
    pub fn accelerator(&self) -> &Arc<CdnAccelerator> {
        &self.accelerator
    }

    // ── Lifecycle methods ──────────────────────────────────────────────

    /// Kick off a CDN speed test in a background task.
    pub async fn start_test(self: &Arc<Self>, settings: AppSettings) -> anyhow::Result<()> {
        self.accelerator.start_test(settings).await
    }

    /// Cancel any running speed test.
    pub fn cancel_test(&self) {
        self.accelerator.cancel_test();
    }

    /// Build an accelerated client for the given IP and store it.
    pub async fn apply_ip(
        &self,
        ip: IpAddr,
        speed_mbps: f64,
        settings: &AppSettings,
    ) -> anyhow::Result<()> {
        self.accelerator.apply_ip(ip, speed_mbps, settings).await
    }

    /// Reset the accelerator to idle, dropping all state.
    pub async fn clear(&self) {
        self.accelerator.clear().await;
    }

    /// Restore accelerator state from persisted settings.
    pub async fn init_from_settings(self: &Arc<Self>, settings: &AppSettings) {
        self.accelerator.init_from_settings(settings).await;
    }

    // ── Status queries ────────────────────────────────────────────────

    pub async fn status(&self) -> AccelState {
        self.accelerator.status().await
    }

    pub async fn active_ip(&self) -> Option<IpAddr> {
        self.accelerator.active_ip().await
    }

    pub async fn active_speed_mbps(&self) -> Option<f64> {
        self.accelerator.active_speed_mbps().await
    }

    pub async fn phase(&self) -> Option<CdnTestPhase> {
        self.accelerator.phase().await
    }

    pub async fn phase_progress(&self) -> (u64, u64) {
        self.accelerator.phase_progress().await
    }

    pub async fn candidates(&self) -> Vec<SpeedTestResult> {
        self.accelerator.candidates().await
    }

    pub async fn default_node(&self) -> Option<DefaultNodeResult> {
        self.accelerator.default_node().await
    }

    /// Return the accelerated reqwest client (if state is Ready).
    pub async fn get_client(&self) -> Option<reqwest::Client> {
        self.accelerator.get_client().await
    }

    /// Return a clone of the IP range cache, or None if no test has run yet.
    pub async fn ip_cache(&self) -> Option<CdnIpCache> {
        self.accelerator.ip_cache().await
    }

    // ── Test monitoring ───────────────────────────────────────────────

    /// Poll the accelerator until a running test completes.
    ///
    /// Publishes [`DownloadEvent::CdnProgress`] during testing and
    /// [`DownloadEvent::CdnComplete`] when the test finishes (Ready or Error).
    ///
    /// This is the shared monitoring loop used by both Tauri and NAS handlers.
    /// Returns the final [`CdnTestOutcome`] for settings persistence by the caller.
    pub async fn monitor_test(self: &Arc<Self>, event_bus: Arc<EventBus>) -> CdnTestOutcome {
        let mut was_testing = false;

        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;

            let st = self.status().await;
            match st {
                AccelState::Testing => {
                    was_testing = true;
                    if let Some(phase) = self.phase().await {
                        let (current, total) = self.phase_progress().await;
                        let phase_str = match phase {
                            CdnTestPhase::FetchingRanges => "fetchingRanges",
                            CdnTestPhase::Screening => "screening",
                            CdnTestPhase::MeasuringThroughput => "measuringThroughput",
                        };
                        event_bus.publish(DownloadEvent::CdnProgress {
                            phase: phase_str.to_string(),
                            current,
                            total,
                        });
                    }
                }
                AccelState::Idle if was_testing => {
                    // Test ended without transitioning to Ready/Error
                    tracing::info!("cdn monitor: test ended without Ready/Error");
                    break;
                }
                AccelState::Ready | AccelState::Error(_) => {
                    let ip = self.active_ip().await;
                    let speed = self.active_speed_mbps().await;
                    let candidates = self.candidates().await;
                    let default_node = self.default_node().await;

                    let state_str = match &st {
                        AccelState::Ready => "Ready".to_string(),
                        AccelState::Error(msg) => format!("Error: {msg}"),
                        _ => unreachable!(),
                    };

                    event_bus.publish(DownloadEvent::CdnComplete {
                        state: state_str,
                        active_ip: ip.map(|i| i.to_string()),
                        active_speed_mbps: speed,
                    });

                    return CdnTestOutcome {
                        state: st,
                        active_ip: ip,
                        active_speed_mbps: speed,
                        candidates,
                        default_node,
                    };
                }
                _ => {}
            }
        }

        CdnTestOutcome {
            state: AccelState::Idle,
            active_ip: None,
            active_speed_mbps: None,
            candidates: Vec::new(),
            default_node: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    /// Smoke: basic construction and default state.
    #[tokio::test]
    async fn test_default_state() {
        let svc = CdnService::new();
        assert_eq!(svc.status().await, AccelState::Idle);
    }

    /// Verify `from_accelerator` shares the same inner object.
    #[tokio::test]
    async fn test_from_accelerator_sharing() {
        let acc = Arc::new(CdnAccelerator::new());
        let svc = CdnService::from_accelerator(acc.clone());

        // Mutate via service...
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let settings = AppSettings::default();
        svc.apply_ip(ip, 50.0, &settings).await.unwrap();

        // ...must be visible through the original Arc.
        assert_eq!(acc.active_ip().await, Some(ip));
    }
}
