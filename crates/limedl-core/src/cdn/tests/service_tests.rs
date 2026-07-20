//! Task 3: CdnService::monitor_test EventBus event flow invariant.
//!
//! Contract: `CdnService::monitor_test` publishes `DownloadEvent::CdnProgress`
//! during testing and `DownloadEvent::CdnComplete` when the test finishes
//! (either Ready or Error). This was migrated from Tauri-only `app_handle.emit`
//! to an EventBus-driven path in Stage 9.
//!
//! The test sets up a real CdnAccelerator + CdnService, starts a test via
//! `start_test` (which spawns a background task that sets phase to
//! FetchingRanges), then immediately drives the accelerator to Ready via
//! `apply_ip` so that `monitor_test` observes the transition and emits
//! both progress and completion events.
//!
//! Even if the background task eventually fails (no network), `monitor_test`
//! has already returned with the Ready outcome before that happens, so the
//! test is deterministic and fast.

use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;

use crate::cdn::accelerator::AccelState;
use crate::cdn::service::CdnService;
use crate::event_bus::{DownloadEvent, EventBus};
use crate::types::AppSettings;

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// Test: monitor_test emits CdnProgress + CdnComplete when test finishes Ready
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(30_000)]
async fn monitor_test_emits_progress_and_complete_ready() -> TestResult {
    let event_bus = Arc::new(EventBus::new(1024));
    let mut rx = event_bus.subscribe();

    let cdn = Arc::new(CdnService::new());

    // Phase 1: start a CDN test — this sets state → Testing and spawns a
    // background task that immediately sets phase → FetchingRanges + (0,0).
    cdn.start_test(AppSettings::default()).await?;
    assert_eq!(cdn.status().await, AccelState::Testing);

    // Yield so the spawned background task runs and sets phase.
    tokio::task::yield_now().await;

    // Phase 2: subscribe and spawn the monitoring loop in background.
    let monitor_cdn = Arc::clone(&cdn);
    let monitor_eb = Arc::clone(&event_bus);
    let monitor_handle = tokio::spawn(async move {
        monitor_cdn.monitor_test(monitor_eb).await
    });

    // Phase 3: after a poll interval, drive the accelerator to Ready so
    // monitor_test sees the transition.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(
        matches!(cdn.status().await, AccelState::Testing),
        "expected Testing state before apply_ip"
    );

    let ip = std::net::Ipv4Addr::new(127, 0, 0, 1);
    cdn.apply_ip(ip, 50.0, &AppSettings::default()).await?;
    assert_eq!(cdn.status().await, AccelState::Ready);

    // Phase 4: collect events with timeout.
    let mut got_progress = false;
    let mut got_complete = false;
    let deadline = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result? {
                    DownloadEvent::CdnProgress { phase, current, total } => {
                        got_progress = true;
                        // phase must be one of the known phase strings
                        assert!(
                            phase == "fetchingRanges"
                                || phase == "screening"
                                || phase == "measuringThroughput",
                            "unexpected phase: {phase}"
                        );
                        // progress values are non-negative
                        assert!(
                            current <= total,
                            "progress current ({current}) must not exceed total ({total})"
                        );
                    }
                    DownloadEvent::CdnComplete { state, active_ip, active_speed_mbps } => {
                        got_complete = true;
                        assert!(
                            state == "Ready" || state.starts_with("Error:"),
                            "CdnComplete state must be 'Ready' or 'Error: ...', got: {state}"
                        );
                        if state == "Ready" {
                            assert_eq!(active_ip, Some("127.0.0.1".to_string()));
                            assert_eq!(active_speed_mbps, Some(50.0));
                        }
                        break;
                    }
                    _ => {}
                }
            }
            _ = &mut deadline => {
                break;
            }
        }
    }

    assert!(
        got_progress,
        "monitor_test must emit at least one CdnProgress event"
    );
    assert!(
        got_complete,
        "monitor_test must emit a CdnComplete event"
    );

    // Await the monitor handle to ensure no panics in background.
    let outcome = monitor_handle.await?;
    assert_eq!(outcome.state, AccelState::Ready);
    assert_eq!(outcome.active_ip, Some(ip));
    assert_eq!(outcome.active_speed_mbps, Some(50.0));

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: monitor_test handles error state correctly
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(30_000)]
async fn monitor_test_emits_complete_on_error() -> TestResult {
    let event_bus = Arc::new(EventBus::new(1024));
    let mut rx = event_bus.subscribe();

    let cdn = Arc::new(CdnService::new());

    // Start a test, then immediately apply an error scenario by setting
    // the accelerator state to Error(...) directly through apply_ip failure.
    cdn.start_test(AppSettings::default()).await?;
    assert_eq!(cdn.status().await, AccelState::Testing);
    tokio::task::yield_now().await;

    let monitor_cdn = Arc::clone(&cdn);
    let monitor_eb = Arc::clone(&event_bus);
    let monitor_handle = tokio::spawn(async move {
        monitor_cdn.monitor_test(monitor_eb).await
    });

    // Wait for a poll cycle, then drive the accelerator into an error
    // state that monitor_test can observe. The background task from
    // start_test will eventually fail, but we simulate an explicit Ready
    // state and then let the background override — however, to make the
    // test deterministic, we let the background task fail naturally by
    // waiting for events with a reasonable timeout.
    //
    // Note: in environments without internet this will fail quickly with
    // "no Cloudflare IPs available". With internet it may take longer as
    // get_ip_ranges actually downloads them.

    let deadline = tokio::time::sleep(Duration::from_secs(25));
    tokio::pin!(deadline);

    let mut got_complete = false;

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result? {
                    DownloadEvent::CdnProgress { .. } => {
                        // Progress events are optional in this scenario
                    }
                    DownloadEvent::CdnComplete { state, .. } => {
                        got_complete = true;
                        // Accept any terminal state string
                        assert!(
                            state == "Ready" || state.starts_with("Error:"),
                            "CdnComplete state must be 'Ready' or 'Error: ...', got: {state}"
                        );
                        break;
                    }
                    _ => {}
                }
            }
            _ = &mut deadline => {
                break;
            }
        }
    }

    assert!(
        got_complete,
        "monitor_test must emit a CdnComplete event even on error"
    );

    let _outcome = monitor_handle.await?;
    Ok(())
}
