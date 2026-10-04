//! Crash reporting for the native client.
//!
//! The release profile builds with `panic = "abort"` (see the workspace
//! `Cargo.toml`) to keep unwinding machinery out of the binary. The panic hook
//! is still invoked on the way to `abort()`, so installing one here is the only
//! chance to leave evidence behind: without it a panic in a GUI process — which
//! has no console (`windows_subsystem = "windows"`) — makes the window vanish
//! with nothing written anywhere.
//!
//! The report goes to `<state_dir>/logs/crash.log`, next to the regular log, and
//! is also echoed to stderr for terminal launches.

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// Rotation threshold: once the crash log passes this size the previous file is
/// moved aside, so a crash loop cannot fill the disk.
const CRASH_LOG_MAX_BYTES: u64 = 1024 * 1024;

/// Where the crash report is written, given the core state directory.
pub fn crash_log_path(state_dir: &Path) -> PathBuf {
    state_dir.join("logs").join("crash.log")
}

/// Install the process-wide panic hook. Call once, as early as possible.
pub fn install_panic_hook(state_dir: &Path) {
    let path = crash_log_path(state_dir);
    rotate_if_needed(&path);

    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let report = format_crash_report(
            thread.name().unwrap_or("<unnamed>"),
            info.payload_as_str().unwrap_or("<non-string panic payload>"),
            info.location().map(|l| (l.file(), l.line())),
            &std::backtrace::Backtrace::force_capture().to_string(),
        );
        write_crash_report(&path, &report);
    }));
}

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

/// Render one crash, including a `file:line` location and the payload.
fn format_crash_report(
    thread: &str,
    payload: &str,
    location: Option<(&str, u32)>,
    backtrace: &str,
) -> String {
    let location = match location {
        Some((file, line)) => format!("{file}:{line}"),
        None => "<unknown>".to_string(),
    };
    format!(
        "\n===== limedl panic at {} =====\nthread: {thread}\nlocation: {location}\nmessage: {payload}\n--- backtrace ---\n{backtrace}\n",
        timestamp_utc(),
    )
}

/// Append `report` to `path`, creating the parent directory if needed.
///
/// Best effort by design: a panic hook that itself panics would abort the
/// process without a report, so every failure here is swallowed after being
/// mirrored to stderr.
fn write_crash_report(path: &Path, report: &str) {
    eprintln!("{report}");
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        eprintln!("[limedl] could not create crash log directory: {error}");
        return;
    }
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(mut file) => {
            if let Err(error) = file.write_all(report.as_bytes()) {
                eprintln!("[limedl] could not write crash log: {error}");
            }
        }
        Err(error) => eprintln!("[limedl] could not open crash log {}: {error}", path.display()),
    }
}

/// Rotate `path` to `<name>.1` once it exceeds the size cap.
fn rotate_if_needed(path: &Path) {
    let too_big = std::fs::metadata(path)
        .map(|meta| meta.len() > CRASH_LOG_MAX_BYTES)
        .unwrap_or(false);
    if !too_big {
        return;
    }
    let rotated = path.with_extension("log.1");
    if let Err(error) = std::fs::rename(path, &rotated) {
        eprintln!(
            "[limedl] could not rotate crash log {}: {error}",
            path.display()
        );
    }
}

/// `YYYY-MM-DDThh:mm:ssZ`, built from calendar components so it needs neither
/// the `formatting` feature nor a resolvable local UTC offset (the hook must
/// never be the thing that fails).
fn timestamp_utc() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
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
        assert!(path.with_extension("log.1").exists(), "rotated copy missing");
    }
}
