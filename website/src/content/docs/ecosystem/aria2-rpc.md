---
title: Aria2 RPC 协议对接
description: 深入了解 limedl 支持的 Aria2 JSON-RPC 2.0 接口规范、方法列表、前端对接与代码调用示例
---

# Aria2 RPC 协议对接

limedl 实现了高兼容性的 **Aria2 JSON-RPC 2.0** 接口规范。借助于此，全网现存针对 Aria2 开发的数以千计的生态工具（如 AriaNg Web 控制台、各类自动化下载脚本、命令行管理工具）均可无缝切换指向 limedl，无需进行二次开发。

---

## 服务端连接端点与鉴权

limedl 在本地默认监听 `6800` 端口，并同时提供 HTTP POST 与 WebSocket 全双工通道：

- **HTTP POST 端点**：`http://127.0.0.1:6800/jsonrpc`
- **WebSocket 端点**：`ws://127.0.0.1:6800/jsonrpc`

### 身份鉴权机制
如果启用了 **RPC 密钥 (Secret Token)**，limedl 按 aria2 标准要求在 `params` 数组第一个元素传入字符串 `token:<你的密钥>`（所有方法都必须携带，包括 `system.multicall` 的每一层；嵌套调用可重复携带）。

> limedl 不支持通过 HTTP `Authorization` 标头做 RPC 鉴权（aria2 本身也不支持）；`header` 选项中的 `Authorization` 只作用于**下载请求**本身。

---

## 核心方法 API 参考列表

### 1. `aria2.addUri` — 添加网络下载任务
向引擎添加 HTTP/HTTPS 或磁力链接（BitTorrent Magnet）下载任务。磁力链接会被路由到 BT 后端；`.torrent` URL 按 aria2 语义仍作为普通文件下载（解析型种子请用 `aria2.addTorrent`）。

**请求示例**：
```json
{
  "jsonrpc": "2.0",
  "id": "req-001",
  "method": "aria2.addUri",
  "params": [
    "token:my_secret_token",
    ["https://example.com/archive.zip"],
    {
      "dir": "/home/user/downloads",
      "out": "renamed_archive.zip",
      "header": ["Cookie: session_id=12345", "Referer: https://example.com/"]
    }
  ]
}
```

**响应示例**：
```json
{
  "jsonrpc": "2.0",
  "id": "req-001",
  "result": "a1b2c3d4e5f60718"
}
```
*(返回 16 位十六进制 GID，由 XXH3(TaskId) 计算得出，重启后保持不变)*

---

### 2. `aria2.addTorrent` — 添加 BitTorrent 种子任务
上传以 Base64 编码的 `.torrent` 文件二进制数据。

**请求参数**：
- `params[0]`: 鉴权 Token（可选）
- `params[1]`: Base64 编码的种子内容字符串
- `params[2]`: 任务定制选项（可选）：`dir`、`out`、`pause`、`select-file`（1 起始的逗号分隔文件索引）

---

### 3. `aria2.getGlobalStat` — 获取全局实时状态
获取系统全局的下载/上传实时吞吐及任务统计。

**响应示例**：
```json
{
  "jsonrpc": "2.0",
  "id": "req-002",
  "result": {
    "downloadSpeed": "52428800",
    "uploadSpeed": "1048576",
    "numActive": "3",
    "numWaiting": "2",
    "numStopped": "18"
  }
}
```

---

### 4. `aria2.tellStatus` — 查询指定任务详情
获取特定任务的详细进度、已下载大小、分块位图等。

**请求示例**：
```json
{
  "jsonrpc": "2.0",
  "id": "req-003",
  "method": "aria2.tellStatus",
  "params": [
    "token:my_secret_token",
    "a1b2c3d4e5f60718",
    ["gid", "status", "totalLength", "completedLength", "downloadSpeed", "files"]
  ]
}
```
可选第三个参数 `keys` 用于只返回指定字段（limedl 已支持；BT 任务的 `files` 为真实文件列表，索引从 1 开始）。

---

### 5. 任务控制方法群

| 方法名 | 参数签名 | 作用说明 |
| :--- | :--- | :--- |
| `aria2.pause` | `([secret], gid)` | 暂停指定进行中的任务。 |
| `aria2.unpause` | `([secret], gid)` | 恢复指定已暂停的任务。 |
| `aria2.remove` | `([secret], gid)` | 取消并移除任务。 |
| `aria2.tellActive` | `([secret], [keys])` | 分页返回当前所有处于活跃传输状态的任务列表。 |
| `aria2.tellWaiting`| `([secret], offset, num, [keys])` | 分页查询等待队列中的任务。 |
| `aria2.tellStopped`| `([secret], offset, num, [keys])` | 分页查询已完成或已停止的历史任务（含被内存淘汰的任务）。 |
| `aria2.changeOption` | `([secret], gid, options)` | 运行期修改单个任务的选项：`pause`、BT 的 `select-file` / `max-download-limit` / `max-upload-limit`；其它选项会明确报错。 |
| `aria2.removeDownloadResult` | `([secret], gid)` | 删除单条已停止（完成/失败/已移除）的任务记录（保留文件）。 |
| `aria2.purgeDownloadResult`| `([secret])` | 清空所有已停止/已完成的任务历史记录。 |
| `aria2.getVersion` | `([secret])` | 查询 limedl 引擎版本号与已启用的功能列表。 |

---

## 对接 AriaNg Web 控制台

如果你需要远程管理或偏爱纯 Web 控制台（例如配合局域网内的下载服务器或 NAS）：

1. 访问公开托管的 [AriaNg 网页版](http://ariang.mayswind.net/)（或自行通过 Docker 本地部署 AriaNg）；
2. 点击左侧菜单底部的 **“AriaNg 设置”**，切换到 **“RPC”** 标签页；
3. 将 **Aria2 RPC 地址** 填写为运行 limedl 的主机 IP，端口填写 `6800`，协议选择 `WebSocket` 或 `HTTP`；
4. 若设置了 Token，在 **Aria2 RPC 密钥** 中填入密码；
5. 点击 **“重新加载 AriaNg”**。连接成功后，顶部状态将变为绿色，你可以立即在网页端实时监控和调控 limedl 的所有下载！

![AriaNg 连接状态截图](../../../assets/ariang-connection.png)

---

## 自动化代码调用示例

### 1. cURL 命令行添加任务
```bash
curl -X POST http://127.0.0.1:6800/jsonrpc \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "aria2.addUri",
    "params": [
      ["https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso"],
      {"dir": "/Downloads"}
    ]
  }'
```

### 2. Python 脚本调用
```python
import requests

payload = {
    "jsonrpc": "2.0",
    "id": "py-client",
    "method": "aria2.addUri",
    "params": [
        ["https://example.com/data.tar.gz"],
        {"out": "custom_filename.tar.gz"}
    ]
}

res = requests.post("http://127.0.0.1:6800/jsonrpc", json=payload)
print("Created GID:", res.json()["result"])
```

### 3. Node.js (Fetch) 调用
```javascript
const res = await fetch('http://127.0.0.1:6800/jsonrpc', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    jsonrpc: '2.0',
    id: Date.now().toString(),
    method: 'aria2.getGlobalStat',
    params: []
  })
});

const data = await res.json();
console.log('Current Download Speed:', data.result.downloadSpeed, 'B/s');
```
