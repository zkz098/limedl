# Self-update signing keys (Hybrid Ed25519 + Post-Quantum ML-DSA-65)

Runbook for the signing keys that authenticate `latest-native.json` and every
desktop artifact. Architecture of the updater itself is documented in
[`.wiki/pages/desktop/self-update-and-distribution.md`](../.wiki/pages/desktop/self-update-and-distribution.md).

## Trust chain in one paragraph

The desktop client now verifies updates via **`sigilup`** (`latest.hsig.json`, DSSEv1
hybrid **Ed25519 + ML-DSA-65** envelope, single-pass **BLAKE3 + SHA-512** streaming
digest, anti-rollback `security_epoch`, and `sigilup-pins.json` trust store).

During the dual-publish transition window, `release.yml` also continues to emit the
legacy `latest-native.json` (+ `.sig` Minisign and `.pqc.sig` ML-DSA-65) via
`cargo xtask` so pre-`sigilup` installs can still auto-upgrade.

The `sigilup` keys are stored as CI secrets:
- `SIGILUP_SIGNING_KEY` (the `.sk.json` or `sigilup-sk:v2:...` key).
- `SIGILUP_SIGNING_KEY_PASSWORD` (when using an encrypted `.sk.json`).
And pinned in `crates/limedl-native/src/update/sigilup-pins.json`.

Legacy transition keys (for `latest-native.json` only):
- `LIMEDL_SIGNING_KEY` + `LIMEDL_SIGNING_KEY_PASSWORD` (`PUBKEY_B64` in `crates/limedl-native/src/update/mod.rs`).
- `LIMEDL_PQC_SIGNING_KEY` (`PQC_PUBKEY_B64` in `crates/limedl-native/src/update/mod.rs`).

`sigilup guard` (and `cargo xtask guard` for legacy assets) refuses to ship a release
unless the CI secrets match the pinned public keys and all signatures verify cleanly.
Downloads are capped (`MAX_UPDATE_BYTES`, 512 MiB).

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
