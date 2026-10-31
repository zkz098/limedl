---
title: Installation
description: How to acquire, install, and verify the limedl native desktop client on Windows, macOS, and Linux
---

# Installation Guide

limedl provides natively compiled release artifacts for all major desktop platforms. Every published binary is cryptographically signed using **Minisign** and verifiable against SHA-256 digests.

:::tip[Quick Download]
You can always head over to the official [Download Page](/en/download/), which automatically highlights the recommended package format for your current operating system.
:::

---

## System Requirements

| Operating System | Architecture | Minimum Version | Runtime Dependencies |
| :--- | :--- | :--- | :--- |
| **Windows** | x86_64 (64-bit) | Windows 10 (1809+) or Windows 11 | Bundled standard C++ runtimes, no extra install |
| **macOS** | Apple Silicon (aarch64) | macOS Monterey 12.0 or later | Native M1 / M2 / M3 / M4 support |
| **Linux** | x86_64 (64-bit) | glibc ≥ 2.17 (e.g. Debian 10+, Ubuntu 18.04+, CentOS 7+) | `xdg-desktop-portal` (Dialogs) · StatusNotifier host (Tray) |

---

## Windows Installation

Windows users can choose from three distribution formats based on their deployment needs:

### 1. Standard Setup (.exe) — Recommended
- **Features**: Guided setup wizard that creates Start Menu and Desktop shortcuts, registers the `magnet:` and `limedl://` protocol handlers in the registry, and enables background in-app updates.
- **Silent Installation Switches**: Ideal for enterprise provisioning or scripting:
  ```cmd
  limedl-native-v0.6.1-windows-x86_64-setup.exe /P /R
  ```
  *(Where `/P` runs silently and `/R` suppresses unexpected system reboots)*

### 2. Portable Archive (.zip) — Advanced / USB Drives
- **Features**: Requires no administrator privileges. Unpack and run anywhere.
- **Storage**: By default, state is kept in `%LOCALAPPDATA%\limedl`. You can also set the `LIMEDL_DATA_DIR` environment variable to create a 100% self-contained portable installation on flash drives.
- **Portable In-App Updates**: The portable build also supports seamless in-place binary swapping during updates.

### 3. MSIX Modern Package (.msix)
- **Features**: Packaged using Windows modern app containerization, offering strict sandbox security, zero residual uninstallation, and seamless Windows 10/11 system integration.
- **Updates**: MSIX builds update through **Microsoft Store**; in-app self-update applies to the Setup.exe and portable builds only.

---

## macOS Installation

A tailored, natively compiled aarch64 binary is provided for Apple Silicon (M-series processors).

### Step-by-Step
1. Download `limedl-native-v0.6.1-darwin-aarch64-portable.tar.gz` from the download page;
2. Extract the archive to obtain `limedl.app`;
3. Drag and drop `limedl.app` into your **Applications** folder.

### Gatekeeper Verification Note

:::caution[First launch shows "Cannot verify developer" or "App is damaged"]
Because community open-source releases use ad-hoc code signing without a paid Apple Developer notarization certificate, macOS Gatekeeper may present a security warning on initial launch. You can bypass this safely:
:::

- **Via Finder (Recommended)**:
  Locate `limedl.app` in Finder, **hold the Control key and right-click the app icon**, choose **Open** from the menu, and confirm by clicking **Open** in the dialog. This is only required once.
- **Via Terminal**:
  Open Terminal and clear the quarantine extended attribute:
  ```bash
  xattr -cr /Applications/limedl.app
  ```

---

## Linux Installation

Targeted at Linux distributions running glibc ≥ 2.17 (Debian 10+, Ubuntu 18.04+, CentOS 7+, Fedora, Arch Linux).

### 1. AppImage Standalone (Recommended)
Self-contained and distribution-agnostic:
```bash
# Grant execution permissions
chmod +x limedl-native-v0.6.1-linux-x86_64.AppImage

# Launch the client
./limedl-native-v0.6.1-linux-x86_64.AppImage
```

### 2. Debian / Ubuntu Package (.deb)
Integrates with desktop menus, mime associations, and icons:
```bash
sudo apt install ./limedl-native-v0.6.1-linux-x86_64.deb
```

### 3. Portable Tarball (.tar.gz)
```bash
tar -xzf limedl-native-v0.6.1-linux-x86_64-portable.tar.gz
cd limedl-native
./limedl-native
```

### Linux Runtime Dependencies
- **Native File Chooser**: limedl utilizes `xdg-desktop-portal` via D-Bus for Wayland-native and sandbox-friendly dialogs. Standard desktops (GNOME, KDE Plasma) include this out of the box. Minimal setups (i3, Sway) should install `xdg-desktop-portal` alongside a backend like `xdg-desktop-portal-gtk`.
- **System Tray**: The tray is a StatusNotifierItem served over the session D-Bus; no GTK is required. GNOME needs the **AppIndicator** extension enabled, while KDE/XFCE and most bars ship a host already.

---

## Integrity & Minisign Verification

### 1. Check SHA-256 Hashes

**Windows (PowerShell)**:
```powershell
Get-FileHash -Algorithm SHA256 .\limedl-native-v0.6.1-windows-x86_64-setup.exe
```

**macOS (Terminal)**:
```bash
shasum -a 256 limedl-native-v0.6.1-darwin-aarch64-portable.tar.gz
```

**Linux (Terminal)**:
```bash
sha256sum limedl-native-v0.6.1-linux-x86_64.AppImage
```

### 2. Minisign Verification

The release pipeline dual-signs all build artifacts with **Minisign (Ed25519) + ML-DSA-65 (NIST FIPS 204 Post-Quantum Cryptography)** (publishing `.sig` and `.pqc.sig` files). You can verify the Ed25519 signature using the standard `minisign` CLI:

Official Release Public Key:
```text
RWSe1Ow8aYLL4Iy0eRFN+40umqxd2kxlPoyg1AqJdPBblBMSMDOhKw+7
```

Verify signature:
```bash
minisign -Vm limedl-native-v0.6.1-windows-x86_64-setup.exe -P "RWSe1Ow8aYLL4Iy0eRFN+40umqxd2kxlPoyg1AqJdPBblBMSMDOhKw+7"
```
If verified, the command outputs `Signature and comment signature verified`.

### 3. ML-DSA-65 Post-Quantum Signature

Every artifact also ships a `.pqc.sig` file (`untrusted comment: ml-dsa-65 signature` followed by one line of base64). The official ML-DSA-65 public key is published in standard base64 at [`/limedl-pqc-pubkey.b64`](/limedl-pqc-pubkey.b64) (1952 raw bytes after decoding).

To verify independently, run a FIPS 204 ML-DSA-65 verification over the exact artifact bytes with the context `limedl-artifact`; `latest-native.json` uses the context `limedl-manifest`. The limedl client performs this check automatically during in-app updates, so regular users do not need to verify it manually.

---

## Initial Setup

On first run, the **Setup Wizard** guides you through:
1. **Language & Appearance**: Choose between Chinese/English and Light/Dark themes.
2. **Download Directory & Networking**: Set your default storage folder.

### Default Storage Paths
- **Windows**: `%LOCALAPPDATA%\limedl\`
- **Linux**: `~/.local/share/limedl/`
- **macOS**: `~/Library/Application Support/limedl/`
- **Override**: Set the `LIMEDL_DATA_DIR` environment variable to redirect all state files to a folder of your choice.
