use std::{
    fs::{self, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{Duration, SystemTime},
};

use parking_lot::Mutex;

use anyhow::Context;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, MakeWriter},
    layer::{Layer as _, SubscriberExt},
    reload,
    util::SubscriberInitExt,
};

/// Fallback console filter when `RUST_LOG` is unset (matches the historical
/// console behavior of the desktop/server apps).
const DEFAULT_CONSOLE_FILTER: &str = "info,limedl=debug";

/// Maximum size of the active log file before runtime in-place rotation (10 MiB).
const MAX_LOG_FILE_BYTES: u64 = 10 * 1024 * 1024;

use super::types::{LogLevel, LogSettings};

static LOGGER_CONTROL: OnceLock<LoggerControl> = OnceLock::new();

struct LoggerRuntime {
    enabled: bool,
    file_path: PathBuf,
    retention_count: Option<u32>,
    retention_days: Option<u32>,
    max_file_bytes: u64,
    writer: Option<BufWriter<fs::File>>,
    written_bytes: u64,
}

impl LoggerRuntime {
    fn new(
        enabled: bool,
        file_path: PathBuf,
        retention_count: Option<u32>,
        retention_days: Option<u32>,
    ) -> Self {
        let mut rt = Self {
            enabled,
            file_path,
            retention_count,
            retention_days,
            max_file_bytes: MAX_LOG_FILE_BYTES,
            writer: None,
            written_bytes: 0,
        };
        if enabled {
            rt.open_writer();
        }
        rt
    }

    fn open_writer(&mut self) {
        if !self.enabled {
            self.writer = None;
            self.written_bytes = 0;
            return;
        }

        if let Some(parent) = self.file_path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            eprintln!(
                "[limedl] failed to create log directory {}: {error}",
                parent.display()
            );
            self.writer = None;
            self.written_bytes = 0;
            return;
        }

        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
        {
            Ok(file) => {
                let len = file.metadata().map(|m| m.len()).unwrap_or(0);
                self.writer = Some(BufWriter::with_capacity(8192, file));
                self.written_bytes = len;
            }
            Err(error) => {
                eprintln!(
                    "[limedl] failed to open log file {}: {error}",
                    self.file_path.display()
                );
                self.writer = None;
                self.written_bytes = 0;
            }
        }
    }
}

struct LoggerControl {
    runtime: Arc<Mutex<LoggerRuntime>>,
    level_reload: reload::Handle<LevelFilter, tracing_subscriber::Registry>,
}

#[derive(Clone)]
struct DynamicFileWriter {
    runtime: Arc<Mutex<LoggerRuntime>>,
}

struct DynamicFileWriterGuard<'a> {
    runtime: &'a Mutex<LoggerRuntime>,
}

impl Write for DynamicFileWriterGuard<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut rt = self.runtime.lock();
        if !rt.enabled {
            return Ok(buf.len());
        }
        if rt.writer.is_none() {
            rt.open_writer();
        }
        let n = match &mut rt.writer {
            Some(writer) => writer.write(buf)?,
            None => return Ok(buf.len()),
        };
        rt.written_bytes = rt.written_bytes.saturating_add(n as u64);
        if rt.written_bytes >= rt.max_file_bytes {
            if let Some(mut writer) = rt.writer.take() {
                let _ = writer.flush();
            }
            let path = rt.file_path.clone();
            let count = rt.retention_count;
            let days = rt.retention_days;
            perform_startup_rotation(&path, count, days);
            rt.open_writer();
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut rt = self.runtime.lock();
        if let Some(writer) = &mut rt.writer {
            writer.flush()
        } else {
            Ok(())
        }
    }
}

impl Drop for DynamicFileWriterGuard<'_> {
    fn drop(&mut self) {
        let mut rt = self.runtime.lock();
        if let Some(writer) = &mut rt.writer {
            let _ = writer.flush();
        }
    }
}

impl<'a> MakeWriter<'a> for DynamicFileWriter {
    type Writer = DynamicFileWriterGuard<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        DynamicFileWriterGuard {
            runtime: &self.runtime,
        }
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

    let runtime = Arc::new(Mutex::new(LoggerRuntime::new(
        settings.enabled,
        file_path,
        settings.retention_count,
        settings.retention_days,
    )));

    let (level_layer, level_reload) = reload::Layer::new(to_level_filter(settings.level));

    // Console output keeps `RUST_LOG` semantics via a per-layer static filter.
    let console_layer = fmt::layer().with_target(true).with_ansi(true).with_filter(
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(DEFAULT_CONSOLE_FILTER)),
    );

    let file_layer =
        fmt::layer()
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
    let mut filter_applied = control
        .level_reload
        .modify(|level| *level = to_level_filter(settings.level));
    for _ in 0..10 {
        if filter_applied.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
        filter_applied = control
            .level_reload
            .modify(|level| *level = to_level_filter(settings.level));
    }
    filter_applied.context("failed to update tracing level filter")?;

    let next_path = resolve_log_file_path(settings, state_dir);
    let mut runtime = control.runtime.lock();
    let reopen_needed = runtime.enabled != settings.enabled || runtime.file_path != next_path;
    runtime.enabled = settings.enabled;
    runtime.file_path = next_path;
    runtime.retention_count = settings.retention_count;
    runtime.retention_days = settings.retention_days;
    if reopen_needed {
        if let Some(mut writer) = runtime.writer.take() {
            let _ = writer.flush();
        }
        runtime.open_writer();
    }
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
mod tests;
