---
title: Security & Verification
description: sigilup (Ed25519 + ML-DSA-65) hybrid signatures on limedl releases, the update trust chain, manual verification, and security boundaries
---

# Security & Verification

limedl releases are protected with **`sigilup`** (DSSEv1 **Ed25519 + ML-DSA-65** NIST FIPS 204 post-quantum hybrid signatures). The in-app updater verifies the `latest.hsig.json` hybrid signature and anti-rollback `security_epoch`, parses the manifest only after that check passes, downloads the selected artifact, and verifies its **BLAKE3 + SHA-512** digest in a single streaming pass.

This page covers what is signed, how to verify it manually, and what the scheme does **not** protect against.

---

## Trust chain at a glance

```text
GitHub Release
├── latest.hsig.json              # DSSEv1 hybrid-signed update manifest
├── Desktop assets + .hsig        # installers, portable archives, MSIX, deb, AppImage
└── limedl-server + .hsig         # headless server tar.gz archives

Trust root embedded in the client
└── sigilup-pins.json             # Pinned Ed25519 + ML-DSA-65 hybrid key (keyId: 7F5912C5F0B5971D)
```

- **Verify before parse**: a manifest with an invalid signature or rolled-back `security_epoch` never influences a download URL.
- **Single-pass streaming digests**: `latest.hsig.json` stores **BLAKE3 + SHA-512** digests per self-update asset; the client verifies the downloaded bytes as they stream.
- **Detached `.hsig` envelopes**: every release asset ships with a `.hsig` DSSEv1 hybrid signature envelope for standalone offline verification.
- **Download cap**: updates are limited to 512 MiB.

### What is signed

| Asset | Signatures | Notes |
| :--- | :--- | :--- |
| Windows Setup.exe / portable zip | `.hsig` + BLAKE3/SHA-512 in manifest | Self-update asset |
| macOS `.app` `.tar.gz` | `.hsig` + BLAKE3/SHA-512 in manifest | Self-update asset |
| Linux portable `.tar.gz` | `.hsig` + BLAKE3/SHA-512 in manifest | Self-update asset |
| Windows MSIX | `.hsig` | Standalone asset, not in the self-update manifest |
| Linux `.deb` / `.AppImage` | `.hsig` | Standalone asset, not in the self-update manifest |
| `limedl-server` `.tar.gz` | `.hsig` | Standalone asset, not in the self-update manifest |
| `latest.hsig.json` | Embedded DSSEv1 hybrid signature | The manifest itself |

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

### 2. Verify with `sigilup` (Ed25519 + ML-DSA-65)

Install `sigilup-cli` and verify any asset against [`sigilup-pins.json`](https://github.com/zkz098/limedl/blob/main/crates/limedl-native/src/update/sigilup-pins.json) to check both Ed25519 and ML-DSA-65 signatures together:

```bash
cargo install sigilup-cli --locked
sigilup verify --pins sigilup-pins.json limedl-native-v0.6.1-windows-x86_64-setup.exe
```

The update manifest works the same way:

```bash
sigilup verify --pins sigilup-pins.json latest.hsig.json
```

The client performs this check during updates; regular users do not need to do it manually.

---

## Security boundaries and known limits

- **Signing does not protect against a compromised GitHub account or signing key.** It proves that the bytes match what the official pipeline signed, not that the upstream release process is infallible.
- **Public keys are pinned in the client.** `sigilup-pins.json` is compiled into the client; rotating keys requires shipping a client that trusts the new key first.
- **Hybrid verification is mandatory**: `sigilup` enforces both Ed25519 and ML-DSA-65 together.
- **This is not Windows Authenticode or Apple notarization.** `sigilup` (Ed25519 + ML-DSA-65) is not a commercial code-signing certificate. Windows SmartScreen and macOS Gatekeeper may still warn; see the [installation guide](/en/getting-started/installation/) for the manual allow steps.
- **No telemetry.** The client contains no analytics or telemetry reporting; update checks only contact GitHub Releases for `latest.hsig.json`.

---

## Reporting a vulnerability

Please use the repository's **Security → Report a vulnerability** private advisory first. Do not post exploit details, keys, or private download links in a public issue. If the private entry point is unavailable, open an issue without sensitive details and ask for a private channel.

---

## Related documentation

- [Installation](/en/getting-started/installation/)
- [Self-Update Channels](/en/advanced/self-update-channels/)
