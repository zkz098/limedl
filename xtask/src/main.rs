//! Repository tooling for the desktop self-update channel.
//!
//! This replaces the `npx --yes @tauri-apps/cli signer sign` step the retired
//! Tauri pipeline left behind. The signature format is unchanged (minisign,
//! prehashed BLAKE2b — what `crates/limedl-native/src/update/mod.rs` verifies with
//! `minisign-verify`), but the tool now lives next to the verifier and shares
//! its key handling instead of pulling a Node CLI that belongs to a project we
//! no longer use.
//!
//! ```text
//! cargo xtask generate-key --out-dir <dir>   # new keypair + ready-to-run gh secret commands
//! cargo xtask sign <files...>                # write <file>.sig
//! cargo xtask verify <files...>              # verify <file>.sig against a public key
//! cargo xtask guard <files...>               # release guard (see below)
//! cargo xtask manifest --version ...        # generate latest-native.json
//! cargo xtask fetch-font [--verify]          # pinned MiSans VF (build prerequisite)
//! cargo xtask bump-version <patch|minor|major>
//! ```
//!
//! `guard` is the release safety net: it derives the public key from the CI
//! signing secret and asserts that `PUBKEY_B64` in `update.rs` is that key, then
//! verifies every produced signature. Without it, rotating the CI secret while
//! forgetting the embedded constant would ship a client that rejects every
//! future update.
//!
//! Key sources:
//!
//! - `LIMEDL_SIGNING_KEY` — either a path to the key file or the base64 of the
//!   key file text (what GitHub secrets hold)
//!
//! The matching password comes from `LIMEDL_SIGNING_KEY_PASSWORD`.

use std::fmt::Write as _;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use clap::{Parser, Subcommand};
use fips204::ml_dsa_65;
use fips204::traits::{KeyGen, SerDes, Signer, Verifier};
use minisign::{KeyPair, PublicKey, SecretKey, SecretKeyBox, SignatureBox};
use rand::Rng as _;

mod bump_version;
mod fetch_font;
mod icons;
mod manifest;
mod theme;

/// base64 of the key file text — the form stored in CI secrets and embedded in
/// `update.rs`.
const BASE64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::STANDARD;

const KEY_ENV: &str = "LIMEDL_SIGNING_KEY";
const KEY_PASSWORD_ENV: &str = "LIMEDL_SIGNING_KEY_PASSWORD";
const PUBKEY_ENV: &str = "LIMEDL_SIGNING_PUBKEY";

const PQC_KEY_ENV: &str = "LIMEDL_PQC_SIGNING_KEY";
const PQC_PUBKEY_ENV: &str = "LIMEDL_PQC_SIGNING_PUBKEY";

const PQC_MANIFEST_CTX: &[u8] = b"limedl-manifest";
const PQC_ARTIFACT_CTX: &[u8] = b"limedl-artifact";

/// Where the client's embedded public key lives.
///
/// `release.yml` runs `cargo xtask guard <files>` with no `--update-rs`, so this
/// default is the path the release actually reads — and it points into the split
/// module (`update/mod.rs`, not `update.rs`).
/// `default_update_path_declares_the_embedded_pubkey` fails at test time when
/// the file moves again, instead of failing the `Update manifest` job after the
/// platform artifacts are already uploaded.
const DEFAULT_UPDATE_RS: &str = "crates/limedl-native/src/update/mod.rs";

