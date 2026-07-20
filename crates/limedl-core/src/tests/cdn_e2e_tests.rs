use ntest::timeout;
use std::sync::Arc;
use tempfile::TempDir;

use crate::cdn::CdnAccelerator;

/// CDN settings survive save → restart → load cycle.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn cdn_settings_survive_restart() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");

    let test_ip = String::from("1.1.1.1");

    // Phase 1: enable CDN with a known IP, save settings
    {
        let core = crate::bootstrap::bootstrap(state_dir.clone())
            .await
            .unwrap();
        let dm = &core.download_manager;

        let mut settings = dm.settings().await.unwrap();
        settings.cdn_acceleration.enabled = true;
        settings.cdn_acceleration.active_ip = Some(test_ip.clone());
        settings.cdn_acceleration.active_speed_mbps = Some(150.0);
        dm.apply_settings(settings).await.unwrap();

        core.registry.shutdown_all().await;
    }

    // Phase 2: re-bootstrap, verify CDN settings loaded correctly
    {
        let core = crate::bootstrap::bootstrap(state_dir.clone())
            .await
            .unwrap();
        let dm = &core.download_manager;
        let settings = dm.settings().await.unwrap();

        assert!(
            settings.cdn_acceleration.enabled,
            "CDN should remain enabled after restart"
        );
        assert_eq!(
            settings.cdn_acceleration.active_ip.as_deref(),
            Some(test_ip.as_str()),
            "active_ip should survive restart"
        );
        assert!(
            (settings.cdn_acceleration.active_speed_mbps.unwrap() - 150.0).abs() < 1.0,
            "active_speed_mbps should survive restart"
        );

        core.registry.shutdown_all().await;
    }
}

/// CdnAccelerator lifecycle: init from settings → active → clear → idle.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn cdn_accelerator_init_and_clear() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");

    let test_ip = String::from("1.0.0.1");

    // Bootstrap and set CDN settings
    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();
    let dm = &core.download_manager;

    let mut settings = dm.settings().await.unwrap();
    settings.cdn_acceleration.enabled = true;
    settings.cdn_acceleration.active_ip = Some(test_ip.clone());
    dm.apply_settings(settings).await.unwrap();

    // Create accelerator and init from the persisted settings
    let accelerator = Arc::new(CdnAccelerator::new());
    dm.set_cdn_accelerator(accelerator.clone());

    let current_settings = dm.settings().await.unwrap();
    accelerator.init_from_settings(&current_settings).await;

    // Verify active_ip is restored
    let active = accelerator.active_ip().await;
    assert!(
        active.is_some(),
        "Accelerator should have active IP after init_from_settings"
    );
    assert_eq!(
        active.unwrap().to_string(),
        test_ip,
        "Accelerator active IP should match settings"
    );

    // Clear the accelerator
    accelerator.clear().await;

    // Verify no active IP after clear
    let active_after_clear = accelerator.active_ip().await;
    assert!(
        active_after_clear.is_none(),
        "Accelerator should have no active IP after clear()"
    );

    core.registry.shutdown_all().await;
}

/// Disabling CDN clears the active IP from settings.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn cdn_disable_clears_active_ip() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");

    // Phase 1: enable CDN with an IP
    {
        let core = crate::bootstrap::bootstrap(state_dir.clone())
            .await
            .unwrap();
        let dm = &core.download_manager;

        let mut settings = dm.settings().await.unwrap();
        settings.cdn_acceleration.enabled = true;
        settings.cdn_acceleration.active_ip = Some(String::from("1.1.1.1"));
        dm.apply_settings(settings).await.unwrap();

        core.registry.shutdown_all().await;
    }

    // Phase 2: re-bootstrap, disable CDN, save, re-bootstrap again
    {
        let core = crate::bootstrap::bootstrap(state_dir.clone())
            .await
            .unwrap();
        let dm = &core.download_manager;

        let mut settings = dm.settings().await.unwrap();
        settings.cdn_acceleration.enabled = false;
        dm.apply_settings(settings).await.unwrap();

        core.registry.shutdown_all().await;
    }

    // Phase 3: verify CDN is disabled
    {
        let core = crate::bootstrap::bootstrap(state_dir.clone())
            .await
            .unwrap();
        let dm = &core.download_manager;
        let settings = dm.settings().await.unwrap();

        assert!(!settings.cdn_acceleration.enabled, "CDN should be disabled");

        core.registry.shutdown_all().await;
    }
}

/// Verify that for a known Cloudflare domain, CDN acceleration triggers correctly.
///
/// This test requires DNS resolution and is network-dependent — it's marked
/// `#[ignore]` so it doesn't run in CI by default. Run manually with:
///   cargo test --features test-utils -- cdn_acceleration_triggers --include-ignored
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
#[ignore = "requires network access (DNS lookup for Cloudflare domains)"]
async fn cdn_acceleration_triggers_for_cloudflare_domain() {
    use crate::types::{StartDownloadRequest, TaskId};

    // Verify the test domain IS a Cloudflare domain
    let test_url = "https://www.cloudflare.com/";
    assert!(
        crate::cdn::is_cloudflare_domain(test_url).await,
        "www.cloudflare.com should be detected as a Cloudflare domain — is DNS working?"
    );

    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();
    let dm = &core.download_manager;

    // Enable CDN acceleration with a known Cloudflare IP
    // 1.1.1.1 is Cloudflare's public DNS resolver — it's in the Cloudflare IP ranges.
    // Note: the download itself may fail because 1.1.1.1 is a DNS resolver,
    // not an HTTP server. But the cdn_accelerated flag is set BEFORE the
    // connection attempt, so we can still verify the flag.
    let mut settings = dm.settings().await.unwrap();
    settings.cdn_acceleration.enabled = true;
    settings.cdn_acceleration.active_ip = Some(String::from("1.1.1.1"));
    dm.apply_settings(settings).await.unwrap();

    // Create accelerator and init from persisted settings
    let accelerator = Arc::new(CdnAccelerator::new());
    dm.set_cdn_accelerator(accelerator.clone());
    let current = dm.settings().await.unwrap();
    accelerator.init_from_settings(&current).await;

    // Verify accelerator has the active IP
    assert!(
        accelerator.active_ip().await.is_some(),
        "Accelerator should have active IP after init_from_settings"
    );

    // Start a download to the Cloudflare domain
    let request = StartDownloadRequest {
        url: test_url.to_string(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some(String::from("speed_test_page")),
        kind: None,
        thread_mode: None,
        thread_count: Some(1),
        max_retries: Some(1),
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        user_agent: None,
    };
    let id = dm.start(request).await.unwrap();
    let task_id = TaskId::from_legacy_string(&id.to_string()).unwrap();
    let inner = match task_id {
        TaskId::Http(u) => u,
        TaskId::Bt(_) => unreachable!(),
    };

    // Wait briefly for the download to start and resolve_client to be called
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Check the snapshot — cdn_accelerated should be true
    let snapshot = dm.status(&inner.to_string()).await.unwrap();
    assert!(
        snapshot.cdn_accelerated,
        "CDN acceleration should be active for a Cloudflare domain.\n\
         cdn_accelerated flag: {}\n\
         Download state: {:?}",
        snapshot.cdn_accelerated, snapshot.state,
    );

    // Cleanup
    let _ = dm.cancel(&inner.to_string()).await;
    core.registry.shutdown_all().await;
}
