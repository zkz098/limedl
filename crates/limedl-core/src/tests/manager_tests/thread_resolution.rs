//! Thread-mode resolution and parallelism policy.

use super::*;

#[test]
fn supports_parallelism_small_file_no_ranges() {
    assert!(!supports_parallelism(Some(CHUNK_SIZE), false, CHUNK_SIZE));
}

#[test]
fn supports_parallelism_small_file_with_ranges() {
    assert!(!supports_parallelism(Some(CHUNK_SIZE), true, CHUNK_SIZE));
}

#[test]
fn supports_parallelism_large_file_with_ranges() {
    assert!(supports_parallelism(Some(CHUNK_SIZE * 2), true, CHUNK_SIZE));
}

#[test]
fn supports_parallelism_large_file_no_ranges() {
    assert!(!supports_parallelism(
        Some(CHUNK_SIZE * 2),
        false,
        CHUNK_SIZE
    ));
}

#[test]
fn supports_parallelism_unknown_size() {
    assert!(!supports_parallelism(None, false, CHUNK_SIZE));
    assert!(!supports_parallelism(None, true, CHUNK_SIZE));
}

#[test]
fn supports_parallelism_zero_bytes() {
    assert!(!supports_parallelism(Some(0), false, CHUNK_SIZE));
    assert!(!supports_parallelism(Some(0), true, CHUNK_SIZE));
}

#[test]
fn resolve_traditional_adaptive_mode_default_threads() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Traditional,
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        thread_count: None,
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Fixed);
    assert_eq!(requested, Some(DEFAULT_FIXED_THREADS));
    assert_eq!(desired, Some(DEFAULT_FIXED_THREADS));
    assert_eq!(profile, None);
}

#[test]
fn resolve_traditional_custom_thread_count() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Traditional,
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_count: Some(16),
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Fixed);
    assert_eq!(requested, Some(16));
    assert_eq!(desired, Some(16));
    assert_eq!(profile, None);
}

#[test]
fn resolve_traditional_clamped_to_max() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Traditional,
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_count: Some(100),
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Fixed);
    assert_eq!(requested, Some(MAX_TRADITIONAL_THREADS));
    assert_eq!(desired, Some(MAX_TRADITIONAL_THREADS));
    assert_eq!(profile, None);
}

#[test]
fn resolve_automatic_adaptive_balanced() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                adaptive_profile: AdaptiveProfile::Balanced,
                max_threads_per_task: 8,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        thread_count: None,
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Adaptive);
    assert_eq!(requested, None);
    // Balanced → initial_desired_threads(Balanced, 8) = 6
    assert_eq!(desired, Some(6));
    assert_eq!(profile, Some(AdaptiveProfile::Balanced));
}

#[test]
fn resolve_automatic_adaptive_conservative() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                adaptive_profile: AdaptiveProfile::Conservative,
                max_threads_per_task: 8,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        ..Default::default()
    };
    let (mode, _, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Adaptive);
    // Conservative → initial_desired_threads(Conservative, 8) = 4
    assert_eq!(desired, Some(4));
    assert_eq!(profile, Some(AdaptiveProfile::Conservative));
}

#[test]
fn resolve_automatic_adaptive_aggressive() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                adaptive_profile: AdaptiveProfile::Aggressive,
                max_threads_per_task: 8,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        ..Default::default()
    };
    let (mode, _, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Adaptive);
    // Aggressive → initial_desired_threads(Aggressive, 8) = 8
    assert_eq!(desired, Some(8));
    assert_eq!(profile, Some(AdaptiveProfile::Aggressive));
}

#[test]
fn resolve_automatic_adaptive_capped_by_max_threads() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                adaptive_profile: AdaptiveProfile::Aggressive,
                max_threads_per_task: 2, // cap=2, Aggressive→2 (cap replaces min())
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        ..Default::default()
    };
    let (mode, _, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Adaptive);
    // Aggressive→4, capped at 2, then max(1) = 2
    assert_eq!(desired, Some(2));
    assert_eq!(profile, Some(AdaptiveProfile::Aggressive));
}

#[test]
fn resolve_automatic_adaptive_zero_max_threads_clamped() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                adaptive_profile: AdaptiveProfile::Balanced,
                max_threads_per_task: 0, // .max(1) ensures at least 1
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        ..Default::default()
    };
    let (mode, _, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Adaptive);
    // Balanced→(1*0.75).ceil()=1, cap=max(0,1)=1
    assert_eq!(desired, Some(1));
    assert_eq!(profile, Some(AdaptiveProfile::Balanced));
}

#[test]
fn resolve_automatic_fixed_default_threads() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_threads_per_task: 6,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Fixed),
        thread_count: None, // defaults to DEFAULT_FIXED_THREADS=8
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Fixed);
    // 8 clamped to max_threads_per_task=6
    assert_eq!(requested, Some(6));
    assert_eq!(desired, Some(6));
    assert_eq!(profile, None);
}

#[test]
fn resolve_automatic_fixed_explicit_thread_count() {
    let settings = AppSettings {
        scheduler: SchedulerSettings {
            mode: SchedulerMode::Automatic,
            automatic: AutomaticSchedulerSettings {
                max_threads_per_task: 10,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Fixed),
        thread_count: Some(4),
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, true);
    assert_eq!(mode, ThreadMode::Fixed);
    assert_eq!(requested, Some(4));
    assert_eq!(desired, Some(4));
    assert_eq!(profile, None);
}

#[test]
fn resolve_no_parallelism_returns_fixed_single() {
    let settings = AppSettings::default();
    let request = StartDownloadRequest {
        url: String::new(),
        destination_dir: String::new(),
        thread_mode: Some(ThreadMode::Adaptive),
        thread_count: Some(16), // should be ignored
        ..Default::default()
    };
    let (mode, requested, desired, profile) = resolve_thread_settings(&settings, &request, false);
    assert_eq!(mode, ThreadMode::Fixed);
    assert_eq!(requested, Some(1));
    assert_eq!(desired, Some(1));
    assert_eq!(profile, None);
}

#[test]
fn thread_note_no_parallelism() {
    let note = thread_note(false, ThreadMode::Fixed, None);
    assert_eq!(note, Some(String::from("单线程（服务器不支持分段）")));
}

#[test]
fn thread_note_fixed() {
    let note = thread_note(true, ThreadMode::Fixed, None);
    assert_eq!(note, Some(String::from("固定线程")));
}

#[test]
fn thread_note_adaptive_conservative() {
    let note = thread_note(
        true,
        ThreadMode::Adaptive,
        Some(AdaptiveProfile::Conservative),
    );
    assert_eq!(note, Some(String::from("自适应 / 保守")));
}

#[test]
fn thread_note_adaptive_balanced() {
    let note = thread_note(true, ThreadMode::Adaptive, Some(AdaptiveProfile::Balanced));
    assert_eq!(note, Some(String::from("自适应 / 平衡")));
}

#[test]
fn thread_note_adaptive_aggressive() {
    let note = thread_note(
        true,
        ThreadMode::Adaptive,
        Some(AdaptiveProfile::Aggressive),
    );
    assert_eq!(note, Some(String::from("自适应 / 激进")));
}

#[test]
fn thread_note_adaptive_no_profile() {
    let note = thread_note(true, ThreadMode::Adaptive, None);
    assert_eq!(note, None);
}
