---
title: 跨境与镜像加速
description: 用 CDN 边缘探针、URL 重写、Metalink 多镜像与代理组合，加速 GitHub Releases、HuggingFace 与开源发行版下载
---

# 跨境与镜像加速

## 适合谁

- 经常下载 GitHub Releases、HuggingFace 模型、Linux 发行版镜像；
- 浏览器能打开链接，但下载速度只有几十 KB/s 或频繁中断；
- 不想依赖来历不明的“加速器”，希望自己控制镜像与校验。

这个场景组合使用四项能力：**CDN 探针**、**URL 重写**、**Metalink 多镜像**与**代理**。它们解决的是不同层面的问题，可以单独使用，也可以叠加。

---

## 第一步：先判断瓶颈

1. 用 `curl` 对同一 URL 测一次平均速度，作为基线；
2. 观察是**首字节慢**、**全程慢**，还是**周期性断流**；
3. 记录目标域名属于哪个 CDN（例如响应头中的 `server`、`cf-ray`、`x-served-by`）。

如果 `curl` 也很慢，问题通常在线路/DNS/镜像，而不是 limedl 的并发调度。

---

## 能力一：CDN 边缘探针

适用：目标资源位于 Cloudflare、Fastly 或你指定的自定义 CDN 网段。

原理：

1. 读取 Provider 的 IP CIDR 列表；
2. 并发做 TCP/TLS 握手筛选，留下低延迟候选；
3. 对候选做小分块真实吞吐测速；
4. 将最优 IP 写入传输层解析器，连接时仍发送原始 TLS SNI 与 `Host` 头。

使用位置：**设置 → 实验室 → CDN 网络优选**。

注意：

- 测速本身会消耗约 **20–50 MB** 流量；
- 只对匹配所选 Provider 网段的域名生效，其余域名仍走普通 DNS；
- 如果最优候选并不比默认路由快，limedl 会放弃加速并保留默认路由；
- 切换 Provider 后需要重新测速，旧节点不会自动迁移；
- 如果默认 DNS 解析到私有 IP（例如 TUN/VPN 接管），测速会中止并提示可能冲突。

详见 [CDN 探针加速机制](/guides/cdn/)。

---

## 能力二：URL 重写

适用：目标资源不在所选 CDN 网段，但有稳定的公共镜像。

在 **设置 → 实验室 → URL 重写** 中添加规则。GitHub Releases 的经典写法：

```regex
^https://github\.com/([^/]+)/([^/]+)/releases/download/(.+)$
```

替换为：

```text
https://mirror.example.com/https://github.com/$1/$2/releases/download/$3
```

- 规则支持 Host、前缀、正则与通配符匹配；
- 可以配置前缀代理/替换模板、URL 编码与失败回退（规则本身不含请求头字段）；
- 重写发生在任务派发前，原始 URL、文件名与校验信息应一并保留。

**务必核对镜像内容与原站一致**。镜像站可能缓存旧版本、返回错误页或注入内容；优先使用带校验和的任务，或在下载后执行完整性校验。

---

## 能力三：Metalink 多镜像

适用：资源本身提供多个镜像（`.metalink` / `.meta4`，或响应头中的 Metalink）。

limedl 支持 RFC 5854 / RFC 6249，会：

- 按优先级与 RTT 对镜像打分；
- 动态租约最优镜像，连接数受限；
- 对损坏分片隔离镜像并降低其评分；
- 下载完成后按 Metalink 声明的 Blake3 / SHA-256 / SHA-512 逐片或整体校验。

把 `.metalink` / `.meta4` 拖入窗口，或通过 Aria2 RPC `addMetalink` 提交即可。

详见 [任务管理与 Metalink](/guides/tasks/)。

---

## 能力四：代理

如果瓶颈来自本地网络出口，而不是 CDN 路由：

| 代理模式 | 行为 |
| :--- | :--- |
| 直连 | 不使用代理 |
| 系统代理 | 读取操作系统静态代理配置（Windows Internet 选项 / macOS 网络设置）；**不支持 PAC/WPAD** |
| 手动代理 | 填写完整 URL，例如 `socks5://127.0.0.1:1080` |

代理同样作用于更新检查；更新请求的 UA 会固定为 `limedl-native/<version>`。

---

## 推荐组合

1. **先测基线**：`curl` 记录平均速度；
2. **目标属于 Cloudflare/Fastly**：开启 CDN 探针，测速后应用最优节点；
3. **不在 CDN 网段但有可靠镜像**：添加 URL 重写规则，并保留校验和；
4. **资源提供 Metalink**：优先用 Metalink，让引擎自动选镜像与校验；
5. **仍然慢**：检查代理、IPv6、运营商出口与镜像站 403（UA 反滥用）。

---

## 检查清单

- [ ] 已用 `curl` 记录同 URL 基线。
- [ ] CDN 探针只对匹配 Provider 网段的域名生效。
- [ ] URL 重写镜像与原站内容一致，并已开启校验。
- [ ] Metalink 声明的哈希算法受支持（Blake3 / SHA-256 / SHA-512）。
- [ ] 代理地址与模式正确；系统代理不支持 PAC。
- [ ] 镜像站 403 时先调整 UA，再考虑更换镜像。

---

## 相关文档

- [CDN 探针加速机制](/guides/cdn/)：状态机、Provider 与 DNS 重写细节。
- [任务管理与并发](/guides/tasks/)：AIMD、Metalink 与完整性校验。
- [配置手册](/getting-started/configuration/)：网络、代理与实验室参数。
- [常见问题与故障排查](/guides/troubleshooting/)：403、IPv4/IPv6 与日志定位。
