use ntest::timeout;
use tempfile::TempDir;

/// Verify settings persist to disk and survive restart.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn settings_survive_restart() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");

    let default_download_dir = tmp.path().join("custom-downloads");
    std::fs::create_dir_all(&default_download_dir).unwrap();

    // Phase 1: modify settings and save
    let saved_default_dir = {
        let core = crate::bootstrap::bootstrap(state_dir.clone())
            .await
            .unwrap();
        let dm = &core.download_manager;

        let mut settings = dm.settings().await.unwrap();
        settings.download.default_download_dir = default_download_dir.to_string_lossy().to_string();
        // Also set a non-default speed limit to test numeric persistence
        settings.global_speed_limit_bps = 42 * 1024 * 1024; // 42 MiB/s

        let saved = dm.apply_settings(settings).await.unwrap();
        let dir = saved.download.default_download_dir.clone();

        core.registry.shutdown_all().await;
        dir
    };

    // Verify the JSON file on disk has the right values
    let settings_path = state_dir.parent().unwrap().join("settings.json");
    assert!(
        settings_path.exists(),
        "settings.json should exist after save"
    );
    let disk_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert_eq!(disk_json["globalSpeedLimitBps"], 42 * 1024 * 1024);
    assert_eq!(
        disk_json["download"]["defaultDownloadDir"],
        default_download_dir.to_string_lossy().as_ref()
    );

    // Phase 2: re-bootstrap and verify settings are loaded
    {
        let core = crate::bootstrap::bootstrap(state_dir.clone())
            .await
            .unwrap();
        let dm = &core.download_manager;
        let settings = dm.settings().await.unwrap();

        assert_eq!(
            settings.download.default_download_dir, saved_default_dir,
            "defaultDownloadDir should survive restart"
        );
        assert_eq!(
            settings.global_speed_limit_bps,
            42 * 1024 * 1024,
            "globalSpeedLimitBps should survive restart"
        );

        core.registry.shutdown_all().await;
    }
}

/// Verify that bootstrapping without a settings.json produces valid defaults.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn default_settings_are_valid() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    // No settings.json exists

    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();
    let dm = &core.download_manager;
    let settings = dm.settings().await.unwrap();

    // All major sections should exist with non-empty defaults
    assert!(
        !settings.download.default_user_agent.is_empty(),
        "default user agent should be set"
    );
    assert!(
        settings.scheduler.traditional.max_parallel_tasks > 0,
        "max_parallel_tasks should be > 0"
    );
    // IO baseline should have reasonable defaults
    assert!(
        settings.io_baseline.buffer_limit_mb > 0,
        "buffer_limit_mb should be > 0"
    );

    core.registry.shutdown_all().await;
}
