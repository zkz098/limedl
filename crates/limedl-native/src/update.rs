// Self-update integration for limedl-native.
//
// Three distribution channels with separate update paths:
//
// - **MSIX / Microsoft Store**: the OS signs and updates the package. We only
//   surface `StoreContext` update checks/installs (see [`store`]). Package
//   identity (`Package::Current()`) is the discriminator.
// - **GitHub installer** (NSIS setup.exe, per-user): download → minisign
//   verify → spawn the installer in passive mode; the installer handles
//   stopping the running app and relaunching.
// - **GitHub portable** (single exe zip / linux tar.gz): download → verify →
//   `self_replace` in place → relaunch.
//
// All GitHub-channel artifacts are described by `latest-native.json`, hosted
// as a release asset and reached through the permanently-named
// `releases/latest/download/...` URL (GitHub excludes draft/prerelease
// releases there, so stable users never see rc/alpha builds).
//
// Trust chain, each step checked before the next one happens:
//
// 1. `latest-native.json.sig` (minisign, written by `cargo xtask sign`) is
//    verified over the exact manifest bytes, so a tampered manifest cannot even
//    choose a download URL.
// 2. Every artifact `url` must be a GitHub release download of this repo for the
//    version the manifest advertises.
// 3. The artifact's `signature` field is base64(minisign signature text) over the
//    exact downloaded bytes, verified against [`PUBKEY_B64`].
//
// The signing key lives in the CI secret `LIMEDL_SIGNING_KEY`;
// `cargo xtask generate-key` creates a new pair and `cargo xtask guard` (run by
// the release job) keeps that secret and [`PUBKEY_B64`] in sync.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use serde::Deserialize;

// ── Constants ────────────────────────────────────────────────────────────────

/// minisign public key (base64 of the key file text). Public by design — the
/// private key only lives in the CI secret `LIMEDL_SIGNING_KEY`.
///
/// `cargo xtask guard` (release job) refuses to ship a build whose constant does
/// not match the key the artifacts were signed with, so rotating the secret
/// cannot silently break the update channel. Rotation runbook:
/// `.opencode/guides/subsystem-self-update.md` (keynum E0CB82693CECD49E).
const PUBKEY_B64: &str =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEUwQ0I4MjY5M0NFQ0Q0OUUKUldTZTFPdzhhWUxMNEl5MGVSRk4rNDB1bXF4ZDJreGxQb3lnMUFxSmRQQmJsQk1TTURPaEt3KzcK";

/// Repo hosting the release assets; must match `$Repo` in
/// `scripts/gen-native-manifest.ps1` and the release workflow.
const RELEASE_REPO: &str = "zkz098/limedl";

/// Permanent URLs that always resolve to the newest stable manifest assets
/// (GitHub excludes draft/prerelease releases there, so stable users never see
/// rc/alpha builds).
const MANIFEST_URL: &str =
    "https://github.com/zkz098/limedl/releases/latest/download/latest-native.json";
const MANIFEST_SIG_URL: &str =
    "https://github.com/zkz098/limedl/releases/latest/download/latest-native.json.sig";

/// Upper bound for an update download. Installers and portable archives are
/// tens of megabytes; anything beyond this is a broken or hostile manifest.
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
    Installer,
    /// Portable single-executable distribution.
    Portable,
}

/// One platform entry of `latest-native.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct PlatformAsset {
    /// `"installer"` or `"portable"`.
    pub kind: String,
    /// Direct download URL (browser_download_url — no API quota consumed).
    pub url: String,
    /// base64(minisign signature text) over the exact artifact bytes.
    pub signature: String,
    /// Optional secondary integrity check (lowercase hex sha256).
    #[serde(default)]
    pub sha256: Option<String>,
}

/// Parsed `latest-native.json`.
#[derive(Debug, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub platforms: HashMap<String, PlatformAsset>,
}

impl UpdateManifest {
    /// True when the release carries any asset for this OS/arch (any install
    /// kind). Used to tell "this release simply does not serve your platform"
    /// apart from "your distribution channel is missing from it".
    fn has_assets_for(&self, platform_base: &str) -> bool {
        let prefixed = format!("{platform_base}-");
        self.platforms
            .keys()
            .any(|key| key == platform_base || key.starts_with(&prefixed))
    }
}

