---
type: desktop
title: Self-Update and Distribution Channels
description: How the limedl desktop client detects its install channel, verifies and installs signed updates across portable/NSIS/MSIX/macOS/Linux channels, and how the release pipeline signs artifacts and builds the single latest-native.json manifest.
tags: [self-update, release, minisign, distribution, packaging]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T12:58:25.182Z
sources:
  - id: openwiki-source-4d1d392666be6dfdd7a91a2e
    resource: repo://.github/workflows/release.yml
  - id: openwiki-source-e5a81e0b5d28c08eec080c86
    resource: repo://crates/limedl-native/src/autostart.rs
  - id: openwiki-source-9ee0f9bd931517c4214e0b30
    resource: repo://crates/limedl-native/src/handlers/updater.rs
  - id: openwiki-source-531d47474acaff10ab445eeb
    resource: repo://crates/limedl-native/src/update/mod.rs
  - id: openwiki-source-f3f2dba31f6d00a54b1bb695
    resource: repo://packaging/msix/AppxManifest.xml
  - id: openwiki-source-3e467e67d349677035f0363f
    resource: repo://xtask/src/main.rs
  - id: openwiki-source-c74f60d1c3f2961e83a2a521
    resource: repo://xtask/src/manifest.rs
generated: { by: "pi", at: "2026-10-04T12:58:25.182Z" }
---

# Self-Update and Distribution Channels

The desktop client updates itself in-app through a minisign-signed manifest. The
implementation lives in `crates/limedl-native/src/update/mod.rs`, the release
tooling in `xtask/`, and the release workflow in
`.github/workflows/release.yml`.

## Install-channel detection

`detect_install_kind()` runs once at startup and checks, in order:

1. Windows package identity (`Package::Current()`) → `Store` (MSIX / Microsoft
   Store). Registry `Run` and other per-machine state are virtualized in MSIX.
2. The HKCU `...\Uninstall\limedl-native` key → `Installer` (written by NSIS).
3. Otherwise → `Portable`.

On non-Windows platforms it always reports `Portable`. The detected kind drives
both the update path and the UI copy.

Evidence: `repo://crates/limedl-native/src/update/mod.rs#L171-L209`.

## Manifest identity and channel keys

The release repo is `zkz098/limedl` and the manifest is fetched from the
permanently named URL `.../releases/latest/download/latest-native.json` with its
sidecar `.sig`. GitHub excludes drafts and prereleases from `latest`, so stable
users never see rc/alpha builds and `api.github.com` quota is not consumed.

The client trusts exactly one key: the `PUBKEY_B64` constant in `update/mod.rs`
(a minisign public key). Downloads are capped at `MAX_UPDATE_BYTES` (512 MiB) and
the manifest signature is mandatory — a missing `.sig` aborts the check rather
than trusting the JSON.

Evidence: `repo://crates/limedl-native/src/update/mod.rs#L30-L50`.

Keys are `{os}-{arch}` with macOS spelled `darwin`, and `-portable` is appended
for the portable/Store channel:

| Kind | Key |
| --- | --- |
| Windows installer | `windows-x86_64` |
| Windows portable | `windows-x86_64-portable` |
| macOS portable | `darwin-aarch64-portable` |
| Linux portable | `linux-x86_64-portable` |

Evidence: `repo://crates/limedl-native/src/update/mod.rs#L212-L232`.

## Check flow

`check_for_update(settings)` refuses to run for Store installs (the OS owns that
channel), fetches and **verifies the manifest signature before parsing it**, then:

- compares `manifest.version` against the compiled `CARGO_PKG_VERSION` and
  returns `None` when not newer;
- looks up the key for the detected kind; if the platform has no assets at all it
  returns `None` ("no update") rather than an error, but if the platform exists
  and only this channel is missing it errors with the available keys;
- validates the asset `kind` matches the channel and that the URL is a GitHub
  release download of this repo for exactly that version.

