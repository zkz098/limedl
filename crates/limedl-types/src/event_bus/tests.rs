use super::*;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

use crate::types::{DownloadProgress, DownloadState, DownloadSummary, TaskKind, ThreadMode};

// ── Sample event helpers ──────────────────────────────────────────────

fn sample_summary() -> DownloadSummary {
    DownloadSummary {
        id: "t1".into(),
        kind: TaskKind::Http,
        state: DownloadState::Downloading,
        url: "https://example.com/file.bin".into(),
        file_name: "file.bin".into(),
        destination_path: "/tmp/file.bin".into(),
        total_bytes: Some(100),
        downloaded_bytes: 10,
        connection_count: 1,
        thread_mode: ThreadMode::Fixed,
        requested_thread_count: None,
        desired_thread_count: None,
        allocated_thread_count: None,
        adaptive_profile: None,
        thread_note: None,
        speed_bytes_per_second: None,
        eta_seconds: None,
        uploaded_bytes: None,
        upload_speed_bytes_per_second: None,
        peer_count: None,
        upload_status: None,
        info_hash: None,
        expected_checksum: None,
        error: None,
        cdn_accelerated: false,
        cdn_node_ip: None,
        created_at_ms: 0,
        priority: Default::default(),
        seed_count: None,
        leech_count: None,
        download_limit_bps: None,
        upload_limit_bps: None,
        chunks: Vec::new(),
        mirror_url: None,
    }
}

fn sample_progress() -> DownloadProgress {
    DownloadProgress {
        id: "t1".into(),
        state: DownloadState::Downloading,
        downloaded_bytes: 10,
        total_bytes: Some(100),
        speed_bytes_per_second: None,
        eta_seconds: None,
        connection_count: 1,
        allocated_thread_count: None,
        error: None,
        uploaded_bytes: None,
        upload_speed_bytes_per_second: None,
        peer_count: None,
        upload_status: None,
        degraded: false,
        disk_type: None,
        flushing: false,
    }
}

fn ev_updated() -> DownloadEvent {
    DownloadEvent::Updated {
        summary: Box::new(sample_summary()),
    }
}

fn ev_progress() -> DownloadEvent {
    DownloadEvent::Progress {
        progress: sample_progress(),
    }
}

fn ev_aria2() -> DownloadEvent {
    DownloadEvent::Aria2Notification {
        event_name: "bt-on-download-complete".into(),
        gid: "gid-1".into(),
    }
}

fn ev_cdn_progress() -> DownloadEvent {
    DownloadEvent::CdnProgress {
        phase: "probing".into(),
        current: 3,
        total: 10,
    }
}

fn ev_cdn_complete() -> DownloadEvent {
    DownloadEvent::CdnComplete {
        state: "completed".into(),
        active_ip: Some("1.2.3.4".into()),
        active_speed_mbps: Some(50.0),
    }
}

