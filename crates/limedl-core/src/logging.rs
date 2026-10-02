use std::{
    fs::{self, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{Duration, SystemTime},
};

use parking_lot::RwLock;

use anyhow::Context;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    fmt::{self, MakeWriter},
    layer::{Layer as _, SubscriberExt},
    reload,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Fallback console filter when `RUST_LOG` is unset (matches the historical
/// console behavior of the desktop/server apps).
const DEFAULT_CONSOLE_FILTER: &str = "info,limedl=debug";

use super::types::{LogLevel, LogSettings};

static LOGGER_CONTROL: OnceLock<LoggerControl> = OnceLock::new();

#[derive(Clone)]
struct LoggerRuntime {
    enabled: bool,
    file_path: PathBuf,
}

struct LoggerControl {
    runtime: Arc<RwLock<LoggerRuntime>>,
    level_reload: reload::Handle<LevelFilter, tracing_subscriber::Registry>,
}

#[derive(Clone)]
struct DynamicFileWriter {
    runtime: Arc<RwLock<LoggerRuntime>>,
}

struct DynamicFileWriterGuard {
    file: Option<BufWriter<fs::File>>,
}

impl Write for DynamicFileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(file) = &mut self.file {
            file.write(buf)
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = &mut self.file {
            file.flush()
        } else {
            Ok(())
        }
    }
}

impl<'a> MakeWriter<'a> for DynamicFileWriter {
    type Writer = DynamicFileWriterGuard;

    fn make_writer(&'a self) -> Self::Writer {
        let runtime = self.runtime.read();

        if !runtime.enabled {
            return DynamicFileWriterGuard { file: None };
        }

        if let Some(parent) = runtime.file_path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            eprintln!(
                "[limedl] failed to create log directory {}: {error}",
                parent.display()
            );
            return DynamicFileWriterGuard { file: None };
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&runtime.file_path)
            .map_err(|error| {
                eprintln!(
                    "[limedl] failed to open log file {}: {error}",
                    runtime.file_path.display()
                );
                error
            })
            .ok()
            .map(|f| BufWriter::with_capacity(8192, f));

        DynamicFileWriterGuard { file }
    }
}

pub fn init_logging(settings: &LogSettings, state_dir: &Path) -> anyhow::Result<()> {
    if LOGGER_CONTROL.get().is_some() {
        return apply_logging_settings(settings, state_dir);
    }

    let file_path = resolve_log_file_path(settings, state_dir);

    // Perform startup log rotation and retention cleanup
    perform_startup_rotation(
        &file_path,
        settings.retention_count,
        settings.retention_days,
    );

    let runtime = Arc::new(RwLock::new(LoggerRuntime {
        enabled: settings.enabled,
        file_path,
    }));

    let (level_layer, level_reload) = reload::Layer::new(to_level_filter(settings.level));

    // Console output keeps `RUST_LOG` semantics via a per-layer static filter.
    let console_layer = fmt::layer()
        .with_target(true)
        .with_ansi(true)
        .with_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(DEFAULT_CONSOLE_FILTER)),
        );

    let file_layer = fmt::layer()
        .with_target(true)
        .with_ansi(false)
        .with_writer(DynamicFileWriter {
            runtime: runtime.clone(),
        });

    let init_result = tracing_subscriber::registry()
        .with(level_layer)
        .with(console_layer)
        .with(file_layer)
        .try_init();

    if let Err(e) = &init_result {
        // A global subscriber is already installed (an app that set up its own
        // tracing, or a parallel test). The layers built above — including the
        // `reload::Layer` behind `level_reload` — are dropped together with
        // the subscriber, so the handle MUST NOT be stored: a dead handle
        // would make every later apply_logging_settings() fail permanently
        // (Weak::upgrade → SubscriberGone), which surfaced as "failed to
        // update tracing level filter" on the second settings save. Degrade
        // gracefully instead — the pre-installed subscriber keeps working.
        tracing::warn!(
            "global tracing subscriber already installed; file logging and runtime level reload are unavailable: {e}"
        );
        return Ok(());
    }

    let _ = LOGGER_CONTROL.set(LoggerControl {
        runtime,
        level_reload,
    });

    tracing::info!("logging initialized");
    Ok(())
}

