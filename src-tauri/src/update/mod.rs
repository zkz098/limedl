use std::time::Duration;

use anyhow::Context;
use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::UpdaterExt;

use limedl_core::{
    Dispatcher,
    manager::AppState,
    types::{DownloadState, SerializableError, StartDownloadRequest},
};

type CommandResult<T> = std::result::Result<T, SerializableError>;

/// Convert an anyhow error into a frontend-friendly [`SerializableError`].
/// Uses the same chain-join format as `download/commands.rs` for consistency.
fn into_command_result<T>(result: anyhow::Result<T>) -> CommandResult<T> {
    result.map_err(|error| {
        let kind = limedl_core::error::extract_kind_from_anyhow(&error).to_string();
        let message = format_anyhow_chain(error);
        SerializableError { kind, message }
    })
}

fn format_anyhow_chain(error: anyhow::Error) -> String {
    let mut chain = error.chain();
    let mut messages = Vec::new();
    if let Some(first) = chain.next() {
        messages.push(first.to_string());
    }
    for cause in chain {
        let cause = cause.to_string();
        if messages.last().is_none_or(|last| last != &cause) {
            messages.push(cause);
        }
    }
    messages.join(": ")
}

/// Read the updater's minisign public key from the Tauri app config
/// (`plugins.updater.pubkey` in `tauri.conf.json`).
fn read_updater_pubkey(app: &AppHandle) -> String {
    let config = app.config();
    config
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Maximum wall-clock time to wait for the update download to complete.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60); // 30 minutes

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckUpdateResult {
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    download_url: String,
    signature: String,
    current_version: String,
}

/// Check for updates and return full metadata including `downloadUrl`.
/// Exposes fields normally hidden by the JS plugin API so the frontend
/// can display version info before initiating the download.
#[cfg(desktop)]
#[tauri::command]
pub async fn check_update_full(app: AppHandle) -> CommandResult<Option<CheckUpdateResult>> {
    into_command_result(
        async {
            let updater = app
                .updater()
                .map_err(|e| anyhow::anyhow!("updater plugin not available: {e}"))?;
            let update = updater
                .check()
                .await
                .context("failed to check for updates")?;
            Ok(update.map(|u| CheckUpdateResult {
                version: u.version,
                body: u.body,
                date: u.date.map(|d| d.to_string()),
                download_url: u.download_url.to_string(),
                signature: u.signature,
                current_version: u.current_version,
            }))
        }
        .await,
    )
}

