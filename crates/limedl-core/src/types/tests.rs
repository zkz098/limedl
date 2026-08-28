use super::*;
use crate::error::DownloadError;

#[test]
fn test_cdn_settings_round_trip() {
    let original = CdnAccelerationSettings {
        enabled: true,
        provider: "cloudflare".into(),
        custom_test_url: None,
        custom_cidrs: None,
        active_ip: Some("192.168.1.100".into()),
        active_speed_mbps: Some(45.5),
        last_test_at_ms: Some(1700000000000),
        last_error: Some("timeout".into()),
    };
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: CdnAccelerationSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(original, deserialized);
}

#[test]
fn test_cdn_settings_defaults() {
    let json = "{}";
    let settings: CdnAccelerationSettings = serde_json::from_str(json).unwrap();
    assert!(!settings.enabled);
    assert!(settings.provider.is_empty());
    assert!(settings.custom_test_url.is_none());
    assert!(settings.custom_cidrs.is_none());
    assert!(settings.active_ip.is_none());
    assert!(settings.active_speed_mbps.is_none());
    assert!(settings.last_test_at_ms.is_none());
    assert!(settings.last_error.is_none());
}

#[test]
fn test_app_settings_backward_compat() {
    let json = r#"{
        "appearance": {"themeColor": "lime", "backgroundOpacity": "default", "colorMode": "system", "showDetailInfo": true, "showHeatmap": true, "sortKey": "added_at", "sortDirection": "desc", "compactView": false, "visibleColumns": ["file", "size", "downloaded", "status", "progress", "speed", "eta"]},
        "proxy": {"mode": "system", "manualUrl": ""},
        "scheduler": {"mode": "traditional", "traditional": {"maxParallelTasks": 3}, "automatic": {"maxParallelThreads": 8, "maxThreadsPerTask": 4, "minThreadsPerTask": 1, "adaptiveProfile": "conservative"}},
        "download": {"defaultDownloadDir": "", "defaultMaxRetries": 3, "defaultChecksum": "blake3", "defaultUserAgent": ""},
        "bt": {"pauseUploadWhenLimitReached": false, "uploadLimitBytes": 0, "uploadRatioLimit": 0, "dhtEnabled": true, "trackerList": "", "trackerListUrl": ""},
        "networkLearning": {"deviceMode": "fixed", "currentSceneId": "", "scenes": []},
        "logging": {"enabled": false, "level": "info", "filePath": ""},
        "aria2Rpc": {"enabled": false, "port": 6800, "secret": null}
    }"#;
    let settings: AppSettings = serde_json::from_str(json).unwrap();
    assert!(!settings.cdn_acceleration.enabled);
}

#[test]
fn test_settings_round_trip_with_cdn() {
    let original = AppSettings {
        cdn_acceleration: CdnAccelerationSettings {
            enabled: true,
            provider: "fastly".into(),
            custom_test_url: None,
            custom_cidrs: None,
            active_ip: Some("10.0.0.1".into()),
            active_speed_mbps: Some(88.3),
            last_test_at_ms: Some(1700000000000),
            last_error: None,
        },
        ..AppSettings::default()
    };
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: AppSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(original.cdn_acceleration, deserialized.cdn_acceleration);
    assert!(deserialized.cdn_acceleration.enabled);
    assert_eq!(
        deserialized.cdn_acceleration.active_ip.as_deref(),
        Some("10.0.0.1")
    );
    assert_eq!(deserialized.cdn_acceleration.active_speed_mbps, Some(88.3));
}

// ── TaskId serde round-trip ────────────────────────────────────────

#[test]
fn task_id_http_round_trip() {
    let uuid = uuid::Uuid::new_v4();
    let original = TaskId::Http(uuid);
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: TaskId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, original);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["kind"], "http");
    assert_eq!(parsed["id"], uuid.to_string());
}

#[cfg(feature = "bt")]
#[test]
fn task_id_bt_round_trip() {
    let hash = irontide::core::Id20::from([0xab; 20]);
    let original = TaskId::Bt(hash);
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: TaskId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, original);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["kind"], "bt");
    assert_eq!(parsed["id"], "abababababababababababababababababababab");
}