fn ev_warning() -> DownloadEvent {
    DownloadEvent::Warning {
        id: "t1".into(),
        message: "a warning".into(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[test]
fn new_event_bus_has_no_subscribers_initially() {
    let bus = EventBus::new(8);
    let mut rx = bus.subscribe();
    match rx.try_recv() {
        Err(TryRecvError::Empty) => {} // expected
        other => panic!("expected Empty, got {other:?}"),
    }
}

#[test]
fn publish_delivers_to_all_active_subscribers() {
    let bus = EventBus::new(16);
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();
    let mut rx3 = bus.subscribe();

    bus.publish(ev_updated());

    for (i, rx) in [&mut rx1, &mut rx2, &mut rx3].iter_mut().enumerate() {
        match rx.try_recv().unwrap() {
            DownloadEvent::Updated { summary } => {
                assert_eq!(summary.id, "t1", "subscriber {i}")
            }
            other => panic!("subscriber {i} expected Updated, got {other:?}"),
        }
    }
}

#[test]
fn publish_with_capacity_one_does_not_crash() {
    // broadcast::channel(0) would panic, so we test the smallest safe capacity.
    let bus = EventBus::new(1);
    let mut rx = bus.subscribe();
    bus.publish(ev_updated());
    match rx.try_recv().unwrap() {
        DownloadEvent::Updated { summary } => assert_eq!(summary.id, "t1"),
        other => panic!("expected Updated, got {other:?}"),
    }
}

#[tokio::test]
async fn slow_subscriber_lags_receives_lagged_error() {
    let bus = EventBus::new(2);
    let mut rx = bus.subscribe();

    // Publish more values than capacity before the subscriber reads.
    for _ in 0..5 {
        bus.publish(ev_updated());
    }

    // The subscriber must have lost at least one message.
    match rx.recv().await {
        Err(RecvError::Lagged(n)) => {
            assert!(n > 0, "expected lag > 0, got {n}");
        }
        other => panic!("expected Lagged, got {other:?}"),
    }

    // After a Lagged error the receiver may still hold buffered messages
    // that were published before the lag was reported. Drain them so the
    // next recv() reflects a fresh publish, not a stale buffered one.
    while rx.try_recv().is_ok() {}

    // Channel is still usable after a Lagged error.
    bus.publish(ev_progress());
    match rx.recv().await {
        Ok(DownloadEvent::Progress { .. }) => {} // correct
        other => panic!("expected Progress after lag recovery, got {other:?}"),
    }
}

#[test]
fn publish_returns_silently_when_no_subscribers() {
    // publish() should not panic or error when the send channel has no receivers.
    let bus = EventBus::new(8);
    // No subscriber → the `_ = self.tx.send(event)` pattern suppresses
    // the `SendError` (which is expected when there are no active receivers).
    bus.publish(ev_updated());
    bus.publish(ev_progress());
    // If we reach here, the test passes (no panic).
}

#[test]
fn clone_is_cheap_and_shares_underlying_channel() {
    let bus = EventBus::new(16);
    let bus2 = bus.clone();
    let mut rx = bus.subscribe();

    // Publish via bus2 — the subscriber on `bus` should still receive.
    bus2.publish(ev_updated());

    match rx.try_recv().unwrap() {
        DownloadEvent::Updated { summary } => assert_eq!(summary.id, "t1"),
        other => panic!("expected Updated, got {other:?}"),
    }
}

#[test]
fn publish_accepts_all_event_variants() {
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    bus.publish(ev_updated());
    bus.publish(ev_progress());
    bus.publish(ev_aria2());
    bus.publish(ev_cdn_progress());
    bus.publish(ev_cdn_complete());
    bus.publish(ev_warning());

    assert!(matches!(
        rx.try_recv().unwrap(),
        DownloadEvent::Updated { .. }
    ));
    assert!(matches!(
        rx.try_recv().unwrap(),
        DownloadEvent::Progress { .. }
    ));
    assert!(matches!(
        rx.try_recv().unwrap(),
        DownloadEvent::Aria2Notification { .. }
    ));
    assert!(matches!(
        rx.try_recv().unwrap(),
        DownloadEvent::CdnProgress { .. }
    ));
    assert!(matches!(
        rx.try_recv().unwrap(),
        DownloadEvent::CdnComplete { .. }
    ));
    assert!(matches!(
        rx.try_recv().unwrap(),
        DownloadEvent::Warning { .. }
    ));
}

#[test]
fn subscribe_then_publish_preserves_order() {
    let bus = EventBus::new(8);
    let mut rx = bus.subscribe();

    bus.publish(ev_updated());
    bus.publish(ev_progress());
    bus.publish(ev_warning());

    assert!(matches!(
        rx.try_recv().unwrap(),
        DownloadEvent::Updated { .. }
    ));
    assert!(matches!(
        rx.try_recv().unwrap(),
        DownloadEvent::Progress { .. }
    ));
    assert!(matches!(
        rx.try_recv().unwrap(),
        DownloadEvent::Warning { .. }
    ));
}
