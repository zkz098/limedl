// Self-update integration for limedl-native (backed by `sigilup`).
//
// Three distribution channels with separate update paths:
//
// - **MSIX / Microsoft Store**: the OS signs and updates the package. We only
//   surface `StoreContext` update checks/installs (see [`store`]). Package
//   identity (`Package::Current()`) is the discriminator.
// - **GitHub installer** (NSIS setup.exe, per-user): download + streaming
//   `BLAKE3 + SHA-512` verification via `sigilup` -> spawn the installer in
//   passive mode; the installer handles stopping the running app and
//   relaunching.
// - **GitHub portable** (single exe zip / linux & macOS tar.gz): download +
//   streaming verification -> `sigilup::replace::install_portable_update`
//   (extract + `self_replace` + macOS `.app` ad-hoc re-sign) -> relaunch.
//
// All GitHub-channel artifacts are described by `latest.hsig.json`, a
// self-contained DSSEv1 hybrid-signed (`Ed25519 + ML-DSA-65`) manifest hosted as
// a release asset and reached through `releases/latest/download/latest.hsig.json`
// (GitHub excludes draft/prerelease releases there, so stable users never see
// rc/alpha builds).

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use limedl_core::http_client_factory::configure_client_builder;
use limedl_core::types::AppSettings;
use sigilup::manifest::{FileEpochStore, KeyPinFile, semver};
use sigilup::transport::ReqwestUpdater;
use sigilup::{
    EpochStore, TrustStore, UpdateDecision, UpdateEvaluator, UrlPolicy, VerificationPolicy,
    VerifyContext,
};

// ── Constants ────────────────────────────────────────────────────────────────

/// Embedded `sigilup` trust-root pin file (`Ed25519 + ML-DSA-65`).
///
/// The private signing keys live in CI secrets (`SIGILUP_SIGNING_KEY` /
/// `SIGILUP_SIGNING_KEY_PASSWORD`); `sigilup guard` in the release workflow
/// refuses to ship a build whose signing key is not pinned here.
const EMBEDDED_PINS_JSON: &str = include_str!("sigilup-pins.json");

/// Repo hosting the release assets.
const RELEASE_REPO: &str = "zkz098/limedl";

/// GitHub mirror prefix used as a fallback when `github.com` is slow or unreachable.
const GH_PROXY_PREFIX: &str = "https://gh-proxy.com/";

/// Permanent URL prefixes allowed for release downloads of this repository
/// (official GitHub Releases first, `gh-proxy.com` mirror as fallback).
const ALLOWED_DOWNLOAD_PREFIXES: &[&str] = &[
    "https://github.com/zkz098/limedl/releases/download/",
    "https://gh-proxy.com/https://github.com/zkz098/limedl/releases/download/",
];

/// Permanent URLs that always resolve to the newest stable signed manifest
/// (GitHub excludes draft/prerelease releases there, so stable users never see
/// rc/alpha builds). Tried in order: official GitHub first, `gh-proxy.com`
/// mirror as fallback.
const MANIFEST_URLS: &[&str] = &[
    "https://github.com/zkz098/limedl/releases/latest/download/latest.hsig.json",
    "https://gh-proxy.com/https://github.com/zkz098/limedl/releases/latest/download/latest.hsig.json",
];

/// Upper bound for an update download (512 MiB).
const MAX_UPDATE_BYTES: u64 = 512 * 1024 * 1024;

/// Subdirectory (under the app state dir) used for update downloads/staging.
pub const UPDATE_WORK_DIR: &str = "update";

// ── Types ────────────────────────────────────────────────────────────────────

/// How the currently running binary was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    /// Packaged MSIX installed via the Microsoft Store (or sideloaded).
    Store,
    /// Per-user NSIS installer (registry Uninstall entry present).
    #[cfg_attr(not(windows), allow(dead_code))]
    Installer,
    /// Portable single-executable distribution.
    Portable,
}

/// A newer release usable for this install kind.
#[derive(Debug, Clone)]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: String,
    pub inner: sigilup::AvailableUpdate,
}

/// Result of installing a verified update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    /// Portable replacement done in place; caller should relaunch.
    ReplacedRestartPending,
    /// Installer spawned; caller should exit so it can take over.
    ///
    /// Only the Windows NSIS channel (`install_via_installer`) spawns an installer,
    /// but the variant stays in the shared enum so the UI match is exhaustive on
    /// every platform.
    #[cfg_attr(not(windows), allow(dead_code))]
    InstallerLaunched,
}

// ── Install-kind detection ───────────────────────────────────────────────────

/// True when running with package identity (MSIX / Store install).
#[cfg(windows)]
pub fn has_package_identity() -> bool {
    windows::ApplicationModel::Package::Current().is_ok()
}

