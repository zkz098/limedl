//! HTTP download execution — extracted from manager.rs to reduce the god object.
//!
//! Contains the HTTP-specific download flow: probing, single-stream and chunked
//! parallel downloads, chunk worker, and finalization with checksum verification.
//!
//! `HttpExecutor` is an independent actor type.  All its methods receive a
//! `&DownloadManager` or `Arc<DownloadManager>` parameter to access shared
//! state, avoiding any ownership cycle with `DownloadManager` (which holds
//! `Arc<HttpExecutor>`).

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::{Client, StatusCode, header};
use tokio::{task::JoinSet, time::sleep};
use tokio_util::sync::CancellationToken;

use crate::{
    aimd::AimdState,
    buffer_pool::DownloadBuffer,
    calculate_checksum,
    database::Database,
    error::{DownloadError, Result, io_error_with_path},
    event_bus::DownloadEvent,
    file_ops::{
        check_disk_space, finalize_temp_file, open_download_file, reset_download_file, write_all_at,
    },
    http::{
        ANTI_ABUSE_SNIFF_LIMIT, anti_abuse_forbidden_error, apply_extra_headers,
        build_segment_request, extract_total_bytes, has_header, header_string, if_range_header,
        infer_candidate_referers, infer_file_name, is_too_many_requests_error,
        looks_like_anti_abuse_page, read_body_prefix, supports_ranges, validate_probe_response,
        validate_segment_response,
    },
    download::{
        ChunkWorkerOutcome, ManagedDownload, PERSIST_INTERVAL, RunOutcome,
        cancellation_chunk_outcome, cancellation_outcome, record_progress_on_managed,
        supports_parallelism,
    },
    manager::DownloadManager,
    manifest::{
        ChunkManifest, RemoteMetadata, contiguous_prefix_end, has_partial_chunk_progress,
        plan_chunks, resolve_chunk_size, validators_changed,
    },
    now_ms,
    persistence::persist_manifest_snapshot,
    rate_limiter::RateLimiter,
    retry::request_with_retry,
    types::{ChecksumMode, DiskType, DownloadState, StartDownloadRequest, TaskKind, ThreadMode},
};

/// Zero-sized actor type for HTTP download execution.
///
/// All methods receive `&DownloadManager` or `Arc<DownloadManager>` to access
/// shared state.  `DownloadManager` holds `Arc<HttpExecutor>` for delegation.
pub struct HttpExecutor;

/// Tail Sprint: stall window for detecting a slow last-chunk connection.
const TAIL_SPRINT_STALL_WINDOW_SECS: u64 = 8;
/// Tail Sprint: minimum remaining bytes to qualify for chunk splitting (1 MiB).
const TAIL_SPRINT_MIN_SPLIT_SIZE: u64 = 1024 * 1024;
/// Work Stealing: minimum remaining bytes of an active chunk to qualify for splitting (2 MiB).
const WORK_STEAL_MIN_SPLIT_SIZE: u64 = 2 * 1024 * 1024;

mod chunked;
mod finalize;
mod run;
mod single;
mod worker;

#[cfg(test)]
pub(crate) use worker::mark_chunk_released;

#[cfg(test)]
mod tests;
