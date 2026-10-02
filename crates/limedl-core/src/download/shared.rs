//! Shared download policy helpers: threading-mode resolution, thread notes,
//! destination-path uniquing and background-error logging. These live outside
//! `manager.rs` so the executor, scheduler and lifecycle do not have to depend
//! on the `DownloadManager` facade for them.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::Url;
use uuid::Uuid;

use crate::aimd;
use crate::types::{AdaptiveProfile, AppSettings, SchedulerMode, StartDownloadRequest, ThreadMode};

pub const DEFAULT_FIXED_THREADS: usize = 8;
pub(crate) const DEFAULT_RETRIES: u32 = 4;
pub(crate) const PERSIST_INTERVAL: Duration = Duration::from_millis(300);
pub const MAX_TRADITIONAL_THREADS: usize = 32;
pub(crate) fn supports_parallelism(
    total: Option<u64>,
    supports_ranges: bool,
    chunk_size: u64,
) -> bool {
    supports_ranges && total.map(|value| value >= chunk_size * 2).unwrap_or(false)
}
pub(crate) fn resolve_thread_settings(
    settings: &AppSettings,
    request: &StartDownloadRequest,
    supports_parallel: bool,
) -> (
    ThreadMode,
    Option<usize>,
    Option<usize>,
    Option<AdaptiveProfile>,
) {
    if !supports_parallel {
        return (ThreadMode::Fixed, Some(1), Some(1), None);
    }

    match settings.scheduler.mode {
        SchedulerMode::Traditional => {
            let requested = request
                .thread_count
                .unwrap_or(DEFAULT_FIXED_THREADS)
                .clamp(1, MAX_TRADITIONAL_THREADS);
            (ThreadMode::Fixed, Some(requested), Some(requested), None)
        }
        SchedulerMode::Automatic => match request.thread_mode.unwrap_or(ThreadMode::Adaptive) {
            ThreadMode::Adaptive => {
                let profile = settings.scheduler.automatic.adaptive_profile;
                let max_threads = settings.scheduler.automatic.max_threads_per_task.max(1);
                let desired = aimd::initial_desired_threads(profile, max_threads);
                (
                    ThreadMode::Adaptive,
                    None,
                    Some(desired.max(1)),
                    Some(profile),
                )
            }
            ThreadMode::Fixed => {
                let requested = request
                    .thread_count
                    .unwrap_or(DEFAULT_FIXED_THREADS)
                    .clamp(1, settings.scheduler.automatic.max_threads_per_task.max(1));
                (ThreadMode::Fixed, Some(requested), Some(requested), None)
            }
        },
    }
}
pub(crate) fn thread_note(
    supports_parallel: bool,
    thread_mode: ThreadMode,
    adaptive_profile: Option<AdaptiveProfile>,
) -> Option<String> {
    if !supports_parallel {
        return Some(String::from("单线程（服务器不支持分段）"));
    }

    match thread_mode {
        ThreadMode::Fixed => Some(String::from("固定线程")),
        ThreadMode::Adaptive => adaptive_profile.map(|profile| match profile {
            AdaptiveProfile::Conservative => String::from("自适应 / 保守"),
            AdaptiveProfile::Balanced => String::from("自适应 / 平衡"),
            AdaptiveProfile::Aggressive => String::from("自适应 / 激进"),
        }),
    }
}
pub(crate) fn unique_destination_path(destination_dir: &Path, file_name: &str) -> PathBuf {
    let base = destination_dir.join(file_name);
    if !base.exists() {
        return base;
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("download");
    let extension = Path::new(file_name)
        .extension()
        .and_then(OsStr::to_str)
        .map(|value| format!(".{value}"))
        .unwrap_or_default();

    for index in 1..10_000 {
        let candidate = destination_dir.join(format!("{stem} ({index}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    destination_dir.join(format!("{}-{}{}", stem, Uuid::new_v4(), extension))
}
pub(crate) fn initial_file_name_from_url(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(ToOwned::to_owned))
        })
        .map(sanitize_filename::sanitize)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("download"))
}
pub(crate) fn log_background_error(context: &str, error: impl std::fmt::Display) {
    tracing::warn!(context, %error, "background error");
}