#[cfg(feature = "bt")]
#[test]
fn task_id_legacy_bt_string() {
    let json = "\"bt:abcdef0123456789abcdef0123456789abcdef01\"";
    let deserialized: TaskId = serde_json::from_str(json).unwrap();
    assert!(matches!(deserialized, TaskId::Bt(_)));
    assert_eq!(
        deserialized.raw_id(),
        "abcdef0123456789abcdef0123456789abcdef01"
    );
}

#[test]
fn task_id_legacy_http_string() {
    let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
    let json = format!("\"http:{uuid_str}\"");
    let deserialized: TaskId = serde_json::from_str(&json).unwrap();
    assert_eq!(
        deserialized,
        TaskId::Http(uuid::Uuid::parse_str(uuid_str).unwrap())
    );
}

#[test]
fn task_id_legacy_bare_uuid() {
    let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
    let json = format!("\"{uuid_str}\"");
    let deserialized: TaskId = serde_json::from_str(&json).unwrap();
    assert_eq!(
        deserialized,
        TaskId::Http(uuid::Uuid::parse_str(uuid_str).unwrap())
    );
}

#[test]
fn task_id_malformed_uuid_returns_error() {
    let json = "\"http:not-a-uuid\"";
    let result: Result<TaskId, _> = serde_json::from_str(json);
    assert!(result.is_err(), "malformed UUID should fail");
}

#[test]
fn task_id_missing_kind_field_returns_error() {
    let json = r#"{"id": "550e8400-e29b-41d4-a716-446655440000"}"#;
    let result: Result<TaskId, _> = serde_json::from_str(json);
    assert!(result.is_err(), "missing kind should fail");
}

#[test]
fn task_id_missing_id_field_returns_error() {
    let json = r#"{"kind": "http"}"#;
    let result: Result<TaskId, _> = serde_json::from_str(json);
    assert!(result.is_err(), "missing id should fail");
}

// ── classify_kind ──────────────────────────────────────────────────

#[cfg(feature = "bt")]
#[test]
fn classify_kind_explicit_kind_wins() {
    let req = StartDownloadRequest {
        kind: Some(TaskKind::Bt),
        url: "https://example.com/file.zip".into(),
        ..StartDownloadRequest::default()
    };
    assert_eq!(req.classify_kind().unwrap(), TaskKind::Bt);
}

#[cfg(feature = "bt")]
#[test]
fn classify_kind_magnet_link() {
    let req = StartDownloadRequest {
        kind: None,
        url: "magnet:?xt=urn:btih:abcdef0123456789abcdef0123456789abcdef01".into(),
        ..StartDownloadRequest::default()
    };
    assert_eq!(req.classify_kind().unwrap(), TaskKind::Bt);
}

#[cfg(feature = "bt")]
#[test]
fn classify_kind_torrent_url_suffix() {
    let req = StartDownloadRequest {
        kind: None,
        url: "https://example.com/file.torrent".into(),
        ..StartDownloadRequest::default()
    };
    assert_eq!(req.classify_kind().unwrap(), TaskKind::Bt);
}

#[cfg(feature = "bt")]
#[test]
fn classify_kind_torrent_extension_checks_path() {
    let req = StartDownloadRequest {
        kind: None,
        url: "/some/path/debian-12.torrent".into(),
        ..StartDownloadRequest::default()
    };
    assert_eq!(req.classify_kind().unwrap(), TaskKind::Bt);
}

#[test]
fn classify_kind_http_url() {
    let req = StartDownloadRequest {
        kind: None,
        url: "https://cdn.example.com/file.zip".into(),
        ..StartDownloadRequest::default()
    };
    assert_eq!(req.classify_kind().unwrap(), TaskKind::Http);
}

#[test]
fn classify_kind_unknown_scheme_returns_error() {
    let req = StartDownloadRequest {
        kind: None,
        url: "ftp://example.com/file.zip".into(),
        ..StartDownloadRequest::default()
    };
    let result = req.classify_kind();
    assert!(matches!(result, Err(DownloadError::UnsupportedScheme)));
}
