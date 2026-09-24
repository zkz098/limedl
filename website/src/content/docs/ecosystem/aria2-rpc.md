---
title: Aria2 RPC 协议对接
description: 了解 limedl 支持的 Aria2 JSON-RPC 2.0 接口以及与 AriaNg / 脚本的集成
---

# Aria2 RPC 协议对接

limedl 实现了广泛兼容的 **Aria2 JSON-RPC 2.0** 接口规范。你可以将现有的任何 Aria2 客户端生态无缝指向 limedl。

## 支持的常用 RPC 方法

- `aria2.addUri([secret], [uris], [options])`：添加 HTTP/HTTPS 任务
- `aria2.addTorrent([secret], torrent_base64, [uris], [options])`：添加种子任务
- `aria2.getGlobalStat([secret])`：获取全局实时下载/上传速率
- `aria2.tellStatus([secret], gid, [keys])`：查询任务进度与块状态
- `aria2.pause([secret], gid)` / `aria2.unpause([secret], gid)`：暂停与恢复任务
- `aria2.remove([secret], gid)`：删除任务

## 对接 AriaNg Web 控制台

如果你更偏爱 Web 控制台（例如局域网远程管理或配合 NAS 场景）：
1. 访问任意托管版 [AriaNg](http://ariang.mayswind.net/) 或本地部署的 AriaNg。
2. 在 AriaNg 设置中，将 **RPC 地址** 设置为运行 limedl 的机器 IP，端口填写 `6800`。
3. 连接成功后即可在 Web 浏览器内远程管理 limedl 的所有下载任务。

![AriaNg 连接状态截图](../../../assets/ariang-connection.png)