pub fn apply_logging_settings(settings: &LogSettings, state_dir: &Path) -> anyhow::Result<()> {
    let Some(control) = LOGGER_CONTROL.get() else {
        return init_logging(settings, state_dir);
    };

    // `reload::Handle::modify` uses a try-lock internally, so concurrent
    // settings saves (e.g. settings + labs dialogs saved at once, or a
    // double-invoked save) can briefly contend on the lock and return a
    // spurious "poisoned" error. Retry briefly before failing the save.
    let mut filter_applied =
        control.level_reload.modify(|level| *level = to_level_filter(settings.level));
    for _ in 0..10 {
        if filter_applied.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
        filter_applied =
            control.level_reload.modify(|level| *level = to_level_filter(settings.level));
    }
    filter_applied.context("failed to update tracing level filter")?;

    let mut runtime = control.runtime.write();
    runtime.enabled = settings.enabled;
    runtime.file_path = resolve_log_file_path(settings, state_dir);
    let log_path = runtime.file_path.clone();
    drop(runtime);

    tracing::info!(
        enabled = settings.enabled,
        level = ?settings.level,
        path = %log_path.display(),
        "logging settings updated"
    );
    Ok(())
}

fn resolve_log_file_path(settings: &LogSettings, state_dir: &Path) -> PathBuf {
    let configured = settings.file_path.trim();
    if configured.is_empty() {
        return state_dir.join("logs").join("limedl.log");
    }

    PathBuf::from(configured)
}

/// Resolve the directory that contains the log file, creating it if missing.
/// Used by the "open current log directory" action in settings.
pub fn log_dir_for(settings: &LogSettings, state_dir: &Path) -> io::Result<PathBuf> {
    let file_path = resolve_log_file_path(settings, state_dir);
    let dir = file_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(state_dir);
    fs::create_dir_all(dir)?;
    Ok(dir.to_path_buf())
}

fn to_level_filter(level: LogLevel) -> LevelFilter {
    match level {
        LogLevel::Trace => LevelFilter::TRACE,
        LogLevel::Debug => LevelFilter::DEBUG,
        LogLevel::Info => LevelFilter::INFO,
        LogLevel::Warn => LevelFilter::WARN,
        LogLevel::Error => LevelFilter::ERROR,
    }
}

/// Find all rotated log files matching `{stem}.{N}.{ext}` in the log directory,
/// sorted by rotation number descending.
fn find_rotated_logs(log_path: &Path) -> Vec<(PathBuf, u32)> {
    let dir = match log_path.parent() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let stem = match log_path.file_stem() {
        Some(s) => s,
        None => return Vec::new(),
    };
    let ext = match log_path.extension() {
        Some(e) => e,
        None => return Vec::new(),
    };

    let prefix = {
        let mut p = stem.to_string_lossy().to_string();
        p.push('.');
        p
    };

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut result: Vec<(PathBuf, u32)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension() != Some(ext) {
            continue;
        }
        let Some(file_stem_os) = path.file_stem() else {
            continue;
        };
        let name_str = file_stem_os.to_string_lossy();
        if !name_str.starts_with(&prefix) {
            continue;
        }
        let num_str = &name_str[prefix.len()..];
        if let Ok(num) = num_str.parse::<u32>() {
            result.push((path, num));
        }
    }

    // Sort descending by rotation number so we rename highest first
    result.sort_by_key(|b| std::cmp::Reverse(b.1));
    result
}

