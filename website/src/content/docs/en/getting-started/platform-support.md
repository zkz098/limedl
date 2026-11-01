---
title: Platform Support
description: Supported operating systems, CPU architectures, packages, runtime dependencies, and update channels for the limedl desktop client and headless server
---

# Platform Support

limedl ships two release lines: the **desktop client** and the **headless server**. They share the same Rust download engine, but their supported platforms, CPU baselines, and update paths differ.

:::caution[In short]
- Desktop builds exist only for **Windows x86_64, macOS Apple Silicon, and Linux x86_64**.
- There is **no** Windows ARM64, Intel Mac, or Linux ARM64 desktop build, and no 32-bit build.
- Desktop x86_64 builds require `x86-64-v3` (Haswell 2013+, AVX2); headless server x86_64 builds use the broader `x86-64-v2` baseline (Nehalem 2008+).
:::

---

## Desktop client

| Platform | Architecture | Minimum OS | Packages | In-app update |
| :--- | :--- | :--- | :--- | :--- |
| **Windows** | x86_64 (x86-64-v3) | Windows 10 1809+ / Windows 11 | Setup.exe, portable zip, MSIX | Setup and portable builds update in place; MSIX updates through Microsoft Store |
| **macOS** | Apple Silicon (aarch64, `apple-m1` baseline) | macOS 11.0+ | `.app` in a `.tar.gz` | Portable build replaces in place and re-applies an ad-hoc signature |
| **Linux** | x86_64 (x86-64-v3) | glibc ≥ 2.17 | AppImage, `.deb`, portable `.tar.gz` | Portable tarball updates in place; use the AppImage/deb channel for those packages |

### Windows

- **OS version**: the package manifest declares `10.0.17763.0` (Windows 10 1809) as the minimum. Older Windows 10 builds, Windows 7, and Windows 8 are unsupported.
- **CPU**: desktop builds enable `x86-64-v3` (AVX2 / BMI / FMA), so Haswell (2013) and newer x86_64 processors are required. Older x86_64 CPUs may fail to launch.
- **Not available**: a native Windows ARM64 package. ARM64 Windows may run the x86_64 build under emulation, but that combination is not officially supported.
- **Protocol registration**: Setup.exe registers `magnet:` and `limedl://` handlers.
- **Updates**: the installer downloads a new Setup package and runs it with `/P /R`; the portable build swaps the executable in place; MSIX is owned by Microsoft Store and skips in-app updates.

### macOS

- **Chip**: Apple Silicon only (M1/M2/M3/M4). The release workflow builds `darwin-aarch64` only — there is **no Intel Mac build**.
- **OS version**: `Info.plist` declares `LSMinimumSystemVersion` `11.0` (macOS Big Sur or later).
- **Signing**: builds are ad-hoc signed and not notarized. After a browser download, right-click → Open once, or run `xattr -cr /Applications/limedl.app`.
- **Updates**: the portable `.app` supports in-app updates; the updater replaces the binary and re-signs the bundle. If the app lives in `/Applications` and the user cannot write there, the update fails with an install-location error.

### Linux

- **glibc**: releases target `x86_64-unknown-linux-gnu.2.17`, and CI runs `scripts/check-glibc-floor.sh` to prevent a dependency from raising that floor. Debian 10+, Ubuntu 18.04+, CentOS 7+, and similar systems qualify.
- **CPU**: like the Windows desktop build, Linux enables `x86-64-v3`. **A new enough glibc does not imply a new enough CPU** — pre-2013 x86_64 machines may lack AVX2 and fail to run.
- **Desktop dependencies**:
  - File dialogs go through `xdg-desktop-portal` + D-Bus, including Wayland and sandboxed sessions; tiling window managers need a backend such as `xdg-desktop-portal-gtk`.
  - The tray uses StatusNotifierItem; GNOME needs the AppIndicator extension, while KDE/XFCE provide a host.
- **Updates**: the portable `.tar.gz` updates in place; an AppImage cannot replace its mounted bundle, so download the new AppImage; upgrade `.deb` through the package manager.
- **Not available**: a Linux ARM64 desktop build.

### Data directories

| Platform | Default data directory |
| :--- | :--- |
| Windows | `%LOCALAPPDATA%\limedl\` |
| macOS | `~/Library/Application Support/limedl/` |
| Linux | `~/.local/share/limedl/` |

Set `LIMEDL_DATA_DIR` to override the location; pointing it at removable media gives a portable installation.

---

## Headless server (`limedl-server`)

| Platform | Architecture | Runtime | Packages |
| :--- | :--- | :--- | :--- |
| Linux | x86_64 (x86-64-v2) | Fully static musl, no glibc needed | `.tar.gz`, Docker image |
| Linux | aarch64 | Fully static musl | `.tar.gz`, Docker image |

- Server builds use `x86-64-v2` (Nehalem 2008+), a wider baseline than the desktop client, aimed at older NAS hardware and soft routers.
- Static musl binaries do not depend on the host glibc, so they run on Alpine, OpenWrt, NAS systems, and minimal containers.
- The server exposes only the Aria2 JSON-RPC 2.0 API; there is no Slint GUI. Drive it with AriaNg, Motrix, or scripts.
- Server assets are **not** part of the desktop self-update signing chain; verify them against the SHA-256 digest shown on the GitHub Release page.

---

## Unsupported combinations

| Scenario | Status | Suggestion |
| :--- | :--- | :--- |
| Intel Mac (x86_64) | Unsupported | Use another platform, or build from source (not covered by release guarantees) |
| Windows on ARM | No native package | The x86_64 build may run under emulation; not an official combination |
| Linux ARM64 desktop | Unsupported | Use the aarch64 `limedl-server` build for headless downloads |
| 32-bit x86 | Unsupported | — |
| x86_64 CPU without AVX2 | Desktop may not launch | Use the server build, or compile with a lower `target-cpu` |
| glibc < 2.17 | Unsupported | Upgrade the OS, or use the static musl server |
| macOS < 11.0 | Unsupported | Upgrade the OS |

---

## Related documentation

- [Installation](/en/getting-started/installation/): per-platform install, Gatekeeper, and signature verification.
- [Security & Verification](/en/security/): release signatures, public keys, and the update trust chain.
- [Headless server deployment](/en/ecosystem/server-daemon/): NAS, soft router, Docker, and systemd.
- [Changelog](/en/changelog): version history and platform-specific fixes.