#[derive(Parser)]
#[command(
    name = "xtask",
    about = "limedl repository tooling (signing, release manifest, font fetch, version bump)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate minisign and post-quantum (ML-DSA-65) keypairs for the self-update channel.
    GenerateKey {
        /// Directory to write key files into.
        #[arg(long)]
        out_dir: PathBuf,
        /// Base name of the key files.
        #[arg(long, default_value = "limedl-signing")]
        name: String,
        /// Password encrypting the secret key. Generated when omitted.
        #[arg(long)]
        password: Option<String>,
        /// Overwrite existing key files.
        #[arg(long)]
        force: bool,
        /// Generate only the post-quantum (ML-DSA-65) keypair, keeping the existing Minisign key.
        #[arg(long)]
        pqc_only: bool,
    },
    /// Sign files, writing minisign `<file>.sig` and (when PQC key is configured) ML-DSA-65 `<file>.pqc.sig` next to each one.
    Sign {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Verify signatures for each file.
    Verify {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Public key: base64 of the key file text, the key file text, or the
        /// bare base64 payload.
        #[arg(long)]
        pubkey: Option<String>,
        /// Read the public key from a `.pub` file.
        #[arg(long)]
        pubkey_file: Option<PathBuf>,
        /// Post-quantum public key: base64 of ML-DSA-65 public key bytes.
        #[arg(long)]
        pqc_pubkey: Option<String>,
        /// Read the post-quantum public key from a `.pub` file.
        #[arg(long)]
        pqc_pubkey_file: Option<PathBuf>,
    },
    /// Release guard: signing keys must match the keys embedded in the client
    /// and every signature must verify.
    Guard {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Path to the client's `update` module source holding `PUBKEY_B64` and `PQC_PUBKEY_B64`.
        #[arg(long, default_value = DEFAULT_UPDATE_RS)]
        update_rs: PathBuf,
    },
    /// Generate `latest-native.json`, the self-update manifest.
    Manifest {
        /// Released version, without the leading `v`.
        #[arg(long)]
        version: String,
        /// Asset map produced by the release job: `{ key: { kind, path } }`.
        #[arg(long)]
        assets_json: PathBuf,
        /// Release notes (the changelog body) embedded in the manifest.
        #[arg(long)]
        notes: String,
        /// Where to write the manifest.
        #[arg(long)]
        out_file: PathBuf,
        /// `owner/repo` used for the release download URLs.
        #[arg(long, default_value = "zkz098/limedl")]
        repo: String,
    },
    /// Fetch the pinned MiSans VF font into `crates/limedl-native/assets/fonts/`.
    FetchFont {
        /// Re-download even when the local copy already matches the pinned
        /// size/hash.
        #[arg(long)]
        force: bool,
        /// Check the local copy only; never touch the network. The CI jobs that
        /// receive the font as an artifact run this.
        #[arg(long)]
        verify: bool,
        /// Take the extracted `MiSansVF.ttf` from this local file or URL
        /// instead of Xiaomi's CDN (the pinned hash is still enforced).
        /// Defaults to `$LIMEDL_MISANS_TTF`.
        #[arg(long)]
        from_path: Option<String>,
        /// Alternate URL for the `MiSans.zip` archive (a mirror).
        #[arg(long, default_value = fetch_font::DEFAULT_ZIP_URL)]
        zip_url: String,
    },
    /// Bump the workspace version in Cargo.toml, Cargo.lock and website/.
    BumpVersion {
        /// Which version component to increment.
        #[arg(value_enum)]
        level: bump_version::Level,
        /// Edit the files but skip commit/tag/push.
        #[arg(long)]
        no_push: bool,
        /// Print what would change and exit without touching any file.
        #[arg(long)]
        dry_run: bool,
    },
    /// Slint theme operations (generate theme.slint, apply tokens, check unmapped colors).
    #[command(subcommand)]
    Theme(theme::ThemeCommand),
    /// Generate desktop packaging icons (MSIX, Linux hicolor, macOS iconset).
    #[command(subcommand)]
    GenIcons(icons::IconsCommand),
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::GenerateKey {
            out_dir,
            name,
            password,
            force,
            pqc_only,
        } => generate_key(&out_dir, &name, password, force, pqc_only),
        Command::Sign { files } => {
            let (sk, source) = load_secret_key()?;
            let pk = PublicKey::from_secret_key(&sk).context("derive public key from secret")?;
            println!("signing with {source} (keynum {})", keynum_hex(&pk));
            for file in &files {
                let sig_path = sign_file(&sk, &pk, file)?;
                println!("signed {} -> {}", file.display(), sig_path.display());
            }

            if let Some((pqc_sk, pqc_pk, pqc_source)) = load_pqc_key()? {
                println!("signing with PQC {pqc_source} (ML-DSA-65)");
                for file in &files {
                    let pqc_sig_path = sign_file_pqc(&pqc_sk, &pqc_pk, file)?;
                    println!("signed PQC {} -> {}", file.display(), pqc_sig_path.display());
                }
            }
            Ok(())
        }
        Command::Verify {
            files,
            pubkey,
            pubkey_file,
            pqc_pubkey,
            pqc_pubkey_file,
        } => {
            let pk = resolve_public_key(pubkey.as_deref(), pubkey_file.as_deref())?;
            println!("verifying with keynum {}", keynum_hex(&pk));
            for file in &files {
                verify_file(&pk, file)?;
                println!("verified {}", file.display());
            }

            let pqc_pk = resolve_pqc_public_key(pqc_pubkey.as_deref(), pqc_pubkey_file.as_deref())?;
            if let Some(pqc_pk) = pqc_pk {
                println!("verifying PQC signatures (ML-DSA-65)");
                for file in &files {
                    if pqc_signature_path(file).is_file() {
                        verify_file_pqc(&pqc_pk, file)?;
                        println!("verified PQC {}", file.display());
                    }
                }
            }
            Ok(())
        }
        Command::Guard { files, update_rs } => guard(&files, &update_rs),
        Command::Manifest {
            version,
            assets_json,
            notes,
            out_file,
            repo,
        } => manifest::run(&version, &assets_json, &notes, &out_file, &repo),
        Command::FetchFont {
            force,
            verify,
            from_path,
            zip_url,
        } => fetch_font::run(
            &repo_root()?,
            &fetch_font::Options {
                force,
                verify,
                from_path,
                zip_url,
            },
        ),
        Command::BumpVersion {
            level,
            no_push,
            dry_run,
        } => bump_version::run(
            &repo_root()?,
            level,
            &bump_version::Options { dry_run, no_push },
        ),
        Command::Theme(cmd) => theme::run(cmd),
        Command::GenIcons(cmd) => icons::run(cmd),
    }
}

