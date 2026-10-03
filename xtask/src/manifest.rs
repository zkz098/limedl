//! Generates `latest-native.json` — the self-update manifest for limedl-native.
//!
//! Migrated from `scripts/gen-native-manifest.ps1`. Keeping the generator in
//! `xtask` puts it next to the verifier (`crates/limedl-native/src/update/mod.rs`)
//! and inside the test gate: the manifest is what installed clients parse after
//! verifying its signature, so a shape drift here silently turns into "every
//! installed client stops updating" — the exact failure `cargo xtask guard`
//! exists to prevent on the signing side.
//!
//! Input is the set of artifacts to advertise, described as JSON so one tool
//! serves every platform: the per-platform release jobs produce their own
//! entries and the `native-manifest` job merges them (see release.yml). Keeping
//! the generator platform-agnostic matters because the client derives its
//! lookup key from the running OS/arch (`update::manifest_key`): a key nobody
//! generates is an update that silently never appears.
//!
//! ```text
//! cargo xtask manifest --version 0.3.6 --assets-json dist/native-assets.json \
//!     --notes "<changelog>" --out-file dist/latest-native.json
//! ```
//!
//! `assets` shape — a map of manifest key -> artifact path. Order is preserved:
//!
//! ```json
//! {
//!   "windows-x86_64":          { "kind": "installer", "path": "dist/setup.exe" },
//!   "windows-x86_64-portable": { "kind": "portable",  "path": "dist/app.zip" },
//!   "darwin-aarch64-portable": { "kind": "portable",  "path": "dist/app.tar.gz" }
//! }
//! ```
//!
//! Valid `kind` values are exactly the two the client validates against
//! (`update::check_for_update`): "installer" (Windows NSIS) and "portable"
//! (zip/tar.gz replaced in place).
//!
//! Reads the minisign `.sig` files written by `cargo xtask sign` next to each
//! artifact, base64-encodes them into the manifest, and computes sha256 digests.
//! The manifest itself is signed afterwards (`cargo xtask sign
//! dist/latest-native.json`), because the client verifies
//! `latest-native.json.sig` before it parses anything.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use base64::Engine as _;
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{BASE64, sha256_hex};

const VALID_KINDS: [&str; 2] = ["installer", "portable"];

/// One `assets` entry from the release job.
#[derive(Debug, Deserialize)]
struct AssetEntry {
    kind: String,
    path: PathBuf,
}

/// The asset map, with insertion order preserved. `serde_json`'s default `Map`
/// is a `BTreeMap` (alphabetical); the release job builds the map in a
/// deliberate Windows → macOS → Linux order and the generator used to keep it,
/// so parse into a `Vec` instead of sorting it away.
#[derive(Debug)]
struct Assets(Vec<(String, AssetEntry)>);

impl<'de> Deserialize<'de> for Assets {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct AssetsVisitor;

        impl<'de> Visitor<'de> for AssetsVisitor {
            type Value = Assets;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an object mapping manifest keys to { kind, path }")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut assets = Vec::new();
                while let Some((key, entry)) = map.next_entry::<String, AssetEntry>()? {
                    assets.push((key, entry));
                }
                Ok(Assets(assets))
            }
        }

        deserializer.deserialize_map(AssetsVisitor)
    }
}

/// One platform entry; field order is the serialization order.
#[derive(Debug, Serialize)]
struct PlatformEntry {
    kind: String,
    url: String,
    signature: String,
    sha256: String,
}

/// `platforms` as an ordered map. `serde_json` writes object keys in the order
/// `serialize_map` yields them, which keeps the Windows/macOS grouping.
struct Platforms<'a>(&'a [(String, PlatformEntry)]);

impl Serialize for Platforms<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, entry) in self.0 {
            map.serialize_entry(key, entry)?;
        }
        map.end()
    }
}

#[derive(Serialize)]
struct Manifest<'a> {
    version: &'a str,
    notes: &'a str,
    platforms: Platforms<'a>,
}

