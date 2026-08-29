//! Reproduction for the "failed to update tracing level filter" error on
//! repeated settings saves within one process lifetime.
//!
//! Scenario 1: clean process — init + repeated applies must all succeed.
//! Scenario 2: app pre-installs its own global subscriber (like the old
//! native/server mains did) — saves must degrade gracefully instead of
//! failing with a dead reload handle.

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

#[test]
fn saves_with_preinstalled_subscriber_do_not_fail() {
    // Mimic the old native/server mains: a plain fmt subscriber takes the
    // global slot before core's init_logging runs. Every subsequent settings
    // save used to fail with "failed to update tracing level filter".
    tracing_subscriber::fmt().init();
    let dir = tmp_dir("preinstalled");
    init_logging(&LogSettings::default(), &dir).expect("init failed");
    for i in 1..=5 {
        apply_variant(i, &dir);
    }
}
