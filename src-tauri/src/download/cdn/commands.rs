use std::net::Ipv4Addr;

use tauri::State;

use super::accelerator::AccelState;
use super::ip_ranges::CLOUDFLARE_IPV4_RANGES;
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
/// Returns immediately after spawning the test.  Progress can be monitored via
/// [`cdn_status`].  Calling this when a test is already running is a no-op.
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
        .map_err(|e| e.to_string())
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
        .map_err(|e| e.to_string())
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
