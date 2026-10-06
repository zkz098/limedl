---
title: 浏览器下载接管
description: 如何在 Chrome、Edge 和 Firefox 等主流浏览器中无缝拦截网页下载并调用 limedl
---

# 浏览器下载接管

主流现代浏览器（Chrome、Edge、Firefox）内置的下载器大多采用单线程顺序拉取，不仅无法跑满宽带，且遭遇网络波动或页面关闭时极易中断报错，更无法直接处理磁力链接与 BT 种子。

借助 limedl 内置的 **Aria2 JSON-RPC 2.0 服务端**，你可以搭配浏览器商店中成熟的 Aria2 接管扩展，实现**网页点击任意下载链接自动唤醒 limedl 原生下载**。

---

## 推荐的浏览器扩展程序

| 扩展名称 | 适用浏览器 | 特点与推荐度 |
| :--- | :--- | :--- |
| **Aria2 Explorer** | Chrome / Edge / Firefox | **强烈推荐**。功能最全面，支持文件后缀过滤、右键菜单拦截、快捷键强行接管/放行，具备完整的连接测试工具。 |
| **Camtd** | Chrome / Edge | 极简风格，界面轻量纯净，适合只需静默后台拦截的用户。 |
| **Tampermonkey 油猴脚本** | 全平台主流浏览器 | 配合各类网盘下载助手脚本（百度网盘、123云盘、夸克等），一键将私有文件解析并直推至 limedl。 |

---

## 3 步完成配置

### 步骤 1：开启 limedl RPC 服务
1. 打开 limedl 桌面客户端，点击左侧导航栏的 **设置** 图标；
2. 切换至 **Aria2 RPC** 设置项：
   - 勾选 **启用 Aria2 RPC**（新安装默认关闭以保障安全）；
   - 查看或修改 **RPC 端口**（默认为 `6800`）；
   - 设置 **RPC 密钥 (Secret Token)**：推荐设置自定义密码或配置客户端 Token，保障接口安全。

### 步骤 2：在浏览器中安装扩展
前往你所使用浏览器的官方扩展商店，搜索并安装 **Aria2 Explorer**：
- [Chrome 网上应用店获取](https://chromewebstore.google.com/detail/aria2-explorer/mpkodccbngfoacfalldjimigbofkhgjn)
- [Edge 外接程序获取](https://microsoftedge.microsoft.com/addons/detail/aria2-explorer/jjfgljkagddikjhjnamjfdhobaomflbe)
- [Firefox 附加组件获取](https://addons.mozilla.org/zh-CN/firefox/addon/aria2-explorer/)

### 步骤 3：在扩展中填入连接参数
安装完成后，点击浏览器右上角的扩展图标，进入“选项 / 设置”页：
- **主机 (Host)**：`127.0.0.1` 或 `localhost`
- **端口 (Port)**：`6800`（需与 limedl 设置一致）
- **协议 (Protocol)**：选择 `WebSocket` 或 `HTTP`
- **密钥 (Secret)**：若 limedl 中设置了 Token 则填入，否则留空。

![Aria2 扩展设置截图](../../../assets/browser-extension-setting.png)

点击底部的 **“测试连接”**。当状态指示灯变为绿色并显示“连接成功”时，即表示配置大功告成！

---

## 高级接管规则调优

在 Aria2 Explorer 的设置中，你可以进一步按需定制拦截策略，避免小文件频繁弹窗：

### 1. 过滤小文件下载
- 设置 **“最小下载文件大小”** 为 `10 MB` 或 `20 MB`；
- 浏览器在下载几 KB 的图片、PDF 文档或小文本时将由浏览器自身直接保存，只有大体积文件才会触发 limedl 极速拉取。

### 2. 按文件扩展名智能拦截
你可以配置触发拦截的文件扩展名白名单：
```text
zip, rar, 7z, tar, gz, iso, exe, msi, mp4, mkv, flv, mov, dmg, pkg, deb, rpm
```

### 3. 临时快捷键放行 / 强制接管
Aria2 Explorer 支持快捷键逻辑：
- **按住 `Alt` 键点击下载链接**：强制绕过扩展，使用浏览器原生下载；
- **按住 `Shift` 键点击下载链接**：强行调用 limedl 接管（即使该文件类型不在规则列表中）。

---

## 自动传递 Cookie 与 Referer 防盗链

许多网盘、论坛或私有下载站设置了严格的防盗链与会话鉴权：要求必须附带当前登录状态的 `Cookie` 与来源页面 `Referer`。

当通过 Aria2 扩展接管时，扩展会自动提取当前标签页的 `Cookie`、`User-Agent` 与 `Referer`，并以标准 JSON 数组打包随 `aria2.addUri` 派发给 limedl：
```json
{
  "header": [
    "Cookie: PHPSESSID=xxxxxx; auth_token=yyyyyy",
    "Referer: https://example.com/download-page",
    "User-Agent: Mozilla/5.0 ..."
  ]
}
```
limedl 的 HTTP 执行器会自动解析这些 Header 并在分块请求中附带，确保会员权限、防盗链资源百分之百下载成功，绝不报 401 Unauthorized 或 403 Forbidden 错误。
