use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, RwLock},
};

use anyhow::Context;
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
    file: Option<fs::File>,
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
        let runtime = match self.runtime.read() {
            Ok(runtime) => runtime,
            Err(poisoned) => {
                tracing::warn!("logger runtime lock poisoned, recovering runtime state");
                poisoned.into_inner()
            }
        };

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
            .ok();

        DynamicFileWriterGuard { file }
    }
}

pub fn init_logging(settings: &LogSettings, state_dir: &Path) -> anyhow::Result<()> {
    if LOGGER_CONTROL.get().is_some() {
        return apply_logging_settings(settings, state_dir);
    }

    let runtime = Arc::new(RwLock::new(LoggerRuntime {
        enabled: settings.enabled,
        file_path: resolve_log_file_path(settings, state_dir),
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

    let mut runtime = match control.runtime.write() {
        Ok(runtime) => runtime,
        Err(poisoned) => {
            tracing::warn!("logger runtime lock poisoned, recovering runtime state");
            poisoned.into_inner()
        }
    };
    runtime.enabled = settings.enabled;
    runtime.file_path = resolve_log_file_path(settings, state_dir);

    tracing::info!(
        enabled = settings.enabled,
        level = ?settings.level,
        path = %runtime.file_path.display(),
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
