//! Minisign + ML-DSA-65 keypair generation for the desktop self-update channel.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use minisign::KeyPair;
use ml_dsa::{KeyExport as _, Keypair as _, MlDsa65, Seed, SigningKey};
use rand::Rng as _;

use crate::{BASE64, DEFAULT_UPDATE_RS, KEY_ENV, KEY_PASSWORD_ENV, PQC_KEY_ENV, pubkey_b64};

pub(crate) fn generate_key(
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
    fs::create_dir_all(out_dir).with_context(|| format!("create {}", out_dir.display()))?;

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
    let pqc_pk = SigningKey::<MlDsa65>::from_seed(&Seed::from(pqc_seed)).verifying_key();
    let pqc_seed_b64 = BASE64.encode(pqc_seed);
    let embedded_pqc = BASE64.encode(pqc_pk.to_bytes());

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
