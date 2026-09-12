//! Graceful degradation when the app already owns the global tracing
//! subscriber.
//!
//! Mimics the desktop/server mains: a plain fmt subscriber takes the global
//! slot before `init_logging` runs. Every subsequent settings save used to fail
//! with "failed to update tracing level filter" because the dead reload handle
//! had been stored anyway.
//!
//! This MUST stay in its own integration-test file: cargo runs the tests of one
//! file in the same process, `tracing_subscriber::fmt().init()` panics when the
//! global slot is taken, and any sibling test that calls `init_logging` first
//! would take it (the original CI flake: whichever test won the race, the other
//! panicked inside tracing-subscriber).

use limedl_core::logging::{apply_logging_settings, init_logging};
use limedl_core::types::{LogLevel, LogSettings};
use std::path::{Path, PathBuf};

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("limedl-logging-preinstalled-{tag}"));
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
fn saves_with_preinstalled_subscriber_do_not_fail() {
    tracing_subscriber::fmt().init();
    let dir = tmp_dir("preinstalled");
    init_logging(&LogSettings::default(), &dir).expect("init failed");
    for i in 1..=5 {
        apply_variant(i, &dir);
    }
}
