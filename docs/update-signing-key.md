# Self-update signing keys (`sigilup`: Hybrid Ed25519 + Post-Quantum ML-DSA-65)

Runbook for the signing keys that authenticate `latest.hsig.json` and every
release artifact. Architecture of the updater itself is documented in
[`.wiki/pages/desktop/self-update-and-distribution.md`](../.wiki/pages/desktop/self-update-and-distribution.md).

## Trust chain in one paragraph

The desktop client verifies updates via **`sigilup`** (`latest.hsig.json`, DSSEv1
hybrid **Ed25519 + ML-DSA-65** envelope, single-pass **BLAKE3 + SHA-512** streaming
digest, anti-rollback `security_epoch`, and `sigilup-pins.json` trust store).

The `sigilup` keys are stored as CI secrets:
- `SIGILUP_SIGNING_KEY` (the `.sk.json` or `sigilup-sk:v2:...` key).
- `SIGILUP_SIGNING_KEY_PASSWORD` (when using an encrypted `.sk.json`).

And pinned in `crates/limedl-native/src/update/sigilup-pins.json`.

`sigilup guard` refuses to ship a release unless the CI secrets match the pinned
public keys and all signatures verify cleanly. Downloads are capped
(`MAX_UPDATE_BYTES`, 512 MiB).

## Rotating the signing keys

```powershell
# 1. Generate a new hybrid sigilup keypair (Ed25519 + ML-DSA-65).
sigilup keygen --out-dir $env:TEMP\sigilup-signing

# 2. Update `crates/limedl-native/src/update/sigilup-pins.json` with the new
#    public key pin and store the CI secrets:
Get-Content $env:TEMP\sigilup-signing\sigilup.sk.json | gh secret set SIGILUP_SIGNING_KEY
Get-Content $env:TEMP\sigilup-signing\sigilup.password | gh secret set SIGILUP_SIGNING_KEY_PASSWORD

# 3. Prove the round-trip without publishing: the workflow signs a throwaway
#    file and verifies it against `sigilup-pins.json`.
gh workflow run sign-check
gh run watch

# 4. Delete the temporary key files.
```

**Rotate before the first release that ships the new public keys.** Clients only
accept signatures from the keys pinned in `sigilup-pins.json`. A stale pin does
not ship silently: `sigilup guard` fails the release when the signing key does
not match `sigilup-pins.json`.

The `Signing check` workflow (`.github/workflows/sign-check.yml`) is the manual
counterpart: run it after a rotation to confirm the stored secrets decrypt and
match the client pin, without cutting a release.
