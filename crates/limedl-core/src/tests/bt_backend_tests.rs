//! Task 1: max_concurrent_bt Arc shared dynamic synchronization invariant.
//!
//! Contract: after `DownloadManager::apply_settings` modifies `limits.max_concurrent_bt`,
//! `IrontideBtBackend` immediately sees the new value through the shared `Arc<AtomicUsize>`.
//!
//! The test creates both subsystems via `bootstrap()` (matching the real initialization
//! sequence in bootstrap.rs:48-57) so they share the exact same `Arc<AtomicUsize>`.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use ntest::timeout;
use tempfile::TempDir;

use crate::bootstrap::bootstrap;
use crate::types::{AppSettings, DownloadLimits};

// ---------------------------------------------------------------------------
// Helper: settings with a specific max_concurrent_bt value
// ---------------------------------------------------------------------------

fn settings_with_max_bt(max: usize) -> AppSettings {
    AppSettings {
        download_limits: Some(DownloadLimits {
            max_concurrent_http: 5,
            max_concurrent_bt: max,
        }),
        ..AppSettings::default()
    }
}

// ---------------------------------------------------------------------------
// Test 1 – Arc sharing: BT backend sees the new value immediately
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn bt_max_concurrent_arc_sync_after_apply_settings() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");

    let core = bootstrap(state_dir.clone()).await.unwrap();

    // Capture the shared Arc reference that both DM and BT backend hold.
    let bt_side = core.bt_backend.max_concurrent_bt.clone();

    // Verify initial value matches DM's initial value
    assert_eq!(
        bt_side.load(Ordering::Acquire),
        core.download_manager.limits.max_concurrent_bt.load(Ordering::Acquire),
        "initial: BT backend and DM must share the same Arc (same value)"
    );

    // Change to a new value via apply_settings
    core.download_manager
        .apply_settings(settings_with_max_bt(7))
        .await
        .unwrap();

    // BT backend side must see the new value immediately (no yield/sleep needed
    // because Arc<AtomicUsize> is lock-free — the store used Ordering::Release).
    assert_eq!(
        bt_side.load(Ordering::Acquire),
        7,
        "BT backend must see new max_concurrent_bt=7 after apply_settings"
    );

    // DM side must also see the same value (same Arc allocation)
    assert_eq!(
        core.download_manager
            .limits
            .max_concurrent_bt
            .load(Ordering::Acquire),
        7,
        "DM side must also see max_concurrent_bt=7"
    );

    // Reverse direction: change again and verify BT side picks it up
    core.download_manager
        .apply_settings(settings_with_max_bt(1))
        .await
        .unwrap();

    assert_eq!(
        bt_side.load(Ordering::Acquire),
        1,
        "BT backend must see new max_concurrent_bt=1 after second apply_settings"
    );

    assert_eq!(
        core.download_manager
            .limits
            .max_concurrent_bt
            .load(Ordering::Acquire),
        1,
        "DM side must also see max_concurrent_bt=1"
    );

    // Clean shutdown
    core.registry.shutdown_all().await;
}

// ---------------------------------------------------------------------------
// Test 2 – Arc pointer identity (same allocation, not just same value)
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn bt_max_concurrent_arc_pointer_identity() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");

    let core = bootstrap(state_dir.clone()).await.unwrap();

    // Both must point to the exact same Arc allocation
    assert!(
        Arc::ptr_eq(
            &core.bt_backend.max_concurrent_bt,
            &core.download_manager.limits.max_concurrent_bt,
        ),
        "BT backend and DM must share the exact same Arc<AtomicUsize> allocation"
    );

    core.registry.shutdown_all().await;
}

// ---------------------------------------------------------------------------
// Test 3 – BT backend initializes with zero runtime status
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn bt_backend_initializes_with_zero_runtime_status() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");

    let core = bootstrap(state_dir.clone()).await.unwrap();

    let status = core.bt_backend.runtime_status();
    assert_eq!(status.torrent_count, 0, "no torrents initially");
    assert_eq!(status.peer_count, 0, "no peers initially");
    assert_eq!(status.uploaded_bytes, 0, "no uploaded bytes initially");
    assert!(status.dht_enabled, "DHT should be enabled by default");

    core.registry.shutdown_all().await;
}

// ---------------------------------------------------------------------------
// Test 4 – BT backend settings propagate after apply_settings
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn bt_backend_settings_propagate_after_apply_settings() {
    use crate::lock;

    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");

    let core = bootstrap(state_dir.clone()).await.unwrap();

    // Verify default settings
    {
        let bt = lock(&core.bt_backend.bt_settings);
        assert!(bt.dht_enabled, "DHT enabled by default");
        assert!(bt.enable_pex, "PEX enabled by default");
        assert!(!bt.upnp_enabled, "UPnP disabled by default");
    }

    // Apply new settings through the backend directly
    let mut settings = AppSettings::default();
    settings.bt.dht_enabled = false;
    settings.bt.enable_pex = false;
    settings.bt.upnp_enabled = true;
    core.bt_backend.apply_settings(&settings);

    // Verify settings propagated to the shared bt_settings
    {
        let bt = lock(&core.bt_backend.bt_settings);
        assert!(!bt.dht_enabled, "dht_enabled should be false after apply");
        assert!(!bt.enable_pex, "enable_pex should be false after apply");
        assert!(bt.upnp_enabled, "upnp_enabled should be true after apply");
    }

    core.registry.shutdown_all().await;
}

// ---------------------------------------------------------------------------
// Test 5 – BT backend get_torrent_files returns error for unknown hash
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn bt_backend_get_torrent_files_returns_error_for_unknown() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");

    let core = bootstrap(state_dir.clone()).await.unwrap();

    let fake_hash = irontide::core::Id20::from([0u8; 20]);
    let result = core.bt_backend.get_torrent_files(fake_hash);
    assert!(result.is_err(), "expected error for unknown info hash");

    core.registry.shutdown_all().await;
}