/// True when running with package identity (never on other platforms).
#[cfg(not(windows))]
pub fn has_package_identity() -> bool {
    false
}

/// Detect how this binary was installed (drives both update path and UI copy).
pub fn detect_install_kind() -> InstallKind {
    #[cfg(windows)]
    {
        if has_package_identity() {
            return InstallKind::Store;
        }
        if installer_registry_entry_exists() {
            return InstallKind::Installer;
        }
        InstallKind::Portable
    }
    #[cfg(not(windows))]
    {
        InstallKind::Portable
    }
}

#[cfg(windows)]
fn installer_registry_entry_exists() -> bool {
    const UNINSTALL_KEY: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\limedl-native";
    windows_registry::CURRENT_USER.open(UNINSTALL_KEY).is_ok()
}

/// Expected `(channel, expected_kind)` pair for [`UpdateEvaluator`].
fn evaluator_channel_and_kind(kind: InstallKind) -> (Option<&'static str>, &'static str) {
    match kind {
        InstallKind::Installer => (None, "installer"),
        InstallKind::Portable | InstallKind::Store => (Some("portable"), "portable"),
    }
}

/// Release artifacts must be this repo's GitHub release downloads for exactly
/// the advertised version.
///
/// Defence in depth on top of `sigilup`'s `UrlPolicy::with_prefixes`: keeps a
/// manifest bug from pointing at a different tag's asset directory.
fn validate_asset_url(url: &str, version: &str) -> Result<()> {
    let github_prefix = format!("https://github.com/{RELEASE_REPO}/releases/download/v{version}/");
    let mirror_prefix = format!("{GH_PROXY_PREFIX}{github_prefix}");
    let file_name = url
        .strip_prefix(&github_prefix)
        .or_else(|| url.strip_prefix(&mirror_prefix))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "artifact URL '{url}' is not under '{github_prefix}' or '{mirror_prefix}'"
            )
        })?;
    let sane = !file_name.is_empty()
        && file_name.len() <= 128
        && file_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'));
    if !sane {
        bail!("artifact URL '{url}' does not end in a plain file name");
    }
    Ok(())
}

// ── Trust store & evaluator ──────────────────────────────────────────────────

fn load_pin_file() -> Result<KeyPinFile> {
    let json = {
        #[cfg(test)]
        if let Some(override_json) = TEST_PINS_JSON.get() {
            override_json.as_str()
        } else {
            EMBEDDED_PINS_JSON
        }
        #[cfg(not(test))]
        EMBEDDED_PINS_JSON
    };
    KeyPinFile::from_json_str(json).context("parse embedded sigilup-pins.json")
}

#[cfg(test)]
static TEST_PINS_JSON: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn build_evaluator(
    trust_store: TrustStore,
    current_version: semver::Version,
    kind: InstallKind,
    url_policy: UrlPolicy,
) -> UpdateEvaluator {
    let (channel, expected_kind) = evaluator_channel_and_kind(kind);
    let mut evaluator = UpdateEvaluator::new(trust_store, current_version)
        .with_policy(VerificationPolicy::strict())
        .with_url_policy(url_policy)
        .with_expected_kind(expected_kind);
    if let Some(ch) = channel {
        evaluator = evaluator.with_channel(ch);
    }
    evaluator
}

// ── Check for update ─────────────────────────────────────────────────────────

/// Fetch the latest signed manifest and return a newer update usable by this
/// install kind, or `None` when already up to date / no matching asset.
pub async fn check_for_update(
    settings: &AppSettings,
    state_dir: &Path,
) -> Result<Option<AvailableUpdate>> {
    let kind = detect_install_kind();
    if kind == InstallKind::Store {
        bail!("store installs update via Microsoft Store; use store::check_update_available");
    }

    let pins = load_pin_file()?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .context("parse CARGO_PKG_VERSION as semver")?;
    let mut evaluator = build_evaluator(
        pins.to_trust_store(),
        current,
        kind,
        UrlPolicy::with_prefixes(ALLOWED_DOWNLOAD_PREFIXES.iter().copied()),
    );
    let mut epoch_store = FileEpochStore::new(state_dir.join("sigilup-epoch"));
    let updater =
        ReqwestUpdater::new(http_client(settings)?).with_max_artifact_bytes(MAX_UPDATE_BYTES);
    let mut ctx = VerifyContext::at_unix(unix_now());
    ctx.min_security_epoch = pins.min_security_epoch;

    let decision = check_for_update_with(
        &updater,
        MANIFEST_URLS,
        &mut evaluator,
        &mut epoch_store,
        ctx,
        true,
    )
    .await?;

    Ok(decision)
}