pub fn run(
    version: &str,
    assets_json: &Path,
    notes: &str,
    out_file: &Path,
    repo: &str,
) -> Result<()> {
    if !assets_json.is_file() {
        bail!("asset map not found: {}", assets_json.display());
    }
    let raw = fs::read_to_string(assets_json)
        .with_context(|| format!("read {}", assets_json.display()))?;
    let assets: Assets = serde_json::from_str(&raw)
        .with_context(|| format!("parse {} as an asset map", assets_json.display()))?;
    if assets.0.is_empty() {
        bail!("asset map is empty: {}", assets_json.display());
    }

    let mut platforms = Vec::with_capacity(assets.0.len());
    for (key, entry) in &assets.0 {
        platforms.push((key.clone(), platform_entry(key, entry, version, repo)?));
    }

    let manifest = Manifest {
        version,
        notes,
        platforms: Platforms(&platforms),
    };
    // `version`, `notes` and `platforms` are the only fields the client parses
    // (`update::UpdateManifest`); anything else is ignored, so none is emitted.
    let mut json = serde_json::to_string_pretty(&manifest).context("serialize manifest")?;
    json.push('\n');

    if let Some(parent) = out_file.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(out_file, &json).with_context(|| format!("write {}", out_file.display()))?;

    let keys: Vec<&str> = platforms.iter().map(|(key, _)| key.as_str()).collect();
    println!("wrote {} ({})", out_file.display(), keys.join(", "));
    println!("{json}");
    Ok(())
}

fn platform_entry(key: &str, entry: &AssetEntry, version: &str, repo: &str) -> Result<PlatformEntry> {
    if !VALID_KINDS.contains(&entry.kind.as_str()) {
        bail!(
            "asset '{key}' has kind '{}' (expected one of: {})",
            entry.kind,
            VALID_KINDS.join(", ")
        );
    }
    if !entry.path.is_file() {
        bail!("artifact for '{key}' not found: {}", entry.path.display());
    }

    let mut sig_path = entry.path.clone().into_os_string();
    sig_path.push(".sig");
    let sig_path = PathBuf::from(sig_path);
    if !sig_path.is_file() {
        bail!(
            "signature file missing for {} — run 'cargo xtask sign' first",
            entry.path.display()
        );
    }
    // base64(minisign signature file text) — the form `minisign_verify`'s
    // Signature::decode expects after the client base64-decodes it.
    let sig_text = fs::read_to_string(&sig_path)
        .with_context(|| format!("read {}", sig_path.display()))?;
    let signature = BASE64.encode(sig_text.trim().as_bytes());

    let bytes = fs::read(&entry.path)
        .with_context(|| format!("read {}", entry.path.display()))?;
    let sha256 = sha256_hex(&bytes);

    let file_name = entry
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("artifact path has no file name: {}", entry.path.display()))?;

    Ok(PlatformEntry {
        kind: entry.kind.clone(),
        url: format!("https://github.com/{repo}/releases/download/v{version}/{file_name}"),
        signature,
        sha256,
    })
}

