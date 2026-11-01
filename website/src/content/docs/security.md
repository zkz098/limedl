---
title: 安全与验签
description: limedl 发布资产的 Minisign + ML-DSA-65 双签名、更新信任链、手动验签步骤与安全边界
---

# 安全与验签

limedl 的桌面发布资产使用 **Minisign (Ed25519)** 与 **ML-DSA-65 (NIST FIPS 204 后量子密码学)** 双签名。应用内更新会先验证 `latest-native.json` 的签名，再解析清单并下载资产，最后验证资产本身的签名。

本文说明哪些内容被签名、如何手动复核，以及这套机制**不能**防住什么。

---

## 信任链一览

```text
GitHub Release
├── latest-native.json            # 更新清单
├── latest-native.json.sig        # Minisign (Ed25519)，强制校验
├── latest-native.json.pqc.sig    # ML-DSA-65
└── 桌面资产 + .sig / .pqc.sig     # 安装包、便携包、MSIX、deb、AppImage

客户端内置公钥
├── PUBKEY_B64       # Minisign / Ed25519，始终强制
└── PQC_PUBKEY_B64   # ML-DSA-65，配置后启用
```

- **清单先验签、后解析**：签名不通过时，清单内容不会影响任何下载 URL。
- **资产签名内嵌在清单中**：`latest-native.json` 为每个资产保存 `signature` 与可选的 `pqcSignature`；客户端下载完成后对原始字节再验一次。
- **体积上限**：更新下载限制为 512 MiB，异常清单无法让客户端无限拉取。
- **域名分离**：ML-DSA-65 对清单使用 `limedl-manifest` 上下文，对资产使用 `limedl-artifact` 上下文，避免签名被跨用途重放。

### 哪些资产被签名

| 资产 | 签名 | 说明 |
| :--- | :--- | :--- |
| Windows Setup.exe / 便携 zip | `.sig` + `.pqc.sig`，并内嵌在清单 | 自更新资产 |
| macOS `.app` `.tar.gz` | `.sig` + `.pqc.sig`，并内嵌在清单 | 自更新资产 |
| Linux 便携 `.tar.gz` | `.sig` + `.pqc.sig`，并内嵌在清单 | 自更新资产 |
| Windows MSIX | `.sig` + `.pqc.sig` | 独立资产，不在自更新清单内 |
| Linux `.deb` / `.AppImage` | `.sig` + `.pqc.sig` | 独立资产，不在自更新清单内 |
| `latest-native.json` | `.sig` + `.pqc.sig` | 清单本身 |
| `limedl-server` 二进制 / Docker 镜像 | **不签名** | 不参与桌面自更新信任链 |

:::note[历史版本]
极少数旧版本的某些资产可能没有单独的 `.sig` / `.pqc.sig` 文件；这些资产的签名仍内嵌在 `latest-native.json` 中，客户端校验不受影响。手动复核时请以该版本实际发布的资产为准。
:::

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

### 2. Minisign (Ed25519)

官方 Minisign 公钥：

```text
RWSe1Ow8aYLL4Iy0eRFN+40umqxd2kxlPoyg1AqJdPBblBMSMDOhKw+7
```

验签时用 `-x` 指定 `.sig` 文件：

```bash
minisign -Vm limedl-native-v0.6.1-windows-x86_64-setup.exe \
  -x limedl-native-v0.6.1-windows-x86_64-setup.exe.sig \
  -P "RWSe1Ow8aYLL4Iy0eRFN+40umqxd2kxlPoyg1AqJdPBblBMSMDOhKw+7"
```

输出 `Signature and comment signature verified` 表示文件由官方流水线签名且未被修改。

验证更新清单同理：

```bash
minisign -Vm latest-native.json -x latest-native.json.sig \
  -P "RWSe1Ow8aYLL4Iy0eRFN+40umqxd2kxlPoyg1AqJdPBblBMSMDOhKw+7"
```

### 3. ML-DSA-65（后量子）

官方 ML-DSA-65 公钥以标准 base64 发布在 [`/limedl-pqc-pubkey.b64`](/limedl-pqc-pubkey.b64)，解码后为 1952 字节原始公钥。

`.pqc.sig` 的格式为一行 `untrusted comment: ml-dsa-65 signature`，后跟一行 base64 签名。复核方式：

- 对下载文件的**原始字节**执行 FIPS 204 ML-DSA-65 验证；
- 资产使用上下文 `limedl-artifact`；
- `latest-native.json` 使用上下文 `limedl-manifest`。

客户端在更新时会自动完成这一步，普通用户不需要手动操作。

---

## 安全边界与已知限制

- **签名不防 GitHub 账号或签名密钥失陷**。它保证“下载到的字节与官方流水线签名时一致”，不保证上游发布流程绝对可信。
- **没有密钥滚动链**。公钥编译在客户端内；轮换密钥需要先发布包含新公钥的客户端。旧客户端不会接受新密钥签名的更新。
- **ML-DSA-65 的强制程度**：当前客户端对 `latest-native.json` **强制校验 Ed25519**；ML-DSA-65 公钥已内置并会校验 PQC 签名。若某个清单缺少 `.pqc.sig`，客户端会记录警告并仅以 Ed25519 继续，而不是拒绝更新。
- **不是 Windows Authenticode / Apple 公证**。limedl 使用 Minisign + ML-DSA-65，不等于商业代码签名证书。Windows SmartScreen 与 macOS Gatekeeper 仍可能提示，手动放行方式见[安装指南](/getting-started/installation/)。
- **服务端资产不签名**。`limedl-server` 的 tar.gz 与容器镜像不在自更新信任链内，请使用 GitHub Release 的 SHA-256 摘要核对。
- **无遥测**。客户端不包含分析或遥测上报；更新检查只访问 GitHub Releases 上的 `latest-native.json` 及其签名。

---

## 报告安全问题

请优先通过 GitHub 仓库的 **Security → Report a vulnerability** 私密报告，避免在公开 Issue 中贴出可利用细节、密钥或私人下载链接。若私密入口不可用，请先提交一个不含敏感信息的问题，说明需要私下沟通。

---

## 相关文档

- [安装指南](/getting-started/installation/)：各平台安装与手动验签命令。
- [平台支持](/getting-started/platform-support/)：签名覆盖的平台与更新通道。
- [系统架构](/advanced/architecture/)：自更新与签名机制的实现细节。
- [更新日志](/changelog)：安全相关修复。