async fn check_for_update_with<S: EpochStore>(
    updater: &ReqwestUpdater,
    manifest_urls: &[&str],
    evaluator: &mut UpdateEvaluator,
    epoch_store: &mut S,
    ctx: VerifyContext,
    enforce_version_url_prefix: bool,
) -> Result<Option<AvailableUpdate>> {
    match updater
        .check_for_update_any::<serde_json::Value, S>(manifest_urls, evaluator, epoch_store, ctx)
        .await
        .context("check for update via sigilup")?
    {
        UpdateDecision::UpToDate => Ok(None),
        UpdateDecision::UpdateAvailable(inner) => {
            let version = inner.version.to_string();
            if enforce_version_url_prefix {
                for url in inner.asset.all_urls() {
                    validate_asset_url(url, &version).with_context(|| {
                        format!("release v{version} declares an unusable artifact URL")
                    })?;
                }
            }
            let notes = inner.notes.clone();
            Ok(Some(AvailableUpdate {
                version,
                notes,
                inner: *inner,
            }))
        }
    }
}

/// Build the HTTP client used for every update request (manifest, artifact).
///
/// Goes through limedl-core's [`configure_client_builder`] so the user's proxy
/// setting (`Manual` / `Disabled` / `System`) applies to updates just as it
/// does to downloads.
fn http_client(settings: &AppSettings) -> Result<reqwest::Client> {
    configure_client_builder(reqwest::Client::builder(), settings)
        .context("configure update http client")?
        .user_agent(format!("limedl-native/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("build update http client")
}

// ── Download + verify ────────────────────────────────────────────────────────

/// Stream-download the update artifact and verify `size + BLAKE3 + SHA-512` in
/// flight against the signed manifest digest, returning the verified file path.
///
/// `progress` receives `(bytes_downloaded, total_bytes_if_known)`.
pub async fn download_and_verify(
    update: &AvailableUpdate,
    settings: &AppSettings,
    state_dir: &Path,
    progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
) -> Result<PathBuf> {
    let work_dir = update_work_dir(state_dir);
    let updater =
        ReqwestUpdater::new(http_client(settings)?).with_max_artifact_bytes(MAX_UPDATE_BYTES);
    download_and_verify_with(&updater, update, &work_dir, progress).await
}

async fn download_and_verify_with(
    updater: &ReqwestUpdater,
    update: &AvailableUpdate,
    work_dir: &Path,
    progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
) -> Result<PathBuf> {
    updater
        .download_and_verify(&update.inner, work_dir, |done, total| {
            progress(done, total);
        })
        .await
        .context("download and verify update artifact")
}

/// Install an already-verified artifact for the current install kind.
pub fn install_verified(update: &AvailableUpdate, verified_file: &Path) -> Result<InstallOutcome> {
    match detect_install_kind() {
        InstallKind::Store => bail!("store installs must be updated via Microsoft Store"),
        InstallKind::Installer => install_via_installer(verified_file),
        InstallKind::Portable => install_via_self_replace(update, verified_file),
    }
}

/// NSIS installer: run passive (+ restart-after-install) and let it take over.
#[cfg(windows)]
fn install_via_installer(setup_exe: &Path) -> Result<InstallOutcome> {
    std::process::Command::new(setup_exe)
        .args(["/P", "/R"])
        .spawn()
        .with_context(|| format!("launch installer {}", setup_exe.display()))?;
    Ok(InstallOutcome::InstallerLaunched)
}

#[cfg(not(windows))]
fn install_via_installer(_file: &Path) -> Result<InstallOutcome> {
    bail!("installer channel is only supported on Windows")
}

/// Portable: extract the executable from the verified archive and replace in place.
fn install_via_self_replace(
    update: &AvailableUpdate,
    verified_file: &Path,
) -> Result<InstallOutcome> {
    let staging = verified_file
        .parent()
        .ok_or_else(|| anyhow::anyhow!("verified archive has no parent dir"))?
        .join(format!("extracted-{}", update.version));
    let exe_name = format!("limedl-native{}", std::env::consts::EXE_SUFFIX);

    let res = sigilup::replace::install_portable_update(
        verified_file,
        &staging,
        &exe_name,
        MAX_UPDATE_BYTES,
    )
    .context("install portable update via sigilup");
    let _ = std::fs::remove_dir_all(&staging);
    res?;

    Ok(InstallOutcome::ReplacedRestartPending)
}

// ── Restart ──────────────────────────────────────────────────────────────────

static RESTART_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Ask the Slint event loop to quit cleanly so `run()` in `main.rs` can flush
/// manifests, checkpoint the SQLite WAL, and shut down the BitTorrent session.
pub fn request_quit() {
    let _ = slint::invoke_from_event_loop(|| {
        let _ = slint::quit_event_loop();
    });
}

/// Request a graceful shutdown followed by spawning the updated executable.
pub fn restart_application() -> Result<()> {
    // Validate that the executable path resolves before initiating shutdown.
    let _exe = std::env::current_exe().context("resolve current executable")?;
    RESTART_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
    request_quit();
    Ok(())
}

/// Take the pending restart flag after the event loop and engine have shut down.
pub fn take_restart_requested() -> bool {
    RESTART_REQUESTED.swap(false, std::sync::atomic::Ordering::SeqCst)
}

/// Spawn the replacement executable after the single-instance lock is released.
pub fn spawn_replacement_process() -> Result<()> {
    let exe = std::env::current_exe().context("resolve current executable")?;
    std::process::Command::new(exe)
        .spawn()
        .context("relaunch after update")?;
    Ok(())
}

// ── Work dir housekeeping ────────────────────────────────────────────────────

pub fn update_work_dir(state_dir: &Path) -> PathBuf {
    state_dir.join(UPDATE_WORK_DIR)
}

/// Remove stale downloads from a previous (possibly interrupted) update.
pub fn clean_update_work_dir(state_dir: &Path) {
    let dir = update_work_dir(state_dir);
    if dir.exists()
        && let Err(e) = std::fs::remove_dir_all(&dir)
    {
        tracing::debug!("failed to clean update work dir {}: {e:#}", dir.display());
    }
}

// ── Check throttling ─────────────────────────────────────────────────────────

/// True when at least `min_interval` has passed since the last recorded check.
pub fn should_check_for_update_now(state_dir: &Path, min_interval: Duration) -> bool {
    let stamp = state_dir.join("update-check.stamp");
    match std::fs::read_to_string(&stamp)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
    {
        // A stamp from the future (clock rolled back) is treated as fresh.
        Some(secs) => Duration::from_secs(unix_now().saturating_sub(secs)) >= min_interval,
        None => true,
    }
}

/// Record that an update check just happened.
pub fn record_update_check(state_dir: &Path) {
    let stamp = state_dir.join("update-check.stamp");
    if let Some(parent) = stamp.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&stamp, unix_now().to_string());
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Microsoft Store channel (MSIX) ───────────────────────────────────────────

/// Store-driven updates: the OS owns download, signature and install; we only
/// surface checks and trigger the (optionally silent) update flow.
///
/// The real implementation is Windows-only (`StoreContext` WinRT); the stub at
/// the end of this section mirrors the same API on macOS/Linux, where the Store
/// channel does not exist (the calls fail and the UI reports the error).
///
/// All calls must run on the UI thread (`slint::spawn_local`) —
/// `StoreContext::GetDefault` in a desktop app associates with the window and
/// fails with `ERROR_INVALID_WINDOW_HANDLE` off-thread.
#[cfg(windows)]
pub mod store {
    use anyhow::{Context, Result};

    /// Check whether the Store has a package update for this app.
    pub async fn check_update_available() -> Result<bool> {
        use windows::Services::Store::StoreContext;

        let ctx = StoreContext::GetDefault().context("StoreContext::GetDefault")?;
        let updates = ctx
            .GetAppAndOptionalStorePackageUpdatesAsync()
            .context("query store package updates")?
            .await
            .context("store package update query failed")?;
        Ok(updates.Size().context("read update list size")? > 0)
    }

    /// Download + install the pending Store update (shows the Store confirm
    /// dialog, then the OS replaces the package and relaunches).
    pub async fn trigger_update() -> Result<()> {
        use windows::Services::Store::StoreContext;

        let ctx = StoreContext::GetDefault().context("StoreContext::GetDefault")?;
        let updates = ctx
            .GetAppAndOptionalStorePackageUpdatesAsync()
            .context("query store package updates")?
            .await
            .context("store package update query failed")?;
        ctx.RequestDownloadAndInstallStorePackageUpdatesAsync(&updates)
            .context("request store package update install")?
            .await
            .context("store package update install failed")?;
        Ok(())
    }
}

/// Non-Windows stub: the Microsoft Store channel is Windows-only.
#[cfg(not(windows))]
pub mod store {
    use anyhow::Result;

    /// Always fails on non-Windows platforms.
    pub async fn check_update_available() -> Result<bool> {
        anyhow::bail!("the Microsoft Store update channel is only available on Windows")
    }

    /// Always fails on non-Windows platforms.
    pub async fn trigger_update() -> Result<()> {
        anyhow::bail!("the Microsoft Store update channel is only available on Windows")
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