/// Shift-right rotate log files: `limedl.log` → `limedl.1.log`,
/// `limedl.N.log` → `limedl.(N+1).log`.
fn rotate_startup_logs(log_path: &Path) {
    let dir = match log_path.parent() {
        Some(d) => d,
        None => return,
    };
    let stem = match log_path.file_stem() {
        Some(s) => s.to_string_lossy().to_string(),
        None => return,
    };
    let ext = match log_path.extension() {
        Some(e) => e.to_string_lossy().to_string(),
        None => return,
    };

    // Rename existing rotated logs from highest to lowest to avoid collision
    let logs = find_rotated_logs(log_path);
    for (path, num) in &logs {
        let new_name = format!("{stem}.{}.{ext}", num + 1);
        let new_path = dir.join(&new_name);
        if let Err(e) = fs::rename(path, &new_path) {
            eprintln!(
                "[limedl] failed to rotate log file {} -> {}: {e}",
                path.display(),
                new_path.display()
            );
        }
    }

    // Rename current log file to .1
    let current = log_path;
    if current.exists() {
        let first_name = format!("{stem}.1.{ext}");
        let first_path = dir.join(&first_name);
        if let Err(e) = fs::rename(current, &first_path) {
            eprintln!(
                "[limedl] failed to rotate current log file {} -> {}: {e}",
                current.display(),
                first_path.display()
            );
        }
    }
}

/// Delete rotated log files where rotation_number > `count`.
fn cleanup_by_count(log_path: &Path, count: u32) {
    let logs = find_rotated_logs(log_path);
    for (path, num) in &logs {
        if *num > count
            && let Err(e) = fs::remove_file(path)
        {
            eprintln!(
                "[limedl] failed to remove old log file {}: {e}",
                path.display()
            );
        }
    }
}

