use std::net::IpAddr;

use serde::Serialize;
use tauri::State;

use limedl_core::{
    AppState, CdnTestOutcome, Dispatcher,
    cdn::{
        accelerator::AccelState,
        ip_ranges::{CLOUDFLARE_IPV4_RANGES, CLOUDFLARE_IPV6_RANGES},
        speed_test::{DefaultNodeResult, SpeedTestResult},
    },
};

/// Persist CDN test results to settings via Dispatcher.
async fn persist_cdn_outcome(outcome: &CdnTestOutcome, dispatcher: &Dispatcher) {
    let now_ms = limedl_core::now_ms();
    if let Ok(mut current) = dispatcher.get_settings().await {
        match &outcome.state {
            AccelState::Ready => {
                current.cdn_acceleration.active_ip = outcome.active_ip.map(|i| i.to_string());
                current.cdn_acceleration.active_speed_mbps = outcome.active_speed_mbps;
                current.cdn_acceleration.last_test_at_ms = Some(now_ms);
                current.cdn_acceleration.last_error = None;
            }
            AccelState::Error(msg) => {
                current.cdn_acceleration.last_error = Some(msg.clone());
                current.cdn_acceleration.last_test_at_ms = Some(now_ms);
            }
            _ => {}
        }
        let _ = dispatcher.save_settings(&current).await;
    }
}

/// Return both static Cloudflare IPv4 and IPv6 CIDR range strings from the bundled fallback list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdnIpRanges {
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
}

#[tauri::command]
pub async fn cdn_fetch_ranges(_state: State<'_, AppState>) -> Result<CdnIpRanges, String> {
    Ok(CdnIpRanges {
        ipv4: CLOUDFLARE_IPV4_RANGES.iter().map(|s| s.to_string()).collect(),
        ipv6: CLOUDFLARE_IPV6_RANGES.iter().map(|s| s.to_string()).collect(),
    })
}

/// Kick off a CDN speed test in a background task.
#[tauri::command]
pub async fn cdn_test(state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.dispatcher.get_settings().await.map_err(|e| e.to_string())?;

    state
        .cdn_service
        .start_test(settings)
        .await
        .map_err(|e| e.to_string())?;

    let cdn = state.cdn_service.clone();
    let event_bus = state.event_bus.clone();
    let dispatcher = state.dispatcher.clone();

    tauri::async_runtime::spawn(async move {
        let outcome = cdn.monitor_test(event_bus).await;
        persist_cdn_outcome(&outcome, &dispatcher).await;
    });

    Ok(())
}

/// Build an accelerated reqwest client for the given IP and speed estimate.
#[tauri::command]
pub async fn cdn_apply(
    state: State<'_, AppState>,
    ip: String,
    speed_mbps: f64,
) -> Result<(), String> {
    let ip: IpAddr = ip.parse().map_err(|e| format!("Invalid IP address: {e}"))?;
    let settings = state.dispatcher.get_settings().await.map_err(|e| e.to_string())?;

    state
        .cdn_service
        .apply_ip(ip, speed_mbps, &settings)
        .await
        .map_err(|e| e.to_string())?;

    if let Ok(mut current) = state.dispatcher.get_settings().await {
        current.cdn_acceleration.active_ip = Some(ip.to_string());
        current.cdn_acceleration.active_speed_mbps = Some(speed_mbps);
        current.cdn_acceleration.last_test_at_ms = Some(limedl_core::now_ms());
        current.cdn_acceleration.last_error = None;
        let _ = state.dispatcher.save_settings(&current).await;
    }

    Ok(())
}

/// Reset the accelerator to idle state, dropping any accelerated client.
#[tauri::command]
pub async fn cdn_clear(state: State<'_, AppState>) -> Result<(), String> {
    state.cdn_service.clear().await;
    Ok(())
}

/// Return a human-readable string describing the current accelerator state.
#[tauri::command]
pub async fn cdn_status(state: State<'_, AppState>) -> Result<String, String> {
    let st = state.cdn_service.status().await;
    Ok(match st {
        AccelState::Idle => "Idle".to_string(),
        AccelState::Testing => "Testing".to_string(),
        AccelState::Ready => "Ready".to_string(),
        AccelState::Error(msg) => format!("Error: {msg}"),
    })
}

/// Cancel any running speed test and reset the accelerator to idle.
#[tauri::command]
pub async fn cdn_cancel(state: State<'_, AppState>) -> Result<(), String> {
    state.cdn_service.cancel_test();
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
pub async fn cdn_detail(state: State<'_, AppState>) -> Result<CdnDetail, String> {
    let st = state.cdn_service.status().await;
    let ip = state
        .cdn_service
        .active_ip()
        .await
        .map(|i| i.to_string());
    let speed = state.cdn_service.active_speed_mbps().await;
    let phase = state.cdn_service.phase().await.map(|p| match p {
        limedl_core::cdn::CdnTestPhase::FetchingRanges => "FetchingRanges".to_string(),
        limedl_core::cdn::CdnTestPhase::Screening => "Screening".to_string(),
        limedl_core::cdn::CdnTestPhase::MeasuringThroughput => "MeasuringThroughput".to_string(),
    });
    let (current, total) = state.cdn_service.phase_progress().await;
    let phase_progress = if total > 0 {
        Some(PhaseProgress { current, total })
    } else {
        None
    };
    let candidates = state.cdn_service.candidates().await;
    let default_node = state.cdn_service.default_node().await;

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
pub async fn cdn_candidates(state: State<'_, AppState>) -> Result<Vec<SpeedTestResult>, String> {
    Ok(state.cdn_service.candidates().await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    /// `cdn_fetch_ranges` must return static fallback CIDRs (15 IPv4 + 6 IPv6).
    #[test]
    fn fetch_ranges_returns_correct_counts() {
        let ipv4 = CLOUDFLARE_IPV4_RANGES.to_vec();
        assert_eq!(ipv4.len(), 15);
        assert!(ipv4.contains(&"104.16.0.0/13"));

        let ipv6 = CLOUDFLARE_IPV6_RANGES.to_vec();
        assert_eq!(ipv6.len(), 6);
        assert!(ipv6.contains(&"2606:4700::/32"));
    }

    /// `cdn_apply` must reject invalid IP strings before calling the accelerator.
    #[tokio::test]
    async fn apply_rejects_invalid_ip() {
        let bad: Result<IpAddr, _> = "not-an-ip".parse();
        assert!(bad.is_err());

        let bad2: Result<IpAddr, _> = "999.999.999.999".parse();
        assert!(bad2.is_err());

        let good: IpAddr = "1.2.3.4".parse().unwrap();
        assert_eq!(good, IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)));

        // IPv6 should also parse
        let good2: IpAddr = "::1".parse().unwrap();
        assert_eq!(good2, IpAddr::V6(Ipv6Addr::LOCALHOST));
    }
}
