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

/// Legacy minisign public key retained solely for `cargo xtask guard` while
/// `release.yml` dual-publishes `latest-native.json` for older clients during
/// the `sigilup` migration window. Remove in the post-transition cleanup.
#[allow(dead_code)]
const PUBKEY_B64: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEUwQ0I4MjY5M0NFQ0Q0OUUKUldTZTFPdzhhWUxMNEl5MGVSRk4rNDB1bXF4ZDJreGxQb3lnMUFxSmRQQmJsQk1TTURPaEt3KzcK";

/// Legacy ML-DSA-65 public key retained solely for `cargo xtask guard` during
/// the `latest-native.json` dual-publish transition window.
#[allow(dead_code)]
const PQC_PUBKEY_B64: &str = "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";

/// Repo hosting the release assets.
const RELEASE_REPO: &str = "zkz098/limedl";

/// Permanent URL prefix for release downloads of this repository.
const RELEASE_DOWNLOAD_PREFIX: &str = "https://github.com/zkz098/limedl/releases/download/";

/// Permanent URL that always resolves to the newest stable signed manifest
/// (GitHub excludes draft/prerelease releases there, so stable users never see
/// rc/alpha builds).
const MANIFEST_URL: &str =
    "https://github.com/zkz098/limedl/releases/latest/download/latest.hsig.json";

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
    let prefix = format!("https://github.com/{RELEASE_REPO}/releases/download/v{version}/");
    let Some(file_name) = url.strip_prefix(&prefix) else {
        bail!("artifact URL '{url}' is not under '{prefix}'");
    };
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
        UrlPolicy::with_prefixes([RELEASE_DOWNLOAD_PREFIX]),
    );
    let mut epoch_store = FileEpochStore::new(state_dir.join("sigilup-epoch"));
    let updater =
        ReqwestUpdater::new(http_client(settings)?).with_max_artifact_bytes(MAX_UPDATE_BYTES);
    let mut ctx = VerifyContext::at_unix(unix_now());
    ctx.min_security_epoch = pins.min_security_epoch;

    let decision = check_for_update_with(
        &updater,
        MANIFEST_URL,
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
    manifest_url: &str,
    evaluator: &mut UpdateEvaluator,
    epoch_store: &mut S,
    ctx: VerifyContext,
    enforce_version_url_prefix: bool,
) -> Result<Option<AvailableUpdate>> {
    match updater
        .check_for_update::<serde_json::Value, S>(manifest_url, evaluator, epoch_store, ctx)
        .await
        .context("check for update via sigilup")?
    {
        UpdateDecision::UpToDate => Ok(None),
        UpdateDecision::UpdateAvailable(inner) => {
            let version = inner.version.to_string();
            if enforce_version_url_prefix {
                validate_asset_url(&inner.asset.url, &version).with_context(|| {
                    format!("release v{version} declares an unusable artifact URL")
                })?;
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
