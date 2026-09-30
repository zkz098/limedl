use crate::download::{DEFAULT_FIXED_THREADS, MAX_TRADITIONAL_THREADS};
use crate::manifest::Manifest;
use crate::types::{
    AppSettings, AutomaticSchedulerSettings, DownloadState, Priority, SchedulerMode,
    SchedulerSettings, ThreadMode, TraditionalSchedulerSettings,
};

// ── helpers ───────────────────────────────────────────────────────────

fn manifest(url: &str) -> Manifest {
    Manifest {
        id: String::new(),
        url: url.to_string(),
        final_url: url.to_string(),
        user_agent: String::new(),
        extra_headers: vec![],
        destination_dir: String::new(),
        file_name: String::new(),
        file_name_locked: false,
        destination_path: String::new(),
        temp_path: String::new(),
        total_bytes: None,
        downloaded_bytes: 0,
        supports_ranges: true,
        chunk_size: 4194304,
        connection_count: 0,
        thread_mode: ThreadMode::Adaptive,
        requested_thread_count: None,
        desired_thread_count: None,
        allocated_thread_count: None,
        adaptive_profile_snapshot: None,
        thread_note: None,
        etag: None,
        last_modified: None,
        state: DownloadState::Queued,
        cdn_accelerated: false,
        cdn_node_ip: None,
        checksum_mode: crate::types::ChecksumMode::None,
        checksum: None,
        expected_checksum: None,
        error: None,
        created_at_ms: 0,
        updated_at_ms: 0,
        chunks: vec![],
        mirror_url: None,
        mirror_urls: Vec::new(),
        current_mirror_index: 0,
        priority: Priority::Normal,
    }
}

fn settings_traditional() -> AppSettings {
    AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Traditional,
            traditional: TraditionalSchedulerSettings {
                max_parallel_tasks: 3,
            },
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    }
}

fn settings_automatic() -> AppSettings {
    AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    }
}

// ── effective_allocation_cap ──────────────────────────────────────────

#[test]
fn cap_no_ranges_returns_one() {
    let mut m = manifest("https://example.com/file.bin");
    m.supports_ranges = false;
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        1
    );
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_automatic()),
        1
    );
}

#[test]
fn cap_traditional_uses_requested_threads() {
    let mut m = manifest("https://example.com/file.bin");
    m.requested_thread_count = Some(12);
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        12
    );
}

#[test]
fn cap_traditional_falls_back_to_desired() {
    let mut m = manifest("https://example.com/file.bin");
    m.requested_thread_count = None;
    m.desired_thread_count = Some(6);
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        6
    );
}

#[test]
fn cap_traditional_default_when_none_set() {
    let m = manifest("https://example.com/file.bin");
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        DEFAULT_FIXED_THREADS
    );
}

#[test]
fn cap_traditional_clamps_to_max() {
    let mut m = manifest("https://example.com/file.bin");
    m.requested_thread_count = Some(99);
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        MAX_TRADITIONAL_THREADS
    );
}

#[test]
fn cap_traditional_clamps_to_min() {
    let mut m = manifest("https://example.com/file.bin");
    m.requested_thread_count = Some(0);
    assert_eq!(
        super::effective_allocation_cap(&m, &settings_traditional()),
        1
    );
}

#[test]
fn cap_automatic_fixed_uses_requested() {
    let mut m = manifest("https://example.com/file.bin");
    m.thread_mode = ThreadMode::Fixed;
    m.requested_thread_count = Some(5);
    let s = settings_automatic();
    assert_eq!(super::effective_allocation_cap(&m, &s), 5);
}

#[test]
fn cap_automatic_fixed_defaults_to_one() {
    let mut m = manifest("https://example.com/file.bin");
    m.thread_mode = ThreadMode::Fixed;
    m.requested_thread_count = None;
    let s = settings_automatic();
    assert_eq!(super::effective_allocation_cap(&m, &s), 1);
}

#[test]
fn cap_automatic_adaptive_uses_desired() {
    let mut m = manifest("https://example.com/file.bin");
    m.thread_mode = ThreadMode::Adaptive;
    m.desired_thread_count = Some(4);
    let s = settings_automatic();
    assert_eq!(super::effective_allocation_cap(&m, &s), 4);
}