/// Release artifacts must be this repo's GitHub release downloads for exactly
/// the advertised version.
///
/// The manifest is signed, so this is defence in depth: it keeps a manifest bug
/// (or a future mirror/fork) from pointing the updater at an arbitrary URL and
/// making clients download unrelated bytes before the signature check fails.
fn validate_asset_url(url: &str, version: &str) -> Result<()> {
    let prefix = format!(
        "https://github.com/{RELEASE_REPO}/releases/download/v{version}/"
    );
    let Some(file_name) = url.strip_prefix(&prefix) else {
        bail!("artifact URL '{url}' is not under '{prefix}'");
    };
    let sane = !file_name.is_empty()
        && file_name.len() <= 128
        && file_name.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+')
        });
    if !sane {
        bail!("artifact URL '{url}' does not end in a plain file name");
    }
    Ok(())
}

/// A newer release usable for this install kind.
#[derive(Debug, Clone)]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: String,
    pub asset: PlatformAsset,
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
    use winreg::enums::HKEY_CURRENT_USER;
    const UNINSTALL_KEY: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\limedl-native";
    winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(UNINSTALL_KEY)
        .is_ok()
}

/// The OS/arch half of a manifest key (`windows-x86_64`, `linux-aarch64`, …).
fn platform_base() -> String {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "darwin",
        _ => "linux",
    };
    format!("{}-{}", os, std::env::consts::ARCH)
}

/// The manifest key for this platform + install kind.
///
/// `windows-x86_64` → installer; `windows-x86_64-portable` → portable zip;
/// `linux-x86_64-portable` / `darwin-aarch64-portable` → tar.gz.
fn manifest_key(kind: InstallKind) -> String {
    let base = platform_base();
    match kind {
        InstallKind::Installer => base,
        InstallKind::Portable | InstallKind::Store => format!("{base}-portable"),
    }
}

// ── Check for update ─────────────────────────────────────────────────────────

/// Fetch the latest manifest and return a newer update usable by this
/// install kind, or `None` when already up to date / no matching asset.
pub async fn check_for_update() -> Result<Option<AvailableUpdate>> {
    let kind = detect_install_kind();
    if kind == InstallKind::Store {
        bail!("store installs update via Microsoft Store; use store::check_update_available");
    }

    let manifest = fetch_manifest().await?;
    let current = env!("CARGO_PKG_VERSION");
    if !is_newer_version(&manifest.version, current) {
        return Ok(None);
    }

    let key = manifest_key(kind);
    let asset = match manifest.platforms.get(&key) {
        Some(asset) => asset.clone(),
        None => {
            let base = platform_base();
            if !manifest.has_assets_for(&base) {
                // The release carries nothing for this OS/arch at all — e.g. the
                // desktop client is Windows-only today. That is not a failure
                // the user can act on, so report "no update" instead.
                tracing::debug!(
                    "update manifest for v{} has no '{base}' assets; skipping",
                    manifest.version
                );
                return Ok(None);
            }
            bail!(
                "release v{} has no '{key}' asset for this distribution channel (available: {:?})",
                manifest.version,
                manifest.platforms.keys().collect::<Vec<_>>()
            );
        }
    };
    let expected_kind = if kind == InstallKind::Installer { "installer" } else { "portable" };
    if asset.kind != expected_kind {
        bail!(
            "asset '{}' is kind '{}', expected '{expected_kind}'",
            key,
            asset.kind
        );
    }
    validate_asset_url(&asset.url, &manifest.version).with_context(|| {
        format!(
            "release v{} declares an unusable artifact URL",
            manifest.version
        )
    })?;

    Ok(Some(AvailableUpdate {
        version: manifest.version,
        notes: manifest.notes,
        asset,
    }))
}

/// Fetch the manifest, verify its signature *before* parsing it, and only then
/// hand the bytes to serde.
///
/// The signature is a separate release asset (`latest-native.json.sig`, written
/// by `cargo xtask sign`). Verifying it first means a tampered or truncated
/// manifest is rejected without ever influencing a download URL.
async fn fetch_manifest() -> Result<UpdateManifest> {
    let client = http_client()?;
    let bytes = fetch_asset(&client, MANIFEST_URL, "update manifest").await?;
    let sig_text = fetch_text(&client, MANIFEST_SIG_URL)
        .await
        .context("update manifest signature is missing — refusing an unsigned manifest")?;
    verify_signature_text(&bytes, &sig_text)
        .context("update manifest signature verification failed")?;
    serde_json::from_slice(&bytes).context("parse update manifest")
}

