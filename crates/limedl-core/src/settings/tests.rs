use super::*;
use crate::types::{BtChokingAlgorithm, BtPortRange, BtSeedChokingAlgorithm, MatchType, ReplacementMode};
use std::io::Write;
use tempfile::tempdir;

// -----------------------------------------------------------------------
// normalize_proxy_settings
// -----------------------------------------------------------------------
#[test]
fn test_normalize_proxy_disabled_clears_url() {
    let input = ProxySettings {
        mode: ProxyMode::Disabled,
        manual_url: "http://should-be-cleared".into(),
    };
    let result = normalize_proxy_settings(input).unwrap();
    assert_eq!(result.mode, ProxyMode::Disabled);
    assert!(result.manual_url.is_empty());
}

#[test]
fn test_normalize_proxy_system_clears_url() {
    let input = ProxySettings {
        mode: ProxyMode::System,
        manual_url: "http://should-be-cleared".into(),
    };
    let result = normalize_proxy_settings(input).unwrap();
    assert_eq!(result.mode, ProxyMode::System);
    assert!(result.manual_url.is_empty());
}

#[test]
fn test_normalize_proxy_manual_valid_url_ok() {
    let input = ProxySettings {
        mode: ProxyMode::Manual,
        manual_url: "http://proxy:8080".into(),
    };
    let result = normalize_proxy_settings(input).unwrap();
    assert_eq!(result.mode, ProxyMode::Manual);
    assert_eq!(result.manual_url, "http://proxy:8080");
}

#[test]
fn test_normalize_proxy_manual_empty_url_err() {
    let input = ProxySettings {
        mode: ProxyMode::Manual,
        manual_url: String::new(),
    };
    let err = normalize_proxy_settings(input).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidProxy(_)));
}

#[test]
fn test_normalize_proxy_manual_invalid_url_err() {
    let input = ProxySettings {
        mode: ProxyMode::Manual,
        manual_url: "not-a-url".into(),
    };
    let err = normalize_proxy_settings(input).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidProxy(_)));
}

#[test]
fn test_normalize_proxy_manual_url_trimmed() {
    let input = ProxySettings {
        mode: ProxyMode::Manual,
        manual_url: "  http://proxy:8080  ".into(),
    };
    let result = normalize_proxy_settings(input).unwrap();
    assert_eq!(result.manual_url, "http://proxy:8080");
}

// -----------------------------------------------------------------------
// normalize_min_threads
// -----------------------------------------------------------------------
#[test]
fn test_normalize_min_threads_zero_with_max_eight() {
    assert_eq!(normalize_min_threads(0, 8), 4);
}

#[test]
fn test_normalize_min_threads_zero_with_max_one() {
    assert_eq!(normalize_min_threads(0, 1), 1);
}

#[test]
fn test_normalize_min_threads_within_range() {
    assert_eq!(normalize_min_threads(3, 8), 3);
}

#[test]
fn test_normalize_min_threads_clamped_to_max() {
    assert_eq!(normalize_min_threads(10, 8), 8);
}

#[test]
fn test_normalize_min_threads_zero_with_max_two() {
    assert_eq!(normalize_min_threads(0, 2), 1);
}

// -----------------------------------------------------------------------
// normalize_logging_settings
// -----------------------------------------------------------------------
#[test]
fn test_normalize_logging_retention_count_clamped() {
    let input = LogSettings {
        retention_count: Some(500),
        ..LogSettings::default()
    };
    let result = normalize_logging_settings(input);
    assert_eq!(result.retention_count, Some(500));

    let input = LogSettings {
        retention_count: Some(2000),
        ..LogSettings::default()
    };
    let result = normalize_logging_settings(input);
    assert_eq!(result.retention_count, Some(1000));

    let input = LogSettings {
        retention_count: Some(0),
        ..LogSettings::default()
    };
    let result = normalize_logging_settings(input);
    assert_eq!(result.retention_count, Some(0));
}

#[test]
fn test_normalize_logging_retention_days_clamped() {
    let input = LogSettings {
        retention_days: Some(365),
        ..LogSettings::default()
    };
    let result = normalize_logging_settings(input);
    assert_eq!(result.retention_days, Some(365));

    let input = LogSettings {
        retention_days: Some(5000),
        ..LogSettings::default()
    };
    let result = normalize_logging_settings(input);
    assert_eq!(result.retention_days, Some(3650));

    let input = LogSettings {
        retention_days: Some(0),
        ..LogSettings::default()
    };
    let result = normalize_logging_settings(input);
    assert_eq!(result.retention_days, Some(0));
}

