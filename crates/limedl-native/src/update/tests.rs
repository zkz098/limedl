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
                    "pqcSignature": "cHFjX3NpZw==",
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
    assert_eq!(
        m.platforms["windows-x86_64"].pqc_signature.as_deref(),
        Some("cHFjX3NpZw==")
    );
    assert!(m.platforms["windows-x86_64-portable"].pqc_signature.is_none());
    assert!(m.platforms["windows-x86_64-portable"].sha256.is_none());
}

#[test]
fn manifest_keys_follow_install_kind() {
    let base = platform_base();
    assert_eq!(manifest_key(InstallKind::Installer), base);
    assert_eq!(
        manifest_key(InstallKind::Portable),
        format!("{base}-portable")
    );
}

/// `platform_base()` derives the OS half of the key from `consts::OS`, which
/// spells macOS as `macos`, while the manifest key (and the release asset
/// name) uses `darwin`. The mapping only ever runs on the platform it names,
/// so the concordance with `cargo xtask manifest` cannot be
/// asserted by calling it here — the URL contract is covered by
/// `asset_urls_must_point_at_this_repos_release` instead.
#[test]
fn platform_base_is_os_dash_arch() {
    // Guards the mapping's shape: a non-Windows base is always `<os>-<arch>`
    // with a single dash, which is what the generator keys are built from.
    let base = platform_base();
    assert_eq!(
        base.matches('-').count(),
        1,
        "unexpected platform base {base}"
    );
    let (os, arch) = base.split_once('-').unwrap();
    assert!(
        ["windows", "darwin", "linux"].contains(&os),
        "unexpected OS half in {base}"
    );
    assert_eq!(arch, std::env::consts::ARCH);
}

/// The macOS release ships a `.app` bundle, so the binary sits at
/// `<name>.app/Contents/MacOS/limedl-native` inside the tarball. The updater
/// matches the member by file name, so the nested path must still be found —
/// a miss means `install_via_self_replace` reports "no matching executable"
/// on every macOS update.
#[cfg(not(windows))]
#[test]
fn targz_member_is_found_inside_an_app_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let archive_path = dir
        .path()
        .join("limedl-native-v9.9.9-darwin-aarch64-portable.tar.gz");
    let payload = b"fake mach-o payload";

    {
        let file = std::fs::File::create(&archive_path).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);

        let mut header = tar::Header::new_gnu();
        header.set_size(payload.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(
            &mut header,
            "limedl.app/Contents/MacOS/limedl-native",
            &payload[..],
        )
        .unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }

    let update = AvailableUpdate {
        version: "9.9.9".to_string(),
        notes: String::new(),
        asset: PlatformAsset {
            kind: "portable".to_string(),
            url: "https://example.com/x.tar.gz".to_string(),
            signature: String::new(),
            pqc_signature: None,
            sha256: None,
        },
    };

    let extracted = extract_executable(&update, &archive_path)
        .unwrap()
        .expect("the bundle member must be extracted");
    assert_eq!(std::fs::read(&extracted).unwrap(), payload);
    assert_eq!(extracted.file_name().unwrap(), "limedl-native");
}

/// A plain-binary tarball (a Linux-style archive, or a macOS build made
/// without the bundle layout) must keep working.
#[cfg(not(windows))]
#[test]
fn targz_member_is_found_at_the_archive_root() {
    let dir = tempfile::tempdir().unwrap();
    let archive_path = dir
        .path()
        .join("limedl-native-v9.9.9-linux-x86_64-portable.tar.gz");

    {
        let file = std::fs::File::create(&archive_path).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        let mut header = tar::Header::new_gnu();
        header.set_size(4);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, "limedl-native", &b"body"[..])
            .unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }

    let update = AvailableUpdate {
        version: "9.9.9".to_string(),
        notes: String::new(),
        asset: PlatformAsset {
            kind: "portable".to_string(),
            url: "https://example.com/x.tar.gz".to_string(),
            signature: String::new(),
            pqc_signature: None,
            sha256: None,
        },
    };

    let extracted = extract_executable(&update, &archive_path).unwrap().unwrap();
    assert_eq!(std::fs::read(&extracted).unwrap(), b"body");
    // The extracted copy must stay owner-only executable (0o700, see the
    // rust:S2612 hardening on the staging write) — `self_replace` keeps the
    // *old* file's mode, so a lost exec bit here would only show up as a
    // launch failure after the update.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&extracted).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o700,
            "extracted binary must stay owner-only executable"
        );
    }
}

#[test]
fn asset_urls_must_point_at_this_repos_release() {
    // Each release asset name the pipeline actually produces (see the upload
    // steps in .github/workflows/release.yml) must satisfy the validator.
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

#[test]
fn unsigned_pqc_manifest_bytes_are_rejected() {
    for bogus in ["", "not a signature", "untrusted comment: x\nAAAA\n"] {
        let err = verify_pqc_signature_text(b"{}", bogus)
            .expect_err("bogus PQC signature must be rejected")
            .to_string();
        assert!(
            err.contains("signature") || err.contains("decode") || err.contains("invalid length"),
            "expected a signature error, got: {err}"
        );
    }
}

// ── HTTP client / proxy ──────────────────────────────────────────────────────

/// The update client must be built from the app settings, so the user's proxy
/// choice reaches the update channel the same way it reaches downloads.
///
/// A manual proxy with an empty URL is rejected while configuring the builder,
/// so reaching that error proves `http_client` consulted `settings.proxy`; the
/// previous bare `reqwest::Client::builder()` built successfully and silently
/// ignored the setting.
#[test]
fn update_http_client_applies_proxy_settings() {
    let with_manual_proxy = |manual_url: &str| AppSettings {
        proxy: limedl_core::types::ProxySettings {
            mode: limedl_core::types::ProxyMode::Manual,
            manual_url: manual_url.to_string(),
        },
        ..Default::default()
    };

    assert!(http_client(&with_manual_proxy("")).is_err());
    assert!(http_client(&with_manual_proxy("http://127.0.0.1:7890")).is_ok());

    for mode in [
        limedl_core::types::ProxyMode::Disabled,
        limedl_core::types::ProxyMode::System,
    ] {
        let settings = AppSettings {
            proxy: limedl_core::types::ProxySettings {
                mode,
                manual_url: String::new(),
            },
            ..Default::default()
        };
        assert!(http_client(&settings).is_ok());
    }
}