async fn fetch_asset(client: &reqwest::Client, url: &str, what: &str) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("fetch {what}"))?
        .error_for_status()
        .with_context(|| format!("{what} request failed"))?;
    if let Some(len) = resp.content_length()
        && len > MAX_UPDATE_BYTES
    {
        bail!("{what} is {len} bytes, above the {MAX_UPDATE_BYTES} byte limit");
    }
    let bytes = resp
        .bytes()
        .await
        .with_context(|| format!("read {what}"))?;
    if bytes.len() as u64 > MAX_UPDATE_BYTES {
        bail!("{what} exceeded the {MAX_UPDATE_BYTES} byte limit");
    }
    Ok(bytes.to_vec())
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let bytes = fetch_asset(client, url, "text asset").await?;
    String::from_utf8(bytes).context("asset is not UTF-8")
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(format!("limedl-native/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("build update http client")
}

// ── Download + verify ────────────────────────────────────────────────────────

/// Download the update artifact, verify integrity (sha256 when present) and
/// authenticity (minisign, always), returning the verified file path.
///
/// `progress` receives `(bytes_downloaded, total_bytes_if_known)`.
pub async fn download_and_verify(
    update: &AvailableUpdate,
    state_dir: &Path,
    progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
) -> Result<PathBuf> {
    let work_dir = update_work_dir(state_dir);
    std::fs::create_dir_all(&work_dir)
        .with_context(|| format!("create update work dir {}", work_dir.display()))?;

    let file_name = update
        .asset
        .url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("limedl-update.bin");
    let dest = work_dir.join(file_name);

    let client = http_client()?;
    let mut resp = client
        .get(&update.asset.url)
        .send()
        .await
        .context("download update")?
        .error_for_status()
        .context("update download request failed")?;
    let total = resp.content_length();
    if let Some(total) = total
        && total > MAX_UPDATE_BYTES
    {
        bail!(
            "update artifact is {total} bytes, above the {MAX_UPDATE_BYTES} byte limit \
             (refusing to download)"
        );
    }

    let mut file = std::fs::File::create(&dest)
        .with_context(|| format!("create {}", dest.display()))?;
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    let mut downloaded: u64 = 0;
    while let Some(chunk) = resp
        .chunk()
        .await
        .context("read update download stream")?
    {
        file.write_all(&chunk).context("write update download")?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        if downloaded > MAX_UPDATE_BYTES {
            bail!(
                "update download exceeded the {MAX_UPDATE_BYTES} byte limit — aborting"
            );
        }
        progress(downloaded, total);
    }
    file.flush().ok();
    drop(file);

    let bytes =
        std::fs::read(&dest).with_context(|| format!("re-read {}", dest.display()))?;
    if let Some(expected) = update.asset.sha256.as_deref() {
        verify_sha256(&bytes, expected)
            .context("sha256 mismatch (update download corrupted?)")?;
    }
    verify_signature(&bytes, &update.asset.signature)
        .context("minisign signature verification failed (update not authentic)")?;

    Ok(dest)
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

/// Portable: extract the executable from the archive and replace in place.
fn install_via_self_replace(update: &AvailableUpdate, verified_file: &Path) -> Result<InstallOutcome> {
    let Some(new_exe) = extract_executable(update, verified_file)? else {
        bail!(
            "portable archive from v{} contains no matching executable",
            update.version
        );
    };
    self_replace::self_replace(&new_exe)
        .context("replace running executable (in-place self-update)")?;
    if let Some(parent) = new_exe.parent() {
        // Remove leftovers (archive stays behind; safe to ignore errors).
        let _ = std::fs::remove_dir_all(parent);
    }
    Ok(InstallOutcome::ReplacedRestartPending)
}

/// Extract the executable member from the verified archive.
///
/// Windows portable archives are zips containing `limedl-native.exe`;
/// linux/macOS archives are `.tar.gz` containing `limedl-native`.
#[allow(clippy::needless_return)]
fn extract_executable(update: &AvailableUpdate, verified_file: &Path) -> Result<Option<PathBuf>> {
    let staging = verified_file
        .parent()
        .ok_or_else(|| anyhow!("archive has no parent dir"))?
        .join(format!("extracted-{}", update.version));
    std::fs::create_dir_all(&staging).context("create extraction dir")?;

    #[cfg(windows)]
    {
        let archive = std::fs::File::open(verified_file).context("open downloaded archive")?;
        let mut zip = zip::ZipArchive::new(std::io::BufReader::new(archive))
            .context("open portable zip archive")?;
        // Take the first *.exe member — portable zips ship exactly one.
        let exe_index = (0..zip.len())
            .find(|&i| {
                zip.by_index(i)
                    .map(|f| f.name().ends_with(".exe"))
                    .unwrap_or(false)
            })
            .context("no .exe inside portable zip")?;
        let member_name = zip.by_index(exe_index)?.name().to_string();
        let out_path = staging.join(
            Path::new(&member_name)
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("limedl-native.exe")),
        );
        let mut src = zip.by_index(exe_index)?;
        let mut out = std::fs::File::create(&out_path)?;
        std::io::copy(&mut src, &mut out)?;
        return Ok(Some(out_path));
    }

    #[cfg(not(windows))]
    {
        let _ = update;
        let archive = std::fs::File::open(verified_file).context("open downloaded archive")?;
        let gz = flate2::read::GzDecoder::new(std::io::BufReader::new(archive));
        let mut tar = tar::Archive::new(gz);
        for entry in tar.entries().context("read tar entries")? {
            let mut entry = entry.context("read tar entry")?;
            let path = entry.path().context("tar entry path")?.into_owned();
            let is_exec = path.file_name().is_some_and(|n| n == "limedl-native");
            if !is_exec {
                continue;
            }
            let out_path = staging.join("limedl-native");
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(0o755));
            }
            return Ok(Some(out_path));
        }
        return Ok(None);
    }
}

