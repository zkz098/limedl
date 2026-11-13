use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use sigilup::core::{ArtifactHasher, SecretKeyBundle};
use sigilup::manifest::{
    FileEpochStore, KeyPinFile, MemoryEpochStore, PlatformAsset, SignedManifest, UpdateManifest,
    current_platform_base, semver,
};
use sigilup::transport::ReqwestUpdater;
use sigilup::{UrlPolicy, VerifyContext};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;

async fn spawn_http_server(routes: Vec<(&'static str, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test http listener");
    let addr = listener.local_addr().expect("local addr");
    let routes = Arc::new(routes);

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let routes = Arc::clone(&routes);
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");

                let body = routes
                    .iter()
                    .find(|(p, _)| *p == path)
                    .map(|(_, b)| b.as_slice());

                let response = match body {
                    Some(bytes) => {
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            bytes.len()
                        );
                        [header.as_bytes(), bytes].concat()
                    }
                    None => {
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_vec()
                    }
                };
                let _ = socket.write_all(&response).await;
            });
        }
    });

    format!("http://{addr}")
}

fn build_signed_test_manifest(
    sk: &SecretKeyBundle,
    version: &str,
    security_epoch: u64,
    asset_url: &str,
    artifact: &[u8],
) -> Vec<u8> {
    let key = format!("{}-portable", current_platform_base());
    let mut assets = BTreeMap::new();
    assets.insert(
        key,
        PlatformAsset {
            kind: "portable".to_string(),
            url: asset_url.to_string(),
            digest: ArtifactHasher::hash_bytes(artifact),
            metadata: None::<serde_json::Value>,
        },
    );

    let manifest = UpdateManifest {
        schema_version: 2,
        version: semver::Version::parse(version).expect("valid version"),
        security_epoch,
        min_supported_version: None,
        published_at: 1_760_000_000,
        expires_at: None,
        notes: "Release notes".to_string(),
        assets,
        key_rotations: Vec::new(),
    };
    let signed = SignedManifest::sign(&manifest, sk, 1_760_000_000).expect("sign manifest");
    serde_json::to_vec(&signed).expect("serialize signed manifest")
}

#[test]
fn embedded_pins_json_is_valid_and_loads_trust_store() {
    let pins = load_pin_file().expect("embedded sigilup-pins.json must parse");
    assert_eq!(pins.version, 2);
    assert!(
        !pins.trusted_keys.is_empty(),
        "sigilup-pins.json must pin at least one trusted key"
    );
    let _store = pins.to_trust_store();
}

#[test]
fn evaluator_channel_and_kind_match_install_kind() {
    assert_eq!(
        evaluator_channel_and_kind(InstallKind::Installer),
        (None, "installer")
    );
    assert_eq!(
        evaluator_channel_and_kind(InstallKind::Portable),
        (Some("portable"), "portable")
    );
}

