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
  limedl-native-v0.5.0-windows-x86_64-setup.exe /P /R
  ```
  *(Where `/P` runs silently and `/R` suppresses unexpected system reboots)*

### 2. Portable Archive (.zip) — Advanced / USB Drives
- **Features**: Requires no administrator privileges. Unpack and run anywhere.
- **Storage**: By default, state is kept in `%LOCALAPPDATA%\limedl`. You can also set the `LIMEDL_DATA_DIR` environment variable to create a 100% self-contained portable installation on flash drives.
- **Portable In-App Updates**: The portable build also supports seamless in-place binary swapping during updates.

### 3. MSIX Modern Package (.msix)
- **Features**: Packaged using Windows modern app containerization, offering strict sandbox security, zero residual uninstallation, and seamless Windows 10/11 system integration.

---

## macOS Installation

A tailored, natively compiled aarch64 binary is provided for Apple Silicon (M-series processors).

### Step-by-Step
1. Download `limedl-native-v0.5.0-darwin-aarch64-portable.tar.gz` from the download page;
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
chmod +x limedl-native-v0.5.0-linux-x86_64.AppImage

# Launch the client
./limedl-native-v0.5.0-linux-x86_64.AppImage
```

### 2. Debian / Ubuntu Package (.deb)
Integrates with desktop menus, mime associations, and icons:
```bash
sudo apt install ./limedl-native-v0.5.0-linux-x86_64.deb
```

### 3. Portable Tarball (.tar.gz)
```bash
tar -xzf limedl-native-v0.5.0-linux-x86_64-portable.tar.gz
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
Get-FileHash -Algorithm SHA256 .\limedl-native-v0.5.0-windows-x86_64-setup.exe
```

**macOS (Terminal)**:
```bash
shasum -a 256 limedl-native-v0.5.0-darwin-aarch64-portable.tar.gz
```

**Linux (Terminal)**:
```bash
sha256sum limedl-native-v0.5.0-linux-x86_64.AppImage
```

### 2. Minisign Verification

Official Release Public Key:
```text
RWTN2zWlB8Qz0bI6Xq4l4p9J7gYQx4fR8uV2kP3m9w0L
```

Verify signature:
```bash
minisign -Vm limedl-native-v0.5.0-windows-x86_64-setup.exe -P "RWTN2zWlB8Qz0bI6Xq4l4p9J7gYQx4fR8uV2kP3m9w0L"
```
If verified, the command outputs `Signature and comment signature verified`.

---

## Initial Setup & Data Migration

On first run, the **Setup Wizard** guides you through:
1. **Language & Appearance**: Choose between Chinese/English and Light/Dark themes.
2. **Download Directory & Networking**: Set your default storage folder.
3. **Legacy Data Migration**: If you previously used the legacy Tauri-based version of limedl, the client will **automatically detect and safely import** your settings (`settings.json`), download history (`downloads.db`), and active torrent states without manual intervention.

### Default Storage Paths
- **Windows**: `%LOCALAPPDATA%\limedl\`
- **Linux**: `~/.local/share/limedl/`
- **macOS**: `~/Library/Application Support/limedl/`
- **Override**: Set the `LIMEDL_DATA_DIR` environment variable to redirect all state files to a folder of your choice.
