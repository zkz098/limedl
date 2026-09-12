//! Reproduction for the "failed to update tracing level filter" error on
//! repeated settings saves within one process lifetime.
//!
//! Scenario: clean process — init + repeated applies must all succeed.
//!
//! The companion scenario (an app that pre-installs its own global subscriber)
//! lives in `logging_preinstalled_subscriber.rs`: it must run in its own test
//! binary, because it takes the process-wide tracing global slot and `init()`
//! panics when another test in the same binary got there first.

use limedl_core::logging::{apply_logging_settings, init_logging};
use limedl_core::types::{LogLevel, LogSettings};
use std::path::{Path, PathBuf};

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("limedl-logging-repro-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn apply_variant(i: usize, dir: &Path) {
    let next = LogSettings {
        level: match i % 3 {
            0 => LogLevel::Debug,
            1 => LogLevel::Info,
            _ => LogLevel::Warn,
        },
        ..LogSettings::default()
    };
    if let Err(e) = apply_logging_settings(&next, dir) {
        panic!("apply_logging_settings #{i} failed: {e:#}");
    }
}

#[test]
fn apply_logging_settings_repeatedly() {
    let dir = tmp_dir("repeat");
    init_logging(&LogSettings::default(), &dir).expect("init failed");
    for i in 1..=5 {
        apply_variant(i, &dir);
    }
}
