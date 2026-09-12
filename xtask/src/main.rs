//! Repository tooling for the desktop self-update channel.
//!
//! This replaces the `npx --yes @tauri-apps/cli signer sign` step the retired
//! Tauri pipeline left behind. The signature format is unchanged (minisign,
//! prehashed BLAKE2b — what `crates/limedl-native/src/update.rs` verifies with
//! `minisign-verify`), but the tool now lives next to the verifier and shares
//! its key handling instead of pulling a Node CLI that belongs to a project we
//! no longer use.
//!
//! ```text
//! cargo xtask generate-key --out-dir <dir>   # new keypair + ready-to-run gh secret commands
//! cargo xtask sign <files...>                # write <file>.sig
//! cargo xtask verify <files...>              # verify <file>.sig against a public key
//! cargo xtask guard <files...>               # release guard (see below)
//! ```
//!
//! `guard` is the release safety net: it derives the public key from the CI
//! signing secret and asserts that `PUBKEY_B64` in `update.rs` is that key, then
//! verifies every produced signature. Without it, rotating the CI secret while
//! forgetting the embedded constant would ship a client that rejects every
//! future update.
//!
//! Key sources (first match wins):
//!
//! - `LIMEDL_SIGNING_KEY` — either a path to the key file or the base64 of the
//!   key file text (what GitHub secrets hold)
//! - `TAURI_SIGNING_PRIVATE_KEY` — the retired Tauri name, accepted so a
//!   half-migrated environment can still release; drop once secrets are renamed
//!
//! The matching password comes from `LIMEDL_SIGNING_KEY_PASSWORD`
//! (`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as the fallback).

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use base64::Engine as _;
use clap::{Parser, Subcommand};
use minisign::{KeyPair, PublicKey, SecretKey, SecretKeyBox, SignatureBox};

/// base64 of the key file text — the form stored in CI secrets and embedded in
/// `update.rs`.
const BASE64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::STANDARD;

const KEY_ENV: &str = "LIMEDL_SIGNING_KEY";
const KEY_PASSWORD_ENV: &str = "LIMEDL_SIGNING_KEY_PASSWORD";
const PUBKEY_ENV: &str = "LIMEDL_SIGNING_PUBKEY";

/// Names from the retired Tauri pipeline. Accepted only so a partially migrated
/// CI environment keeps releasing; they are removed in a follow-up commit.
const LEGACY_KEY_ENV: &str = "TAURI_SIGNING_PRIVATE_KEY";
const LEGACY_KEY_PASSWORD_ENV: &str = "TAURI_SIGNING_PRIVATE_KEY_PASSWORD";

/// Where the client's embedded public key lives.
const DEFAULT_UPDATE_RS: &str = "crates/limedl-native/src/update.rs";

#[derive(Parser)]
#[command(
    name = "xtask",
    about = "limedl repository tooling (update-channel signing)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a minisign keypair for the self-update channel.
    GenerateKey {
        /// Directory to write `<name>.key` and `<name>.key.pub` into.
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
    },
    /// Sign files, writing minisign `<file>.sig` next to each one.
    Sign {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Verify `<file>.sig` for each file.
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
    },
    /// Release guard: the signing key must match the key embedded in the client
    /// and every signature must verify.
    Guard {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Path to the client's `update.rs` holding `PUBKEY_B64`.
        #[arg(long, default_value = DEFAULT_UPDATE_RS)]
        update_rs: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::GenerateKey {
            out_dir,
            name,
            password,
            force,
        } => generate_key(&out_dir, &name, password, force),
        Command::Sign { files } => {
            let (sk, source) = load_secret_key()?;
            let pk = PublicKey::from_secret_key(&sk).context("derive public key from secret")?;
            println!("signing with {source} (keynum {})", keynum_hex(&pk));
            for file in &files {
                let sig_path = sign_file(&sk, &pk, file)?;
                println!("signed {} -> {}", file.display(), sig_path.display());
            }
            Ok(())
        }
        Command::Verify {
            files,
            pubkey,
            pubkey_file,
        } => {
            let pk = resolve_public_key(pubkey.as_deref(), pubkey_file.as_deref())?;
            println!("verifying with keynum {}", keynum_hex(&pk));
            for file in &files {
                verify_file(&pk, file)?;
                println!("verified {}", file.display());
            }
            Ok(())
        }
        Command::Guard { files, update_rs } => guard(&files, &update_rs),
    }
}

