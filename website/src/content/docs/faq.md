---
title: 常见问题
description: limedl 与 Aria2/Motrix 的区别、遥测、SmartScreen、RPC 安全、数据备份、平台限制与升级方式
---

# 常见问题

## limedl 和 Aria2、Motrix、迅雷有什么区别？

limedl 不是 Aria2 的图形外壳，也不是 Electron 封装：它用 Rust 实现了完整的 HTTP 分块下载、AIMD 自适应并发、BitTorrent（librqbit）、Metalink 镜像调度与磁盘缓冲池，桌面界面使用 Slint 原生渲染。它**兼容** Aria2 JSON-RPC 2.0，因此可以接入 AriaNg、Aria2 Explorer、Camtd 等现成前端，但内核与 Aria2 是两套实现。

与商业下载器相比，limedl 不含广告、会员限速或遥测，协议为 GPL-3.0。

## 能直接替代我正在用的 Aria2 吗？

大多数日常场景可以：开启内置 Aria2 RPC 后，把前端指向 `http://127.0.0.1:6800/jsonrpc` 并填入 Token 即可。当前实现覆盖 AriaNg 等前端常用的 37 个方法，包括 `addUri`、`addTorrent`、`tellStatus`、`getGlobalStat`、队列操作与 `system.multicall`。

但它不是 Aria2 所有选项的逐项克隆：未路由的方法会返回 method-not-found，部分 Aria2 专有配置项没有对应开关。迁移前请阅读 [Aria2 RPC 对接](/ecosystem/aria2-rpc/) 中的支持范围与差异说明。

## limedl 会收集遥测或要求登录账号吗？

不会。客户端不包含分析、埋点或遥测上报，也不需要账号。唯一的网络请求是更新检查：访问 GitHub Releases 上的 `latest-native.json` 及其签名；更新请求会遵循你在设置中配置的代理。

## 为什么 Windows 会弹出 SmartScreen 或杀毒软件警告？

limedl 使用 Minisign + ML-DSA-65 对发布资产签名，但**没有购买 Windows Authenticode 商业代码签名证书**，因此 SmartScreen 仍可能提示“未知发布者”。这不是文件被篡改的证据，也不代表安装包一定安全。

建议先核对 GitHub Release 显示的 SHA-256 摘要，再按[安全与验签](/security/)手动验证 `.sig` / `.pqc.sig`。如果你不能接受这个风险，可以从源码自行构建。

## macOS 提示“无法验证开发者”或“文件已损坏”怎么办？

macOS 构建使用 ad-hoc 签名且未做 Apple 公证，Gatekeeper 会拦截首次启动。任选一种方式放行：

- 在访达中按住 `Control` 右键点击 `limedl.app`，选择“打开”，再确认“打开”；
- 或执行 `xattr -cr /Applications/limedl.app` 移除隔离属性。

详见[安装指南](/getting-started/installation/)。

## 支持 Intel Mac、Windows ARM 或 Linux ARM 桌面吗？

不支持。桌面端只提供 Windows x86_64、macOS Apple Silicon 与 Linux x86_64 三种构建；没有 Intel Mac、Windows ARM64 原生包或 Linux ARM64 桌面版。

如果目标是 ARM 设备做无头下载，可以使用 `limedl-server` 的 aarch64 musl 构建。完整矩阵见[平台支持](/getting-started/platform-support/)。

## 设置、任务记录和种子状态保存在哪里？

默认数据目录：

| 平台 | 路径 |
| :--- | :--- |
| Windows | `%LOCALAPPDATA%\limedl\` |
| macOS | `~/Library/Application Support/limedl/` |
| Linux | `~/.local/share/limedl/` |

可通过环境变量 `LIMEDL_DATA_DIR` 覆盖。备份时请先退出应用，再复制整个目录；只复制 `settings.json` 会丢失任务数据库与种子状态。

## 如何升级、回滚或迁移到新机器？

- **升级**：Setup 与便携版支持应用内更新；MSIX 通过 Microsoft Store；AppImage 与 `.deb` 请下载新包替换或交给包管理器。
- **回滚**：可以从 GitHub Releases 安装旧版本，但新版本可能已经迁移设置或数据库结构。回滚前务必备份整个数据目录。
- **迁移**：退出应用后复制数据目录到新机器，并把 `defaultDownloadDir` 等绝对路径改成新机器的路径；下载目录本身需要另行拷贝。

## 把 Aria2 RPC 暴露到局域网/公网安全吗？

默认只监听 `127.0.0.1:6800` 且默认关闭，这是最安全的配置。`limedl-server` 在绑定非回环地址时**强制要求**设置 Token，否则拒绝启动。

不要把 6800 端口直接暴露到公网。推荐保持回环监听，用 Caddy/Nginx 反向代理 `/jsonrpc` 并提供 TLS；同时为每个客户端使用独立 Token，并限制 CORS Origin。详见[无头服务端部署](/ecosystem/server-daemon/) 与 [Aria2 RPC 对接](/ecosystem/aria2-rpc/)。

## 下载镜像站报 403 Forbidden 怎么办？

部分高校/开源镜像站会按 `User-Agent` 做反滥用校验。把设置中的默认 UA 改成当前稳定版 Chrome UA，或改成合法的 `limedl/<version>` 标识；如果运营商 IPv4 出口被整体限流，尝试 IPv6 或换用其他镜像。

更多排查步骤见[常见问题与故障排查](/guides/troubleshooting/)。

## BitTorrent 速度为 0 / 连不上 Peer？

先确认种子是否还有做种用户；再检查 DHT 节点数是否在 1–2 分钟内升到 100+。若长期为 0，检查防火墙是否放行 limedl 的 UDP 流量，并在路由器开启 UPnP 端口映射。冷门资源可以补充公共 Tracker 列表。

详见[BitTorrent 调优](/guides/bittorrent/) 与[排错文档](/guides/troubleshooting/)。

## 支持 HTTP/3 吗？

支持。客户端默认启用 HTTP/3 Alt-Svc 探测：对声明 Alt-Svc 的主机尝试 QUIC，失败会熔断并回退 TCP。该开关位于 `settings.json` 的 `scheduler.http3AltSvcEnabled`，不在设置对话框中；使用前请确保本机 UDP 443 可用。

## 能直接下载网盘文件吗？

limedl 没有内置网盘账号协议或解析逻辑。你可以通过 Aria2 浏览器扩展/油猴脚本，把带有 `Cookie`、`Referer` 等请求头的任务派发给 limedl；能否成功取决于脚本与网盘服务本身。请遵守对应服务的使用条款。

## limedl 是开源的吗？

是，协议为 GPL-3.0。源码、Issue 与发布归档都在 [GitHub](https://github.com/zkz098/limedl)。你也可以按[源码编译与开发](/advanced/development/)自行构建。

---

## 没找到答案？

- [常见问题与故障排查](/guides/troubleshooting/)：403、BT、Gatekeeper、Linux 对话框与托盘、日志定位。
- [安全与验签](/security/)：签名、公钥与更新信任链。
- [GitHub Issues](https://github.com/zkz098/limedl/issues)：提交问题时请附系统版本、limedl 版本、任务类型与脱敏后的日志。
