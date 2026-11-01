---
title: Self-Update Channels & Rollback
description: How limedl detects install types, verifies update manifests and assets, updates per platform, and handles MSIX/AppImage limits and rollback
---

# Self-Update Channels & Rollback

The desktop client can update itself, but different installation methods take different paths. Knowing which channel you are on avoids “I clicked update and nothing changed” or “it will not start after updating”.

---

## Install kinds and detection order

At startup the client checks, in order:

1. **Store**: a Windows package identity exists (MSIX / Microsoft Store);
2. **Installer**: the HKCU `limedl-native` uninstall entry exists (written by the NSIS Setup.exe);
3. **Portable**: everything else; macOS and Linux are always treated as Portable.

| Installation | Detected as | Update path |
| :--- | :--- | :--- |
| Windows Setup.exe | Installer | Download the new Setup, run `/P /R`, then exit |
| Windows portable zip | Portable | Replace the executable in place and restart |
| Windows MSIX | Store | **In-app checks are skipped**; Microsoft Store updates it |
| macOS `.app` `.tar.gz` | Portable | Replace in place and re-apply the ad-hoc signature |
| Linux portable `.tar.gz` | Portable | Replace in place |
| Linux AppImage | Portable | **Refuses in-place replacement**; download the new AppImage |
| Linux `.deb` | Portable | No package-manager hook; upgrade with the package manager or replace manually |

---

## Manifest and verification

The client reads fixed assets from GitHub Releases:

```text
latest-native.json
latest-native.json.sig
latest-native.json.pqc.sig
```

Flow:

1. **Verify the manifest signature before parsing JSON**; a failure aborts the check.
2. Compare the manifest version with the running `CARGO_PKG_VERSION`; an equal or older version means no update.
3. Select an asset by `{os}-{arch}` (macOS uses `darwin`, and portable/Store channels append `-portable`).
4. After downloading, verify the artifact bytes against the manifest's embedded `signature` / `pqcSignature`.
5. Downloads are capped at **512 MiB**.

See [Security & Verification](/en/security/) for details and known limits. GitHub's `releases/latest` excludes pre-releases, so stable users never receive rc/alpha builds.

---

## Per-platform behavior

### Windows Installer

- Downloads the new Setup.exe, launches it with `/P /R`, and exits so the installer can replace the binary.
- If the install directory is not writable or the installer is blocked, the update fails and keeps the old version.

### Windows portable

- Extracts the new package and replaces the executable in place; a restart is requested.
- The data directory is untouched.

### MSIX

- In-app updates are skipped for Store installs; Microsoft Store owns the channel.
- The MSIX startup task cannot pass `--hidden`; the client infers a login launch by comparing process creation time with the shell's login window.

### macOS

- The portable `.app` supports in-place replacement; the updater must re-apply the ad-hoc signature or macOS refuses to launch it.
- If the app lives in `/Applications` and the user cannot write there, the update fails with an install-location error; move the app somewhere writable or replace it manually.
- The first launch may still be blocked by Gatekeeper; see [Installation](/en/getting-started/installation/).

### Linux

- The portable `.tar.gz` updates in place.
- **An AppImage cannot replace its mounted bundle**: the client refuses self-update; download the new AppImage.
- `.deb` has no package-manager hook; upgrade with `apt`/`dpkg` or replace manually.
- Requires glibc ≥ 2.17 and `x86-64-v3`; see [Platform Support](/en/getting-started/platform-support/).

---

## Rollback

- Older installers remain available on [GitHub Releases](https://github.com/zkz098/limedl/releases).
- **Back up the entire data directory before rolling back**: a newer version may have migrated `settings.json` or the `downloads.db` schema, which an older version may not read.
- Rollback does not downgrade the database automatically; restore the backed-up data directory if the old version refuses to open it.
- The updater has no key-rollover chain; after a key rotation, old clients reject updates signed by the new key and need a client that embeds the new public key first.

---

## Troubleshooting

| Symptom | Likely cause | Action |
| :--- | :--- | :--- |
| No update found | Store install, already current, or GitHub blocked | Update through Store; check proxy and logs |
| Downloaded but not replaced | Unwritable install dir, AppImage, blocked installer | Replace manually or use the matching channel |
| Will not start after update | Data/version incompatibility or bad signature | Restore the backup and reinstall the current version |
| Missing PQC signature warning | Manifest only carries Ed25519 | Continue with Ed25519; wait for a complete signature set |

---

## Related documentation

- [Security & Verification](/en/security/): trust chain, public keys, and manual verification.
- [Installation](/en/getting-started/installation/): per-platform setup and Gatekeeper.
- [Platform Support](/en/getting-started/platform-support/): update channels and OS limits.
- [Data Backup & Migration](/en/guides/data-backup-and-migration/): how to back up before updating.
