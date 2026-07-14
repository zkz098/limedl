use std::{
    fs::{self, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{Duration, SystemTime},
};

use parking_lot::RwLock;

use anyhow::Context;
use fs4::fs_std::FileExt;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    fmt::{self, MakeWriter},
    layer::SubscriberExt,
    reload,
    util::SubscriberInitExt,
};

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
                "[downloader] failed to create log directory {}: {error}",
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
                    "[downloader] failed to open log file {}: {error}",
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
    perform_startup_rotation(&file_path, settings.retention_count, settings.retention_days);

    let runtime = Arc::new(RwLock::new(LoggerRuntime {
        enabled: settings.enabled,
        file_path,
    }));

    let (level_layer, level_reload) = reload::Layer::new(to_level_filter(settings.level));
    let fmt_layer =
        fmt::layer()
            .with_target(true)
            .with_ansi(false)
            .with_writer(DynamicFileWriter {
                runtime: runtime.clone(),
            });

    tracing_subscriber::registry()
        .with(level_layer)
        .with(fmt_layer)
        .try_init()
        .context("failed to initialize tracing subscriber")?;

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

    control
        .level_reload
        .modify(|level| *level = to_level_filter(settings.level))
        .context("failed to update tracing level filter")?;

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
        return state_dir.join("logs").join("downloader.log");
    }

    PathBuf::from(configured)
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

/// Shift-right rotate log files: `downloader.log` → `downloader.1.log`,
/// `downloader.N.log` → `downloader.(N+1).log`.
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
                "[downloader] failed to rotate log file {} -> {}: {e}",
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
                "[downloader] failed to rotate current log file {} -> {}: {e}",
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
                "[downloader] failed to remove old log file {}: {e}",
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
                            "[downloader] failed to remove old log file {}: {e}",
                            path.display()
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[downloader] failed to get modified time for {}: {e}",
                        path.display()
                    );
                }
            },
            Err(e) => {
                eprintln!(
                    "[downloader] failed to stat log file {}: {e}",
                    path.display()
                );
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
            "[downloader] failed to remove old current log file {}: {e}",
            log_path.display()
        );
    }
}

/// Perform startup log rotation and retention cleanup.
/// Acquires an exclusive file lock on `<log_dir>/.lock` to prevent concurrent
/// rotation from multiple instances.
fn perform_startup_rotation(log_path: &Path, retention_count: Option<u32>, retention_days: Option<u32>) {
    let log_dir = match log_path.parent() {
        Some(d) => d,
        None => return,
    };

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
                "[downloader] failed to open log lock file {}: {e}",
                lock_path.display()
            );
            return;
        }
    };

    match lock_file.try_lock_exclusive() {
        Ok(true) => { /* lock acquired */ }
        Ok(false) => {
            eprintln!(
                "[downloader] another process holds the log lock ({}), skipping rotation",
                lock_path.display()
            );
            return;
        }
        Err(e) => {
            eprintln!(
                "[downloader] failed to acquire log lock on {}: {e}",
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
