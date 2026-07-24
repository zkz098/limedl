use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use crate::cdn::accelerator::{AccelState, CdnAccelerator};
use crate::types::AppSettings;

/// Integration smoke test: full lifecycle (new → apply_ip → Ready → clear → Idle),
/// covering all new fields (phase, phase_progress, candidates).
#[tokio::test]
async fn smoke_accelerator_lifecycle() {
    let acc = Arc::new(CdnAccelerator::new());

    // ── Initial state ────────────────────────────────────────────
    assert_eq!(acc.status().await, AccelState::Idle);
    assert!(acc.get_client().await.is_none());
    assert!(acc.active_ip().await.is_none());
    assert!(acc.active_speed_mbps().await.is_none());
    assert!(acc.phase().await.is_none());
    assert_eq!(acc.phase_progress().await, (0, 0));
    assert!(acc.candidates().await.is_empty());

    // ── Apply an IP → Ready ──────────────────────────────────────
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
    let settings = AppSettings::default();
    acc.apply_ip(ip, 250.0, &settings).await.unwrap();

    assert_eq!(acc.status().await, AccelState::Ready);
    assert_eq!(acc.active_ip().await, Some(ip));
    assert_eq!(acc.active_speed_mbps().await, Some(250.0));
    assert!(acc.get_client().await.is_some());

    // ── Clear → everything reset ─────────────────────────────────
    acc.clear().await;

    assert_eq!(acc.status().await, AccelState::Idle);
    assert!(acc.get_client().await.is_none());
    assert!(acc.active_ip().await.is_none());
    assert!(acc.active_speed_mbps().await.is_none());
    assert!(acc.phase().await.is_none());
    assert_eq!(acc.phase_progress().await, (0, 0));
    assert!(acc.candidates().await.is_empty());
}

/// `cancel_test` on a fresh accelerator is safe (no-op).
#[tokio::test]
async fn smoke_cancel_idempotent() {
    let acc = Arc::new(CdnAccelerator::new());
    acc.cancel_test();
    assert_eq!(acc.status().await, AccelState::Idle);

    // Double-cancel is safe.
    acc.cancel_test();
    assert_eq!(acc.status().await, AccelState::Idle);
}

/// `candidates()` returns an independent clone — mutating the result
/// does not affect the accelerator.
#[tokio::test]
async fn smoke_candidates_clone_is_independent() {
    let acc = Arc::new(CdnAccelerator::new());
    let mut cloned = acc.candidates().await;
    cloned.push(Default::default());
    assert!(acc.candidates().await.is_empty());
}