#[test]
fn cap_automatic_adaptive_defaults_to_one() {
    let mut m = manifest("https://example.com/file.bin");
    m.thread_mode = ThreadMode::Adaptive;
    m.desired_thread_count = None;
    let s = settings_automatic();
    assert_eq!(super::effective_allocation_cap(&m, &s), 1);
}

#[test]
fn cap_automatic_clamps_to_effective_task_cap() {
    let mut m = manifest("https://example.com/file.bin");
    m.thread_mode = ThreadMode::Adaptive;
    m.desired_thread_count = Some(99);
    let s = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_threads_per_task: 3,
                ..AutomaticSchedulerSettings::default()
            },
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    };
    assert_eq!(super::effective_allocation_cap(&m, &s), 3);
}

// ── hostname_from_manifest ────────────────────────────────────────────

#[test]
fn hostname_valid_url_returns_host() {
    let m = manifest("https://cdn.example.com/path/file.zip");
    assert_eq!(
        super::hostname_from_manifest(&m),
        Some("cdn.example.com".to_string())
    );
}

#[test]
fn hostname_domain_with_port() {
    let m = manifest("http://localhost:8080/download/file.iso");
    assert_eq!(
        super::hostname_from_manifest(&m),
        Some("localhost".to_string())
    );
}

#[test]
fn hostname_no_host_returns_none() {
    let m = manifest("file:///C:/path/to/file.txt");
    // file:// URIs may have no host_str
    assert_eq!(super::hostname_from_manifest(&m), None);
}

#[test]
fn hostname_invalid_url_does_not_panic() {
    // Completely unparseable
    let m = manifest("\0invalid url\t\n");
    let result = super::hostname_from_manifest(&m);
    assert!(result.is_none());
}

#[test]
fn hostname_empty_string_does_not_panic() {
    let m = manifest("");
    let result = super::hostname_from_manifest(&m);
    assert!(result.is_none());
}

// ── remaining_bytes ───────────────────────────────────────────────────

#[test]
fn remaining_normal_case() {
    let mut m = manifest("https://example.com/file.bin");
    m.total_bytes = Some(1000);
    m.downloaded_bytes = 300;
    assert_eq!(super::remaining_bytes(&m), 700);
}

#[test]
fn remaining_no_total_bytes_returns_zero() {
    let mut m = manifest("https://example.com/file.bin");
    m.total_bytes = None;
    m.downloaded_bytes = 500;
    assert_eq!(super::remaining_bytes(&m), 0);
}

#[test]
fn remaining_total_less_than_downloaded_returns_zero() {
    let mut m = manifest("https://example.com/file.bin");
    m.total_bytes = Some(100);
    m.downloaded_bytes = 500;
    assert_eq!(super::remaining_bytes(&m), 0);
}

#[test]
fn remaining_equal_values_returns_zero() {
    let mut m = manifest("https://example.com/file.bin");
    m.total_bytes = Some(500);
    m.downloaded_bytes = 500;
    assert_eq!(super::remaining_bytes(&m), 0);
}

#[test]
fn remaining_zero_total_and_downloaded() {
    let mut m = manifest("https://example.com/file.bin");
    m.total_bytes = Some(0);
    m.downloaded_bytes = 0;
    assert_eq!(super::remaining_bytes(&m), 0);
}

// ── effective_automatic_task_cap ──────────────────────────────────────

#[test]
fn task_cap_normal() {
    let s = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_threads_per_task: 8,
                ..AutomaticSchedulerSettings::default()
            },
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    };
    assert_eq!(super::effective_automatic_task_cap(&s), 8);
}

#[test]
fn task_cap_zero_clamps_to_one() {
    let s = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_threads_per_task: 0,
                ..AutomaticSchedulerSettings::default()
            },
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    };
    assert_eq!(super::effective_automatic_task_cap(&s), 1);
}

#[test]
fn task_cap_default_is_at_least_one() {
    let s = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            ..SchedulerSettings::default()
        },
        ..AppSettings::default()
    };
    // Default max_threads_per_task is 8, so cap is 8
    assert_eq!(super::effective_automatic_task_cap(&s), 8);
}