// ── Restart ──────────────────────────────────────────────────────────────────

/// Spawn the (already replaced) executable and exit the current process.
pub fn restart_application() -> Result<()> {
    let exe = std::env::current_exe().context("resolve current executable")?;
    std::process::Command::new(exe)
        .spawn()
        .context("relaunch after update")?;
    std::process::exit(0);
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

// ── Crypto verification ──────────────────────────────────────────────────────

fn verify_signature(data: &[u8], signature_b64: &str) -> Result<()> {
    let sig_text = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.trim())
        .context("decode artifact signature")?;
    verify_signature_text(data, std::str::from_utf8(&sig_text)?)
}

/// Verify a signature given as the plain minisign `.sig` text (what
/// `cargo xtask sign` writes next to each artifact).
fn verify_signature_text(data: &[u8], signature_text: &str) -> Result<()> {
    use minisign_verify::{PublicKey, Signature};

    let pubkey_text = base64::engine::general_purpose::STANDARD
        .decode(PUBKEY_B64)
        .context("decode embedded updater public key")?;
    let pubkey = PublicKey::decode(std::str::from_utf8(&pubkey_text)?)
        .context("parse embedded updater public key")?;

    let sig = Signature::decode(signature_text.trim()).context("parse artifact signature")?;

    pubkey
        .verify(data, &sig, false)
        .map_err(|e| anyhow!("minisign verification rejected artifact: {e}"))
}

fn verify_sha256(data: &[u8], expected_hex: &str) -> Result<()> {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(data);
    let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if !actual.eq_ignore_ascii_case(expected_hex.trim()) {
        bail!("sha256 mismatch: expected {expected_hex}, got {actual}");
    }
    Ok(())
}

// ── Version comparison ───────────────────────────────────────────────────────

/// Strictly-newer semver comparison tolerant of a leading `v` and pre-release
/// suffixes (`1.2.3-rc.1 < 1.2.3`). Non-parseable candidates are never newer.
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    let (Some((c_nums, c_pre)), Some((cur_nums, cur_pre))) =
        (parse_version(candidate), parse_version(current))
    else {
        return false;
    };
    match c_nums.cmp(&cur_nums) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => match (c_pre, cur_pre) {
            // Same numeric version: a pre-release is older than the release.
            (Some(_), None) => false,
            (None, Some(_)) => true,
            (Some(a), Some(b)) => a > b,
            (None, None) => false,
        },
    }
}