// ── Key handling ─────────────────────────────────────────────────────────────

/// Load the signing secret from the environment: `LIMEDL_SIGNING_KEY` (path or
/// base64) with the legacy Tauri name as a fallback.
fn load_secret_key() -> Result<(SecretKey, String)> {
    let password = key_password();
    for (name, kind) in [
        (KEY_ENV, "current"),
        (LEGACY_KEY_ENV, "legacy (rename the CI secret to LIMEDL_SIGNING_KEY)"),
    ] {
        let Some(value) = env_var(name) else { continue };
        let sk = parse_secret_key(&value, password.clone())
            .with_context(|| format!("load signing key from {name} — {kind}"))?;
        return Ok((sk, format!("{name} [{kind}]")));
    }
    bail!("no signing key found: set {KEY_ENV} to the key file path or to base64 of the key file text")
}

fn key_password() -> Option<String> {
    env_var(KEY_PASSWORD_ENV).or_else(|| env_var(LEGACY_KEY_PASSWORD_ENV))
}

fn env_var(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

/// Accepts a path to a key file (the local/dev form) or base64 of the key file
/// text (the CI secret form), mirroring what the previous tooling accepted.
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

    for file in files {
        verify_file(&pk, file)?;
        println!("verified {}", file.display());
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

// ── Key generation ───────────────────────────────────────────────────────────

fn generate_key(out_dir: &Path, name: &str, password: Option<String>, force: bool) -> Result<()> {
    let sk_path = out_dir.join(format!("{name}.key"));
    let pk_path = out_dir.join(format!("{name}.key.pub"));
    if !force && (sk_path.exists() || pk_path.exists()) {
        bail!(
            "{} already exists — pass --force to overwrite (rotating the key invalidates\n\
             the PUBKEY_B64 currently embedded in {}",
            sk_path.display(),
            DEFAULT_UPDATE_RS
        );
    }
    fs::create_dir_all(out_dir)
        .with_context(|| format!("create {}", out_dir.display()))?;

    let password = password.unwrap_or_else(random_password);
    let keypair = KeyPair::generate_and_write_encrypted_keypair(
        &mut fs::File::create(&pk_path).context("create public key file")?,
        &mut fs::File::create(&sk_path).context("create secret key file")?,
        None,
        Some(password.clone()),
    )
    .context("generate keypair")?;

    restrict_permissions(&sk_path)?;

    let embedded = pubkey_b64(&keypair.pk)?;
    let key_b64 = BASE64.encode(fs::read(&sk_path)?);

    println!("wrote {} and {}", sk_path.display(), pk_path.display());
    println!();
    println!("1. Put this in PUBKEY_B64 in {DEFAULT_UPDATE_RS}:");
    println!();
    println!("const PUBKEY_B64: &str =");
    println!("    \"{embedded}\";");
    println!();
    println!("2. Store the key in CI (the private key never belongs in git):");
    println!();
    println!("   gh secret set {KEY_ENV} --body \"{key_b64}\"");
    println!("   gh secret set {KEY_PASSWORD_ENV} --body \"{password}\"");
    println!();
    println!("3. Keep the generated files out of the repository and delete them once stored.");
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
mod tests {
    use super::*;
    use minisign_verify::{PublicKey as VerifyKey, Signature as VerifySignature};

    /// The client side of the contract: `update.rs::verify_signature` decodes
    /// base64 → `minisign_verify::PublicKey::decode` → `Signature::decode` →
    /// `verify(data, sig, false)`. If this test passes, artifacts signed by this
    /// tool are accepted by installed clients.
    fn client_verifies(pk: &PublicKey, data: &[u8], sig_text: &str) -> Result<()> {
        let pk_b64 = pubkey_b64(pk)?;
        let pk_text = decode_base64_text(&pk_b64)?;
        let key = VerifyKey::decode(&pk_text).context("minisign-verify: decode public key")?;
        let signature =
            VerifySignature::decode(sig_text).context("minisign-verify: decode signature")?;
        key.verify(data, &signature, false)
            .context("minisign-verify: verify")
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "limedl-xtask-test-{tag}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn signatures_are_accepted_by_the_client_verifier() {
        let dir = temp_dir("roundtrip");
        let data_path = dir.join("artifact.bin");
        let payload = b"pretend installer bytes".repeat(1024);
        fs::write(&data_path, &payload).unwrap();

        let keypair = KeyPair::generate_unencrypted_keypair().unwrap();
        sign_file(&keypair.sk, &keypair.pk, &data_path).unwrap();

        let sig_text = fs::read_to_string(signature_path(&data_path)).unwrap();
        client_verifies(&keypair.pk, &payload, &sig_text).unwrap();

        // A single flipped byte must be rejected.
        let mut tampered = payload.clone();
        tampered[0] ^= 0x01;
        assert!(client_verifies(&keypair.pk, &tampered, &sig_text).is_err());
    }

    #[test]
    fn encrypted_key_round_trips_through_the_ci_secret_shape() {
        let dir = temp_dir("secret");
        let password = "test-password".to_string();
        let keypair = KeyPair::generate_and_write_encrypted_keypair(
            &mut fs::File::create(dir.join("k.pub")).unwrap(),
            &mut fs::File::create(dir.join("k.key")).unwrap(),
            None,
            Some(password.clone()),
        )
        .unwrap();

        // CI stores base64 of the key file text, not a path.
        let key_b64 = BASE64.encode(fs::read(dir.join("k.key")).unwrap());
        let loaded = parse_secret_key(&key_b64, Some(password.clone())).unwrap();
        assert_eq!(
            PublicKey::from_secret_key(&loaded).unwrap(),
            keypair.pk,
            "a key loaded from the CI secret must derive the same public key"
        );

        let data_path = dir.join("artifact.bin");
        fs::write(&data_path, b"payload").unwrap();
        sign_file(&loaded, &keypair.pk, &data_path).unwrap();
        verify_file(&keypair.pk, &data_path).unwrap();

        // Wrong password must fail loudly instead of producing a bad signature.
        assert!(parse_secret_key(&key_b64, Some("wrong".into())).is_err());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pubkey_parses_from_every_stored_form() {
        let keypair = KeyPair::generate_unencrypted_keypair().unwrap();
        let pk = &keypair.pk;
        let b64 = pubkey_b64(pk).unwrap();
        let text = decode_base64_text(&b64).unwrap();

        assert_eq!(&parse_public_key(&b64).unwrap(), pk, "base64 of key file text");
        assert_eq!(&parse_public_key(&text).unwrap(), pk, "key file text");
        assert_eq!(
            &parse_public_key(&pk.to_base64()).unwrap(),
            pk,
            "bare base64 payload"
        );
        assert!(parse_public_key("not a key").is_err());
    }

    #[test]
    fn guard_reads_the_embedded_pubkey() {
        let src = r#"
            /// docs
            const PUBKEY_B64: &str =
                "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc2RTVFQzcwMjEyMDYyQTYK";
        "#;
        assert_eq!(
            extract_pubkey_b64(src).unwrap(),
            "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc2RTVFQzcwMjEyMDYyQTYK"
        );
        assert!(extract_pubkey_b64("fn main() {}").is_err());
        assert!(extract_pubkey_b64("const PUBKEY_B64: &str = \"\";").is_err());
    }

    #[test]
    fn keygen_and_guard_agree_on_the_embedded_value() {
        let dir = temp_dir("guard");
        let password = "pw".to_string();
        let keypair = KeyPair::generate_and_write_encrypted_keypair(
            &mut fs::File::create(dir.join("k.pub")).unwrap(),
            &mut fs::File::create(dir.join("k.key")).unwrap(),
            None,
            Some(password),
        )
        .unwrap();
        let embedded = pubkey_b64(&keypair.pk).unwrap();

        // What generate_key prints is exactly what the guard compares against.
        let update_rs = dir.join("update.rs");
        fs::write(
            &update_rs,
            format!("const PUBKEY_B64: &str =\n    \"{embedded}\";\n"),
        )
        .unwrap();
        assert_eq!(extract_pubkey_b64(&fs::read_to_string(&update_rs).unwrap()).unwrap(), embedded);

        fs::remove_dir_all(&dir).ok();
    }
}