/// Lowercase hex SHA-256, the form the client compares against.
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "limedl-xtask-manifest-{tag}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    struct Fixture {
        dir: PathBuf,
        assets: PathBuf,
    }

    /// Writes two artifacts with signatures and the matching asset map.
    fn fixture() -> Fixture {
        let dir = temp_dir("fixture");
        let app = dir.join("app.zip");
        fs::write(&app, b"portable bytes").unwrap();
        fs::write(dir.join("app.zip.sig"), "untrusted comment: sig\nRWSIG\n").unwrap();

        let setup = dir.join("setup.exe");
        fs::write(&setup, b"installer bytes").unwrap();
        fs::write(dir.join("setup.exe.sig"), "untrusted comment: setup\nRWSIG2").unwrap();

        let assets = dir.join("assets.json");
        fs::write(
            &assets,
            format!(
                r#"{{
                  "windows-x86_64": {{ "kind": "installer", "path": "{}" }},
                  "darwin-aarch64-portable": {{ "kind": "portable", "path": "{}" }}
                }}"#,
                setup.display(),
                app.display()
            ),
        )
        .unwrap();
        Fixture { dir, assets }
    }

    #[test]
    fn writes_manifest_in_asset_map_order() {
        let fixture = fixture();
        let out = fixture.dir.join("latest-native.json");
        run("0.4.1", &fixture.assets, "notes", &out, "zkz098/limedl").unwrap();

        let text = fs::read_to_string(&out).unwrap();
        let version = text.find("\"version\": \"0.4.1\"").unwrap();
        let windows = text.find("\"windows-x86_64\"").unwrap();
        let darwin = text.find("\"darwin-aarch64-portable\"").unwrap();
        assert!(version < windows && windows < darwin, "{text}");

        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let entry = &value["platforms"]["windows-x86_64"];
        assert_eq!(entry["kind"], "installer");
        assert_eq!(
            entry["url"],
            "https://github.com/zkz098/limedl/releases/download/v0.4.1/setup.exe"
        );
        assert_eq!(entry["sha256"], sha256_hex(b"installer bytes"));

        // The signature is base64 of the trimmed signature file text.
        let expected = BASE64.encode(b"untrusted comment: setup\nRWSIG2");
        assert_eq!(entry["signature"], expected);
    }

    #[test]
    fn signature_keeps_the_file_text_verbatim_after_trim() {
        let fixture = fixture();
        let out = fixture.dir.join("latest-native.json");
        run("0.4.1", &fixture.assets, "notes", &out, "zkz098/limedl").unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
        let decoded = BASE64
            .decode(value["platforms"]["darwin-aarch64-portable"]["signature"].as_str().unwrap())
            .unwrap();
        assert_eq!(decoded, b"untrusted comment: sig\nRWSIG");
    }

    #[test]
    fn rejects_unknown_kind() {
        let dir = temp_dir("kind");
        let artifact = dir.join("app.zip");
        fs::write(&artifact, b"x").unwrap();
        fs::write(dir.join("app.zip.sig"), "sig").unwrap();
        let assets = dir.join("assets.json");
        fs::write(
            &assets,
            format!(r#"{{"k": {{"kind": "squirrel", "path": "{}"}}}}"#, artifact.display()),
        )
        .unwrap();
        let err = run("0.4.1", &assets, "", &dir.join("out.json"), "r").unwrap_err();
        assert!(format!("{err:#}").contains("kind 'squirrel'"), "{err:#}");
    }

    #[test]
    fn rejects_missing_signature_and_artifact() {
        let dir = temp_dir("missing");
        let artifact = dir.join("app.zip");
        fs::write(&artifact, b"x").unwrap();
        let assets = dir.join("assets.json");
        fs::write(
            &assets,
            format!(r#"{{"k": {{"kind": "portable", "path": "{}"}}}}"#, artifact.display()),
        )
        .unwrap();
        let err = run("0.4.1", &assets, "", &dir.join("out.json"), "r").unwrap_err();
        assert!(format!("{err:#}").contains("signature file missing"), "{err:#}");

        let absent = dir.join("absent.zip");
        fs::write(
            &assets,
            format!(r#"{{"k": {{"kind": "portable", "path": "{}"}}}}"#, absent.display()),
        )
        .unwrap();
        let err = run("0.4.1", &assets, "", &dir.join("out.json"), "r").unwrap_err();
        assert!(format!("{err:#}").contains("not found"), "{err:#}");
    }

    #[test]
    fn rejects_empty_asset_map() {
        let dir = temp_dir("empty");
        let assets = dir.join("assets.json");
        fs::write(&assets, "{}").unwrap();
        let err = run("0.4.1", &assets, "", &dir.join("out.json"), "r").unwrap_err();
        assert!(format!("{err:#}").contains("empty"), "{err:#}");
    }
}