#[test]
fn asset_urls_must_point_at_this_repos_release() {
    for good in [
        "https://github.com/zkz098/limedl/releases/download/v0.3.0/\
             limedl-native-v0.3.0-windows-x86_64-setup.exe",
        "https://github.com/zkz098/limedl/releases/download/v0.3.0/\
             limedl-native-v0.3.0-windows-x86_64-portable.zip",
        "https://github.com/zkz098/limedl/releases/download/v0.3.0/\
             limedl-native-v0.3.0-windows-x86_64.msix",
        "https://github.com/zkz098/limedl/releases/download/v0.3.0/\
             limedl-native-v0.3.0-darwin-aarch64-portable.tar.gz",
    ] {
        assert!(validate_asset_url(good, "0.3.0").is_ok(), "rejected {good}");
    }

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

#[tokio::test]
async fn end_to_end_check_and_download_and_verify_with_sigilup() {
    let sk = SecretKeyBundle::generate().unwrap();
    let pins = KeyPinFile::new(vec![sk.public_key().clone()]);
    let artifact = b"limedl-portable-payload-bytes".to_vec();

    let artifact_server = spawn_http_server(vec![("/limedl.tar.gz", artifact.clone())]).await;
    let manifest_bytes = build_signed_test_manifest(
        &sk,
        "9.9.9",
        1,
        &format!("{artifact_server}/limedl.tar.gz"),
        &artifact,
    );
    let manifest_server = spawn_http_server(vec![("/latest.hsig.json", manifest_bytes)]).await;

    let mut url_policy = UrlPolicy::with_prefixes([format!("{artifact_server}/")]);
    url_policy.require_https = false;

    let mut evaluator = build_evaluator(
        pins.to_trust_store(),
        semver::Version::new(0, 1, 0),
        InstallKind::Portable,
        url_policy,
    );
    let mut epoch_store = MemoryEpochStore::new(0);
    let updater = ReqwestUpdater::new(reqwest::Client::new());
    let ctx = VerifyContext::at_unix(1_760_000_000);

    let update = check_for_update_with(
        &updater,
        &format!("{manifest_server}/latest.hsig.json"),
        &mut evaluator,
        &mut epoch_store,
        ctx,
        false,
    )
    .await
    .expect("check_for_update_with")
    .expect("expected available update");

    assert_eq!(update.version, "9.9.9");
    assert_eq!(update.notes, "Release notes");
    assert_eq!(epoch_store.load_min_epoch().unwrap(), 1);

    let temp = tempfile::tempdir().unwrap();
    let verified_path = download_and_verify_with(&updater, &update, temp.path(), &|_, _| {})
        .await
        .expect("download_and_verify_with");
    assert_eq!(std::fs::read(&verified_path).unwrap(), artifact);
}

#[tokio::test]
async fn end_to_end_rejects_tampered_artifact_and_untrusted_manifest() {
    let sk = SecretKeyBundle::generate().unwrap();
    let untrusted_sk = SecretKeyBundle::generate().unwrap();
    let pins = KeyPinFile::new(vec![sk.public_key().clone()]);
    let artifact = b"authentic-artifact".to_vec();

    // 1. Untrusted signature must be rejected.
    let bad_manifest = build_signed_test_manifest(
        &untrusted_sk,
        "9.9.9",
        1,
        "http://127.0.0.1/limedl.tar.gz",
        &artifact,
    );
    let bad_server = spawn_http_server(vec![("/latest.hsig.json", bad_manifest)]).await;
    let mut url_policy = UrlPolicy::with_prefixes([format!("{bad_server}/")]);
    url_policy.require_https = false;
    let mut evaluator = build_evaluator(
        pins.to_trust_store(),
        semver::Version::new(0, 1, 0),
        InstallKind::Portable,
        url_policy,
    );
    let mut epoch_store = MemoryEpochStore::new(0);
    let updater = ReqwestUpdater::new(reqwest::Client::new());
    let ctx = VerifyContext::at_unix(1_760_000_000);
    assert!(
        check_for_update_with(
            &updater,
            &format!("{bad_server}/latest.hsig.json"),
            &mut evaluator,
            &mut epoch_store,
            ctx,
            false,
        )
        .await
        .is_err()
    );

    // 2. Valid manifest pointing at tampered artifact bytes must fail verification.
    let good_server =
        spawn_http_server(vec![("/limedl.tar.gz", b"tampered-bytes!!".to_vec())]).await;
    let good_manifest = build_signed_test_manifest(
        &sk,
        "9.9.9",
        2,
        &format!("{good_server}/limedl.tar.gz"),
        &artifact,
    );
    let manifest_server = spawn_http_server(vec![("/latest.hsig.json", good_manifest)]).await;

    let mut url_policy = UrlPolicy::with_prefixes([format!("{good_server}/")]);
    url_policy.require_https = false;
    let mut evaluator = build_evaluator(
        pins.to_trust_store(),
        semver::Version::new(0, 1, 0),
        InstallKind::Portable,
        url_policy,
    );
    let update = check_for_update_with(
        &updater,
        &format!("{manifest_server}/latest.hsig.json"),
        &mut evaluator,
        &mut epoch_store,
        ctx,
        false,
    )
    .await
    .unwrap()
    .unwrap();

    let temp = tempfile::tempdir().unwrap();
    assert!(
        download_and_verify_with(&updater, &update, temp.path(), &|_, _| {})
            .await
            .is_err()
    );
}

#[tokio::test]
async fn end_to_end_rejects_rollback_epoch() {
    let sk = SecretKeyBundle::generate().unwrap();
    let pins = KeyPinFile::new(vec![sk.public_key().clone()]);
    let artifact = b"artifact".to_vec();
    let manifest = build_signed_test_manifest(
        &sk,
        "9.9.9",
        4,
        "https://github.com/zkz098/limedl/releases/download/v9.9.9/a.tar.gz",
        &artifact,
    );
    let server = spawn_http_server(vec![("/latest.hsig.json", manifest)]).await;

    let temp = tempfile::tempdir().unwrap();
    let epoch_path = temp.path().join("sigilup-epoch");
    std::fs::write(&epoch_path, "5\n").unwrap();
    let mut epoch_store = FileEpochStore::new(epoch_path);

    let mut evaluator = build_evaluator(
        pins.to_trust_store(),
        semver::Version::new(0, 1, 0),
        InstallKind::Portable,
        UrlPolicy::with_prefixes([RELEASE_DOWNLOAD_PREFIX]),
    );
    let updater = ReqwestUpdater::new(reqwest::Client::new());
    let ctx = VerifyContext::at_unix(1_760_000_000);

    let err = check_for_update_with(
        &updater,
        &format!("{server}/latest.hsig.json"),
        &mut evaluator,
        &mut epoch_store,
        ctx,
        false,
    )
    .await
    .expect_err("rollback epoch must be rejected");
    assert!(
        format!("{err:#}").to_lowercase().contains("rollback")
            || format!("{err:#}").to_lowercase().contains("epoch"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn check_throttling_respects_min_interval() {
    let temp = tempfile::tempdir().unwrap();
    assert!(should_check_for_update_now(
        temp.path(),
        Duration::from_secs(3600)
    ));
    record_update_check(temp.path());
    assert!(!should_check_for_update_now(
        temp.path(),
        Duration::from_secs(3600)
    ));
    assert!(should_check_for_update_now(
        temp.path(),
        Duration::from_secs(0)
    ));
}