/// The workspace root: `xtask` lives one level below it.
fn repo_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask sits one level below the repo root")
}

/// Lowercase hex SHA-256 — the form the update manifest stores and the font
/// check compares against.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Writing into a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

// ── Key handling ─────────────────────────────────────────────────────────────

/// Load the signing secret from `LIMEDL_SIGNING_KEY` (path or base64).
fn load_secret_key() -> Result<(SecretKey, String)> {
    let password = key_password();
    let Some(value) = env_var(KEY_ENV) else {
        bail!(
            "no signing key found: set {KEY_ENV} to the key file path or to base64 of the key file text"
        );
    };
    let sk = parse_secret_key(&value, password)
        .with_context(|| format!("load signing key from {KEY_ENV}"))?;
    Ok((sk, KEY_ENV.to_string()))
}

fn key_password() -> Option<String> {
    env_var(KEY_PASSWORD_ENV)
}

fn env_var(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

/// Accepts a path to a key file (the local/dev form) or base64 of the key file
/// text (the CI secret form).
fn parse_secret_key(value: &str, password: Option<String>) -> Result<SecretKey> {
    let trimmed = value.trim();
    let path = Path::new(trimmed);
    if path.is_file() {
        return SecretKey::from_file(path, password).context("read secret key file");
    }
    let text = decode_base64_text(trimmed).context("decode base64 secret key")?;
    SecretKey::from_box(SecretKeyBox::from_string(&text)?, password)
        .context("decrypt secret key (wrong password?)")
}

/// Resolve a public key from an explicit value, a file, the environment, or —
/// as a last resort — the signing secret.
fn resolve_public_key(explicit: Option<&str>, file: Option<&Path>) -> Result<PublicKey> {
    if let Some(value) = explicit {
        return parse_public_key(value).context("parse --pubkey");
    }
    if let Some(path) = file {
        let text = fs::read_to_string(path)
            .with_context(|| format!("read public key file {}", path.display()))?;
        return parse_public_key(&text)
            .with_context(|| format!("parse public key file {}", path.display()));
    }
    if let Some(value) = env_var(PUBKEY_ENV) {
        return parse_public_key(&value).context("parse {PUBKEY_ENV}");
    }
    let (sk, _) = load_secret_key()?;
    PublicKey::from_secret_key(&sk).context("derive public key from the signing secret")
}

/// Accepts every form the key appears in: base64 of the key file text (what
/// `update.rs` embeds), the key file text itself, or the bare base64 payload.
fn parse_public_key(value: &str) -> Result<PublicKey> {
    let trimmed = value.trim();
    if let Ok(text) = decode_base64_text(trimmed)
        && let Ok(pk) = PublicKey::from_box(text.into())
    {
        return Ok(pk);
    }
    if let Ok(pk) = PublicKey::from_box(trimmed.to_string().into()) {
        return Ok(pk);
    }
    PublicKey::from_base64(trimmed).context("public key is neither key file text nor base64")
}

/// base64 of the key file text — the exact string that belongs in `PUBKEY_B64`.
fn pubkey_b64(pk: &PublicKey) -> Result<String> {
    let box_text = pk.to_box().context("serialize public key")?;
    Ok(BASE64.encode(box_text.to_bytes()))
}

fn keynum_hex(pk: &PublicKey) -> String {
    let keynum = pk.keynum();
    let mut n = [0u8; 8];
    for (slot, byte) in n.iter_mut().zip(keynum) {
        *slot = *byte;
    }
    format!("{:016X}", u64::from_le_bytes(n))
}

fn decode_base64_text(value: &str) -> Result<String> {
    let bytes = BASE64.decode(value).context("base64 decode")?;
    String::from_utf8(bytes).context("decoded payload is not UTF-8")
}

// ── Signing / verification ───────────────────────────────────────────────────

fn sign_file(sk: &SecretKey, pk: &PublicKey, file: &Path) -> Result<PathBuf> {
    let data = fs::read(file).with_context(|| format!("read {}", file.display()))?;
    let file_name = file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let trusted_comment = format!(
        "timestamp:{}\tfile:{file_name}\tprehashed",
        unix_now()
    );
    let signature = minisign::sign(Some(pk), sk, Cursor::new(&data), Some(&trusted_comment), None)
        .with_context(|| format!("sign {}", file.display()))?;

    // Self-check before writing: a bad signature is cheaper to catch here than
    // in a published release.
    minisign::verify(pk, &signature, Cursor::new(&data), true, false, false)
        .with_context(|| format!("self-check failed for {}", file.display()))?;

    let sig_path = signature_path(file);
    fs::write(&sig_path, signature.to_string())
        .with_context(|| format!("write {}", sig_path.display()))?;
    Ok(sig_path)
}

fn verify_file(pk: &PublicKey, file: &Path) -> Result<()> {
    let sig_path = signature_path(file);
    let data = fs::read(file).with_context(|| format!("read {}", file.display()))?;
    let sig_text = fs::read_to_string(&sig_path)
        .with_context(|| format!("read signature {} (run `cargo xtask sign` first)", sig_path.display()))?;
    let signature = SignatureBox::from_string(&sig_text)
        .with_context(|| format!("parse signature {}", sig_path.display()))?;
    minisign::verify(pk, &signature, Cursor::new(&data), true, false, false)
        .with_context(|| format!("signature rejected for {}", file.display()))
}

fn signature_path(file: &Path) -> PathBuf {
    let mut name = file.as_os_str().to_os_string();
    name.push(".sig");
    PathBuf::from(name)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn sign_file_pqc(
    sk: &ml_dsa_65::PrivateKey,
    pk: &ml_dsa_65::PublicKey,
    file: &Path,
) -> Result<PathBuf> {
    let data = fs::read(file).with_context(|| format!("read {}", file.display()))?;
    let ctx = pqc_context_for_file(file);
    let sig = sk
        .try_sign(&data, ctx)
        .map_err(|e| anyhow!("ML-DSA-65 sign {}: {e:?}", file.display()))?;

    // Self-check before writing: a bad signature is cheaper to catch here than
    // in a published release.
    if !pk.verify(&data, &sig, ctx) {
        bail!("ML-DSA-65 self-check failed for {}", file.display());
    }

    let sig_path = pqc_signature_path(file);
    let sig_b64 = BASE64.encode(sig);
    let content = format!("untrusted comment: ml-dsa-65 signature\n{sig_b64}\n");
    fs::write(&sig_path, content)
        .with_context(|| format!("write {}", sig_path.display()))?;
    Ok(sig_path)
}

fn verify_file_pqc(pk: &ml_dsa_65::PublicKey, file: &Path) -> Result<()> {
    let sig_path = pqc_signature_path(file);
    let data = fs::read(file).with_context(|| format!("read {}", file.display()))?;
    let sig_text = fs::read_to_string(&sig_path).with_context(|| {
        format!(
            "read PQC signature {} (run `cargo xtask sign` first)",
            sig_path.display()
        )
    })?;
    let raw_b64 = sig_text
        .lines()
        .find(|l| !l.starts_with("untrusted comment:"))
        .unwrap_or(&sig_text)
        .trim();
    let sig_bytes = BASE64.decode(raw_b64).context("base64 decode PQC signature")?;
    let sig_array: [u8; ml_dsa_65::SIG_LEN] = sig_bytes
        .try_into()
        .map_err(|_| anyhow!("PQC signature in {} has invalid length", sig_path.display()))?;
    let ctx = pqc_context_for_file(file);
    if !pk.verify(&data, &sig_array, ctx) {
        bail!("ML-DSA-65 post-quantum signature rejected for {}", file.display());
    }
    Ok(())
}

fn pqc_signature_path(file: &Path) -> PathBuf {
    let mut name = file.as_os_str().to_os_string();
    name.push(".pqc.sig");
    PathBuf::from(name)
}

fn pqc_context_for_file(file: &Path) -> &'static [u8] {
    if file.file_name().and_then(std::ffi::OsStr::to_str) == Some("latest-native.json") {
        PQC_MANIFEST_CTX
    } else {
        PQC_ARTIFACT_CTX
    }
}

fn load_pqc_key() -> Result<Option<(ml_dsa_65::PrivateKey, ml_dsa_65::PublicKey, String)>> {
    let Some(value) = env_var(PQC_KEY_ENV) else {
        return Ok(None);
    };
    let (sk, pk) = parse_pqc_secret_key(&value)
        .with_context(|| format!("load PQC signing key from {PQC_KEY_ENV}"))?;
    Ok(Some((sk, pk, PQC_KEY_ENV.to_string())))
}

fn parse_pqc_secret_key(value: &str) -> Result<(ml_dsa_65::PrivateKey, ml_dsa_65::PublicKey)> {
    let trimmed = value.trim();
    let text = if Path::new(trimmed).is_file() {
        fs::read_to_string(trimmed).context("read PQC secret key file")?
    } else {
        trimmed.to_string()
    };
    let text_trimmed = text.trim();
    let raw_b64 = text_trimmed
        .lines()
        .find(|l| !l.starts_with("untrusted comment:"))
        .unwrap_or(text_trimmed)
        .trim();
    let bytes = BASE64.decode(raw_b64).context("base64 decode PQC secret key")?;
    if bytes.len() == 32 {
        let seed: [u8; 32] = bytes.try_into().unwrap();
        let (pk, sk) = ml_dsa_65::KG::keygen_from_seed(&seed);
        Ok((sk, pk))
    } else {
        bail!(
            "PQC secret key seed must be 32 bytes (got {} bytes after base64 decode)",
            bytes.len()
        );
    }
}

fn resolve_pqc_public_key(
    explicit: Option<&str>,
    file: Option<&Path>,
) -> Result<Option<ml_dsa_65::PublicKey>> {
    if let Some(value) = explicit {
        return parse_pqc_public_key(value).map(Some).context("parse --pqc-pubkey");
    }
    if let Some(path) = file {
        let text = fs::read_to_string(path)
            .with_context(|| format!("read PQC public key file {}", path.display()))?;
        return parse_pqc_public_key(&text)
            .map(Some)
            .with_context(|| format!("parse PQC public key file {}", path.display()));
    }
    if let Some(value) = env_var(PQC_PUBKEY_ENV) {
        return parse_pqc_public_key(&value).map(Some).context("parse {PQC_PUBKEY_ENV}");
    }
    if let Some((_, pk, _)) = load_pqc_key()? {
        return Ok(Some(pk));
    }
    Ok(None)
}

fn parse_pqc_public_key(value: &str) -> Result<ml_dsa_65::PublicKey> {
    let trimmed = value.trim();
    let text = if Path::new(trimmed).is_file() {
        fs::read_to_string(trimmed).context("read PQC public key file")?
    } else {
        trimmed.to_string()
    };
    let text_trimmed = text.trim();
    let raw_b64 = text_trimmed
        .lines()
        .find(|l| !l.starts_with("untrusted comment:"))
        .unwrap_or(text_trimmed)
        .trim();
    let bytes = BASE64.decode(raw_b64).context("base64 decode PQC public key")?;
    let pk_array: [u8; ml_dsa_65::PK_LEN] = bytes
        .try_into()
        .map_err(|_| anyhow!("PQC public key has invalid length (expected {} bytes)", ml_dsa_65::PK_LEN))?;
    ml_dsa_65::PublicKey::try_from_bytes(pk_array)
        .map_err(|e| anyhow!("parse ML-DSA-65 public key: {e}"))
}

// ── Guard ────────────────────────────────────────────────────────────────────

fn guard(files: &[PathBuf], update_rs: &Path) -> Result<()> {
    let (sk, source) = load_secret_key()?;
    let pk = PublicKey::from_secret_key(&sk).context("derive public key from secret")?;
    let derived = pubkey_b64(&pk)?;

    let src = fs::read_to_string(update_rs)
        .with_context(|| format!("read {}", update_rs.display()))?;
    let embedded = extract_pubkey_b64(&src)
        .with_context(|| format!("find PUBKEY_B64 in {}", update_rs.display()))?;

    if derived != embedded {
        bail!(
            "signing key does not match the client's embedded public key.\n\
             {source} derives:\n  {derived}\n\
             but {} embeds:\n  {embedded}\n\
             Update PUBKEY_B64 in {} to the derived value (that constant decides\n\
             which signatures installed clients accept).",
            update_rs.display(),
            update_rs.display()
        );
    }
    println!(
        "public key matches {} (keynum {})",
        update_rs.display(),
        keynum_hex(&pk)
    );

    let pqc_guard = if let Some((_, pqc_pk, pqc_source)) = load_pqc_key()? {
        let pqc_embedded = extract_pqc_pubkey_b64(&src)
            .with_context(|| format!("find PQC_PUBKEY_B64 in {}", update_rs.display()))?;
        let pqc_derived = BASE64.encode(pqc_pk.clone().into_bytes());
        if pqc_derived != pqc_embedded {
            bail!(
                "PQC signing key does not match the client's embedded PQC public key.\n\
                 {pqc_source} derives:\n  {pqc_derived}\n\
                 but {} embeds:\n  {pqc_embedded}\n\
                 Update PQC_PUBKEY_B64 in {} to the derived value.",
                update_rs.display(),
                update_rs.display()
            );
        }
        println!(
            "PQC public key matches {} (ML-DSA-65)",
            update_rs.display()
        );
        Some(pqc_pk)
    } else {
        println!("notice: {PQC_KEY_ENV} not set; skipping PQC key guard");
        None
    };

    for file in files {
        verify_file(&pk, file)?;
        if let Some(pqc_pk) = &pqc_guard {
            verify_file_pqc(pqc_pk, file)?;
            println!("verified (hybrid) {}", file.display());
        } else {
            println!("verified {}", file.display());
        }
    }
    Ok(())
}

/// Pull the `PUBKEY_B64` string literal out of the client source.
fn extract_pubkey_b64(source: &str) -> Result<String> {
    const MARKER: &str = "PUBKEY_B64";
    let start = source
        .find(MARKER)
        .context("PUBKEY_B64 declaration not found")?;
    let rest = &source[start + MARKER.len()..];
    let open = rest.find('"').context("PUBKEY_B64 has no string literal")?;
    let after_open = &rest[open + 1..];
    let close = after_open
        .find('"')
        .context("PUBKEY_B64 string literal is not terminated")?;
    let value = after_open[..close].trim().to_string();
    if value.is_empty() {
        bail!("PUBKEY_B64 is empty");
    }
    Ok(value)
}

/// Pull the `PQC_PUBKEY_B64` string literal out of the client source.
fn extract_pqc_pubkey_b64(source: &str) -> Result<String> {
    const MARKER: &str = "PQC_PUBKEY_B64";
    let start = source
        .find(MARKER)
        .context("PQC_PUBKEY_B64 declaration not found")?;
    let rest = &source[start + MARKER.len()..];
    let open = rest.find('"').context("PQC_PUBKEY_B64 has no string literal")?;
    let after_open = &rest[open + 1..];
    let close = after_open
        .find('"')
        .context("PQC_PUBKEY_B64 string literal is not terminated")?;
    let value = after_open[..close].trim().to_string();
    if value.is_empty() {
        bail!("PQC_PUBKEY_B64 is empty");
    }
    Ok(value)
}

// ── Key generation ───────────────────────────────────────────────────────────

fn generate_key(
    out_dir: &Path,
    name: &str,
    password: Option<String>,
    force: bool,
    pqc_only: bool,
) -> Result<()> {
    let sk_path = out_dir.join(format!("{name}.key"));
    let pk_path = out_dir.join(format!("{name}.key.pub"));
    let sk_b64_path = out_dir.join(format!("{name}.key.b64"));
    let password_path = out_dir.join(format!("{name}.password"));

    let pqc_sk_b64_path = out_dir.join(format!("{name}.pqc.key.b64"));
    let pqc_pk_path = out_dir.join(format!("{name}.pqc.pub"));

    let check_paths: &[&PathBuf] = if pqc_only {
        &[&pqc_sk_b64_path, &pqc_pk_path]
    } else {
        &[
            &sk_path,
            &pk_path,
            &sk_b64_path,
            &password_path,
            &pqc_sk_b64_path,
            &pqc_pk_path,
        ]
    };

    if !force && check_paths.iter().any(|p| p.exists()) {
        bail!(
            "Key files already exist in {} — pass --force to overwrite",
            out_dir.display(),
        );
    }
    fs::create_dir_all(out_dir)
        .with_context(|| format!("create {}", out_dir.display()))?;

    let embedded_minisign = if !pqc_only {
        let password = password.unwrap_or_else(random_password);
        let keypair = KeyPair::generate_and_write_encrypted_keypair(
            &mut fs::File::create(&pk_path).context("create public key file")?,
            &mut fs::File::create(&sk_path).context("create secret key file")?,
            None,
            Some(password.clone()),
        )
        .context("generate minisign keypair")?;

        restrict_permissions(&sk_path)?;

        let embedded_minisign = pubkey_b64(&keypair.pk)?;
        fs::write(&sk_b64_path, BASE64.encode(fs::read(&sk_path)?))
            .context("write secret key payload")?;
        restrict_permissions(&sk_b64_path)?;
        fs::write(&password_path, &password).context("write password file")?;
        restrict_permissions(&password_path)?;
        Some(embedded_minisign)
    } else {
        None
    };

    // Generate ML-DSA-65 post-quantum keypair from 32-byte secure seed
    let mut pqc_seed = [0u8; 32];
    rand::rng().fill_bytes(&mut pqc_seed);
    let (pqc_pk, _) = ml_dsa_65::KG::keygen_from_seed(&pqc_seed);
    let pqc_seed_b64 = BASE64.encode(pqc_seed);
    let embedded_pqc = BASE64.encode(pqc_pk.into_bytes());

    fs::write(&pqc_sk_b64_path, &pqc_seed_b64).context("write PQC secret key seed payload")?;
    restrict_permissions(&pqc_sk_b64_path)?;
    fs::write(&pqc_pk_path, &embedded_pqc).context("write PQC public key file")?;

    if let Some(embedded_minisign) = embedded_minisign {
        println!(
            "wrote {name}.{{key,key.pub,key.b64,password,pqc.key.b64,pqc.pub}} in {}",
            out_dir.display()
        );
        println!();
        println!("1. Put these in {DEFAULT_UPDATE_RS}:");
        println!();
        println!("const PUBKEY_B64: &str =\n    \"{embedded_minisign}\";");
        println!("const PQC_PUBKEY_B64: &str =\n    \"{embedded_pqc}\";");
        println!();
        println!("2. Store the secrets from the files (values never need to be printed):");
        println!();
        println!(
            "   Get-Content {} | gh secret set {KEY_ENV}",
            sk_b64_path.display()
        );
        println!(
            "   Get-Content {} | gh secret set {KEY_PASSWORD_ENV}",
            password_path.display()
        );
        println!(
            "   Get-Content {} | gh secret set {PQC_KEY_ENV}",
            pqc_sk_b64_path.display()
        );
    } else {
        println!(
            "wrote {name}.{{pqc.key.b64,pqc.pub}} in {}",
            out_dir.display()
        );
        println!();
        println!("1. Put this in {DEFAULT_UPDATE_RS}:");
        println!();
        println!("const PQC_PUBKEY_B64: &str =\n    \"{embedded_pqc}\";");
        println!();
        println!("2. Store the PQC secret from the file (values never need to be printed):");
        println!();
        println!(
            "   Get-Content {} | gh secret set {PQC_KEY_ENV}",
            pqc_sk_b64_path.display()
        );
    }
    println!();
    println!("3. Prove the round-trip without publishing anything:");
    println!();
    println!("   gh workflow run sign-check");
    println!();
    println!(
        "4. Delete {} once the check is green and never commit it.",
        out_dir.display()
    );
    Ok(())
}

/// 32 hex chars from a v4 UUID: 122 bits of entropy, no extra dependency.
fn random_password() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("chmod 600 {}", path.display()))
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests;