/// Delete log files (rotated and current) older than `days` days.
fn cleanup_by_age(log_path: &Path, days: u32) {
    let max_age = Duration::from_secs(days as u64 * 86400);
    let now = SystemTime::now();

    // Check rotated logs
    let logs = find_rotated_logs(log_path);
    for (path, _num) in &logs {
        match fs::metadata(path) {
            Ok(meta) => match meta.modified() {
                Ok(modified) => {
                    if let Ok(age) = now.duration_since(modified)
                        && age > max_age
                        && let Err(e) = fs::remove_file(path)
                    {
                        eprintln!(
                            "[limedl] failed to remove old log file {}: {e}",
                            path.display()
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[limedl] failed to get modified time for {}: {e}",
                        path.display()
                    );
                }
            },
            Err(e) => {
                eprintln!("[limedl] failed to stat log file {}: {e}", path.display());
            }
        }
    }

    // Also check the current log file (may be old if logging was disabled)
    if let Ok(meta) = fs::metadata(log_path)
        && let Ok(modified) = meta.modified()
        && let Ok(age) = now.duration_since(modified)
        && age > max_age
        && let Err(e) = fs::remove_file(log_path)
    {
        eprintln!(
            "[limedl] failed to remove old current log file {}: {e}",
            log_path.display()
        );
    }
}

/// Perform startup log rotation and retention cleanup.
/// Acquires an exclusive file lock on `<log_dir>/.lock` to prevent concurrent
/// rotation from multiple instances.
fn perform_startup_rotation(
    log_path: &Path,
    retention_count: Option<u32>,
    retention_days: Option<u32>,
) {
    let log_dir = match log_path.parent() {
        Some(d) => d,
        None => return,
    };

    // Ensure the directory exists before touching the lock/log files
    if let Err(e) = fs::create_dir_all(log_dir) {
        eprintln!(
            "[limedl] failed to create log directory {}: {e}",
            log_dir.display()
        );
        return;
    }

    // Acquire an exclusive file lock — skip rotation if another instance holds it
    let lock_path = log_dir.join(".lock");
    let lock_file = match fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "[limedl] failed to open log lock file {}: {e}",
                lock_path.display()
            );
            return;
        }
    };

    match lock_file.try_lock() {
        Ok(()) => { /* lock acquired */ }
        Err(_) => {
            eprintln!(
                "[limedl] failed to acquire log lock on {} (lock held by another process)",
                lock_path.display()
            );
            return;
        }
    }
    // Lock is released when `lock_file` is dropped at end of this function

    // Rotate current log
    rotate_startup_logs(log_path);

    // Cleanup by count
    if let Some(count) = retention_count {
        cleanup_by_count(log_path, count);
    }

    // Cleanup by age
    if let Some(days) = retention_days {
        cleanup_by_age(log_path, days);
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use ntest::timeout;
    use tempfile::tempdir;

    use super::*;

    fn write_file(path: &Path, content: &str) {
        fs::write(path, content).expect("write test log file");
    }

    /// Move a file's mtime into the past so age-based retention can be tested
    /// without waiting days (`File::set_modified`, stable since Rust 1.75).
    fn age_file(path: &Path, age: Duration) {
        let file = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("open test log file");
        file.set_modified(SystemTime::now() - age)
            .expect("set modified time");
    }

    #[timeout(30_000)]
    #[test]
    fn resolve_log_file_path_uses_state_dir_or_configured_path() {
        let state = tempdir().expect("tempdir");
        assert_eq!(
            resolve_log_file_path(&LogSettings::default(), state.path()),
            state.path().join("logs").join("limedl.log")
        );

        let custom = LogSettings {
            file_path: "  /var/log/limedl/custom.log  ".to_string(),
            ..Default::default()
        };
        assert_eq!(
            resolve_log_file_path(&custom, state.path()),
            PathBuf::from("/var/log/limedl/custom.log"),
            "a configured path wins over the state-dir default and is trimmed"
        );
    }

    #[timeout(30_000)]
    #[test]
    fn log_dir_for_creates_the_configured_parent() {
        let tmp = tempdir().expect("tempdir");
        let custom = tmp.path().join("nested").join("logs").join("limedl.log");
        let settings = LogSettings {
            file_path: custom.to_string_lossy().into_owned(),
            ..Default::default()
        };

        let dir = log_dir_for(&settings, tmp.path()).expect("create log dir");

        assert_eq!(dir, custom.parent().unwrap());
        assert!(dir.is_dir(), "the full parent chain must be created");
    }

    #[timeout(30_000)]
    #[test]
    fn find_rotated_logs_filters_and_sorts_descending() {
        let tmp = tempdir().expect("tempdir");
        let current = tmp.path().join("limedl.log");
        for name in [
            "limedl.log",
            "limedl.1.log",
            "limedl.2.log",
            "limedl.x.log",
            "limedl.1.txt",
            "other.1.log",
        ] {
            write_file(&tmp.path().join(name), name);
        }

        let logs = find_rotated_logs(&current);
        let found: Vec<String> = logs
            .iter()
            .map(|(path, num)| format!("{}={num}", path.file_name().unwrap().to_string_lossy()))
            .collect();

        assert_eq!(
            found,
            vec!["limedl.2.log=2", "limedl.1.log=1"],
            "only `limedl.<n>.log` counts, and rotation numbers sort descending"
        );
        assert!(
            find_rotated_logs(Path::new("limedl")).is_empty(),
            "a path without an extension has no rotated siblings"
        );
    }

    /// Renaming must go from the highest rotation number down. Ascending, the
    /// first rename would clobber `.2.log` before it was moved to `.3.log` and
    /// silently destroy a log file.
    #[timeout(30_000)]
    #[test]
    fn rotate_startup_logs_shifts_without_losing_content() {
        let tmp = tempdir().expect("tempdir");
        let current = tmp.path().join("limedl.log");
        write_file(&current, "current");
        write_file(&tmp.path().join("limedl.1.log"), "one");
        write_file(&tmp.path().join("limedl.2.log"), "two");

        rotate_startup_logs(&current);

        assert!(!current.exists(), "the current log becomes the .1 file");
        assert_eq!(
            fs::read_to_string(tmp.path().join("limedl.1.log")).unwrap(),
            "current"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("limedl.2.log")).unwrap(),
            "one"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("limedl.3.log")).unwrap(),
            "two",
            "the oldest rotated file must move to .3, not be overwritten"
        );
    }

    #[timeout(30_000)]
    #[test]
    fn cleanup_by_count_keeps_exactly_the_limit() {
        let tmp = tempdir().expect("tempdir");
        let current = tmp.path().join("limedl.log");
        for n in 1..=4u32 {
            write_file(&tmp.path().join(format!("limedl.{n}.log")), &format!("{n}"));
        }

        cleanup_by_count(&current, 2);

        assert!(tmp.path().join("limedl.1.log").exists());
        assert!(tmp.path().join("limedl.2.log").exists());
        assert!(!tmp.path().join("limedl.3.log").exists());
        assert!(!tmp.path().join("limedl.4.log").exists());

        // Documented meaning of `Some(0)`: delete every old log.
        cleanup_by_count(&current, 0);
        assert!(!tmp.path().join("limedl.1.log").exists());
        assert!(!tmp.path().join("limedl.2.log").exists());
    }

    #[timeout(30_000)]
    #[test]
    fn cleanup_by_age_removes_only_old_files() {
        let tmp = tempdir().expect("tempdir");
        let current = tmp.path().join("limedl.log");
        let old_rotated = tmp.path().join("limedl.1.log");
        let fresh_rotated = tmp.path().join("limedl.2.log");
        write_file(&current, "current");
        write_file(&old_rotated, "old");
        write_file(&fresh_rotated, "fresh");
        age_file(&old_rotated, Duration::from_secs(10 * 86400));
        // The current file is checked too: it may be stale after logging was off.
        age_file(&current, Duration::from_secs(10 * 86400));

        cleanup_by_age(&current, 7);

        assert!(!old_rotated.exists(), "an old rotated file must be removed");
        assert!(
            !current.exists(),
            "an old current file must be removed as well"
        );
        assert_eq!(
            fs::read_to_string(&fresh_rotated).unwrap(),
            "fresh",
            "files inside the window must survive"
        );
    }

    /// Startup rotation is skipped while another process holds the lock, and
    /// runs as soon as it is released.
    #[timeout(30_000)]
    #[test]
    fn perform_startup_rotation_skips_while_lock_is_held() {
        let tmp = tempdir().expect("tempdir");
        let current = tmp.path().join("limedl.log");
        write_file(&current, "current");

        let lock_path = tmp.path().join(".lock");
        let held = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .expect("open lock file");
        held.try_lock().expect("test must acquire the lock");

        perform_startup_rotation(&current, None, None);
        assert!(
            current.exists(),
            "rotation must be skipped while the lock is held"
        );
        assert!(!tmp.path().join("limedl.1.log").exists());

        drop(held);
        perform_startup_rotation(&current, Some(1), None);
        assert!(!current.exists(), "rotation runs once the lock is free");
        assert_eq!(
            fs::read_to_string(tmp.path().join("limedl.1.log")).unwrap(),
            "current"
        );
    }

    #[timeout(30_000)]
    #[test]
    fn dynamic_file_writer_honors_enabled_and_survives_open_failure() {
        let tmp = tempdir().expect("tempdir");
        let runtime = |enabled, path: PathBuf| DynamicFileWriter {
            runtime: Arc::new(RwLock::new(LoggerRuntime {
                enabled,
                file_path: path,
            })),
        };

        // Disabled: writes are accepted and discarded, no file appears.
        let disabled_path = tmp.path().join("disabled.log");
        runtime(false, disabled_path.clone())
            .make_writer()
            .write_all(b"dropped")
            .expect("a disabled writer must accept writes");
        assert!(!disabled_path.exists());

        // Enabled: the parent chain is created and the bytes reach the file.
        let nested = tmp.path().join("nested").join("logs").join("limedl.log");
        runtime(true, nested.clone())
            .make_writer()
            .write_all(b"kept")
            .expect("an enabled writer must accept writes");
        assert_eq!(fs::read_to_string(&nested).unwrap(), "kept");

        // Open failure (the path points at a directory) degrades to a sink —
        // the error is reported on stderr, never as a panic in a tracing call.
        let as_directory = tmp.path().join("as-directory.log");
        fs::create_dir(&as_directory).unwrap();
        runtime(true, as_directory.clone())
            .make_writer()
            .write_all(b"discarded")
            .expect("an unopenable log file must degrade to a sink");
        assert!(as_directory.is_dir());
    }
}
