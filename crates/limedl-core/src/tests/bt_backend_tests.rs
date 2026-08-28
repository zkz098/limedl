//! Shared `max_concurrent_bt` `Arc<AtomicUsize>` invariants between
//! `DownloadManager` and `IrontideBtBackend`.
//!
//! Tests create both subsystems via `bootstrap()` (matching the real
//! initialization sequence in bootstrap.rs:48-57) so they share the exact same
//! `Arc<AtomicUsize>` allocation.

#![cfg(feature = "bt")]

use std::sync::Arc;

use ntest::timeout;
use tempfile::TempDir;

use crate::bootstrap::bootstrap;
use crate::types::AppSettings;

// ---------------------------------------------------------------------------
// Test 1 – Arc pointer identity (same allocation, not just same value)
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
            &core.download_manager.concurrency.max_concurrent_bt,
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