#[test]
fn test_normalize_logging_file_path_trimmed() {
    let input = LogSettings {
        file_path: "  /var/log/app/  ".into(),
        ..LogSettings::default()
    };
    let result = normalize_logging_settings(input);
    assert_eq!(result.file_path, "/var/log/app/");
}

#[test]
fn test_normalize_logging_none_preserved() {
    let input = LogSettings {
        retention_count: None,
        retention_days: None,
        ..LogSettings::default()
    };
    let result = normalize_logging_settings(input);
    assert!(result.retention_count.is_none());
    assert!(result.retention_days.is_none());
}

// -----------------------------------------------------------------------
// normalize_download_dir
// -----------------------------------------------------------------------
#[test]
fn test_normalize_download_dir_empty() {
    assert_eq!(normalize_download_dir(""), "");
}

#[test]
fn test_normalize_download_dir_whitespace_only() {
    assert_eq!(normalize_download_dir("   \t  "), "");
}

#[test]
fn test_normalize_download_dir_relative_path() {
    assert_eq!(normalize_download_dir("downloads"), "");
}

#[cfg(unix)]
#[test]
fn test_normalize_download_dir_absolute_unix() {
    assert_eq!(
        normalize_download_dir("/home/user/downloads"),
        "/home/user/downloads"
    );
}

#[cfg(windows)]
#[test]
fn test_normalize_download_dir_absolute_unix_is_not_absolute_on_windows() {
    assert_eq!(normalize_download_dir("/home/user/downloads"), "",);
}

#[cfg(windows)]
#[test]
fn test_normalize_download_dir_absolute_windows() {
    assert_eq!(
        normalize_download_dir(r"C:\Users\test\downloads"),
        r"C:\Users\test\downloads"
    );
}

#[cfg(windows)]
#[test]
fn test_normalize_download_dir_absolute_windows_forward_slashes() {
    // On Windows, `C:/Users/test/downloads` is also absolute
    assert_eq!(
        normalize_download_dir("C:/Users/test/downloads"),
        "C:/Users/test/downloads"
    );
}

// -----------------------------------------------------------------------
// normalize_tracker_list
// -----------------------------------------------------------------------
#[test]
fn test_normalize_tracker_list_dedup_sorted() {
    let input = "udp://tracker.opentrackr.org:1337\nhttp://tracker.example.com\nudp://tracker.opentrackr.org:1337";
    let result = normalize_tracker_list(input).unwrap();
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "http://tracker.example.com/");
    assert_eq!(lines[1], "udp://tracker.opentrackr.org:1337");
}

#[test]
fn test_normalize_tracker_list_empty() {
    let result = normalize_tracker_list("").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_normalize_tracker_list_invalid_scheme() {
    let err = normalize_tracker_list("ftp://tracker.example.com").unwrap_err();
    assert!(matches!(err, DownloadError::InvalidResponse(_)));
    assert!(err.to_string().contains("unsupported tracker scheme"));
}

#[test]
fn test_normalize_tracker_list_empty_lines_filtered() {
    let input = "udp://tracker.opentrackr.org:1337\n\n\nhttp://example.com\n";
    let result = normalize_tracker_list(input).unwrap();
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 2);
}

#[test]
fn test_normalize_tracker_list_lines_trimmed() {
    let input = "  udp://tracker.opentrackr.org:1337  \n  http://example.com  ";
    let result = normalize_tracker_list(input).unwrap();
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 2);
}

#[test]
fn test_normalize_tracker_list_unsupported_scheme_err() {
    let err = normalize_tracker_list("ws://tracker.example.com").unwrap_err();
    assert!(matches!(err, DownloadError::InvalidResponse(_)));
}

// -----------------------------------------------------------------------
// normalize_tracker_list_url
// -----------------------------------------------------------------------
#[test]
fn test_normalize_tracker_list_url_empty_returns_default() {
    let result = normalize_tracker_list_url("").unwrap();
    assert_eq!(result, default_tracker_list_url());
}

#[test]
fn test_normalize_tracker_list_url_valid_http() {
    let result = normalize_tracker_list_url("http://example.com/list.txt").unwrap();
    assert_eq!(result, "http://example.com/list.txt");
}

#[test]
fn test_normalize_tracker_list_url_valid_https() {
    let result = normalize_tracker_list_url("https://trackers.example.com/best.txt").unwrap();
    assert_eq!(result, "https://trackers.example.com/best.txt");
}

