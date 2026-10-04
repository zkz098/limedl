# Self-update signing key

Runbook for the minisign key that authenticates `latest-native.json` and every
desktop artifact. Architecture of the updater itself is documented in
[`openwiki/desktop/self-update-and-distribution.md`](../openwiki/desktop/self-update-and-distribution.md).

## Trust chain in one paragraph

Everything is signed with **minisign** by in-repo tooling (`cargo xtask`). The
manifest's signature (`latest-native.json.sig`) is verified *before* parsing; each
artifact's signature is verified over the exact downloaded bytes. The key is the
CI secret `LIMEDL_SIGNING_KEY` (base64 of the minisign key file text, or a path
locally) plus `LIMEDL_SIGNING_KEY_PASSWORD`. The client trusts exactly one key:
`PUBKEY_B64` in `crates/limedl-native/src/update/mod.rs`. `cargo xtask guard`
derives the public key from the CI secret and **fails the release** unless it
equals `PUBKEY_B64`, then re-verifies every signature — that is what makes key
rotation safe. Downloads are capped (`MAX_UPDATE_BYTES`, 512 MiB) and a missing
`.sig` aborts the check rather than trusting the JSON.

## Rotating the signing key

```powershell
# 1. New keypair. Writes <name>.{key,key.pub,key.b64,password} and prints the
#    PUBKEY_B64 value + the exact gh commands (values stay in files, so no
#    secret has to be copied through the terminal).
cargo xtask generate-key --out-dir $env:TEMP\limedl-signing

# 2. Paste the printed value into PUBKEY_B64 (crates/limedl-native/src/update/mod.rs)
#    and store the secrets from the generated files:
Get-Content $env:TEMP\limedl-signing\limedl-signing.key.b64 | gh secret set LIMEDL_SIGNING_KEY
Get-Content $env:TEMP\limedl-signing\limedl-signing.password | gh secret set LIMEDL_SIGNING_KEY_PASSWORD

# 3. Prove the round-trip without publishing: the workflow decrypts the secret,
#    signs a throwaway file and verifies it against PUBKEY_B64.
gh workflow run sign-check
gh run watch

# 4. Delete the key files.
```

**Rotate before the first release that ships the new `PUBKEY_B64`.** Clients only
accept signatures from the key compiled into them, and there is no key-rollover
chain — an installed client cannot be re-keyed by a release. A stale constant does
not ship silently: `cargo xtask guard` fails the release when it does not match
the secret.

The `Signing check` workflow (`.github/workflows/sign-check.yml`) is the manual
counterpart: run it after a rotation to confirm the stored secret decrypts and
matches the client, without cutting a release.

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