`validate_asset_url` is defence in depth on top of the signature: it requires the
`https://github.com/zkz098/limedl/releases/download/v{version}/` prefix and a
short, plain file name, so a manifest bug or fork cannot point the updater at
unrelated bytes.

The update HTTP client goes through limedl-core's `configure_client_builder`, so
`settings.proxy` applies to update checks and downloads exactly as it does to
downloads; only the User-Agent is overridden to `limedl-native/<version>`.

Evidence: `repo://crates/limedl-native/src/update/mod.rs#L120-L147`,
`repo://crates/limedl-native/src/update/mod.rs#L237-L360`.

## Installing a verified update

`install_verified(update, verified_file)` routes by the detected kind:

- **Store** → error; the Microsoft Store channel is handled by `update::store`,
  which uses `StoreContext::GetDefault` and must run on the UI thread (it
  associates with the window and fails off-thread).
- **Installer** (Windows NSIS) → spawn `setup.exe /P /R` (passive, restart) and
  return `InstallerLaunched`; the caller exits so NSIS can replace the binary.
- **Portable** → `extract_executable` then `self_replace::self_replace` in place,
  returning `ReplacedRestartPending`.

Portable specifics:

- Linux refuses when running inside an AppImage (`APPIMAGE` env var set), because
  the mounted bundle cannot be replaced in place; the user downloads the new
  `.AppImage` directly.
- macOS re-signs the containing `.app` ad-hoc after replacement
  (`codesign --force --sign -`), because replacing the executable invalidates the
  bundle signature and macOS would refuse to launch it. A plain tarball binary has
  no bundle and skips this.
- Replacement failure is wrapped with an explanation that the install location
  may not be user-writable (`/Applications`, `/usr/local/bin`, `/opt`).
- `extract_executable` matches by file name, not path, so the nested macOS
  `limedl.app/Contents/MacOS/limedl-native` member and the Linux
  `limedl-native/limedl-native` nesting both work. On Unix the extracted binary
  gets mode `0o700` (owner-only), since it lives in the per-user update work dir
  and replaces the running process.

Evidence: `repo://crates/limedl-native/src/update/mod.rs#L447-L610`.

## Background checks

The About tab's update card is driven by the `UpdateState` phase machine
(`idle → checking → up-to-date | available → downloading → ready | error`;
installer ends at `InstallerLaunched` and exits). It is scheduled from
`handlers/updater.rs`:

- a silent background check runs **45 s after startup**, so a proxy change made
  at startup is already in effect, and
- it is throttled to **once per 24 h** by the `update-check.stamp` file in the
  base directory. A stamp from the future (clock rolled back) is treated as
  fresh.

Store installs skip the silent check entirely. On finding an update the handler
pushes an `available` state and, when notifications are enabled, an OS
notification.

Evidence: `repo://crates/limedl-native/src/handlers/updater.rs#L296-L344`,
`repo://crates/limedl-native/src/update/mod.rs#L643-L663`.

## The signing chain

Everything is minisign-signed with in-repo tooling:

| Layer | Signed by | Verified by |
| --- | --- | --- |
| `latest-native.json` | `cargo xtask sign` → `latest-native.json.sig` | `verify_signature_text` before parsing |
| each artifact | `cargo xtask sign` → `<file>.sig`, also inlined as `signature` | `verify_signature` over the exact downloaded bytes |

`cargo xtask guard <files>` is the release safety net: it derives the public key
from the `LIMEDL_SIGNING_KEY` secret, asserts it equals the `PUBKEY_B64` constant
extracted from `update/mod.rs`, and then re-verifies every produced signature.
Without it, rotating the CI secret while leaving the constant stale would ship a
client that rejects every future update. There is **no key-rollover chain**, so a
new key must be rotated into `PUBKEY_B64` *before* the first release that ships
it.

Evidence: `repo://xtask/src/main.rs#L1-L22`,
`repo://xtask/src/main.rs#L401-L441`.

## The release pipeline

`.github/workflows/release.yml` is tag-triggered (`v*`) and has these jobs:

- `font` fetches the MiSans VF font once and hands it to every desktop build leg
  as the `misans-vf` artifact.
