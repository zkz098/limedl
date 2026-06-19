#![allow(dead_code)]

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::download::cdn::ip_ranges::{get_ip_ranges, IpRangesCache};
use crate::download::cdn::resolver::build_accelerated_client;
use crate::download::cdn::speed_test::{run_speed_test, SpeedTestConfig};
use crate::download::types::AppSettings;

/// Lifecycle state of the CDN accelerator.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AccelState {
    /// No active test, no accelerated client.
    Idle,
    /// Speed test is in progress (background task running).
    Testing,
    /// Test completed successfully — an accelerated client is available.
    Ready,
    /// Test failed with the given error message.
    Error(String),
}

/// State machine that manages the CDN acceleration lifecycle.
///
/// Callers wrap this in an `Arc<CdnAccelerator>` and pass it to methods that
/// spawn background tasks.
pub(crate) struct CdnAccelerator {
    state: RwLock<AccelState>,
    active_ip: RwLock<Option<Ipv4Addr>>,
    active_speed_mbps: RwLock<Option<f64>>,
    cancel_token: RwLock<Option<CancellationToken>>,
    accelerated_client: RwLock<Option<reqwest::Client>>,
}

impl CdnAccelerator {
    /// Create a new accelerator in [`AccelState::Idle`].
    pub(crate) fn new() -> Self {
        Self {
            state: RwLock::new(AccelState::Idle),
            active_ip: RwLock::new(None),
            active_speed_mbps: RwLock::new(None),
            cancel_token: RwLock::new(None),
            accelerated_client: RwLock::new(None),
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
    pub(crate) async fn start_test(
        self: &Arc<Self>,
        settings: AppSettings,
    ) -> anyhow::Result<()> {
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

        tauri::async_runtime::spawn(async move {
            // ── Phase 1: get IP ranges ──────────────────────────
            let ip_cache =
                tokio::sync::Mutex::new(IpRangesCache {
                    ips: Vec::new(),
                    fetched_at: Instant::now(),
                    from_fallback: true,
                });
            let range_data = get_ip_ranges(&ip_cache).await;

            // Check cancellation before starting the heavy work.
            if token.is_cancelled() {
                *this.state.write().await = AccelState::Idle;
                return;
            }

            let ips = range_data.ips;
            if ips.is_empty() {
                *this.state.write().await =
                    AccelState::Error("no Cloudflare IPs available".into());
                return;
            }

            // ── Phase 2: speed test ─────────────────────────────
            let config = SpeedTestConfig::default();
            let results = tokio::select! {
                _ = token.cancelled() => {
                    *this.state.write().await = AccelState::Idle;
                    return;
                }
                r = run_speed_test(&ips, &config, &settings) => r,
            };

            if token.is_cancelled() {
                *this.state.write().await = AccelState::Idle;
                return;
            }

            // ── Pick best candidate ─────────────────────────────
            let best = results
                .into_iter()
                .find(|r| r.throughput_mbps.is_some());

            match best {
                Some(result) => {
                    let speed = result.throughput_mbps.unwrap_or(0.0);
                    match this
                        .apply_ip(result.ip, speed, &settings)
                        .await
                    {
                        Ok(()) => {
                            // apply_ip already set state to Ready.
                        }
                        Err(e) => {
                            *this.state.write().await =
                                AccelState::Error(format!(
                                    "failed to build accelerated client: {e}"
                                ));
                        }
                    }
                }
                None => {
                    *this.state.write().await =
                        AccelState::Error(
                            "no reachable CDN IPs — all candidates failed".into(),
                        );
                }
            }
        });

        Ok(())
    }

    /// Cancel any running speed test and set state back to [`AccelState::Idle`].
    ///
    /// If no test is active this is a no-op.  Uses [`RwLock::try_write`] so it
    /// is safe to call from within an async runtime (e.g. a `#[tokio::test]` or
    /// Tauri command).  If the lock cannot be acquired immediately the
    /// background task will still observe the cancellation and clean up.
    pub(crate) fn cancel_test(&self) {
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
    pub(crate) async fn status(&self) -> AccelState {
        self.state.read().await.clone()
    }

    /// Build an accelerated `reqwest::Client` for the given IP, store it, and
    /// move the state to [`AccelState::Ready`].
    pub(crate) async fn apply_ip(
        &self,
        ip: Ipv4Addr,
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

    /// Reset the accelerator to [`AccelState::Idle`], dropping all stored data
    /// including the accelerated client.
    pub(crate) async fn clear(&self) {
        *self.state.write().await = AccelState::Idle;
        *self.active_ip.write().await = None;
        *self.active_speed_mbps.write().await = None;
        *self.cancel_token.write().await = None;
        *self.accelerated_client.write().await = None;
    }

    /// Return a clone of the accelerated client if the state is
    /// [`AccelState::Ready`], otherwise `None`.
    pub(crate) async fn get_client(&self) -> Option<reqwest::Client> {
        self.accelerated_client.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke: verify initial state and basic lifecycle via `apply_ip` / `clear`.
    #[tokio::test]
    async fn test_lifecycle() {
        let acc = Arc::new(CdnAccelerator::new());

        // ── Idle ─────────────────────────────────────────────────
        assert_eq!(acc.status().await, AccelState::Idle);
        assert!(acc.get_client().await.is_none());

        // ── Apply IP → Ready ─────────────────────────────────────
        let ip = Ipv4Addr::new(127, 0, 0, 1);
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
        let ip = Ipv4Addr::new(127, 0, 0, 1);
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
    }
}
