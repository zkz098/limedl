---
title: Installation Guide
description: How to install limedl on Windows, macOS, and Linux
---

# Installation Guide

limedl provides official pre-built native binaries verified with Minisign cryptographic signatures.

## Windows (x86_64)

Supported on Windows 10 / 11 64-bit:

- **Installer (Setup.exe)**: Recommended for most users. Creates shortcuts, sets file associations, and supports in-app auto updates.
- **Portable (.zip)**: Extract and run. All configuration and download data are stored alongside the executable.
- **MSIX Package**: Modern sandboxed package for enterprise and isolation-focused environments.

### Verify SHA256 Checksum

In PowerShell:

```powershell
Get-FileHash -Algorithm SHA256 .\limedl-native-v0.3.9-windows-x86_64-setup.exe
```

---

## macOS (Apple Silicon - aarch64)

Native builds for Apple Silicon (M1/M2/M3/M4):

1. Download `limedl-native-v0.3.9-darwin-aarch64-portable.tar.gz` and extract it.
2. Drag `limedl.app` into your **Applications** folder.
3. **First launch**: Because the binary is ad-hoc signed without an Apple Developer ID certificate, right-click (or Control-click) `limedl.app` in Finder and select **Open**.

---

## Linux (x86_64)

Supports modern Linux distributions with glibc ≥ 2.39 (e.g. Ubuntu 24.04+, Fedora, Arch):

```bash
tar -xzf limedl-native-v0.3.9-linux-x86_64-portable.tar.gz
cd limedl
./limedl-native
```
