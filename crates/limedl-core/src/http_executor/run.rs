//! Remote probe and the top-level run loop (single-stream vs chunked).

use super::HttpExecutor;
use crate::aimd::AimdState;
use crate::download::{ManagedDownload, RunOutcome, supports_parallelism};
use crate::error::{DownloadError, Result};
use crate::file_ops::check_disk_space;
use crate::http::{
    ANTI_ABUSE_SNIFF_LIMIT, anti_abuse_forbidden_error, apply_extra_headers, extract_total_bytes,
    has_header, header_string, identity_encoding, infer_candidate_referers, infer_file_name,
    looks_like_anti_abuse_page, read_body_prefix, supports_ranges, validate_probe_response,
};
use crate::manager_state::ManagerState;
use crate::manifest::{
    RemoteMetadata, has_partial_chunk_progress, plan_chunks, resolve_chunk_size, validators_changed,
};
use crate::now_ms;
use crate::types::{
    AdaptiveProfile, ChecksumMode, DownloadState, StartDownloadRequest, TaskKind, ThreadMode,
};
use reqwest::{Client, StatusCode, header};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

impl HttpExecutor {
    /// Probe a remote URL to obtain file metadata (final URL, file name,
    /// content length, ETag, Last-Modified, range support).
    pub async fn probe(
        &self,
        dm: &dyn ManagerState,
        url: &str,
        user_agent: &str,
        extra_headers: &[String],
    ) -> Result<RemoteMetadata> {
        let (client, _, _) = dm.resolve_client(url).await;
        let head = identity_encoding(apply_extra_headers(
            client.head(url).header(header::USER_AGENT, user_agent),
            extra_headers,
        ))
        .send()
        .await;
        let mut response = match head {
            Ok(response) if response.status().is_success() => response,
            _ => {
                identity_encoding(apply_extra_headers(
                    client
                        .get(url)
                        .header(header::USER_AGENT, user_agent)
                        .header(header::RANGE, "bytes=0-0"),
                    extra_headers,
                ))
                .send()
                .await?
            }
        };

        let mut effective_extra_headers = extra_headers.to_vec();

        // A 403 can mean two very different things: an anti-abuse / WAF block
        // (no Referer can fix it — probing candidates only adds more
        // suspicious requests) or anti-hotlink protection (a Referer does fix
        // it). Sniff the small denial body to tell them apart.
        if response.status() == StatusCode::FORBIDDEN {
            let prefix = read_body_prefix(&mut response, ANTI_ABUSE_SNIFF_LIMIT).await;
            if looks_like_anti_abuse_page(&prefix) {
                return Err(anti_abuse_forbidden_error());
            }
        }

        // If probe returned 403 Forbidden and user did not specify Referer,
        // attempt anti-hotlink resolution using candidate Referer headers.
        if response.status() == StatusCode::FORBIDDEN && !has_header(extra_headers, "referer") {
            let effective_url = response.url().as_str();
            let mut candidates = infer_candidate_referers(effective_url);
            for cand in infer_candidate_referers(url) {
                if !candidates.contains(&cand) {
                    candidates.push(cand);
                }
            }

            for cand in candidates {
                let mut test_headers = extra_headers.to_vec();
                test_headers.push(format!("Referer: {cand}"));
                let head_cand = identity_encoding(apply_extra_headers(
                    client
                        .head(effective_url)
                        .header(header::USER_AGENT, user_agent),
                    &test_headers,
                ))
                .send()
                .await;
                let cand_resp = match head_cand {
                    Ok(r) if r.status().is_success() => Some(r),
                    _ => identity_encoding(apply_extra_headers(
                        client
                            .get(effective_url)
                            .header(header::USER_AGENT, user_agent)
                            .header(header::RANGE, "bytes=0-0"),
                        &test_headers,
                    ))
                    .send()
                    .await
                    .ok()
                    .filter(|r| {
                        r.status().is_success() || r.status() == StatusCode::PARTIAL_CONTENT
                    }),
                };
                if let Some(r) = cand_resp {
                    tracing::info!(
                        "Auto-detected required Referer for anti-hotlink protection: {cand}"
                    );
                    response = r;
                    effective_extra_headers = test_headers;
                    break;
                }
            }
        }

        validate_probe_response(&response)?;

        let final_url = response.url().to_string();
        let headers = response.headers().clone();
        let status = response.status();

        let total_bytes = extract_total_bytes(status, &headers);
        let supports_ranges = supports_ranges(status, &headers);
        let file_name =
            infer_file_name(&final_url, &headers).ok_or(DownloadError::MissingFileName)?;

        dm.alt_svc()
            .record_from_response(&final_url, &headers, now_ms());

        let headers_checksum = crate::checksum::detect_checksum_from_headers(&headers);

        Ok(RemoteMetadata {
            final_url,
            file_name,
            total_bytes,
            etag: header_string(&headers, header::ETAG),
            last_modified: header_string(&headers, header::LAST_MODIFIED),
            supports_ranges,
            extra_headers: effective_extra_headers,
            headers_checksum,
        })
    }

