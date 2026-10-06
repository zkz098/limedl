# Self-update signing keys (Hybrid Ed25519 + Post-Quantum ML-DSA-65)

Runbook for the signing keys that authenticate `latest-native.json` and every
desktop artifact. Architecture of the updater itself is documented in
[`openwiki/desktop/self-update-and-distribution.md`](../openwiki/desktop/self-update-and-distribution.md).

## Trust chain in one paragraph

Everything is dual-signed with **Minisign** (Ed25519) and **ML-DSA-65** (NIST FIPS 204
Post-Quantum Cryptography) by in-repo tooling (`cargo xtask`). The manifest's
signatures (`latest-native.json.sig` and `latest-native.json.pqc.sig`) are verified
*before* parsing; each artifact's signatures are verified over the exact downloaded
bytes. ML-DSA-65 signatures enforce domain separation contexts (`limedl-manifest` for the
update manifest, `limedl-artifact` for binary assets).

The keys are stored as CI secrets:
- `LIMEDL_SIGNING_KEY` (base64 of the minisign key file text, or a path locally) plus `LIMEDL_SIGNING_KEY_PASSWORD`.
- `LIMEDL_PQC_SIGNING_KEY` (base64 of the 32-byte ML-DSA-65 private seed, or a path locally).

The client trusts the hardcoded public keys in `crates/limedl-native/src/update/mod.rs`:
- `PUBKEY_B64` (Ed25519)
- `PQC_PUBKEY_B64` (ML-DSA-65)

`cargo xtask guard` derives both public keys from the CI secrets and **fails the release**
unless they match the constants embedded in the client, then re-verifies every signature
on all release assets and manifests — that is what makes key rotation safe. Downloads are
capped (`MAX_UPDATE_BYTES`, 512 MiB) and missing or invalid signatures abort the update.

## Rotating the signing keys

```powershell
# 1. New keypairs (Minisign + ML-DSA-65). Writes <name>.{key,key.pub,key.b64,password,pqc.pub,pqc.key.b64}
#    and prints the PUBKEY_B64 & PQC_PUBKEY_B64 values + the exact gh commands (values stay
#    in files, so no secret has to be copied through the terminal).
cargo xtask generate-key --out-dir $env:TEMP\limedl-signing

# 2. Paste the printed values into PUBKEY_B64 and PQC_PUBKEY_B64 (crates/limedl-native/src/update/mod.rs)
#    and store the secrets from the generated files:
Get-Content $env:TEMP\limedl-signing\limedl-signing.key.b64 | gh secret set LIMEDL_SIGNING_KEY
Get-Content $env:TEMP\limedl-signing\limedl-signing.password | gh secret set LIMEDL_SIGNING_KEY_PASSWORD
Get-Content $env:TEMP\limedl-signing\limedl-signing.pqc.key.b64 | gh secret set LIMEDL_PQC_SIGNING_KEY

# 3. Prove the round-trip without publishing: the workflow tests both Minisign and
#    ML-DSA-65 keys, signs a throwaway file and verifies it against client constants.
gh workflow run sign-check
gh run watch

# 4. Delete the key files.
```

**Rotate before the first release that ships the new public keys.** Clients only
accept signatures from the keys compiled into them, and there is no key-rollover
chain — an installed client cannot be re-keyed by a release. A stale constant does
not ship silently: `cargo xtask guard` fails the release when it does not match
the secrets.

The `Signing check` workflow (`.github/workflows/sign-check.yml`) is the manual
counterpart: run it after a rotation to confirm the stored secrets decrypt/derive and
match the client, without cutting a release.

## Tauri edition retirement

The Tauri desktop app is no longer built or uploaded, so `latest.json` (its
updater manifest, produced by `tauri-action`) is gone:

- Existing Tauri installs keep working, but their in-app updater now gets a 404
  from `releases/latest/download/latest.json` and reports a check failure. There
  is no in-app migration path; users install the Slint build manually (or via the
  Store/MSIX channel).
- `src-tauri/` and its `tauri.conf.json` were **deleted**; nothing in the tree
  references the old update endpoint.
- The minisign keypair is still the same one the Tauri shell used; `cargo xtask
  guard` now enforces that whatever key signs a release matches `PUBKEY_B64` in
  the client, so the secret can be renamed/rotated freely.
- `update/mod.rs`'s `PUBKEY_B64` is the single copy of the update public key in
  the tree — the retired Tauri config that used to duplicate it is gone.

If a grace period is ever wanted, publish a final Tauri release that only contains
a migration notice (keeping `latest.json` alive for that one version).
