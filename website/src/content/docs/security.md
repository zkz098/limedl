---
title: 安全与验签
description: limedl 发布资产的 sigilup (Ed25519 + ML-DSA-65) 混合签名、更新信任链、手动验签步骤与安全边界
---

# 安全与验签

limedl 的发布资产使用 **`sigilup`**（DSSEv1 **Ed25519 + ML-DSA-65** NIST FIPS 204 后量子密码学混合签名）进行保护。应用内更新会先验证 `latest.hsig.json` 的混合签名与 `security_epoch` 防回滚字段，再解析清单并下载资产，最后在流式写入时完成 **BLAKE3 + SHA-512** 校验。

本文说明哪些内容被签名、如何手动复核，以及这套机制**不能**防住什么。

---

## 信任链一览

```text
GitHub Release
├── latest.hsig.json              # DSSEv1 混合签名更新清单
├── 桌面资产 + .hsig               # 安装包、便携包、MSIX、deb、AppImage
└── limedl-server + .hsig         # 服务端 tar.gz 归档

客户端内置信任根
└── sigilup-pins.json             # 固定 Ed25519 + ML-DSA-65 混合公钥 (keyId: 7F5912C5F0B5971D)
```

- **清单先验签、后解析**：签名不通过或 `security_epoch` 回退时，清单内容不会影响任何下载 URL。
- **单遍流式双摘要**：`latest.hsig.json` 为每个自更新资产记录 **BLAKE3 + SHA-512** 摘要；客户端下载时单遍计算并校验。
- **独立 `.hsig` 信封**：每个发布资产旁均附带 `.hsig` DSSEv1 混合签名信封，可用于离线独立验签。
- **体积上限**：更新下载限制为 512 MiB，异常清单无法让客户端无限拉取。

### 哪些资产被签名

| 资产 | 签名 | 说明 |
| :--- | :--- | :--- |
| Windows Setup.exe / 便携 zip | `.hsig` + 清单内 BLAKE3/SHA-512 | 自更新资产 |
| macOS `.app` `.tar.gz` | `.hsig` + 清单内 BLAKE3/SHA-512 | 自更新资产 |
| Linux 便携 `.tar.gz` | `.hsig` + 清单内 BLAKE3/SHA-512 | 自更新资产 |
| Windows MSIX | `.hsig` | 独立资产，不在自更新清单内 |
| Linux `.deb` / `.AppImage` | `.hsig` | 独立资产，不在自更新清单内 |
| `limedl-server` `.tar.gz` | `.hsig` | 独立资产，不在自更新清单内 |
| `latest.hsig.json` | 内置 DSSEv1 混合签名 | 清单本身 |

---

## 手动验签

以下命令以 `v0.6.1` 的文件名为例，请替换为实际下载的版本。

### 1. 核对 SHA-256

GitHub Release 会为每个资产显示 SHA-256 摘要。下载后在本地计算并逐位比对：

```powershell
# Windows
Get-FileHash -Algorithm SHA256 .\limedl-native-v0.6.1-windows-x86_64-setup.exe
```

```bash
# macOS
shasum -a 256 limedl-native-v0.6.1-darwin-aarch64-portable.tar.gz

# Linux
sha256sum limedl-native-v0.6.1-linux-x86_64.AppImage
```

仓库不额外发布 `SHA256SUMS` 清单。

### 2. 使用 `sigilup` 验签 (Ed25519 + ML-DSA-65)

安装 `sigilup-cli` 后，使用仓库中的 [`sigilup-pins.json`](https://github.com/zkz098/limedl/blob/main/crates/limedl-native/src/update/sigilup-pins.json) 即可同时校验 Ed25519 与 ML-DSA-65 混合签名：

```bash
cargo install sigilup-cli --locked
sigilup verify --pins sigilup-pins.json limedl-native-v0.6.1-windows-x86_64-setup.exe
```

也可以直接校验更新清单：

```bash
sigilup verify --pins sigilup-pins.json latest.hsig.json
```

客户端在应用内更新时会自动完成这一步，普通用户不需要手动操作。

---

## 安全边界与已知限制

- **签名不防 GitHub 账号或签名密钥失陷**。它保证“下载到的字节与官方流水线签名时一致”，不保证上游发布流程绝对可信。
- **公钥固定在客户端内**。`sigilup-pins.json` 编译在客户端二进制中；轮换密钥需要先发布包含新公钥的客户端。
- **混合签名强制校验**：`sigilup` 对 Ed25519 与 ML-DSA-65 均执行强制校验，两者缺一不可。
- **不是 Windows Authenticode / Apple 公证**。limedl 使用 `sigilup` (Ed25519 + ML-DSA-65)，不等于商业代码签名证书。Windows SmartScreen 与 macOS Gatekeeper 仍可能提示，手动放行方式见[安装指南](/getting-started/installation/)。
- **无遥测**。客户端不包含分析或遥测上报；更新检查只访问 GitHub Releases 上的 `latest.hsig.json`。

---

## 报告安全问题

请优先通过 GitHub 仓库的 **Security → Report a vulnerability** 私密报告，避免在公开 Issue 中贴出可利用细节、密钥或私人下载链接。若私密入口不可用，请先提交一个不含敏感信息的问题，说明需要私下沟通。

---

## 相关文档

- [安装指南](/getting-started/installation/)
- [自更新通道](/advanced/self-update-channels/)