#[test]
fn test_normalize_tracker_list_url_invalid_scheme() {
    let err = normalize_tracker_list_url("ftp://example.com/list.txt").unwrap_err();
    assert!(matches!(err, DownloadError::InvalidResponse(_)));
}

#[test]
fn test_normalize_tracker_list_url_invalid_url() {
    let err = normalize_tracker_list_url("not a url").unwrap_err();
    assert!(matches!(err, DownloadError::InvalidResponse(_)));
}

// -----------------------------------------------------------------------
// resolve_user_agent
// -----------------------------------------------------------------------
#[test]
fn test_resolve_user_agent_custom() {
    let result = resolve_user_agent(Some("MyAgent/1.0"), "Default/1.0").unwrap();
    assert_eq!(result, "MyAgent/1.0");
}

#[test]
fn test_resolve_user_agent_falls_back_to_default() {
    let result = resolve_user_agent(None, "Default/1.0").unwrap();
    assert_eq!(result, "Default/1.0");
}

#[test]
fn test_resolve_user_agent_empty_string_falls_back() {
    let result = resolve_user_agent(Some(""), "Default/1.0").unwrap();
    assert_eq!(result, "Default/1.0");
}

#[test]
fn test_resolve_user_agent_whitespace_falls_back() {
    let result = resolve_user_agent(Some("   "), "Default/1.0").unwrap();
    assert_eq!(result, "Default/1.0");
}

