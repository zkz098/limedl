---
title: Security & Verification
description: Minisign and ML-DSA-65 signatures on limedl releases, the update trust chain, manual verification, and security boundaries
---

# Security & Verification

Desktop limedl releases are dual-signed with **Minisign (Ed25519)** and **ML-DSA-65 (NIST FIPS 204 post-quantum cryptography)**. The in-app updater verifies the `latest-native.json` signature, parses the manifest only after that check passes, downloads the selected artifact, and then verifies the artifact's own signature.

This page covers what is signed, how to verify it manually, and what the scheme does **not** protect against.

---

## Trust chain at a glance

```text
GitHub Release
├── latest-native.json            # Update manifest
├── latest-native.json.sig        # Minisign (Ed25519), always enforced
├── latest-native.json.pqc.sig    # ML-DSA-65
└── Desktop assets + .sig / .pqc.sig  # installers, portable archives, MSIX, deb, AppImage

Public keys embedded in the client
├── PUBKEY_B64       # Minisign / Ed25519, always enforced
└── PQC_PUBKEY_B64   # ML-DSA-65, enforced once configured
```

- **Verify before parse**: a manifest with an invalid signature never influences a download URL.
- **Artifact signatures are embedded in the manifest**: `latest-native.json` stores a `signature` and optional `pqcSignature` per asset; the client re-verifies the downloaded bytes.
- **Download cap**: updates are limited to 512 MiB.
- **Domain separation**: ML-DSA-65 uses the `limedl-manifest` context for the manifest and `limedl-artifact` for binaries, preventing cross-purpose replay.

### What is signed

| Asset | Signatures | Notes |
| :--- | :--- | :--- |
| Windows Setup.exe / portable zip | `.sig` + `.pqc.sig`, embedded in the manifest | Self-update asset |
| macOS `.app` `.tar.gz` | `.sig` + `.pqc.sig`, embedded in the manifest | Self-update asset |
| Linux portable `.tar.gz` | `.sig` + `.pqc.sig`, embedded in the manifest | Self-update asset |
| Windows MSIX | `.sig` + `.pqc.sig` | Standalone asset, not in the self-update manifest |
| Linux `.deb` / `.AppImage` | `.sig` + `.pqc.sig` | Standalone asset, not in the self-update manifest |
| `latest-native.json` | `.sig` + `.pqc.sig` | The manifest itself |
| `limedl-server` binaries / Docker images | **Unsigned** | Not part of the desktop self-update chain |

:::note[Older releases]
A few assets in older releases may not have separate `.sig` / `.pqc.sig` files; their signatures are still embedded in `latest-native.json`, so client verification is unaffected. For manual checks, use the assets actually published with that release.
:::

---

## Manual verification

The commands below use `v0.6.1` file names; substitute the version you downloaded.

### 1. Check SHA-256

GitHub Releases shows a SHA-256 digest for every asset. Compute it locally and compare byte for byte:

```powershell
# Windows
Get-FileHash -Algorithm SHA256 .\limedl-native-v0.6.1-windows-x86_64-setup.exe
```

```bash
# macOS
shasum -a 256 limedl-native-v0.6.1-darwin-aarch64-portable.tar.gz

# Linux
sha256sum limedl-native-v0.6.1-linux-x86_64.AppImage
```

The project does not publish a separate `SHA256SUMS` file.

### 2. Minisign (Ed25519)

Official Minisign public key:

```text
RWSe1Ow8aYLL4Iy0eRFN+40umqxd2kxlPoyg1AqJdPBblBMSMDOhKw+7
```

Pass the `.sig` file with `-x`:

```bash
minisign -Vm limedl-native-v0.6.1-windows-x86_64-setup.exe \
  -x limedl-native-v0.6.1-windows-x86_64-setup.exe.sig \
  -P "RWSe1Ow8aYLL4Iy0eRFN+40umqxd2kxlPoyg1AqJdPBblBMSMDOhKw+7"
```

`Signature and comment signature verified` means the file came from the official pipeline and was not modified.

The update manifest works the same way:

```bash
minisign -Vm latest-native.json -x latest-native.json.sig \
  -P "RWSe1Ow8aYLL4Iy0eRFN+40umqxd2kxlPoyg1AqJdPBblBMSMDOhKw+7"
```

### 3. ML-DSA-65 (post-quantum)

The official ML-DSA-65 public key is published in standard base64 at [`/limedl-pqc-pubkey.b64`](/limedl-pqc-pubkey.b64); it decodes to a 1952-byte raw public key.

A `.pqc.sig` file contains one `untrusted comment: ml-dsa-65 signature` line followed by one base64 signature line. To verify independently:

- run a FIPS 204 ML-DSA-65 verification over the **exact artifact bytes**;
- use the `limedl-artifact` context for assets;
- use the `limedl-manifest` context for `latest-native.json`.

The client performs this check during updates; regular users do not need to do it manually.

---

## Security boundaries and known limits

- **Signing does not protect against a compromised GitHub account or signing key.** It proves that the bytes match what the official pipeline signed, not that the upstream release process is infallible.
- **There is no key-rollover chain.** Public keys are compiled into the client; rotating them requires shipping a client that trusts the new key first. Old clients will not accept updates signed by a new key.
- **How strictly ML-DSA-65 is enforced**: the client **always enforces Ed25519** on `latest-native.json`, and the ML-DSA-65 key is embedded and used when the PQC signature is present. If a manifest lacks `.pqc.sig`, the client logs a warning and continues with Ed25519 only rather than refusing the update.
- **This is not Windows Authenticode or Apple notarization.** Minisign + ML-DSA-65 is not a commercial code-signing certificate. Windows SmartScreen and macOS Gatekeeper may still warn; see the [installation guide](/en/getting-started/installation/) for the manual allow steps.
- **Server assets are unsigned.** `limedl-server` tarballs and container images are not part of the self-update trust chain; verify them against the SHA-256 digest on the GitHub Release page.
- **No telemetry.** The client contains no analytics or telemetry reporting; update checks only contact GitHub Releases for `latest-native.json` and its signatures.

---

## Reporting a vulnerability

Please use the repository's **Security → Report a vulnerability** private advisory first. Do not post exploit details, keys, or private download links in a public issue. If the private entry point is unavailable, open an issue without sensitive details and ask for a private channel.

---

## Related documentation

- [Installation](/en/getting-started/installation/): per-platform setup and manual verification commands.
- [Platform Support](/en/getting-started/platform-support/): platforms and update channels covered by signing.
- [System Architecture](/en/advanced/architecture/): implementation of the updater and signing.
- [Changelog](/en/changelog): security-related fixes.