fn parse_version(v: &str) -> Option<((u64, u64, u64), Option<String>)> {
    let v = v.trim().trim_start_matches('v').trim_start_matches('V');
    let (nums, pre) = match v.split_once('-') {
        Some((n, p)) => (n, Some(p.to_string())),
        None => (v, None),
    };
    let mut it = nums.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch = it.next()?.parse().ok()?;
    Some(((major, minor, patch), pre))
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
mod tests {
    use super::*;

    #[test]
    fn version_comparison_basics() {
        assert!(is_newer_version("0.3.0", "0.2.1"));
        assert!(is_newer_version("v0.3.0", "0.2.1"));
        assert!(is_newer_version("0.2.10", "0.2.9"));
        assert!(!is_newer_version("0.2.1", "0.2.1"));
        assert!(!is_newer_version("0.2.0", "0.2.1"));
        assert!(!is_newer_version("garbage", "0.2.1"));
        // Pre-release ordering.
        assert!(is_newer_version("0.2.1", "0.2.1-rc.1"));
        assert!(is_newer_version("0.2.1-rc.2", "0.2.1-rc.1"));
        assert!(!is_newer_version("0.2.1-rc.1", "0.2.1"));
    }

    #[test]
    fn manifest_parses_release_json() {
        let json = r#"{
            "version": "0.3.0",
            "notes": "hello",
            "platforms": {
                "windows-x86_64": {
                    "kind": "installer",
                    "url": "https://example.com/setup.exe",
                    "signature": "c2ln",
                    "sha256": "abc"
                },
                "windows-x86_64-portable": {
                    "kind": "portable",
                    "url": "https://example.com/app.zip",
                    "signature": "c2ln"
                }
            }
        }"#;
        let m: UpdateManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.version, "0.3.0");
        assert_eq!(m.platforms.len(), 2);
        assert_eq!(m.platforms["windows-x86_64"].kind, "installer");
        assert!(m.platforms["windows-x86_64-portable"].sha256.is_none());
    }

    #[test]
    fn manifest_keys_follow_install_kind() {
        assert_eq!(manifest_key(InstallKind::Installer), "windows-x86_64");
        assert_eq!(manifest_key(InstallKind::Portable), "windows-x86_64-portable");
    }

    #[test]
    fn asset_urls_must_point_at_this_repos_release() {
        let good = "https://github.com/zkz098/limedl/releases/download/v0.3.0/\
                    limedl-native-v0.3.0-windows-x86_64-setup.exe";
        assert!(validate_asset_url(good, "0.3.0").is_ok());

        // Wrong host / repo / version, or a URL that escapes the release path.
        for bad in [
            "https://evil.example/limedl-native-setup.exe",
            "https://github.com/attacker/limedl/releases/download/v0.3.0/setup.exe",
            "https://github.com/zkz098/limedl/releases/download/v0.2.9/setup.exe",
            "http://github.com/zkz098/limedl/releases/download/v0.3.0/setup.exe",
            "https://github.com/zkz098/limedl/releases/download/v0.3.0/../../other",
            "https://github.com/zkz098/limedl/releases/download/v0.3.0/",
            "https://github.com/zkz098/limedl/releases/download/v0.3.0/a/b.exe",
        ] {
            assert!(
                validate_asset_url(bad, "0.3.0").is_err(),
                "expected '{bad}' to be rejected"
            );
        }
    }

    #[test]
    fn platform_support_is_distinguished_from_a_channel_gap() {
        let json = r#"{
            "version": "0.3.0",
            "platforms": {
                "windows-x86_64": {
                    "kind": "installer",
                    "url": "https://github.com/zkz098/limedl/releases/download/v0.3.0/a.exe",
                    "signature": "c2ln"
                },
                "windows-x86_64-portable": {
                    "kind": "portable",
                    "url": "https://github.com/zkz098/limedl/releases/download/v0.3.0/a.zip",
                    "signature": "c2ln"
                }
            }
        }"#;
        let m: UpdateManifest = serde_json::from_str(json).unwrap();
        assert!(m.has_assets_for("windows-x86_64"));
        assert!(!m.has_assets_for("linux-x86_64"));
        assert!(!m.has_assets_for("darwin-aarch64"));
    }

    #[test]
    fn unsigned_manifest_bytes_are_rejected() {
        // The embedded public key must be well-formed and garbage signatures
        // must be refused (never fall through to parsing the manifest).
        for bogus in ["", "not a signature", "untrusted comment: x\nAAAA\n"] {
            let err = verify_signature_text(b"{}", bogus)
                .expect_err("bogus signature must be rejected")
                .to_string();
            assert!(
                err.contains("signature"),
                "expected a signature error, got: {err}"
            );
            assert!(
                !err.contains("public key"),
                "the embedded PUBKEY_B64 failed to decode: {err}"
            );
        }
    }
}