    /// Main download run loop.  Decides between single-stream and chunked
    /// (parallel) download based on server capabilities.
    pub async fn run_download(
        &self,
        dm: Arc<dyn ManagerState>,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<()> {
        let current_manifest = { managed.lock_core().manifest.clone() };
        let metadata = self
            .probe(
                dm.as_ref(),
                &current_manifest.final_url,
                &current_manifest.user_agent,
                &current_manifest.extra_headers,
            )
            .await?;

        // Check available disk space before starting the download
        if let Some(total_bytes) = metadata.total_bytes {
            let already_downloaded = current_manifest.downloaded_bytes;
            let needed = total_bytes.saturating_sub(already_downloaded);
            check_disk_space(Path::new(&current_manifest.destination_dir), needed)?;
        }

        let settings = dm.settings_service().get().await;
        let chunk_size = if let Some(ref pieces) = *managed.pieces.lock()
            && pieces.piece_length > 0
        {
            pieces.piece_length
        } else if settings.scheduler.expert_mode
            && settings.scheduler.expert.custom_chunk_size_mb > 0
        {
            settings.scheduler.expert.custom_chunk_size_mb * 1024 * 1024
        } else {
            resolve_chunk_size(settings.scheduler.chunk_size_strategy, metadata.total_bytes)
        };
        let supports_parallel =
            supports_parallelism(metadata.total_bytes, metadata.supports_ranges, chunk_size);
        let request = StartDownloadRequest {
            kind: Some(TaskKind::Http),
            url: current_manifest.url.clone(),
            destination_dir: current_manifest.destination_dir.clone(),
            file_name: Some(current_manifest.file_name.clone()),
            user_agent: Some(current_manifest.user_agent.clone()),
            headers: Some(metadata.extra_headers.clone()),
            thread_mode: Some(current_manifest.thread_mode),
            thread_count: current_manifest.requested_thread_count,
            max_retries: None,
            checksum: Some(current_manifest.checksum_mode),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
            priority: None,
        };
        let (mode, requested, desired, profile) =
            crate::download::resolve_thread_settings(&settings, &request, supports_parallel);
        let threads = ThreadPlan {
            mode,
            requested,
            desired,
            profile,
        };
        let plan =
            apply_probe_to_manifest(&managed, &metadata, supports_parallel, chunk_size, &threads);

        // Auto-detect checksum if not provided, not explicitly disabled, and auto-detect is enabled
        if current_manifest.expected_checksum.is_none()
            && current_manifest.checksum_mode != ChecksumMode::None
            && settings.download.auto_detect_sha256
        {
            // Priority 1: Check response headers (RFC 3230 / RFC 9530 / S3 / GCS)
            if let Some((mode, hash)) = metadata.headers_checksum.clone() {
                tracing::info!(
                    "Auto-detected expected checksum from HTTP headers: {mode:?} {hash}"
                );
                let mut core = managed.lock_core();
                if core.manifest.expected_checksum.is_none() {
                    core.manifest.expected_checksum = Some(hash.clone());
                    core.manifest.checksum_mode = mode;
                    core.snapshot.expected_checksum = Some(hash);
                    core.snapshot.checksum_mode = mode;
                }
            } else {
                // Priority 2: Probe external candidates (.sha256, SHA256SUMS)
                detect_and_store_sha256(&client, &managed).await;
            }
        }

        if plan.refresh_aimd {
            let mut aimd = managed.lock_aimd();
            *aimd = AimdState::initial(threads.profile, threads.desired);
        }
        dm.rebalance_allocations().await?;
        dm.controls().rebalance_notify.notify_waiters();
        if plan.reset_progress {
            dm.prepare_fresh_temp_file(&managed)?;
            if plan.force_single_stream_restart {
                dm.reset_progress(&managed, true);
            }
        }

        let outcome = if supports_parallel {
            self.download_chunked(
                dm.clone(),
                managed.clone(),
                client.clone(),
                token.clone(),
                max_retries,
            )
            .await?
        } else {
            self.download_single(
                dm.clone(),
                managed.clone(),
                client.clone(),
                token.clone(),
                max_retries,
            )
            .await?
        };

        self.post_process_run(&dm, &managed, &token, outcome).await
    }

    /// Finalize a finished run, or settle the paused/canceled bookkeeping.
    async fn post_process_run(
        &self,
        dm: &Arc<dyn ManagerState>,
        managed: &Arc<ManagedDownload>,
        token: &CancellationToken,
        outcome: RunOutcome,
    ) -> Result<()> {
        match outcome {
            RunOutcome::Finished => match self
                .finalize_download(dm.clone(), managed.clone(), token.clone())
                .await?
            {
                RunOutcome::Finished => dm.emit_single_summary(managed),
                RunOutcome::Canceled | RunOutcome::Paused => {}
            },
            RunOutcome::Paused => {
                {
                    let mut core = managed.lock_core();
                    core.snapshot.state = DownloadState::Paused;
                    core.snapshot.connection_count = 0;
                    core.snapshot.allocated_thread_count = Some(0);
                    core.snapshot.updated_at_ms = now_ms();
                    core.manifest.state = DownloadState::Paused;
                    core.manifest.connection_count = 0;
                    core.manifest.allocated_thread_count = Some(0);
                    core.manifest.updated_at_ms = now_ms();
                }
                dm.emit_single_summary(managed);
            }
            RunOutcome::Canceled => {
                dm.cleanup_files(managed)?;
            }
        }
        Ok(())
    }
}

/// Thread settings resolved from the request + settings, before the manifest
/// is updated.
struct ThreadPlan {
    mode: ThreadMode,
    requested: Option<usize>,
    desired: Option<usize>,
    profile: Option<AdaptiveProfile>,
}

/// Follow-up actions decided while writing the probe results into the manifest.
struct RunPlan {
    reset_progress: bool,
    force_single_stream_restart: bool,
    refresh_aimd: bool,
}

/// Write the probe results into the manifest and return the follow-up actions.
fn apply_probe_to_manifest(
    managed: &Arc<ManagedDownload>,
    metadata: &RemoteMetadata,
    supports_parallel: bool,
    chunk_size: u64,
    threads: &ThreadPlan,
) -> RunPlan {
    let mut plan = RunPlan {
        reset_progress: false,
        force_single_stream_restart: false,
        refresh_aimd: false,
    };

    let mut core = managed.lock_core();
    let manifest = &mut core.manifest;
    if !manifest.file_name_locked && manifest.downloaded_bytes == 0 {
        let safe_name = sanitize_filename::sanitize(&metadata.file_name);
        if !safe_name.is_empty() && safe_name != manifest.file_name {
            let destination_dir = PathBuf::from(&manifest.destination_dir);
            manifest.file_name = safe_name.clone();
            manifest.destination_path =
                crate::download::unique_destination_path(&destination_dir, &safe_name)
                    .to_string_lossy()
                    .to_string();
        }
        manifest.file_name_locked = true;
    }
    if validators_changed(manifest, metadata)
        || manifest.total_bytes != metadata.total_bytes
        || manifest.supports_ranges != supports_parallel
        || (supports_parallel && manifest.chunks.is_empty())
    {
        manifest.downloaded_bytes = 0;
        manifest.chunks = plan_chunks(metadata.total_bytes, supports_parallel, chunk_size);
        manifest.chunk_size = chunk_size;
        manifest.checksum = None;
        manifest.supports_ranges = supports_parallel;
        plan.reset_progress = true;
    } else if !supports_parallel && has_partial_chunk_progress(manifest) {
        manifest.downloaded_bytes = 0;
        manifest.connection_count = 1;
        manifest.supports_ranges = false;
        manifest.chunks.clear();
        manifest.checksum = None;
        plan.reset_progress = true;
        plan.force_single_stream_restart = true;
    }
    manifest.final_url = metadata.final_url.clone();
    manifest.extra_headers = metadata.extra_headers.clone();
    manifest.supports_ranges = supports_parallel;
    manifest.total_bytes = metadata.total_bytes;
    manifest.etag = metadata.etag.clone();
    manifest.last_modified = metadata.last_modified.clone();
    manifest.thread_mode = threads.mode;
    manifest.requested_thread_count = threads.requested;
    if manifest.desired_thread_count != threads.desired
        || manifest.adaptive_profile_snapshot != threads.profile
    {
        plan.refresh_aimd = true;
    }
    manifest.desired_thread_count = threads.desired;
    manifest.adaptive_profile_snapshot = threads.profile;
    if manifest.thread_note.as_deref() != Some("单线程（429 限流降级）") {
        manifest.thread_note =
            crate::download::thread_note(supports_parallel, threads.mode, threads.profile);
    }
    manifest.updated_at_ms = now_ms();
    manifest.error = None;
    if !supports_parallel {
        manifest.thread_note = Some(String::from("单线程（服务器不支持分段）"));
        manifest.desired_thread_count = Some(1);
    }
    core.sync_snapshot_from_manifest();
    if plan.reset_progress {
        // The caller recreates the temp file, so nothing written before this point
        // still counts as durable progress. The fresh chunks above already carry a
        // durable counter of zero; this clears the task-level one.
        core.durable_bytes = 0;
    }
    plan
}

/// Detect and store a SHA-256 hash when the server or a mirror exposes one.
async fn detect_and_store_sha256(client: &Client, managed: &Arc<ManagedDownload>) {
    let (target_url, file_name, user_agent, extra_headers) = {
        let core = managed.lock_core();
        (
            core.manifest.final_url.clone(),
            core.manifest.file_name.clone(),
            core.manifest.user_agent.clone(),
            core.manifest.extra_headers.clone(),
        )
    };
    if let Some(detected_hash) =
        crate::checksum::detect_sha256(client, &target_url, &file_name, &user_agent, &extra_headers)
            .await
    {
        tracing::info!("Auto-detected SHA-256 for {}: {}", file_name, detected_hash);
        let mut core = managed.lock_core();
        if core.manifest.expected_checksum.is_none() {
            core.manifest.expected_checksum = Some(detected_hash.clone());
            core.manifest.checksum_mode = ChecksumMode::Sha256;
            core.snapshot.expected_checksum = Some(detected_hash);
            core.snapshot.checksum_mode = ChecksumMode::Sha256;
        }
    }
}
