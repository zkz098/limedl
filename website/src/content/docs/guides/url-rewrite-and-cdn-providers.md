---
title: URL 重写与自定义 CDN
description: 配置 limedl 的 URL 重写规则（Host/前缀/正则/通配符、PrefixProxy/Template）与 Cloudflare、Fastly、自定义 CDN Provider
---

# URL 重写与自定义 CDN

limedl 提供两种“换条更快的路”的能力：

- **URL 重写**：在任务派发前，把原始链接改写成镜像链接候选；
- **CDN 探针**：在 Cloudflare、Fastly 或自定义 CDN 网段内挑选更快的边缘 IP。

两者可以单独使用，也可以叠加。本文只讲配置字段与实际匹配行为。

---

## URL 重写

入口：**设置 → 实验室 → URL 重写**。

### 规则结构

每条规则包含：

| 字段 | 说明 |
| :--- | :--- |
| **启用** | 关闭的规则不参与匹配 |
| **匹配类型** | `Host`、`前缀`、`正则`、`通配符` |
| **匹配模式** | 与匹配类型对应的字符串 |
| **替换模式** | `前缀代理 (PrefixProxy)` 或 `模板 (Template)` |
| **目标列表** | 每个目标有 URL 模板、启用状态与顺序 |
| **URL 编码** | 把原始 URL 百分号编码后再拼进镜像地址 |
| **失败回退原链接** | 默认开启；所有目标之后追加原始 URL |
| **顺序** | 数字越小越先匹配；**第一条命中的规则生效**，后面的规则不再尝试 |

### 匹配类型

| 类型 | 行为 | 示例 |
| :--- | :--- | :--- |
| `Host` | 比较 URL 主机名；`*.example.com` 匹配子域 | `github.com` |
| `前缀` | `url.startsWith(pattern)` | `https://github.com/zkz098/` |
| `正则` | Rust `regex` 的 `is_match` | `^https://github\.com/([^/]+)/([^/]+)/releases/download/(.+)$` |
| `通配符` | `*` 任意序列、`?` 单字符 | `https://*.githubusercontent.com/*` |

### 替换模式

**前缀代理**：把目标模板作为前缀，后接原始 URL（可选百分号编码）。

```text
目标模板： https://mirror.example.com/
原始 URL： https://github.com/zkz098/limedl/releases/download/v0.6.1/app.zip
结果：     https://mirror.example.com/https://github.com/zkz098/limedl/releases/download/v0.6.1/app.zip
```

**模板**：正则规则用 `$1`、`$2` 引用捕获组；非正则规则可用 `{url}`（编码后）与 `{raw_url}`（原始）占位符。

```text
匹配类型： 正则
匹配模式： ^https://github\.com/([^/]+)/([^/]+)/releases/download/(.+)$
目标模板： https://mirror.example.com/https://github.com/$1/$2/releases/download/$3
```

### 行为细节

- 多个目标按顺序生成候选 URL，去重后依次尝试；`失败回退原链接` 会把原始 URL 追加到末尾。
- 规则按 `order` 排序，**只使用第一条命中的规则**；想让某个例外优先，把它的 `order` 调小。
- 重写发生在派发前，任务的文件名、校验和等元数据来自原始任务，不会因为换源而改变。
- URL 编码适合“镜像要求把完整 URL 作为路径参数”的服务；如果镜像不要求，保持关闭以免双重编码。
- **当前规则没有独立的请求头字段**：`Cookie`、`Referer`、`User-Agent` 等通过任务选项或浏览器扩展的 `header` 数组传入，而不是写在重写规则里。

### 排查

- 规则未生效：检查是否启用、`order` 是否被更靠前的规则抢先命中、匹配类型是否选错。
- 镜像返回 403：可能是 UA 反滥用或防盗链，见[排错文档](/guides/troubleshooting/)。
- 镜像内容不一致：优先保留校验和，或改用 Metalink 的哈希校验。

---

## CDN Provider

入口：**设置 → 实验室 → CDN 网络优选**。

| Provider | 说明 |
| :--- | :--- |
| **Cloudflare**（默认） | 内置官方 Anycast 网段与测速地址 |
| **Fastly** | 使用 Fastly 的静态网段与测速地址 |
| **自定义 (Custom)** | 填写测速 URL 与 IPv4/IPv6 CIDR 列表（每行一个） |

### 测速流程

1. 读取 Provider 的 IP 段；
2. 并发做 TCP/TLS 握手筛选；
3. 对候选做小分块真实吞吐测速；
4. 选出最优 IP 并写入传输层解析器。

连接时仍发送原始 TLS SNI 与 `Host` 头，证书校验保持合法。

### 限制与注意

- 测速会消耗约 **20–50 MB** 流量。
- 只对匹配所选 Provider 网段的域名生效；其他域名仍走普通 DNS。
- 如果最优候选**不比默认路由快**，limedl 会放弃加速并保留默认路由。
- 切换 Provider 后必须重新测速，旧节点不会自动迁移。
- 默认 DNS 解析到私有 IP（TUN/VPN 接管）时，测速会中止并提示冲突。
- 应用的节点会持久化；后续下载自动使用，直到你重新测速或清除。

---

## 推荐组合

1. 先用 `curl` 记录同 URL 的基线速度；
2. 目标属于 Cloudflare/Fastly：开启 CDN 探针并应用最优节点；
3. 有可靠镜像：添加 URL 重写规则，并保留校验和；
4. 同时有多个镜像：让 Metalink 负责多源聚合与逐片校验；
5. 仍然慢：检查代理、IPv6、运营商出口与镜像站 403。

---

## 相关文档

- [CDN 探针加速机制](/guides/cdn/)：状态机与 DNS 重写细节。
- [Metalink 与镜像选择](/advanced/metalink-and-mirrors/)：多源聚合与校验。
- [跨境与镜像加速](/use-cases/cross-border-mirrors/)：完整工作流。
- [配置手册](/getting-started/configuration/)：实验室与网络参数。