/// Self-contained update flow using limedl's own download engine:
///
/// 1. Checks for updates (single network request).
/// 2. Downloads the installer via limedl-core's `DownloadManager`/
///    `Dispatcher`, emitting `update-download-progress` Tauri events.
/// 3. Verifies the minisign signature against the trusted public key.
/// 4. Launches the platform-native installer.
///
/// On Windows the installer calls `ShellExecuteW` + `process::exit(0)`,
/// so this command never returns a response to the frontend.
/// The last signal to the frontend is the `update-installing` event
/// emitted right before the installer is launched.
#[cfg(desktop)]
#[tauri::command]
pub async fn download_and_install_update(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    into_command_result(
        async {
            // ── 1. Check for updates (single request) ────────────
            let updater = app
                .updater()
                .map_err(|e| anyhow::anyhow!("updater plugin not available: {e}"))?;
            let update = updater
                .check()
                .await
                .context("failed to check for updates")?
                .ok_or_else(|| anyhow::anyhow!("no update available"))?;

            // ── 2. Prepare temp directory ────────────────────────
            let temp_dir = std::env::temp_dir().join("limedl-update");
            tokio::fs::create_dir_all(&temp_dir)
                .await
                .context("failed to create temp directory for update")?;

            // ── 3. Start download via limedl-core engine ─────────
            // NOTE: GitHub's `api.github.com/.../releases/assets/<id>` endpoint only
            // redirects to the real artifact when the request carries
            // `Accept: application/octet-stream`; without it, GitHub responds 200 with
            // the asset's JSON metadata, which would be saved as the "installer" and
            // then fail minisign verification. tauri-plugin-updater does the same
            // (Update::download injects this header) — we must replicate it.
            let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());
            let request = StartDownloadRequest {
                kind: None,
                headers: Some(vec!["Accept: application/octet-stream".to_string()]),
                url: update.download_url.to_string(),
                destination_dir: temp_dir.to_string_lossy().to_string(),
                file_name: None,
                user_agent: None,
                thread_mode: None,
                thread_count: None,
                max_retries: Some(5),
                checksum: None,
                expected_checksum: None,
                selected_file_indices: None,
                start_paused: false,
                mirror_urls: None,
                priority: None,
            };
            let task_id = dispatcher
                .start(request)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            // ── 4. Poll progress until completion (with timeout) ─
            let start = tokio::time::Instant::now();
            let final_snapshot = loop {
                if start.elapsed() > DOWNLOAD_TIMEOUT {
                    return Err(anyhow::anyhow!(
                        "update download timed out after {} minutes",
                        DOWNLOAD_TIMEOUT.as_secs() / 60
                    ));
                }

                let snapshot = dispatcher
                    .status(&task_id)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;

                let total = snapshot.total_bytes.unwrap_or(0);
                let downloaded = snapshot.downloaded_bytes;
                let percent = if total > 0 {
                    ((downloaded as f64 / total as f64) * 100.0).min(99.0) as u32
                } else {
                    0
                };

                let _ = app.emit(
                    "update-download-progress",
                    serde_json::json!({
                        "downloadedBytes": downloaded,
                        "totalBytes": total,
                        "percent": percent,
                    }),
                );

                match snapshot.state {
                    DownloadState::Completed => break snapshot,
                    DownloadState::Failed | DownloadState::Canceled => {
                        let msg = snapshot
                            .error
                            .unwrap_or_else(|| "update download failed".into());
                        return Err(anyhow::anyhow!("{msg}"));
                    }
                    DownloadState::Paused => {
                        // Self-update downloads should never be paused by the user,
                        // but if it somehow happens, fail immediately.
                        return Err(anyhow::anyhow!("update download was paused unexpectedly"));
                    }
                    _ => {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            };

            // ── 5. Read downloaded file ──────────────────────────
            let file_path = &final_snapshot.destination_path;
            let bytes = tokio::fs::read(file_path)
                .await
                .with_context(|| format!("failed to read downloaded update from {file_path}"))?;

            // Clean up temp file (bytes are now in memory)
            let _ = tokio::fs::remove_file(file_path).await;
            let _ = tokio::fs::remove_dir(&temp_dir).await;

            // ── 6. Emit 100% + installing signal ─────────────────
            let _ = app.emit(
                "update-download-progress",
                serde_json::json!({
                    "downloadedBytes": final_snapshot.downloaded_bytes,
                    "totalBytes": final_snapshot.total_bytes.unwrap_or(0),
                    "percent": 100,
                }),
            );
            let _ = app.emit("update-installing", serde_json::json!({}));

            // ── 7. Verify minisign signature ─────────────────────
            let pubkey_str = read_updater_pubkey(&app);
            if pubkey_str.is_empty() {
                return Err(anyhow::anyhow!(
                    "updater public key not found in app config (plugins.updater.pubkey)"
                ));
            }
            let pub_key_decoded = base64::engine::general_purpose::STANDARD
                .decode(&pubkey_str)
                .context("failed to decode updater public key")?;
            let pub_key_str = std::str::from_utf8(&pub_key_decoded)
                .context("public key is not valid UTF-8")?;
            let public_key =
                PublicKey::decode(pub_key_str).context("failed to decode public key")?;
            let sig_decoded = base64::engine::general_purpose::STANDARD
                .decode(&update.signature)
                .context("failed to decode release signature")?;
            let sig_str = std::str::from_utf8(&sig_decoded)
                .context("signature is not valid UTF-8")?;
            let sig =
                Signature::decode(sig_str).context("failed to decode release signature")?;
            public_key
                .verify(&bytes, &sig, true)
                .context("update signature verification failed")?;

            // ── 8. Install via platform-native installer ─────────
            // On Windows this calls ShellExecuteW + process::exit(0),
            // so the Tauri IPC response is never sent.
            update
                .install(&bytes)
                .map_err(|e| anyhow::anyhow!(e))?;

            Ok(())
        }
        .await,
    )
}