// -----------------------------------------------------------------------
// normalize_bt_settings
// -----------------------------------------------------------------------
#[test]
fn test_normalize_bt_empty_tracker_list_ok() {
    let input = BtSettings {
        tracker_list: String::new(),
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert!(result.tracker_list.is_empty());
}

#[test]
fn test_normalize_bt_upload_limit_clamped() {
    const MAX_UPLOAD_LIMIT_BYTES: u64 = 10 * 1024 * 1024 * 1024 * 1024; // 10 TiB
    let input = BtSettings {
        upload_limit_bytes: MAX_UPLOAD_LIMIT_BYTES + 1,
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert_eq!(result.upload_limit_bytes, MAX_UPLOAD_LIMIT_BYTES);

    let input = BtSettings {
        upload_limit_bytes: u64::MAX,
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert_eq!(result.upload_limit_bytes, MAX_UPLOAD_LIMIT_BYTES);
}

#[test]
fn test_normalize_bt_upload_ratio_clamped() {
    let input = BtSettings {
        upload_ratio_limit: 200.0,
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert!((result.upload_ratio_limit - 100.0).abs() < f64::EPSILON);
}

#[test]
fn test_normalize_bt_upload_ratio_nan_inf() {
    let input = BtSettings {
        upload_ratio_limit: f64::NAN,
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert!((result.upload_ratio_limit - 0.0).abs() < f64::EPSILON);

    let input = BtSettings {
        upload_ratio_limit: f64::INFINITY,
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert!((result.upload_ratio_limit - 0.0).abs() < f64::EPSILON);

    let input = BtSettings {
        upload_ratio_limit: f64::NEG_INFINITY,
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert!((result.upload_ratio_limit - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_normalize_bt_listen_port_range_start_gt_end_err() {
    let input = BtSettings {
        listen_port_range: Some(BtPortRange {
            start: 7000,
            end: 6000,
        }),
        ..BtSettings::default()
    };
    let err = normalize_bt_settings(input).unwrap_err();
    assert!(err.to_string().contains("listen_port_range"));
}

#[test]
fn test_normalize_bt_listen_port_range_below_1025_err() {
    let input = BtSettings {
        listen_port_range: Some(BtPortRange {
            start: 1024,
            end: 2048,
        }),
        ..BtSettings::default()
    };
    let err = normalize_bt_settings(input).unwrap_err();
    assert!(err.to_string().contains("listen_port_range"));
}

#[test]
fn test_normalize_bt_listen_port_below_1024_filtered() {
    let input = BtSettings {
        listen_port: Some(1023),
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert!(result.listen_port.is_none());
}

#[test]
fn test_normalize_bt_listen_port_none_preserved() {
    let input = BtSettings {
        listen_port: None,
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert!(result.listen_port.is_none());
}

#[test]
fn test_normalize_bt_valid_port_range_and_settings_ok() {
    let input = BtSettings {
        listen_port_range: Some(BtPortRange {
            start: 6881,
            end: 6889,
        }),
        listen_port: Some(6881),
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert_eq!(
        result.listen_port_range,
        Some(BtPortRange {
            start: 6881,
            end: 6889
        })
    );
    assert_eq!(result.listen_port, Some(6881));
}

#[test]
fn test_normalize_bt_engine_tuning_roundtrip() {
    let input = BtSettings {
        seed_choking_algorithm: BtSeedChokingAlgorithm::AntiLeech,
        choking_algorithm: BtChokingAlgorithm::RateBased,
        max_upload_slots_per_torrent: 8,
        max_peers_per_torrent: 256,
        smart_ban_max_failures: 5,
        smart_ban_parole: false,
        eviction_ban_duration_secs: 1200,
        data_contribution_timeout_secs: 30,
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert_eq!(
        result.seed_choking_algorithm,
        BtSeedChokingAlgorithm::AntiLeech
    );
    assert_eq!(result.choking_algorithm, BtChokingAlgorithm::RateBased);
    assert_eq!(result.max_upload_slots_per_torrent, 8);
    assert_eq!(result.max_peers_per_torrent, 256);
    assert_eq!(result.smart_ban_max_failures, 5);
    assert!(!result.smart_ban_parole);
    assert_eq!(result.eviction_ban_duration_secs, 1200);
    assert_eq!(result.data_contribution_timeout_secs, 30);
}

#[test]
fn test_normalize_bt_engine_tuning_clamped() {
    let input = BtSettings {
        max_upload_slots_per_torrent: 0,
        max_peers_per_torrent: 99999,
        data_contribution_timeout_secs: 999999,
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert_eq!(result.max_upload_slots_per_torrent, 1);
    assert_eq!(result.max_peers_per_torrent, 4096);
    assert_eq!(result.data_contribution_timeout_secs, 86_400);
}

#[test]
fn test_normalize_bt_blocklist_fields() {
    let input = BtSettings {
        blocklist_enabled: true,
        blocklist_path: String::from("  /path/to/blocklist.dat  "),
        ..BtSettings::default()
    };
    let result = normalize_bt_settings(input).unwrap();
    assert!(result.blocklist_enabled);
    assert_eq!(result.blocklist_path, "/path/to/blocklist.dat");
}

// -----------------------------------------------------------------------
// load_settings
// -----------------------------------------------------------------------
#[test]
fn test_load_settings_file_not_found_returns_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nonexistent.json");
    let result = load_settings(&path).unwrap();
    // Compare with default
    let default = AppSettings::default();
    assert_eq!(result.proxy.mode, default.proxy.mode);
    assert_eq!(result.proxy.manual_url, default.proxy.manual_url);
}

#[test]
fn test_load_settings_valid_json_full_fields() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let json = r#"{
            "appearance": {"themeColor": "lime", "backgroundOpacity": "default", "colorMode": "system", "showDetailInfo": true, "showHeatmap": true, "sortKey": "added_at", "sortDirection": "desc", "compactView": false, "visibleColumns": ["file", "size", "downloaded", "status", "progress", "speed", "eta"]},
            "proxy": {"mode": "manual", "manualUrl": "http://proxy:8080"},
            "scheduler": {"mode": "automatic", "traditional": {"maxParallelTasks": 3}, "automatic": {"maxParallelThreads": 8, "maxThreadsPerTask": 4, "minThreadsPerTask": 2, "adaptiveProfile": "balanced"}},
            "download": {"defaultDownloadDir": "", "defaultMaxRetries": 5, "defaultChecksum": "blake3", "defaultUserAgent": "TestAgent/1.0"},
            "bt": {"dhtEnabled": true, "trackerList": "", "trackerListUrl": "", "pauseUploadWhenLimitReached": false, "uploadLimitBytes": 0, "uploadRatioLimit": 0.0},
            "logging": {"enabled": true, "level": "info", "filePath": ""},
            "aria2Rpc": {"enabled": false, "port": 6800, "secret": null},
            "cdnAcceleration": {"enabled": false, "activeIp": null, "activeSpeedMbps": null, "lastTestAtMs": null, "lastError": null},
            "notifications": {"enabled": true},
            "ioBaseline": {"bufferLimitMb": 1024, "gameModeBufferMb": 128, "gameMode": false, "maxParallelHdd": 4, "gameModeMaxParallel": 1}
        }"#;
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    drop(file);

    let result = load_settings(&path).unwrap();
    assert_eq!(result.proxy.mode, ProxyMode::Manual);
    assert_eq!(result.proxy.manual_url, "http://proxy:8080");
}

// -----------------------------------------------------------------------
// load_settings recovery
// -----------------------------------------------------------------------
#[test]
fn test_load_settings_corrupt_json_quarantines_and_uses_defaults() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, b"{ this is not json").unwrap();

    let loaded = load_settings(&path).expect("a corrupt file must not fail the load");

    assert_eq!(loaded.proxy.mode, AppSettings::default().proxy.mode);
    let quarantined = path.with_extension("json.corrupt");
    assert!(
        quarantined.exists(),
        "the unreadable file must be kept for inspection"
    );
    assert!(!path.exists(), "the bad file must be moved out of the way");
}

#[test]
fn test_load_settings_corrupt_json_recovers_the_backup() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(
        path.with_extension("json.bak"),
        br#"{"proxy": {"mode": "manual", "manualUrl": "http://backup:1234"}}"#,
    )
    .unwrap();
    fs::write(&path, b"not json").unwrap();

    let loaded = load_settings(&path).expect("the backup must be used");

    assert_eq!(loaded.proxy.mode, ProxyMode::Manual);
    assert_eq!(loaded.proxy.manual_url, "http://backup:1234");
    assert!(path.with_extension("json.corrupt").exists());
}

// -----------------------------------------------------------------------
// persist_settings roundtrip
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_persist_and_load_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("roundtrip.json");

    let original = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: "http://test-proxy:9090".into(),
        },
        ..AppSettings::default()
    };

    // Persist
    persist_settings(&path, &original).await.unwrap();

    // Load back
    let loaded = load_settings(&path).unwrap();

    // Compare fields
    assert_eq!(loaded.proxy.mode, original.proxy.mode);
    assert_eq!(loaded.proxy.manual_url, original.proxy.manual_url);
    assert!(path.exists());
}

#[tokio::test]
async fn test_persist_settings_snapshots_the_previous_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");

    let first = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: "http://first:1".into(),
        },
        ..AppSettings::default()
    };
    persist_settings(&path, &first).await.unwrap();
    assert!(
        !path.with_extension("json.bak").exists(),
        "the first save has nothing to snapshot"
    );

    let second = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: "http://second:2".into(),
        },
        ..AppSettings::default()
    };
    persist_settings(&path, &second).await.unwrap();

    let backup = load_settings(&path.with_extension("json.bak")).unwrap();
    assert_eq!(backup.proxy.manual_url, "http://first:1");
    assert_eq!(
        load_settings(&path).unwrap().proxy.manual_url,
        "http://second:2"
    );
}

// -----------------------------------------------------------------------
// normalize_settings full-field clamping
// -----------------------------------------------------------------------
#[test]
fn test_normalize_settings_clamps_all_fields() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            traditional: TraditionalSchedulerSettings {
                max_parallel_tasks: 100,
            },
            automatic: AutomaticSchedulerSettings {
                max_parallel_threads: 128,
                max_threads_per_task: 200, // > 32 AND > max_parallel_threads (128)
                ..AutomaticSchedulerSettings::default()
            },
            ..SchedulerSettings::default()
        },
        io_baseline: IoBaselineSettings {
            buffer_limit_mb: 16,        // below 64
            game_mode_buffer_mb: 8192,  // above 4096
            max_parallel_hdd: 32,       // above 16
            game_mode_max_parallel: 10, // above 4
            ..IoBaselineSettings::default()
        },
        download: DownloadDefaultsSettings {
            default_max_retries: 100, // above 20
            ..DownloadDefaultsSettings::default()
        },
        last_setup_step: Some(99),  // above 9
        max_in_memory_downloads: 5, // in the invalid 1–9 range
        ..AppSettings::default()
    };

    let result = normalize_settings(settings).unwrap();

    // Scheduler clamps
    assert_eq!(result.scheduler.traditional.max_parallel_tasks, 32);
    assert_eq!(result.scheduler.automatic.max_parallel_threads, 64);
    // 200.clamp(1, 32) = 32, .min(64) = 32
    assert_eq!(result.scheduler.automatic.max_threads_per_task, 32);

    // IO baseline clamps
    assert_eq!(result.io_baseline.buffer_limit_mb, 64);
    assert_eq!(result.io_baseline.game_mode_buffer_mb, 4096);
    assert_eq!(result.io_baseline.max_parallel_hdd, 16);
    assert_eq!(result.io_baseline.game_mode_max_parallel, 4);

    // Download defaults clamp
    assert_eq!(result.download.default_max_retries, 20);

    // last_setup_step clamp
    assert_eq!(result.last_setup_step, Some(9));

    // max_in_memory_downloads: 5 in 1–9 → clamped to 10
    assert_eq!(result.max_in_memory_downloads, 10);
}

