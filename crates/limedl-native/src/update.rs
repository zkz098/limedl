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
// releases there, so stable users never see rc/alpha builds). The manifest is
// NOT itself signed; every artifact's `signature` field is the base64-encoded
// minisign signature over the exact downloaded bytes, verified against the
// public key shared with the Tauri edition below.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use serde::Deserialize;

// ── Constants ────────────────────────────────────────────────────────────────

/// minisign public key (base64 of the key file text), shared with the Tauri
/// edition so both UIs verify with the same identity. Public by design — the
/// corresponding private key only lives in GitHub Actions secrets.
const PUBKEY_B64: &str =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc2RTVFQzcwMjEyMDYyQTYKUldTbVlpQWhjT3psZHJoTE13aTdHRUhsSkxJSDEyRmtuemVIVXlTMjE1RldpWDZsMlpTcW03aXoK";

/// Permanent URL that always resolves to the newest stable manifest asset.
const MANIFEST_URL: &str =
    "https://github.com/zkz098/limedl/releases/latest/download/latest-native.json";

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

/// The manifest key for this platform + install kind.
///
/// `windows-x86_64` → installer; `windows-x86_64-portable` → portable zip;
/// `linux-x86_64-portable` / `darwin-aarch64-portable` → tar.gz.
fn manifest_key(kind: InstallKind) -> String {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "darwin",
        _ => "linux",
    };
    let base = format!("{}-{}", os, std::env::consts::ARCH);
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
    let asset = manifest.platforms.get(&key).cloned().with_context(|| {
        format!(
            "release v{} has no '{}' asset for this distribution channel",
            manifest.version, key
        )
    })?;
    let expected_kind = if kind == InstallKind::Installer { "installer" } else { "portable" };
    if asset.kind != expected_kind {
        bail!(
            "asset '{}' is kind '{}', expected '{expected_kind}'",
            key,
            asset.kind
        );
    }

    Ok(Some(AvailableUpdate {
        version: manifest.version,
        notes: manifest.notes,
        asset,
    }))
}

async fn fetch_manifest() -> Result<UpdateManifest> {
    let client = http_client()?;
    let resp = client
        .get(MANIFEST_URL)
        .send()
        .await
        .context("fetch update manifest")?
        .error_for_status()
        .context("update manifest request failed")?;
    let bytes = resp.bytes().await.context("read update manifest")?;
    serde_json::from_slice(&bytes).context("parse update manifest")
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

    let name = verified_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    #[cfg(windows)]
    {
        let _ = name;
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
    use minisign_verify::{PublicKey, Signature};

    let pubkey_text = base64::engine::general_purpose::STANDARD
        .decode(PUBKEY_B64)
        .context("decode embedded updater public key")?;
    let pubkey = PublicKey::decode(std::str::from_utf8(&pubkey_text)?)
        .context("parse embedded updater public key")?;

    let sig_text = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.trim())
        .context("decode artifact signature")?;
    let sig = Signature::decode(std::str::from_utf8(&sig_text)?)
        .context("parse artifact signature")?;

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

#[cfg(windows)]
pub mod store {
    //! Store-driven updates: the OS owns download, signature and install; we
    //! only surface checks and trigger the (optionally silent) update flow.
    //!
    //! All calls must run on the UI thread (`slint::spawn_local`) —
    //! `StoreContext::GetDefault` in a desktop app associates with the window
    //! and fails with `ERROR_INVALID_WINDOW_HANDLE` off-thread.

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
    fn manifest_parses_tauri_shaped_json() {
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
}
