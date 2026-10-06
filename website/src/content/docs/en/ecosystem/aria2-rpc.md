---
title: Aria2 RPC Integration
description: Complete JSON-RPC 2.0 API specifications supported by limedl, supported methods, AriaNg setup, and code snippets
---

# Aria2 RPC Integration

limedl implements a highly compliant **Aria2 JSON-RPC 2.0** protocol interface. Consequently, existing tools built for Aria2 (such as AriaNg, user scripts, and CLI utilities) work seamlessly with limedl without modifications.

---

## Server Endpoints & Authentication

limedl provides full HTTP POST and WebSocket duplex endpoints compliant with Aria2 (listening on port `6800` by default). For local machine security, **the RPC service is disabled by default on clean installations** to prevent unauthorized access. Enable it under **Settings → Aria2 RPC**:

- **HTTP POST**: `http://127.0.0.1:6800/jsonrpc`
- **WebSocket**: `ws://127.0.0.1:6800/jsonrpc`

### Authentication Mechanisms
limedl supports two authentication modes, passed as the first element of the RPC `params` array as `"token:<SECRET>"` per the aria2 standard (required on all calls, including nested calls in `system.multicall`):

1. **Shared Secret Token**: A single global passphrase configured in Settings.
2. **Per-Client Tokens**: Isolated tokens issued for specific clients (e.g. AriaNg, browser extensions), stored locally using strong **Argon2** password hashing.

> limedl does **not** support RPC authentication through the HTTP `Authorization` header (neither does aria2); an `Authorization` entry in the `header` download option only applies to the download request itself.

---

## Supported JSON-RPC 2.0 Methods

### 1. `aria2.addUri` — Add HTTP/HTTPS Download
Queues a new HTTP, HTTPS, or BitTorrent magnet task. Magnet links are routed to the BT backend; a `.torrent` URL is still downloaded as a plain file (use `aria2.addTorrent` for the parsed form).

**Sample Request**:
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

**Sample Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "req-001",
  "result": "a1b2c3d4e5f60718"
}
```
*(A 16-character hex GID computed as XXH3(TaskId); it survives restarts)*

---

### 2. `aria2.addTorrent` — Add BitTorrent Task
Submits Base64-encoded `.torrent` file bytes.

**Parameters**:
- `params[0]`: Authentication token (optional)
- `params[1]`: Base64 encoded string of torrent bytes
- `params[2]`: Optional web-seeding URLs
- `params[3]`: Options dictionary

---

### 3. `aria2.addMetalink` — Add Metalink Multi-Mirror Task
Uploads Base64-encoded `.metalink` or `.meta4` XML payload. limedl parses RFC 5854 / RFC 6249 metadata, performs lightweight mirror latency and range probing, and dynamically selects the best mirrors.

**Parameters**:
- `params[0]`: Authentication token (optional)
- `params[1]`: Base64-encoded Metalink XML string
- `params[2]`: Options dictionary (`dir`, `pause`, etc.)

**Sample Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "req-002",
  "result": ["a1b2c3d4e5f60718"]
}
```
*(Returns an array of created GIDs)*

---

### 4. `aria2.getGlobalStat` — Retrieve Global Stats
Fetches real-time global transfer rates and task counts.

**Sample Response**:
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

### 4. `aria2.tellStatus` — Inspect Single Task Status
Queries progress, downloaded byte count, and chunk bitfields.

**Sample Request**:
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
The optional third parameter `keys` restricts the returned fields (supported; BT task `files` contain the real file list, indices start at 1).

---

### 6. Task Control & Extended Methods

limedl implements all 36 standard Aria2 RPC methods plus `system.listNotifications`, and also provides the `aria2.multicall` alias (37 methods in total):

| Method | Parameters | Description |
| :--- | :--- | :--- |
| `aria2.addMetalink` | `([secret], metalink, [options], [pos])` | Parses and adds Metalink 4.0/3.0 multi-mirror task. |
| `aria2.pause` / `forcePause` | `([secret], gid)` | Pauses or force-pauses the specified active task. |
| `aria2.unpause` / `unpauseAll` | `([secret], gid)` | Resumes the specified paused task / all tasks. |
| `aria2.remove` / `forceRemove` | `([secret], gid)` | Cancels and removes the task. |
| `aria2.tellActive` | `([secret], [keys])` | Returns a list of currently active downloads. |
| `aria2.tellWaiting`| `([secret], offset, num, [keys])` | Returns tasks waiting in the queue. |
| `aria2.tellStopped`| `([secret], offset, num, [keys])` | Returns completed or stopped tasks (including results evicted from memory). |
| `aria2.changeOption` | `([secret], gid, options)` | Changes live task options (`pause`, BT `select-file`, rate limits). |
| `aria2.getOption` / `getGlobalOption` | `([secret], [gid])` | Queries task or global options (fully aligned with AriaNg key sets). |
| `aria2.changePosition` | `([secret], gid, pos, how)` | Reorders a waiting task in the queue. |
| `aria2.removeDownloadResult` | `([secret], gid)` | Removes a single stopped result, keeping its files. |
| `aria2.purgeDownloadResult`| `([secret])` | Clears finished tasks from history. |
| `aria2.getVersion` | `([secret])` | Returns engine version and enabled features (truthfully advertises `GZip`, `Brotli`, `Zstd`, `Metalink`, `BitTorrent`). |
| `system.multicall` / `aria2.multicall`| `([calls])` | Batches multiple RPC calls into a single round-trip. |

---

## Connecting AriaNg Web Dashboard

For web-based or remote administration:

1. Open the hosted [AriaNg Web Dashboard](http://ariang.mayswind.net/) (or your self-hosted Docker instance).
2. Go to **"AriaNg Settings"** in the sidebar, then the **"RPC"** tab.
3. Configure the **Aria2 RPC Address** with your machine's IP, port `6800`, and protocol `WebSocket` or `HTTP`.
4. Enter your secret token under **Aria2 RPC Secret** if configured.
5. Click **"Reload AriaNg"**. Once connected, the top status badge turns green, allowing you to manage limedl downloads directly in the browser!

![AriaNg Connection Screenshot](../../../../assets/ariang-connection.png)

---

## Code Examples

### 1. cURL Command
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

### 2. Python Script
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

### 3. Node.js (Fetch)
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
