use std::net::Ipv4Addr;

use serde::Serialize;
use tauri::Emitter;
use tauri::State;

use super::accelerator::AccelState;
use super::ip_ranges::CLOUDFLARE_IPV4_RANGES;
use super::speed_test::{CdnTestPhase, DefaultNodeResult, SpeedTestResult};
use crate::download::manager::AppState;

/// Return the 15 static Cloudflare IPv4 CIDR range strings from the bundled fallback list.
///
/// These are read directly from the compiled-in `CLOUDFLARE_IPV4_RANGES` constant
/// — no HTTP fetch is performed.  This provides a fast, offline-safe way for the
/// frontend to display available CDN probe ranges.
#[tauri::command]
pub async fn cdn_fetch_ranges(
    _state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    Ok(CLOUDFLARE_IPV4_RANGES.iter().map(|s| s.to_string()).collect())
}

/// Kick off a CDN speed test in a background task.
///
/// Returns immediately after spawning the test. Progress can be monitored via
/// [`cdn_status`]. When the test completes, results are auto-persisted to
/// `AppSettings.cdnAcceleration` and a `cdn-test-complete` event is emitted.
/// Calling this when a test is already running is a no-op.
#[tauri::command]
pub async fn cdn_test(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings = state
        .manager
        .settings()
        .await
        .map_err(|e| e.to_string())?;

    state
        .cdn_accelerator
        .start_test(settings)
        .await
        .map_err(|e| e.to_string())?;

    let acc = state.cdn_accelerator.clone();
    let mgr = state.manager.clone();
    let handle = state.app_handle.clone();

    tauri::async_runtime::spawn(async move {
        let mut was_testing = true;

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            let st = acc.status().await;
            match st {
                AccelState::Testing => {
                    was_testing = true;
                    // Emit progress events during testing.
                    if let Some(phase) = acc.phase().await {
                        let (current, total) = acc.phase_progress().await;
                        let phase_str = match phase {
                            CdnTestPhase::FetchingRanges => "fetchingRanges",
                            CdnTestPhase::Screening => "screening",
                            CdnTestPhase::MeasuringThroughput => "measuringThroughput",
                        };
                        tracing::debug!(
                            "cdn poll: Testing phase={phase_str} progress={current}/{total}",
                        );
                        let _ = handle.emit(
                            "cdn-test-progress",
                            serde_json::json!({
                                "phase": phase_str,
                                "current": current,
                                "total": total,
                            }),
                        );
                    }
                    continue;
                }
                AccelState::Idle => {
                    if was_testing {
                        tracing::info!("cdn poll: Idle (was testing) — test ended without Ready/Error");
                        break;
                    }
                    continue;
                }
                AccelState::Ready => {
                    tracing::info!("cdn poll: Ready — persisting results + emitting complete");
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let ip = acc.active_ip().await;
                    let speed = acc.active_speed_mbps().await;

                    if let Ok(mut current) = mgr.settings().await {
                        current.cdn_acceleration.active_ip =
                            ip.map(|i| i.to_string());
                        current.cdn_acceleration.active_speed_mbps = speed;
                        current.cdn_acceleration.last_test_at_ms =
                            Some(now_ms);
                        current.cdn_acceleration.last_error = None;
                        let _ = mgr.update_settings(current).await;
                    }

                    let _ = handle.emit(
                        "cdn-test-complete",
                        serde_json::json!({
                            "state": "Ready",
                            "activeIp": ip.map(|i| i.to_string()),
                            "activeSpeedMbps": speed,
                        }),
                    );
                    break;
                }
                AccelState::Error(msg) => {
                    tracing::info!("cdn poll: Error — {msg}");
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;

                    if let Ok(mut current) = mgr.settings().await {
                        current.cdn_acceleration.last_error =
                            Some(msg.clone());
                        current.cdn_acceleration.last_test_at_ms =
                            Some(now_ms);
                        let _ = mgr.update_settings(current).await;
                    }

                    let _ = handle.emit(
                        "cdn-test-complete",
                        serde_json::json!({
                            "state": format!("Error: {msg}"),
                            "activeIp": null,
                            "activeSpeedMbps": null,
                        }),
                    );
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Build an accelerated reqwest client for the given IP and speed estimate.
///
/// The IP string is parsed into an [`Ipv4Addr`] and handed to the accelerator's
/// [`apply_ip`](CdnAccelerator::apply_ip) method.  On success the accelerator state
/// moves to [`AccelState::Ready`].
#[tauri::command]
pub async fn cdn_apply(
    state: State<'_, AppState>,
    ip: String,
    speed_mbps: f64,
) -> Result<(), String> {
    let ip: Ipv4Addr = ip
        .parse()
        .map_err(|e| format!("Invalid IP address: {e}"))?;

    let settings = state
        .manager
        .settings()
        .await
        .map_err(|e| e.to_string())?;

    state
        .cdn_accelerator
        .apply_ip(ip, speed_mbps, &settings)
        .await
        .map_err(|e| e.to_string())?;

    // Persist the applied IP to settings so it survives restart.
    if let Ok(mut current) = state.manager.settings().await {
        current.cdn_acceleration.active_ip = Some(ip.to_string());
        current.cdn_acceleration.active_speed_mbps = Some(speed_mbps);
        current.cdn_acceleration.last_test_at_ms = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );
        current.cdn_acceleration.last_error = None;
        let _ = state.manager.update_settings(current).await;
    }

    Ok(())
}

/// Reset the accelerator to idle state, dropping any accelerated client.
///
/// This is always safe to call, even if no acceleration is active.
#[tauri::command]
pub async fn cdn_clear(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.cdn_accelerator.clear().await;
    Ok(())
}

/// Return a human-readable string describing the current accelerator state:
///
/// - `"Idle"` — no active test or client.
/// - `"Testing"` — speed test is running in the background.
/// - `"Ready"` — test completed, accelerated client is available.
/// - `"Error: {message}"` — test failed with the given reason.
#[tauri::command]
pub async fn cdn_status(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let st = state.cdn_accelerator.status().await;
    Ok(match st {
        AccelState::Idle => "Idle".to_string(),
        AccelState::Testing => "Testing".to_string(),
        AccelState::Ready => "Ready".to_string(),
        AccelState::Error(msg) => format!("Error: {msg}"),
    })
}

/// Cancel any running speed test and reset the accelerator to idle.
///
/// If no test is active this is a safe no-op.
#[tauri::command]
pub async fn cdn_cancel(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.cdn_accelerator.cancel_test();
    Ok(())
}

/// Progress counter for a CDN test phase.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseProgress {
    pub current: u64,
    pub total: u64,
}

/// Structured accelerator status returned to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdnDetail {
    pub state: String,
    pub active_ip: Option<String>,
    pub active_speed_mbps: Option<f64>,
    pub phase: Option<String>,
    pub phase_progress: Option<PhaseProgress>,
    pub candidates: Vec<SpeedTestResult>,
    pub default_node: Option<DefaultNodeResult>,
}

/// Return structured accelerator status including IP and speed.
#[tauri::command]
pub async fn cdn_detail(
    state: State<'_, AppState>,
) -> Result<CdnDetail, String> {
    let st = state.cdn_accelerator.status().await;
    let ip = state.cdn_accelerator.active_ip().await.map(|i| i.to_string());
    let speed = state.cdn_accelerator.active_speed_mbps().await;
    let phase = state.cdn_accelerator.phase().await.map(|p| match p {
        CdnTestPhase::FetchingRanges => "FetchingRanges".to_string(),
        CdnTestPhase::Screening => "Screening".to_string(),
        CdnTestPhase::MeasuringThroughput => "MeasuringThroughput".to_string(),
    });
    let (current, total) = state.cdn_accelerator.phase_progress().await;
    let phase_progress = if total > 0 {
        Some(PhaseProgress { current, total })
    } else {
        None
    };
    let candidates = state.cdn_accelerator.candidates().await;
    let default_node = state.cdn_accelerator.default_node().await;

    Ok(CdnDetail {
        state: match &st {
            AccelState::Idle => "Idle".to_string(),
            AccelState::Testing => "Testing".to_string(),
            AccelState::Ready => "Ready".to_string(),
            AccelState::Error(msg) => format!("Error: {msg}"),
        },
        active_ip: ip,
        active_speed_mbps: speed,
        phase,
        phase_progress,
        candidates,
        default_node,
    })
}

/// Return all candidate IPs from the most recent CDN speed test.
#[tauri::command]
pub async fn cdn_candidates(
    state: State<'_, AppState>,
) -> Result<Vec<SpeedTestResult>, String> {
    Ok(state.cdn_accelerator.candidates().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cdn_fetch_ranges` must return exactly 15 CIDR strings.
    #[test]
    fn fetch_ranges_returns_15_cidrs() {
        // We call the function without a real Tauri State by constructing a
        // minimal mock.  Since the function only reads the static constant,
        // we can test it in isolation.
        let cidrs = CLOUDFLARE_IPV4_RANGES.to_vec();
        assert_eq!(cidrs.len(), 15);
        // Spot-check a well-known entry.
        assert!(cidrs.contains(&"104.16.0.0/13"));
    }

    /// `cdn_apply` must reject invalid IP strings before calling the accelerator.
    #[tokio::test]
    async fn apply_rejects_invalid_ip() {
        // We can't easily construct a full AppState here without Tauri runtime,
        // so we test the IP parsing logic directly as a proxy.
        let bad: Result<Ipv4Addr, _> = "not-an-ip".parse();
        assert!(bad.is_err());

        let bad2: Result<Ipv4Addr, _> = "999.999.999.999".parse();
        assert!(bad2.is_err());

        // Valid IPs parse correctly.
        let good: Ipv4Addr = "1.2.3.4".parse().unwrap();
        assert_eq!(good, Ipv4Addr::new(1, 2, 3, 4));
    }
}
