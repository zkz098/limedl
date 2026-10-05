---
title: 安装指南
description: 如何在 Windows、macOS 与 Linux 系统上获取、安装与校验 limedl 原生客户端
---

# 安装指南

limedl 为主流桌面操作系统提供了原生编译的发布版本。所有发布文件均经过 **Minisign 密码学防篡改签名** 与 SHA256 哈希固化，确保安全纯净。

:::tip[快捷下载通道]
你随时可以访问官方 [下载页面](/download/)，页面会自动根据你的浏览器环境推荐最适合你当前系统的安装包格式。
:::

---

## 系统环境要求

| 操作系统 | 推荐架构 | 最低系统版本 | 运行依赖 |
| :--- | :--- | :--- | :--- |
| **Windows** | x86_64 (64位) | Windows 10 (1809+) 或 Windows 11 | 系统默认自带 C++ 运行时，无需额外依赖 |
| **macOS** | Apple Silicon (aarch64) | macOS Monterey 12.0 或更高版本 | 原生支持 M1 / M2 / M3 / M4 系列芯片 |
| **Linux** | x86_64 (64位) | glibc ≥ 2.17 (如 Debian 10+, Ubuntu 18.04+, CentOS 7+) | `xdg-desktop-portal` (文件对话框) · StatusNotifier 宿主 (托盘) |

---

## Windows 安装说明

Windows 平台提供三种分发格式，满足不同场景的使用偏好：

### 1. 标准安装包 (Setup.exe) — 推荐普通用户
- **特性**：提供向导式安装，自动创建开始菜单与桌面快捷方式，自动在系统中注册 `magnet:` 磁力链接与 `limedl://` 原生协议关联。
- **自动更新**：支持软件内检测到新版本时一键静默热更新。
- **静默安装参数**：适用于系统管理员或自动化脚本：
  ```cmd
  limedl-native-v0.4.10-windows-x86_64-setup.exe /P /R
  ```
  *(其中 `/P` 为静默模式，`/R` 抑制不必要的系统重启)*

### 2. 绿色便携版 (Portable .zip) — 推荐高级用户与 U 盘使用
- **特性**：无需管理员安装权限，解压缩到任意目录即可运行。
- **数据存储**：默认数据存储于当前用户目录的 `%LOCALAPPDATA%\limedl`，亦可通过配置环境变量 `LIMEDL_DATA_DIR` 实现完全的随身便携化。
- **便携自更新**：便携版同样支持应用内自更新，更新程序会在原地无缝替换可执行文件并安全重启。

### 3. MSIX 现代安全包 (.msix)
- **特性**：利用 Windows 现代应用容器封装，享受安全沙箱保护、纯净卸载无残留、与 Windows 10/11 系统深度集成。

---

## macOS 安装说明

针对 Apple Silicon 芯片（M 系列）提供了深度优化的 原生 aarch64 架构二进制。

### 安装步骤
1. 前往下载页面获取 `limedl-native-v0.4.10-darwin-aarch64-portable.tar.gz`；
2. 解压下载的压缩包，得到 `limedl.app`；
3. 将 `limedl.app` 拖入系统的 **应用程序 (Applications)** 文件夹中。

### 门禁机制放行 (Gatekeeper)

:::caution[初次启动提示“无法验证开发者”或“文件损坏”]
由于开源构建版本目前使用 Ad-hoc 签名且未购买苹果商业开发者公证证书，macOS Gatekeeper 安全防护会在初次运行时弹出拦截提示。可通过以下两种方式放行：
:::

- **图形界面放行（推荐）**：
  在访达（Finder）中找到 `limedl.app`，**按住 Control 键并右键点击应用图标**，在弹出的上下文菜单中点击 **“打开”**，并在出现的安全对话框中确认点击 **“打开”** 即可。此操作仅需执行一次。
- **终端一键清除隔离属性**：
  打开终端（Terminal），执行以下命令移除系统的下载隔离标记：
  ```bash
  xattr -cr /Applications/limedl.app
  ```

