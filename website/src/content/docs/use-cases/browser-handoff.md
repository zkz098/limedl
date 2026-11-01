---
title: 浏览器一键接管
description: 用 Aria2 RPC 与浏览器扩展，把网页下载自动交给 limedl，并安全地透传 Cookie、Referer 与 User-Agent
---

# 浏览器一键接管

## 适合谁

- 日常在 Chrome、Edge 或 Firefox 里点击下载链接；
- 希望大文件自动交给 limedl，小文件仍由浏览器直接保存；
- 需要网盘/论坛链接携带登录态与防盗链请求头。

工作方式是：浏览器扩展拦截下载，把任务通过 **Aria2 JSON-RPC 2.0** 推给本机 limedl。

```text
浏览器扩展 (Aria2 Explorer / Camtd / 油猴脚本)
        │  aria2.addUri / addTorrent (HTTP / WebSocket)
        ▼
   limedl 内置 RPC 服务 (127.0.0.1:6800)
        ▼
   HTTP / BitTorrent 引擎
```

---

## 三步配置

### 1. 开启 limedl RPC

打开 **设置 → Aria2 RPC**：

- 勾选 **启用 Aria2 RPC**（新安装默认关闭）；
- 确认端口 `6800`，或改成你需要的端口；
- 设置 **RPC 密钥 (Secret Token)**；
- 保持监听地址为 `127.0.0.1`，不要直接暴露到公网。

### 2. 安装浏览器扩展

推荐 **Aria2 Explorer**（Chrome / Edge / Firefox），也支持 Camtd 与各类油猴脚本，详见[浏览器接管教程](/ecosystem/browser/)。

### 3. 填写连接参数

在扩展设置中填入：

| 参数 | 值 |
| :--- | :--- |
| Host | `127.0.0.1` |
| Port | `6800`（与 limedl 设置一致） |
| Protocol | HTTP 或 WebSocket |
| Secret | 与 limedl 中的 Token 一致；未设置则留空 |

点击“测试连接”，看到连接成功即可。

---

## 调优拦截规则

- **最小文件大小**：设为 10–20 MB，避免图片、小 PDF 也弹窗；
- **扩展名白名单**：`zip, rar, 7z, tar, gz, iso, exe, msi, mp4, mkv, deb, dmg, pkg` 等；
- **临时放行/接管**：Aria2 Explorer 支持 `Alt` 强制使用浏览器原生下载、`Shift` 强制交给 limedl。

---

## Cookie、Referer 与防盗链

扩展会把当前标签页的 `Cookie`、`Referer`、`User-Agent` 作为 `header` 数组随 `aria2.addUri` 发送：

```json
{
  "header": [
    "Cookie: PHPSESSID=xxxx; auth_token=yyyy",
    "Referer: https://example.com/download-page",
    "User-Agent: Mozilla/5.0 ..."
  ]
}
```

limedl 会把它们带入分块请求，从而通过登录态与防盗链校验。实际效果取决于扩展/脚本是否正确采集这些请求头；部分网盘还需要专用解析脚本。

---

## 安全与排错

### 安全

- RPC **默认关闭**，且默认只监听 `127.0.0.1`；
- 为 RPC 设置 Token；桌面端还支持为不同客户端分配独立 Token；
- 不要将 6800 端口直接映射到公网；
- CORS 默认只允许本地 Origin，按需添加，不要长期使用 `*`。

### 连接失败

1. 确认 limedl 正在运行且 RPC 已启用；
2. 确认端口一致，且本机防火墙未拦截；
3. 确认 Secret 与扩展中填写的一致；
4. 尝试把协议从 HTTP 切换为 WebSocket，或反之；
5. 查看 limedl 日志中的数据目录 `logs/`。

### 任务报 401 / 403

- `401`：Token 不匹配，检查 `token:<secret>`；
- `403`：目标站点反爬或防盗链，检查 UA、Cookie、Referer，或改用其他镜像。

---

## 什么时候不适合

- 大量几十 KB 的小文件：交给浏览器更快，建议用最小文件大小过滤；
- 需要网页登录后动态签名的私有下载：取决于扩展/脚本能力，limedl 不内置站点解析；
- 不允许本机进程访问浏览器的环境：不要开启 RPC。

---

## 相关文档

- [浏览器下载接管](/ecosystem/browser/)：扩展下载链接与图文配置。
- [Aria2 RPC 对接](/ecosystem/aria2-rpc/)：方法、鉴权、CORS 与客户端示例。
- [安全与验签](/security/)：更新与发布资产的安全边界。
- [常见问题](/faq)：RPC 暴露风险、403 与平台限制。
