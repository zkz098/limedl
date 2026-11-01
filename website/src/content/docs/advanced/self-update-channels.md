---
title: 自更新通道与回滚
description: limedl 如何识别安装类型、验证更新清单与资产、在各平台执行更新，以及 MSIX/AppImage 的限制与回滚注意事项
---

# 自更新通道与回滚

limedl 的桌面端内置自更新，但不同安装方式走不同的更新路径。理解这一点可以避免“点了更新却没变化”或“更新后启动失败”。

---

## 安装类型与检测顺序

启动时客户端按以下顺序判断安装类型：

1. **Store**：Windows 包标识存在（MSIX / Microsoft Store）；
2. **Installer**：HKCU 卸载项 `limedl-native` 存在（NSIS Setup.exe 写入）；
3. **Portable**：其他情况；macOS 与 Linux 始终按 Portable 处理。

| 安装方式 | 检测结果 | 更新路径 |
| :--- | :--- | :--- |
| Windows Setup.exe | Installer | 下载新 Setup，执行 `/P /R` 静默升级后退出 |
| Windows 便携 zip | Portable | 原地替换可执行文件并重启 |
| Windows MSIX | Store | **跳过应用内检查**，由 Microsoft Store 更新 |
| macOS `.app` `.tar.gz` | Portable | 原地替换并重新做 ad-hoc 签名 |
| Linux 便携 `.tar.gz` | Portable | 原地替换 |
| Linux AppImage | Portable | **拒绝原地替换**；请下载新 AppImage |
| Linux `.deb` | Portable | 无包管理器集成；建议用包管理器或手动替换 |

---

## 更新清单与验证

客户端从 GitHub Releases 的固定地址读取：

```text
latest-native.json
latest-native.json.sig
latest-native.json.pqc.sig
```

流程：

1. **先验证清单签名，再解析 JSON**；签名失败会中止。
2. 比较清单版本与当前 `CARGO_PKG_VERSION`，不新则视为无更新。
3. 按 `{os}-{arch}` 键选择资产（macOS 写作 `darwin`，便携/Store 追加 `-portable`）。
4. 下载资产后，用清单内嵌的 `signature` / `pqcSignature` 再次校验原始字节。
5. 下载上限为 **512 MiB**。

细节与已知边界见[安全与验签](/security/)。GitHub 的 `releases/latest` 不包含预发布版本，因此稳定版用户不会收到 rc/alpha。

---

## 各平台的更新行为

### Windows Installer

- 下载新的 Setup.exe，以 `/P /R` 启动，然后当前进程退出，让安装器替换二进制。
- 如果安装目录不可写或安装器被拦截，更新会失败并保留旧版本。

### Windows 便携版

- 解压新包并执行原地替换；替换完成后提示重启。
- 数据目录不受影响。

### MSIX

- 应用内更新对 Store 安装直接跳过；更新由 Microsoft Store 负责。
- MSIX 的开机自启动项不能携带 `--hidden` 参数；客户端会通过进程创建时间与登录时间窗口推断“登录自启”。

### macOS

- 便携 `.app` 支持原地替换；替换后必须重新做 ad-hoc 签名，否则系统拒绝启动。
- 若应用位于 `/Applications` 且当前用户无写权限，更新会失败并提示安装位置不可写；请把应用放到用户可写位置，或手动替换。
- 首次运行仍可能被 Gatekeeper 拦截，见[安装指南](/getting-started/installation/)。

### Linux

- 便携 `.tar.gz` 可以原地替换。
- **AppImage 内部无法替换挂载的 bundle**：客户端会拒绝自更新，请直接下载新 AppImage。
- `.deb` 没有包管理器钩子；请用 `apt`/`dpkg` 升级或手动替换。
- 需要 glibc ≥ 2.17 与 `x86-64-v3`，见[平台支持](/getting-started/platform-support/)。

---

## 回滚

- 旧版本安装包仍可从 [GitHub Releases](https://github.com/zkz098/limedl/releases) 获取。
- **回滚前备份整个数据目录**：新版本可能已经迁移 `settings.json` 或 `downloads.db` 结构，旧版本不一定能读回。
- 回滚不会自动降级数据库；如果旧版本拒绝打开，请恢复到备份的数据目录。
- 自更新没有密钥滚动链；如果密钥已轮换，旧客户端不会接受新密钥签名的更新，需要先安装包含新公钥的版本。

---

## 排查

| 现象 | 可能原因 | 处理 |
| :--- | :--- | :--- |
| 检查更新无反应 | Store 安装；或已是最新；或网络/代理阻断 GitHub | 用 Store 更新；检查代理与日志 |
| 下载完成但未替换 | 安装目录不可写、AppImage、安装器被拦截 | 手动替换或用对应渠道更新 |
| 更新后无法启动 | 数据/版本不兼容、签名损坏 | 恢复备份，重新安装当前版本 |
| PQC 签名缺失警告 | 清单只有 Ed25519 签名 | 继续使用 Ed25519 校验；等待完整签名发布 |

---

## 相关文档

- [安全与验签](/security/)：签名链、公钥与手动验证。
- [安装指南](/getting-started/installation/)：各平台安装与 Gatekeeper。
- [平台支持](/getting-started/platform-support/)：更新通道与系统限制。
- [数据备份与迁移](/guides/data-backup-and-migration/)：更新前如何备份。
