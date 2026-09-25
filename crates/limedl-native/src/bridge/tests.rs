use std::collections::HashMap;

use limedl_core::cdn::speed_test::SpeedTestResult;
use limedl_core::types::*;
use slint::{Model, SharedString};

use super::*;
use crate::i18n::Language;
use crate::LabsFormData;

    #[test]
    fn test_setup_form_roundtrip() {
        let mut s = AppSettings::default();
        s.appearance.language = "zh-CN".into();
        s.appearance.color_mode = ColorMode::Dark;
        s.appearance.theme_color = ThemeColor::Sky;
        s.cdn_acceleration.enabled = true;
        s.aria2_rpc.enabled = false;
        s.aria2_rpc.port = 6800;
        s.aria2_rpc.secret = Some("secret-token".into());
        s.download.default_download_dir = "D:\\dl".into();
        s.autostart = true;
        s.notifications.enabled = true;
        s.proxy.mode = ProxyMode::Manual;
        s.proxy.manual_url = "http://127.0.0.1:7890".into();

        let form = app_settings_to_setup_form(&s, Language::ZhCn);
        assert_eq!(form.language_idx, 0);
        assert_eq!(form.color_mode_idx, 2);
        assert_eq!(form.theme_color_idx, 1);
        assert!(form.cdn_enabled);
        assert!(!form.rpc_enabled);
        assert_eq!(form.rpc_port.as_str(), "6800");
        assert_eq!(form.rpc_secret.as_str(), "secret-token");
        assert_eq!(form.default_dir.as_str(), "D:\\dl");
        assert!(form.autostart);
        assert!(form.notifications_enabled);
        assert_eq!(form.proxy_mode_idx, 2);
        assert_eq!(form.proxy_manual_url.as_str(), "http://127.0.0.1:7890");

        let mut back = AppSettings::default();
        update_app_settings_from_setup_form(&mut back, &form, Language::ZhCn).unwrap();
        assert_eq!(back.appearance.language, "zh-CN");
        assert_eq!(back.appearance.color_mode, ColorMode::Dark);
        assert_eq!(back.appearance.theme_color, ThemeColor::Sky);
        assert!(back.cdn_acceleration.enabled);
        assert!(!back.aria2_rpc.enabled);
        assert_eq!(back.aria2_rpc.secret.as_deref(), Some("secret-token"));
        assert_eq!(back.download.default_download_dir, "D:\\dl");
        assert!(back.autostart);
        assert!(back.notifications.enabled);
        assert_eq!(back.proxy.mode, ProxyMode::Manual);
        assert_eq!(back.proxy.manual_url, "http://127.0.0.1:7890");
    }

    #[test]
    fn test_setup_form_scheduler_presets() {
        // Default scheduler (balanced automatic) maps to preset idx 1.
        let s = AppSettings::default();
        let form = app_settings_to_setup_form(&s, Language::EnUs);
        assert_eq!(form.scheduler_preset_idx, 1);
        assert_eq!(form.chunk_strategy_idx, 0);

        // Energy saver preset.
        let mut s = AppSettings::default();
        s.scheduler.mode = SchedulerMode::Automatic;
        s.scheduler.automatic = AutomaticSchedulerSettings {
            max_parallel_threads: 8,
            max_threads_per_task: 4,
            min_threads_per_task: 2,
            adaptive_profile: AdaptiveProfile::Conservative,
        };
        let form = app_settings_to_setup_form(&s, Language::EnUs);
        assert_eq!(form.scheduler_preset_idx, 0);

        // Traditional scheduler => custom.
        let mut s = AppSettings::default();
        s.scheduler.mode = SchedulerMode::Traditional;
        let form = app_settings_to_setup_form(&s, Language::EnUs);
        assert_eq!(form.scheduler_preset_idx, 3);

        // Applying the max-speed preset writes the full config.
        let mut back = AppSettings::default();
        let mut form = app_settings_to_setup_form(&back, Language::EnUs);
        form.scheduler_preset_idx = 2;
        update_app_settings_from_setup_form(&mut back, &form, Language::EnUs).unwrap();
        assert_eq!(back.scheduler.mode, SchedulerMode::Automatic);
        assert_eq!(back.scheduler.automatic.max_parallel_threads, 32);
        assert_eq!(back.scheduler.automatic.max_threads_per_task, 16);
        assert_eq!(back.scheduler.automatic.min_threads_per_task, 4);
        assert_eq!(back.scheduler.automatic.adaptive_profile, AdaptiveProfile::Aggressive);
    }

    #[test]
    fn test_setup_form_proxy_manual_requires_url() {
        let mut s = AppSettings::default();
        let mut form = app_settings_to_setup_form(&s, Language::ZhCn);
        form.proxy_mode_idx = 2;
        form.proxy_manual_url = SharedString::default();
        assert!(update_app_settings_from_setup_form(&mut s, &form, Language::ZhCn).is_err());
        // The English wizard must not surface Chinese text.
        let err = update_app_settings_from_setup_form(&mut s, &form, Language::EnUs).unwrap_err();
        assert!(!err.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)));

        // Aria2 port 0 / overflow produce localized errors too.
        let mut form = app_settings_to_setup_form(&s, Language::EnUs);
        form.rpc_enabled = true;
        form.rpc_port = SharedString::from("0");
        let err = update_app_settings_from_setup_form(&mut s, &form, Language::EnUs).unwrap_err();
        assert!(err.contains("Aria2 port"));
        form.rpc_port = SharedString::from("70000");
        let err = update_app_settings_from_setup_form(&mut s, &form, Language::EnUs).unwrap_err();
        assert!(err.contains("0-65535"));
    }

    fn sample_summary(
        id: &str,
        name: &str,
        state: DownloadState,
        downloaded: u64,
        total: Option<u64>,
        speed: f64,
        created: u64,
    ) -> DownloadSummary {
        DownloadSummary {
            id: id.to_string(),
            kind: TaskKind::Http,
            state,
            url: format!("https://example.com/{name}"),
            file_name: name.to_string(),
            destination_path: format!("/downloads/{name}"),
            total_bytes: total,
            downloaded_bytes: downloaded,
            connection_count: 4,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: Some(4),
            adaptive_profile: None,
            thread_note: None,
            speed_bytes_per_second: Some(speed),
            eta_seconds: Some(120),
            uploaded_bytes: None,
            upload_speed_bytes_per_second: None,
            peer_count: None,
            upload_status: None,
            info_hash: None,
            expected_checksum: None,
            error: None,
            cdn_accelerated: false,
            cdn_node_ip: None,
            created_at_ms: created,
            priority: limedl_core::types::Priority::Normal,
            seed_count: None,
            leech_count: None,
            download_limit_bps: None,
            upload_limit_bps: None,
            chunks: Vec::new(),
            mirror_url: None,
        }
    }

    #[test]
    fn test_column_visibility_and_sort_persistence() {
        // Defaults apply when the settings list is empty.
        assert!(column_is_visible(&[], "file"));
        assert!(column_is_visible(&[], "eta"));
        assert!(!column_is_visible(&[], "seeds"));

        // An explicit list wins (and unknown keys simply never match).
        let explicit = vec!["file".to_string(), "seeds".to_string()];
        assert!(column_is_visible(&explicit, "seeds"));
        assert!(!column_is_visible(&explicit, "eta"));

        // Every default column key must be a known key.
        for key in DEFAULT_VISIBLE_COLUMNS {
            assert!(COLUMN_KEYS.contains(&key), "unknown default column {key}");
        }

        // Sort key <-> list sort field round trip (all variants).
        for key in [
            limedl_core::types::SortKey::AddedAt,
            limedl_core::types::SortKey::Name,
            limedl_core::types::SortKey::Size,
            limedl_core::types::SortKey::Progress,
            limedl_core::types::SortKey::Speed,
            limedl_core::types::SortKey::State,
        ] {
            assert_eq!(field_to_sort_key(sort_key_to_field(key)), key);
        }

        // Priority wire codes round trip.
        for code in ["high", "normal", "low"] {
            assert_eq!(priority_code(str_to_priority(code)), code);
        }
        assert_eq!(str_to_priority("bogus"), limedl_core::types::Priority::Normal);
    }

    #[test]
    fn test_view_preference_roundtrip_through_form() {
        let mut settings = AppSettings::default();
        settings.appearance.compact_view = true;
        settings.appearance.visible_columns = vec!["file".to_string(), "seeds".to_string()];
        let form = app_settings_to_form(&settings, false, false, "", "", Language::EnUs);
        assert!(form.appearance_compact_view);
        assert!(form.appearance_column_seeds);
        assert!(!form.appearance_column_eta);

        // Saving from the form round-trips the same selection (canonical order,
        // file always present).
        let mut target = AppSettings::default();
        let mut edited = form;
        edited.appearance_column_eta = true;
        edited.appearance_column_seeds = false;
        update_app_settings_from_form(&mut target, &edited, Language::EnUs).unwrap();
        assert!(target.appearance.compact_view);
        assert_eq!(
            target.appearance.visible_columns,
            vec!["file".to_string(), "eta".to_string()]
        );
    }

    #[test]
    fn test_speed_limit_schedule_parsing() {
        let rows = vec![
            SpeedLimitSlotText {
                start_hour: "22".into(),
                end_hour: "6".into(),
                limit_kb: "512".into(),
            },
            SpeedLimitSlotText {
                start_hour: "9".into(),
                end_hour: "18".into(),
                limit_kb: "0".into(),
            },
        ];
        let slots = parse_speed_limit_slots(&rows, Language::EnUs).expect("valid schedule");
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].start_hour, 22);
        assert_eq!(slots[0].end_hour, 6);
        assert_eq!(slots[0].limit_bps, 512 * 1024);
        assert_eq!(slots[1].limit_bps, 0);

        // Hours above 23 are rejected with a localized message.
        let bad_hour = vec![SpeedLimitSlotText {
            start_hour: "24".into(),
            end_hour: "6".into(),
            limit_kb: "0".into(),
        }];
        let err = parse_speed_limit_slots(&bad_hour, Language::EnUs).unwrap_err();
        assert!(err.contains("0 and 23"), "unexpected error: {err}");

        // Empty / garbage input is rejected.
        let garbage = vec![SpeedLimitSlotText {
            start_hour: "".into(),
            end_hour: "6".into(),
            limit_kb: "abc".into(),
        }];
        assert!(parse_speed_limit_slots(&garbage, Language::ZhCn).is_err());

        // An empty schedule parses to an empty list (feature disabled).
        assert!(parse_speed_limit_slots(&[], Language::EnUs).unwrap().is_empty());
    }

    #[test]
    fn test_speed_limit_schedule_model_roundtrip() {
        let settings = AppSettings {
            speed_limit_schedule: vec![
                limedl_core::types::SpeedLimitSlot {
                    start_hour: 1,
                    end_hour: 5,
                    limit_bps: 1024 * 1024,
                },
                limedl_core::types::SpeedLimitSlot {
                    start_hour: 23,
                    end_hour: 2,
                    limit_bps: 0,
                },
            ],
            ..AppSettings::default()
        };

        let rows = speed_limit_slots_from_settings(&settings);
        assert_eq!(rows[0].limit_kb, "1024");
        let items = speed_limit_slots_to_slint(&rows, Language::EnUs);
        assert_eq!(items.len(), 2);
        // Second row wraps midnight and is flagged for the warn-colored hint.
        assert!(items[1].wraps);
        assert!(!items[0].wraps);
        assert!(items[0].summary.contains("01:00"));

        // Feeding the model text back in reproduces the same slots.
        let round_trip = parse_speed_limit_slots(&rows, Language::EnUs).unwrap();
        assert_eq!(round_trip.len(), settings.speed_limit_schedule.len());
        for (parsed, original) in round_trip.iter().zip(settings.speed_limit_schedule.iter()) {
            assert_eq!(parsed.start_hour, original.start_hour);
            assert_eq!(parsed.end_hour, original.end_hour);
            assert_eq!(parsed.limit_bps, original.limit_bps);
        }
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024 * 5), "5.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 2), "2.00 GB");
    }

    #[test]
    fn test_combo_idx_and_value_roundtrip() {
        let cases: [(&[&str], &str); 18] = [
            (&combo::COLOR_MODES, "dark"),
            (&combo::THEME_COLORS, "sky"),
            (&combo::OPACITY_PRESETS, "frosted"),
            (&combo::LANGUAGES, "en-US"),
            (&combo::CLOSE_BEHAVIORS, "minimizeToTray"),
            (&combo::DOUBLE_CLICK_COMPLETED, "open_in_explorer"),
            (&combo::DOUBLE_CLICK_UNCOMPLETED, "toggle_pause_resume"),
            (&combo::CHECKSUMS, "xxh3_128"),
            (&combo::PROXY_MODES, "manual"),
            (&combo::SCHEDULER_MODES, "traditional"),
            (&combo::ADAPTIVE_PROFILES, "balanced"),
            (&combo::CHUNK_STRATEGIES, "fixed"),
            (&combo::ENCRYPTION_MODES, "forced"),
            (&combo::PREALLOC_MODES, "full"),
            (&combo::ANTI_LEECH_ACTIONS, "limit_slots"),
            (&combo::SEED_CHOKING, "round_robin"),
            (&combo::CHOKING_ALGOS, "rate_based"),
            (&combo::LOG_LEVELS, "warn"),
        ];
        for (list, value) in cases {
            let idx = combo::idx_of(list, value);
            assert!(idx > 0 || list.first() == Some(&value), "idx for {value}");
            assert_eq!(combo::value_at(list, idx), value, "roundtrip {value}");
        }
        // Out-of-range / unknown values fall back safely.
        assert_eq!(combo::idx_of(combo::COLOR_MODES, "nope"), 0);
        assert_eq!(combo::value_at(combo::COLOR_MODES, 99), "system");
        assert_eq!(combo::value_at(combo::COLOR_MODES, -3), "system");
    }

    #[test]
    fn test_format_speed() {
        assert_eq!(format_speed(Some(1024.0 * 1024.0 * 2.5)), "2.50 MB/s");
        assert_eq!(format_speed(None), "");
        assert_eq!(format_speed(Some(0.0)), "");
    }

    #[test]
    fn test_format_eta() {
        assert_eq!(format_eta(Some(30), Language::ZhCn), "剩余 30秒");
        assert_eq!(format_eta(Some(125), Language::ZhCn), "剩余 2分5秒");
        assert_eq!(format_eta(Some(3665), Language::ZhCn), "剩余 1小时1分");
        assert_eq!(format_eta(None, Language::ZhCn), "");
    }

    #[test]
    fn test_piece_map_generation() {
        let pieces = vec![
            BtPieceInfo { index: 0, completed: true },
            BtPieceInfo { index: 1, completed: false },
            BtPieceInfo { index: 2, completed: true },
            BtPieceInfo { index: 3, completed: true },
        ];

        let (_img, text) = generate_piece_map_image(&pieces, Language::ZhCn);
        assert!(text.contains("3 / 4"));
        assert!(text.contains("75.0%"));
    }

    #[test]
    fn test_inspector_conversion() {
        let summary = sample_summary(
            "bt:abc",
            "ubuntu.torrent",
            DownloadState::Downloading,
            500,
            Some(1000),
            500.0,
            12345,
        );

        let info = summary_to_inspector_info(&summary, Language::ZhCn);
        assert_eq!(info.id.as_str(), "bt:abc");
        assert_eq!(info.file_name.as_str(), "ubuntu.torrent");
        assert_eq!(info.state_label.as_str(), "下载中");
        assert_eq!(info.progress, 0.5);
    }

    #[test]
    fn test_peer_and_file_conversions() {
        let peer = BtPeerInfo {
            address: "1.2.3.4:6881".to_string(),
            client: "qBittorrent/5.0.0".to_string(),
            flags: "uI".to_string(),
            download_speed: 1024.0 * 1024.0 * 1.5,
            upload_speed: 1024.0 * 500.0,
            progress: 0.85,
        };
        let p_item = peer_info_to_item(&peer);
        assert_eq!(p_item.address.as_str(), "1.2.3.4:6881");
        assert_eq!(p_item.client.as_str(), "qBittorrent/5.0.0");
        assert_eq!(p_item.download_speed.as_str(), "1.50 MB/s");
        assert_eq!(p_item.progress, 0.85);

        // Verify sanitation of control characters and trimming
        let dirty_peer = BtPeerInfo {
            address: "1.2.3.4:6881".to_string(),
            client: "  Transmission\0\u{0007}  ".to_string(),
            flags: "  uI  ".to_string(),
            download_speed: 0.0,
            upload_speed: 0.0,
            progress: 0.0,
        };
        let sanitized_item = peer_info_to_item(&dirty_peer);
        assert_eq!(sanitized_item.client.as_str(), "Transmission");
        assert_eq!(sanitized_item.flags.as_str(), "uI");

        let file = BtFileStatus {
            index: 0,
            path: "movie/video.mp4".to_string(),
            size: 1024 * 1024 * 100,
            downloaded_bytes: 1024 * 1024 * 50,
            included: true,
        };
        let f_item = file_status_to_item(&file);
        assert_eq!(f_item.index, 0);
        assert_eq!(f_item.path.as_str(), "movie/video.mp4");
        assert_eq!(f_item.size_text.as_str(), "100.00 MB");
        assert_eq!(f_item.downloaded_text.as_str(), "50.00 MB");
        assert_eq!(f_item.progress, 0.5);
    }

    #[test]
    fn test_settings_conversion() {
        let mut settings = AppSettings::default();
        settings.download.default_download_dir = "/custom/downloads".to_string();
        settings.scheduler.traditional.max_parallel_tasks = 5;
        settings.global_speed_limit_bps = 1024 * 500;
        settings.bt.dht_enabled = true;
        settings.bt.listen_port = Some(6882);

        let form = app_settings_to_form(&settings, true, false, "IO OK", "D: SSD", Language::ZhCn);
        assert_eq!(form.default_download_dir.as_str(), "/custom/downloads");
        assert_eq!(form.max_parallel_tasks.as_str(), "5");
        assert_eq!(form.global_speed_limit_kb.as_str(), "500");
        assert!(form.dht_enabled);
        assert_eq!(form.listen_port.as_str(), "6882");
        assert!(form.game_mode);
        assert!(!form.overclock_mode);

        let mut updated = AppSettings::default();
        update_app_settings_from_form(&mut updated, &form, Language::ZhCn)
            .expect("valid form should update");
        assert_eq!(updated.download.default_download_dir, "/custom/downloads");
        assert_eq!(updated.scheduler.traditional.max_parallel_tasks, 5);
        assert_eq!(updated.global_speed_limit_bps, 1024 * 500);
        assert_eq!(updated.bt.listen_port, Some(6882));
    }

    #[test]
    fn test_search_and_sorting() {
        let mut store = TaskStore::new();
        let t1 = sample_summary(
            "t1",
            "ubuntu-24.04.iso",
            DownloadState::Downloading,
            500,
            Some(1000),
            5000.0,
            100,
        );
        let t2 = sample_summary(
            "t2",
            "archlinux.iso",
            DownloadState::Paused,
            200,
            Some(2000),
            1000.0,
            200,
        );
        let t3 = sample_summary(
            "t3",
            "fedora-workstation.iso",
            DownloadState::Completed,
            1500,
            Some(1500),
            0.0,
            300,
        );

        store.insert_or_update(t1);
        store.insert_or_update(t2);
        store.insert_or_update(t3);

        // Default: Sort by Created DESC
        let items = store.filtered_items();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].id.as_str(), "t3");
        assert_eq!(items[1].id.as_str(), "t2");
        assert_eq!(items[2].id.as_str(), "t1");

        // Sort by Size DESC (t2: 2000, t3: 1500, t1: 1000)
        store.set_sort_field(SortField::Size);
        let items = store.filtered_items();
        assert_eq!(items[0].id.as_str(), "t2");
        assert_eq!(items[1].id.as_str(), "t3");
        assert_eq!(items[2].id.as_str(), "t1");

        // Sort by Size ASC
        store.toggle_sort_order();
        let items = store.filtered_items();
        assert_eq!(items[0].id.as_str(), "t1");
        assert_eq!(items[1].id.as_str(), "t3");
        assert_eq!(items[2].id.as_str(), "t2");

        // Search Filter
        store.set_search_query("arch".to_string());
        let items = store.filtered_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id.as_str(), "t2");
    }

    #[test]
    fn test_multi_selection() {
        let mut store = TaskStore::new();
        let t1 = sample_summary(
            "t1",
            "file1.zip",
            DownloadState::Downloading,
            100,
            Some(200),
            10.0,
            10,
        );
        let t2 = sample_summary(
            "t2",
            "file2.zip",
            DownloadState::Downloading,
            100,
            Some(200),
            10.0,
            20,
        );

        store.insert_or_update(t1);
        store.insert_or_update(t2);

        assert_eq!(store.selected_count(), 0);

        store.toggle_select("t1");
        assert_eq!(store.selected_count(), 1);

        store.select_all();
        assert_eq!(store.selected_count(), 2);

        store.clear_selection();
        assert_eq!(store.selected_count(), 0);
    }

    #[test]
    fn test_disk_types_and_io_status() {
        let mut disks = HashMap::new();
        disks.insert("C:\\".to_string(), DiskType::Ssd);
        disks.insert("D:\\".to_string(), DiskType::Hdd);

        let disks_text = format_disk_types_map(&disks, Language::ZhCn);
        assert!(disks_text.contains("SSD"));
        assert!(disks_text.contains("HDD"));

        let empty_disks = HashMap::new();
        assert_eq!(
            format_disk_types_map(&empty_disks, Language::ZhCn),
            "未检测到磁盘信息"
        );
        assert_eq!(
            format_disk_types_map(&empty_disks, Language::EnUs),
            "No disk information detected"
        );

        let io_val = serde_json::json!({
            "allocatedBytes": 1024 * 1024 * 64,
            "capacityBytes": 1024 * 1024 * 1024,
            "activeBuffers": 2
        });
        let io_text = format_io_status_json(&io_val, Language::ZhCn);
        let io_text_en = format_io_status_json(&io_val, Language::EnUs);
        assert!(!io_text_en.contains("已用缓存"));
        assert!(io_text.contains("64.00 MB"));
        assert!(io_text.contains("1.00 GB"));
        assert!(io_text.contains("2 个"));
    }

    #[test]
    fn test_labs_form_and_url_rewrite() {
        let mut settings = AppSettings::default();
        settings.cdn_acceleration.enabled = true;
        settings.cdn_acceleration.active_ip = Some("104.16.0.1".to_string());
        settings.cdn_acceleration.active_speed_mbps = Some(45.2);

        let form = app_settings_to_labs_form(
            &settings,
            false,
            "就绪",
            100.0,
            "测速完成",
            Some("+25%"),
            Some("-15ms"),
            Some("104.16.0.2"),
            "104.16.0.0/12",
            false,
            "https://raw.github.com/user/repo/master/README.md",
            "GitHub 镜像",
            &["https://ghproxy.net/https://raw.github.com/user/repo/master/README.md".to_string()],
            Language::ZhCn,
        );

        assert!(form.cdn_enabled);
        assert_eq!(form.cdn_status_type.as_str(), "ready");
        assert_eq!(form.cdn_active_ip.as_str(), "104.16.0.1");
        assert_eq!(form.url_rewrite_test_matched_rule.as_str(), "GitHub 镜像");

        let gh_rule = create_url_rewrite_preset("github", Language::ZhCn).expect("gh preset");
        assert_eq!(gh_rule.name, "GitHub 镜像代理");
        assert_eq!(gh_rule.targets.len(), 2);

        let (matched_rule, candidates) = evaluate_url_rewrite(
            &[gh_rule],
            "https://raw.github.com/user/repo/master/README.md",
        );
        assert_eq!(matched_rule, "GitHub 镜像代理");
        assert!(candidates.len() >= 2);
    }

    #[test]
    fn test_format_timestamp_ms() {
        // Unix timestamp 0: 1970-01-01 00:00:00
        let formatted = format_timestamp_ms(0);
        assert_eq!(formatted, "1970-01-01 00:00:00");

        // 1_700_000_000_000 ms -> 2023-11-14 22:13:20
        let formatted2 = format_timestamp_ms(1_700_000_000_000);
        assert_eq!(formatted2, "2023-11-14 22:13:20");
    }

    #[test]
    fn test_update_app_settings_from_labs_form_cdn() {
        let mut settings = AppSettings::default();
        let mut form = LabsFormData {
            cdn_enabled: true,
            cdn_provider: SharedString::from("custom"),
            cdn_custom_test_url: SharedString::from("https://example.com/test.bin"),
            cdn_custom_cidrs: SharedString::from("1.2.3.0/24, 4.5.6.0/24"),
            ..LabsFormData::default()
        };

        update_app_settings_from_labs_form(&mut settings, &form);
        assert!(settings.cdn_acceleration.enabled);
        assert_eq!(settings.cdn_acceleration.provider, "custom");
        assert_eq!(
            settings.cdn_acceleration.custom_test_url.as_deref(),
            Some("https://example.com/test.bin")
        );
        assert_eq!(
            settings.cdn_acceleration.custom_cidrs.as_deref(),
            Some("1.2.3.0/24, 4.5.6.0/24")
        );

        // Empty values should turn into None
        form.cdn_custom_test_url = SharedString::from("   ");
        form.cdn_custom_cidrs = SharedString::from("");
        update_app_settings_from_labs_form(&mut settings, &form);
        assert_eq!(settings.cdn_acceleration.custom_test_url, None);
        assert_eq!(settings.cdn_acceleration.custom_cidrs, None);
    }

    #[test]
    fn test_cdn_candidates_to_slint_mapping() {
        let results = vec![
            SpeedTestResult {
                ip: "104.16.0.1".parse().unwrap(),
                tcp_latency_ms: 18.5,
                throughput_mbps: Some(52.34),
                error: None,
            },
            SpeedTestResult {
                ip: "104.16.0.2".parse().unwrap(),
                tcp_latency_ms: 45.0,
                throughput_mbps: None,
                error: Some("timeout".to_string()),
            },
        ];

        let model = cdn_candidates_to_slint(&results, "104.16.0.1");
        assert_eq!(model.row_count(), 2);

        let row0 = model.row_data(0).unwrap();
        assert_eq!(row0.ip.as_str(), "104.16.0.1");
        assert!(row0.is_active);
        assert!(!row0.is_failed);
        assert_eq!(row0.throughput_text.as_str(), "52.34 MB/s");

        let row1 = model.row_data(1).unwrap();
        assert_eq!(row1.ip.as_str(), "104.16.0.2");
        assert!(!row1.is_active);
        assert!(row1.is_failed);
        assert_eq!(row1.throughput_text.as_str(), "-");
    }

    #[test]
    fn test_parse_clipboard_download_text() {
        assert_eq!(parse_clipboard_download_text(""), ClipboardPayload::Empty);
        assert_eq!(parse_clipboard_download_text("   \n\t  "), ClipboardPayload::Empty);
        assert_eq!(
            parse_clipboard_download_text("Hello world, this is not a link"),
            ClipboardPayload::Empty
        );
        assert_eq!(
            parse_clipboard_download_text("  \"https://example.com/file.zip\"  "),
            ClipboardPayload::SingleUrl("https://example.com/file.zip".to_string())
        );
        assert_eq!(
            parse_clipboard_download_text("magnet:?xt=urn:btih:0123456789abcdef"),
            ClipboardPayload::SingleUrl("magnet:?xt=urn:btih:0123456789abcdef".to_string())
        );

        let batch = "https://example.com/1.zip\nhttps://example.com/2.zip\nhttps://example.com/3.zip";
        assert_eq!(
            parse_clipboard_download_text(batch),
            ClipboardPayload::BatchUrls(vec![
                "https://example.com/1.zip".to_string(),
                "https://example.com/2.zip".to_string(),
                "https://example.com/3.zip".to_string(),
            ])
        );

        let mixed = "Just some text\nhttps://example.com/1.zip\nanother text\nhttps://example.com/2.zip";
        assert_eq!(
            parse_clipboard_download_text(mixed),
            ClipboardPayload::BatchUrls(vec![
                "https://example.com/1.zip".to_string(),
                "https://example.com/2.zip".to_string(),
            ])
        );
    }

    #[test]
    fn test_select_range() {
        let mut store = TaskStore::new();
        for i in 1..=5 {
            let summary = sample_summary(
                &format!("http:task-{i}"),
                &format!("file_{i}.zip"),
                DownloadState::Downloading,
                i * 100,
                Some(1000),
                100.0,
                i * 1000,
            );
            store.insert_or_update(summary);
        }

        // Sort by Name asc so order is task-1 .. task-5
        store.set_sort_field(SortField::Name);
        if !store.sort_asc() {
            store.toggle_sort_order();
        }

        // 1. Initial range selection with no prior anchor -> selects only task-2
        store.select_range("http:task-2");
        assert_eq!(store.selected_count(), 1);
        assert!(store.selected_ids().contains(&"http:task-2".to_string()));

        // 2. Forward range selection from task-2 to task-4 -> selects task-2, task-3, task-4
        store.select_range("http:task-4");
        assert_eq!(store.selected_count(), 3);
        assert!(store.selected_ids().contains(&"http:task-2".to_string()));
        assert!(store.selected_ids().contains(&"http:task-3".to_string()));
        assert!(store.selected_ids().contains(&"http:task-4".to_string()));

        // 3. Clear selection resets anchor
        store.clear_selection();
        assert_eq!(store.selected_count(), 0);

        // 4. Backward range selection: toggle task-4 first, then range select task-1
        store.toggle_select("http:task-4");
        store.select_range("http:task-1");
        assert_eq!(store.selected_count(), 4);
        assert!(store.selected_ids().contains(&"http:task-1".to_string()));
        assert!(store.selected_ids().contains(&"http:task-2".to_string()));
        assert!(store.selected_ids().contains(&"http:task-3".to_string()));
        assert!(store.selected_ids().contains(&"http:task-4".to_string()));
    }

    #[test]
    fn test_detect_file_category() {
        assert_eq!(detect_file_category("movie.mp4"), "video");
        assert_eq!(detect_file_category("clip.MKV"), "video");
        assert_eq!(detect_file_category("song.mp3"), "audio");
        assert_eq!(detect_file_category("lossless.flac"), "audio");
        assert_eq!(detect_file_category("archive.7z"), "archive");
        assert_eq!(detect_file_category("image.iso"), "archive");
        assert_eq!(detect_file_category("manual.pdf"), "document");
        assert_eq!(detect_file_category("readme.MD"), "document");
        assert_eq!(detect_file_category("setup.exe"), "installer");
        assert_eq!(detect_file_category("package.msi"), "installer");
        assert_eq!(detect_file_category("photo.png"), "image");
        assert_eq!(detect_file_category("drawing.webp"), "image");
        assert_eq!(detect_file_category("unknown.xyz"), "default");
        assert_eq!(detect_file_category("no_extension"), "default");
    }
