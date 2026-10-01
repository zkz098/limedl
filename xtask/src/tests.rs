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

    assert_eq!(
        &parse_public_key(&b64).unwrap(),
        pk,
        "base64 of key file text"
    );
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
fn keygen_writes_files_the_secret_flow_consumes() {
    let dir = temp_dir("keyfiles");
    generate_key(&dir, "rot", Some("pw-123".into()), false).unwrap();

    let sk_b64 = fs::read_to_string(dir.join("rot.key.b64")).unwrap();
    let password = fs::read_to_string(dir.join("rot.password")).unwrap();
    assert_eq!(password, "pw-123");
    assert!(
        !sk_b64.ends_with('\n'),
        "gh secret set must receive no trailing newline"
    );

    // The file payload is what CI stores, so it has to load back into the
    // same key pair the public key was printed for.
    let sk = parse_secret_key(&sk_b64, Some(password)).unwrap();
    let pk = PublicKey::from_secret_key(&sk).unwrap();
    let printed = pubkey_b64(&pk).unwrap();
    let pub_from_file = fs::read_to_string(dir.join("rot.key.pub")).unwrap();
    assert_eq!(parse_public_key(&printed).unwrap(), pk);
    assert_eq!(parse_public_key(&pub_from_file).unwrap(), pk);

    // Regenerating without --force must refuse to clobber a live key.
    assert!(generate_key(&dir, "rot", Some("other".into()), false).is_err());
    fs::remove_dir_all(&dir).ok();
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
    assert_eq!(
        extract_pubkey_b64(&fs::read_to_string(&update_rs).unwrap()).unwrap(),
        embedded
    );

    fs::remove_dir_all(&dir).ok();
}

/// The guard's default path is the one the release uses: `release.yml` calls
/// `cargo xtask guard <files>` with no `--update-rs`, so a module split that
/// moves the file would otherwise surface only in the `Update manifest` job —
/// after every platform artifact has been uploaded and with no manifest
/// published for installed clients. Failing here keeps it in the gate.
#[test]
fn default_update_path_declares_the_embedded_pubkey() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits one level below the repo root");
    let update_src = repo_root.join(DEFAULT_UPDATE_RS);
    let src = fs::read_to_string(&update_src)
        .unwrap_or_else(|e| panic!("read {}: {e}", update_src.display()));
    let embedded = extract_pubkey_b64(&src)
        .unwrap_or_else(|e| panic!("find PUBKEY_B64 in {}: {e}", update_src.display()));
    // A minisign public key box always starts with `untrusted comment:`; the
    // guard compares this literal against the CI key, so reading some other
    // constant must not pass silently.
    assert!(
        embedded.starts_with("dW50cnVzdGVkIGNvbW1lbnQ6"),
        "unexpected PUBKEY_B64 value in {}",
        update_src.display()
    );
}