- `changelog` generates the release body with git-cliff from Conventional
  Commits between the previous tag and the released tag.
- `build-native` (Windows) produces the portable zip, NSIS setup exe and MSIX.
- `build-native-macos` produces the ad-hoc-signed `.app` tar.gz.
- `build-native-linux` produces the portable tar.gz, `.deb` and AppImage.
- `build-server` cross-compiles the headless `limedl-server` for
  `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` with
  `cargo zigbuild` (zig supplies the musl C toolchain `aws-lc-sys` needs) and
  uploads tarballs that also contain the systemd unit and env example. These are
  **plain release assets**: they are not signed or guarded and never enter
  `latest-native.json`, because the daemon has no self-updater.
- `server-image` assembles those prebuilt static binaries into a thin Alpine
  image, pushes one image per architecture, and merges them into a multi-arch
  `ghcr.io/zkz098/limedl-server` manifest (the `latest` tag is skipped for an
  alpha/beta/rc). It does not rebuild the engine; QEMU only runs the image's
  Alpine `apk add` layer.

`native-manifest` is the **sole writer** of `latest-native.json`. It runs on
`windows-latest` with `if: always()`, downloads only the artifacts whose platform
leg succeeded, builds the asset map from what actually exists, signs each
artifact, generates the manifest with `cargo xtask manifest`, signs the manifest,
then runs `cargo xtask guard` over all of it. A missing platform key is reported
by the client as "no update" instead of an error, which is why a partial release
still updates the platforms that did build. `/P /R` NSIS flags and the
`SilentInstall normal` directive keep the installer usable silently and
interactively.

`native-manifest` also lists `build-server` in its `needs`. That is a sequencing
constraint rather than a dependency: every job uploads to the same GitHub release
with the same computed `prerelease` flag, and `native-manifest` deliberately
flips it to `false` last, so a still-running server leg could otherwise reset the
flag from its own upload.

Evidence: `repo://.github/workflows/release.yml#L1-L31`,
`repo://.github/workflows/release.yml#L77-L121`,
`repo://.github/workflows/release.yml#L517-L592`,
`repo://.github/workflows/release.yml#L608-L693`,
`repo://.github/workflows/release.yml#L703-L840`.

The manifest generator is platform-agnostic: it accepts an `--assets-json` map of
`key → { kind, path }`, validates the kind and that each signature exists, and
preserves the asset-map order in the emitted JSON. `cargo xtask manifest` is
therefore reusable for a new platform matrix leg without code changes.

Evidence: `repo://xtask/src/manifest.rs#L132-L175`.

## Platform caveats

- **macOS** releases are ad-hoc signed and not notarized; a browser download
  carries `com.apple.quarantine` and needs right-click → Open once. Adding a
  Developer ID later means setting `SIGN_IDENTITY` in `scripts/package-macos.sh`
  plus a notarytool step; no client change is needed.
- **Linux** targets `x86_64-unknown-linux-gnu` with `target-cpu=x86-64-v3`, so it
  needs glibc ≥ 2.39 (Ubuntu 24.04+) and a 2013+ CPU.
- **MSIX autostart** cannot carry `--hidden` (the manifest's `startupTask` takes no
  arguments), so the client infers a login launch by comparing process creation
  time against the shell's within a 150 s window when package identity is present
  and autostart is enabled.
- **`update::clean_update_work_dir`** clears leftover files in the update work
  directory at every startup.

Evidence: `repo://crates/limedl-native/src/autostart.rs#L1-L10`,
`repo://packaging/msix/AppxManifest.xml#L54-L66`,
`repo://crates/limedl-native/src/update/mod.rs#L627-L640`.

Related pages: [Native Desktop UI (Slint)](native-ui-architecture.md),
[Headless Server Daemon](../integrations/headless-server-daemon.md),
[Build, Tooling, CI and Release Operations](../operations/build-release-and-ci.md),
[Networking, HTTP Clients and Rate Control](../systems/networking-and-rate-control.md).
