---
title: 安装指南
description: 如何在 Windows、macOS 与 Linux 系统上获取并安装 limedl
---

# 安装指南

limedl 提供面向主流桌面系统的原生预编译包。推荐通过官方下载渠道获取经过 Minisign 密码学验签的二进制文件。

## Windows (x86_64)

Windows 用户推荐使用现代 Windows 10 / 11 64 位系统：

- **安装程序 (Setup.exe)**：推荐普通用户使用。提供桌面快捷方式、文件关联并支持软件内自动静默更新。
- **绿色便携版 (Portable .zip)**：解压即用，所有配置和下载历史默认保存在解压目录中，适合 U 盘携带或免管理员权限运行。
- **MSIX 现代应用包**：适合企业或追求安全沙箱隔离的用户。

### 校验下载完整性

你可以使用 PowerShell 内置命令核对 SHA256 哈希值：

```powershell
Get-FileHash -Algorithm SHA256 .\limedl-native-v0.3.9-windows-x86_64-setup.exe
```

---

## macOS (Apple Silicon - aarch64)

针对 M1/M2/M3/M4 系列芯片原生编译：

1. 下载 `limedl-native-v0.3.9-darwin-aarch64-portable.tar.gz` 并解压。
2. 将 `limedl.app` 拖移至系统的 `应用程序 (Applications)` 文件夹。
3. **首次打开提示**：由于当前版本未通过 Apple 商业开发者公证，初次启动时若提示“无法验证开发者”，请在访达中**按住 Control 键并右键点击** `limedl.app`，选择**打开**即可。

---

## Linux (x86_64)

适用于现代 Linux 发行版（glibc ≥ 2.39，如 Ubuntu 24.04+、Fedora、Arch Linux）：

```bash
# 解压并运行
tar -xzf limedl-native-v0.3.9-linux-x86_64-portable.tar.gz
cd limedl
./limedl-native
```
