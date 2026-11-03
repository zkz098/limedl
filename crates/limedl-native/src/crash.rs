//! Crash reporting for the native client.
//!
//! Shared panic-hook and crash-log formatting/rotation live in
//! [`limedl_core::crash`]; this module adds the GUI startup-failure modal
//! dialog (`rfd::MessageDialog`).

#[cfg(test)]
use limedl_core::crash::{CRASH_LOG_MAX_BYTES, rotate_if_needed, timestamp_utc};
pub use limedl_core::crash::{
    crash_log_path, format_crash_report, install_panic_hook, write_crash_report,
};

/// Surface a fatal startup error to the user and record it.
///
/// The caller has already unwound `main`, so the Slint event loop is not
/// running: a blocking message box is safe here and is the only way a GUI
/// process can tell the user why nothing appeared. The state directory is
/// re-derived because the failure may predate its computation.
pub fn report_startup_failure(error: &anyhow::Error) {
    let state_dir = crate::paths::dirs_or_temp_dir().join("downloads");
    let log_path = crash_log_path(&state_dir);
    write_crash_report(
        &log_path,
        &format_crash_report("main", &format!("startup failed: {error:#}"), None, ""),
    );

    let (title, body) = crate::i18n::format_startup_failure(
        crate::i18n::Language::detect_system(),
        &format!("{error:#}"),
        &log_path.display().to_string(),
    );
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(body)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_carries_thread_location_and_payload() {
        let report = format_crash_report(
            "limedl-worker",
            "index out of bounds",
            Some(("src/foo.rs", 42)),
            " 0: backtrace line",
        );
        assert!(report.contains("limedl-worker"));
        assert!(report.contains("src/foo.rs:42"));
        assert!(report.contains("index out of bounds"));
        assert!(report.contains("backtrace line"));
    }

    #[test]
    fn report_without_location_is_still_well_formed() {
        let report = format_crash_report("main", "boom", None, "");
        assert!(report.contains("location: <unknown>"));
        assert!(report.contains("message: boom"));
    }

    #[test]
    fn timestamp_is_iso8601_utc() {
        let stamp = timestamp_utc();
        assert!(stamp.ends_with('Z'), "unexpected stamp: {stamp}");
        assert_eq!(stamp.len(), 20, "unexpected stamp: {stamp}");
    }

    #[test]
    fn crash_reports_append_to_the_state_dir_log() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = crash_log_path(dir.path());
        assert_eq!(path, dir.path().join("logs").join("crash.log"));

        write_crash_report(&path, "first\n");
        write_crash_report(&path, "second\n");

        let contents = std::fs::read_to_string(&path).expect("crash log");
        assert_eq!(contents, "first\nsecond\n");
    }

    #[test]
    fn oversized_crash_log_is_rotated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = crash_log_path(dir.path());
        std::fs::create_dir_all(path.parent().expect("parent")).expect("logs dir");
        std::fs::write(&path, vec![b'x'; (CRASH_LOG_MAX_BYTES + 1) as usize]).expect("seed log");

        rotate_if_needed(&path);

        assert!(!path.exists(), "the oversized log must be moved aside");
        assert!(
            path.with_extension("log.1").exists(),
            "rotated copy missing"
        );
    }
}