#[test]
fn test_normalize_settings_clamps_io_opposite_ends() {
    // Test buffer_limit_mb above 32768 and game_mode_buffer_mb below 16
    let settings = AppSettings {
        io_baseline: IoBaselineSettings {
            buffer_limit_mb: 65536, // above 32768
            game_mode_buffer_mb: 1, // below 16
            ..IoBaselineSettings::default()
        },
        ..AppSettings::default()
    };
    let result = normalize_settings(settings).unwrap();
    assert_eq!(result.io_baseline.buffer_limit_mb, 32768);
    assert_eq!(result.io_baseline.game_mode_buffer_mb, 16);
}

#[test]
fn test_normalize_settings_max_in_memory_edge_cases() {
    // 0 → stays 0 (unlimited / no eviction)
    let settings = AppSettings {
        max_in_memory_downloads: 0,
        ..AppSettings::default()
    };
    let result = normalize_settings(settings).unwrap();
    assert_eq!(result.max_in_memory_downloads, 0);

    // very high → clamped to 10000
    let settings = AppSettings {
        max_in_memory_downloads: 50000,
        ..AppSettings::default()
    };
    let result = normalize_settings(settings).unwrap();
    assert_eq!(result.max_in_memory_downloads, 10000);
}

#[test]
fn test_normalize_settings_last_setup_step_none_preserved() {
    let settings = AppSettings {
        last_setup_step: None,
        ..AppSettings::default()
    };
    let result = normalize_settings(settings).unwrap();
    assert_eq!(result.last_setup_step, None);
}

