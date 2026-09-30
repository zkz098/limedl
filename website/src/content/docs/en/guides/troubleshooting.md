---
title: Troubleshooting & FAQ
description: Diagnosing common download errors, mirror 403 anti-abuse blocks, BitTorrent zero speeds, cross-platform permissions, and log locations
---

# Troubleshooting & FAQ

This guide compiles common networking, permission, protocol, and cross-platform issues encountered when using limedl, along with verified resolutions.

---

## 1. Mirror Downloads Return 403 Forbidden (e.g. Tsinghua TUNA)

### Symptoms
A download link opens and downloads smoothly in your web browser, but fails immediately in limedl with:
```text
http status 403 Forbidden (server anti-abuse check rejected this client;
update the default User-Agent in Settings or try another mirror)
```

### Root Cause
Major open-source mirror stations (such as Tsinghua University's TUNA mirror `mirrors.tuna.tsinghua.edu.cn`) employ **strict edge anti-abuse filtering**:
- The mirror's edge firewall inspects incoming `User-Agent` (UA) headers;
- If a UA claims to be Chrome but the version number is outdated or not yet released, the edge classifies it as an unauthorized scraper or unrecognized disguised bot, rejecting the download.
- Crucially, `HEAD` probe requests are usually exempted from this filter, so initial file size probing appears normal, but the subsequent concurrent `GET Range` requests trigger a 403 Forbidden response.

### Resolution
1. Open limedl **Settings -> Network & Proxy**;
2. Locate the **Default User-Agent** field;
3. Update it to a current stable Chrome UA string, for example:
   ```text
   Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36
   ```
   Or set a legitimate tool identity:
   ```text
   limedl/0.3.14
   ```
4. **IPv4 vs IPv6 Checks**: If 403 persists, the mirror may have rate-limited your ISP's entire IPv4 subnet. Switch to an IPv6 network if available, or try an alternative mirror endpoint (e.g., USTC, Alibaba Cloud, or Huawei Cloud).

---

## 2. BitTorrent Speed is 0 / Cannot Connect to Peers

### Diagnostic Steps

#### Step 1: Check for "Dead Torrents"
Review the seeder count on the torrent index where you found the file. If there are 0 active seeders worldwide, the BitTorrent engine cannot reconstruct missing pieces.

#### Step 2: Check DHT Node Count
Open the task's Inspector panel and examine the DHT node count:
- Under normal network conditions, DHT nodes should climb to **100 ~ 500+** within 1 to 2 minutes of startup.
- If it remains stuck at `0`, your system firewall or router is likely dropping inbound/outbound UDP packets. Ensure Windows Defender Firewall allows traffic for `limedl-native.exe`.

#### Step 3: Enable Router UPnP
Most consumer connections sit behind carrier-grade or home NAT. External peers cannot initiate connections to your client unless ports are open:
- Access your router's administration interface (`192.168.1.1`);
- Enable **UPnP**;
- limedl will automatically map its listening port (`6881`), dramatically improving peer connectability and download speeds.

#### Step 4: Inject Public Trackers
Under **Settings -> BitTorrent -> Public Trackers**, paste active tracker URLs (e.g., from community-maintained tracker repositories) to supplement DHT lookups for rare torrents.

---

## 3. macOS Shows "Cannot Verify Developer" or "App is Damaged"

### Root Cause
Because open-source release builds use ad-hoc code signing without a paid Apple Developer certificate, macOS Gatekeeper blocks execution by default.

### Resolution
- **Via Finder**:
  Open the Applications folder, **hold the Control key and right-click** `limedl.app`, select **Open**, and click **Open** in the confirmation dialog. This only needs to be performed once.
- **Via Terminal**:
  Remove the quarantine extended attribute manually:
  ```bash
  xattr -cr /Applications/limedl.app
  ```

---

## 4. Linux Cannot Open Native File Picker Dialogs

### Root Cause
limedl uses `xdg-desktop-portal` via D-Bus for Wayland-native, sandbox-friendly file pickers. If the desktop environment is missing a portal service, clicking "Browse Folder" may not respond.

### Resolution
Install the desktop portal package for your distribution:

**Ubuntu / Debian**:
```bash
sudo apt install xdg-desktop-portal xdg-desktop-portal-gtk
```

**Arch Linux / Manjaro**:
```bash
sudo pacman -S xdg-desktop-portal xdg-desktop-portal-gtk
```

**Fedora**:
```bash
sudo dnf install xdg-desktop-portal xdg-desktop-portal-gtk
```

---

## 5. Linux System Tray Icon Missing

On certain GNOME desktop versions, system tray icons are hidden by default:
- Install the **AppIndicator and KStatusNotifierItem Support** GNOME Shell extension;
- Ensure required libraries are installed: `sudo apt install libayatana-appindicator3-1` or `libappindicator3-1`.

---

## 6. Log Locations & Filing Bug Reports

If you experience crashes, interrupted downloads, or unexpected errors, inspect the application log files:

### Log Directories
- **Windows**: `%LOCALAPPDATA%\limedl\logs\`
- **Linux**: `~/.local/share/limedl/logs/`
- **macOS**: `~/Library/Application Support/limedl/logs/`

### Submitting a Helpful Issue
To report a bug, visit [GitHub Issues](https://github.com/zkz098/limedl/issues) with:
1. Operating system and CPU architecture (e.g., Windows 11 x86_64, macOS 15 M3);
2. limedl version (e.g., v0.3.14);
3. Type of task (HTTP chunked, Magnet, or Torrent);
4. Relevant error lines from the log files (please redact private tokens or sensitive URLs).