---

## Linux 安装说明

适用于 Debian 10+、Ubuntu 18.04+、CentOS 7+、Fedora、Arch Linux 等使用 glibc ≥ 2.17 的 Linux 发行版。

### 1. AppImage 独立免安装包（推荐）
免除跨发行版依赖冲突，包含完整的运行时组件：
```bash
# 赋予可执行权限
chmod +x limedl-native-v0.4.10-linux-x86_64.AppImage

# 运行客户端
./limedl-native-v0.4.10-linux-x86_64.AppImage
```

### 2. Debian / Ubuntu 软件包 (.deb)
深度集成桌面环境，自动安装图标、注册 MIME 类型以及协议映射：
```bash
sudo apt install ./limedl-native-v0.4.10-linux-x86_64.deb
```

### 3. 便携归档包 (.tar.gz)
适合喜欢手动放置可执行文件的极客用户：
```bash
tar -xzf limedl-native-v0.4.10-linux-x86_64-portable.tar.gz
cd limedl-native
./limedl-native
```

### Linux 运行依赖说明
- **原生文件选择器**：limedl 使用标准 `xdg-desktop-portal`（通过 D-Bus 与宿主桌面通信，完美适配 Wayland 与沙箱环境）。主流桌面（GNOME、KDE Plasma、XFCE）均默认具备；如在平铺式窗口管理器（i3/Sway）下运行，请确保已安装 `xdg-desktop-portal` 及对应的后端实现（如 `xdg-desktop-portal-gtk`）。
- **系统托盘**：托盘是走会话 D-Bus 的 StatusNotifierItem，不需要 GTK。GNOME 需安装并启用 **AppIndicator** 扩展；KDE/XFCE 等桌面自带宿主。

---

## 校验下载文件的完整性与签名

为了防止文件在下载或镜像传输过程中被篡改或损坏，强烈建议核对发布包的校验和。

### 1. SHA-256 哈希比对

**Windows (PowerShell)**:
```powershell
Get-FileHash -Algorithm SHA256 .\limedl-native-v0.4.10-windows-x86_64-setup.exe
```

**macOS (终端)**:
```bash
shasum -a 256 limedl-native-v0.4.10-darwin-aarch64-portable.tar.gz
```

**Linux (终端)**:
```bash
sha256sum limedl-native-v0.4.10-linux-x86_64.AppImage
```

### 2. Minisign 密码学防篡改验签

limedl 发布流水线为所有构建工件生成 Minisign 签名（`.sig` 文件）。你可以使用标准 `minisign` 工具核对签名：

官方发布公钥：
```text
RWTN2zWlB8Qz0bI6Xq4l4p9J7gYQx4fR8uV2kP3m9w0L
```

执行验签命令：
```bash
minisign -Vm limedl-native-v0.4.10-windows-x86_64-setup.exe -P "RWTN2zWlB8Qz0bI6Xq4l4p9J7gYQx4fR8uV2kP3m9w0L"
```
若终端输出 `Signature and comment signature verified`，说明文件未受任何篡改且确实由官方流水线签名产出。

---

## 首次启动与历史数据迁移

首次启动 limedl 时，应用会启动 **首启动向导**：
1. **语言与主题配置**：可快速选择简体中文/英文，以及浅色/深色主题。
2. **下载路径与网络**：设定默认文件保存目录，测试网络联通性。
3. **老版本自动平滑迁移**：若你此前曾使用过早期基于 Tauri 的旧版 limedl，客户端在首次启动时会**自动检测并安全导入**原有的配置项（`settings.json`）、历史下载数据库（`downloads.db`）以及活跃种子元数据，无需手动重新录入。

### 数据存储路径
- **Windows**: `%LOCALAPPDATA%\limedl\`
- **Linux**: `~/.local/share/limedl/`
- **macOS**: `~/Library/Application Support/limedl/`
- **自定义路径**: 你可以通过设置系统环境变量 `LIMEDL_DATA_DIR` 指向任意自定义存储路径。