#[test]
fn test_normalize_url_rewrite_drops_empty_and_renumbers() {
    let settings = UrlRewriteSettings {
        enabled: true,
        rules: vec![
            UrlRewriteRule {
                id: "".into(),
                name: " Valid Rule ".into(),
                enabled: true,
                match_type: MatchType::Prefix,
                pattern: " https://valid.com ".into(),
                replacement_mode: ReplacementMode::PrefixProxy,
                targets: vec![
                    RewriteTarget {
                        url_template: "   ".into(),
                        enabled: true,
                        order: 10,
                    },
                    RewriteTarget {
                        url_template: " https://target.com ".into(),
                        enabled: true,
                        order: 20,
                    },
                ],
                encode_url: false,
                fallback_to_original: true,
                order: 5,
            },
            UrlRewriteRule {
                id: "empty_pattern".into(),
                name: "Drop Me".into(),
                enabled: true,
                match_type: MatchType::Prefix,
                pattern: "   ".into(),
                targets: vec![],
                replacement_mode: ReplacementMode::PrefixProxy,
                encode_url: false,
                fallback_to_original: true,
                order: 6,
            },
        ],
    };
    let normalized = normalize_url_rewrite_settings(settings);
    assert_eq!(normalized.rules.len(), 1);
    let rule = &normalized.rules[0];
    assert!(!rule.id.is_empty());
    assert_eq!(rule.name, "Valid Rule");
    assert_eq!(rule.pattern, "https://valid.com");
    assert_eq!(rule.order, 0);
    assert_eq!(rule.targets.len(), 1);
    assert_eq!(rule.targets[0].url_template, "https://target.com");
    assert_eq!(rule.targets[0].order, 0);
}

#[test]
fn test_bt_lightweight_mode_defaults_off_for_legacy_json() {
    // A settings.json written before the field existed must still deserialize,
    // and must keep the previous always-on engine behaviour.
    let mut value = serde_json::to_value(BtSettings::default()).unwrap();
    value
        .as_object_mut()
        .expect("BtSettings serializes to an object")
        .remove("lightweightMode");

    let bt: BtSettings = serde_json::from_value(value).unwrap();
    assert!(!bt.lightweight_mode);
}

#[test]
fn test_bt_lightweight_mode_round_trips() {
    let bt = BtSettings {
        lightweight_mode: true,
        ..BtSettings::default()
    };
    let json = serde_json::to_string(&bt).unwrap();
    let restored: BtSettings = serde_json::from_str(&json).unwrap();
    assert!(restored.lightweight_mode);
}
