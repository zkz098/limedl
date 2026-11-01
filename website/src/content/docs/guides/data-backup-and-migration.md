---
title: 数据备份、迁移与 Aria2 迁移
description: limedl 数据目录结构、备份恢复、跨机器迁移、便携模式，以及从 Aria2/Motrix 迁移时的选项映射与限制
---

# 数据备份、迁移与 Aria2 迁移

下载任务、分块进度、种子状态和设置都保存在本地数据目录。升级、回滚或换机器前，先理解哪些文件必须一起备份。

---

## 数据目录结构

默认根目录：

| 平台 | 路径 |
| :--- | :--- |
| Windows | `%LOCALAPPDATA%\limedl\` |
| macOS | `~/Library/Application Support/limedl/` |
| Linux | `$XDG_DATA_HOME/limedl/` 或 `~/.local/share/limedl/` |
| 覆盖 | `LIMEDL_DATA_DIR` 优先级最高 |

典型结构：

```text
<数据根目录>/
├── settings.json              # 应用设置
├── settings.json.bak          # 最近一次成功保存的备份（尽力而为）
├── settings.json.corrupt      # 无法解析时被移到这里
├── downloads/
│   ├── downloads.db           # 任务、分块进度、BT 元数据（SQLite）
│   ├── logs/
│   │   ├── limedl.log
│   │   └── crash.log
│   └── torrents/              # BT 会话/resume 状态
└── update/                    # 更新暂存目录，启动时会清理
```

:::caution[日志路径]
日志在 `<数据根目录>/downloads/logs/`，不是根目录下的 `logs/`。旧版本文档或帖子里的路径可能已经过时。
:::

### 设置的写入方式

- 保存时先写 `settings.json.tmp`，再原子重命名为 `settings.json`。
- 每次成功保存会尽量写一份 `settings.json.bak`。
- 启动时如果 `settings.json` 无法解析，会被移到 `settings.json.corrupt`，再尝试 `.bak`。

---

## 备份与恢复

### 备份

1. **完全退出 limedl**（托盘也要退出），不要只关闭主窗口；
2. 复制整个数据根目录，而不是只复制 `settings.json`；
3. 备份期间不要运行下载任务，避免 SQLite 处于写入中途。

只备份 `settings.json` 会丢失任务数据库、分块进度、BT 会话与日志。

### 恢复

1. 退出应用；
2. 把备份目录整体放回原路径；
3. 如果换了机器或用户名，修改 `settings.json` 中的绝对路径（`defaultDownloadDir`、目录介质覆盖等）；
4. 启动后检查任务列表与下载目录。

### 重置

不要把旧目录直接删掉。先重命名为 `limedl-old` 再启动，确认新数据正常后再决定是否删除；这样出问题时还能换回来。

---

## 跨机器迁移

- 数据目录中的路径大多是**绝对路径**；迁移到新机器后需要改成本机路径。
- 下载目录里的文件不会自动打包；请单独复制已完成文件。
- SQLite 数据库可能记录原绝对路径。跨平台迁移（例如 Windows → Linux）不保证任务能直接续传，必要时重新添加任务。
- `LIMEDL_DATA_DIR` 可以指向移动硬盘实现便携运行；但请用稳定的绝对路径，避免快捷方式改变了工作目录导致数据“消失”。

---

## 便携模式

- 设置 `LIMEDL_DATA_DIR` 指向 U 盘/移动硬盘上的目录即可把设置、数据库、BT 状态一起带走。
- 下载目录仍由 `defaultDownloadDir` 决定；便携化不会自动把下载文件也放进 `LIMEDL_DATA_DIR`。
- 退出应用后再拔出移动盘，避免 SQLite 写入中断。

---

## 从 Aria2 / Motrix 迁移

limedl **不解析** `aria2.conf` 或 `aria2.session`，也没有一键导入；但兼容 Aria2 JSON-RPC 2.0，迁移主要分两步：

1. 在 limedl 中开启 **Aria2 RPC**（默认关闭），设置 Token 与端口；
2. 把 AriaNg、Aria2 Explorer、Motrix 等前端指向 `http://127.0.0.1:6800/jsonrpc`，重新提交任务。

### 常用选项映射

| Aria2 选项 | limedl 行为 |
| :--- | :--- |
| `dir` | 任务下载目录；必须是绝对路径 |
| `out` | 重命名输出文件 |
| `pause` | 以暂停状态创建任务 |
| `select-file` | BT/Metalink 的文件索引（1 起始） |
| `split` | 任务线程数（会按 limedl 的线程上限收敛） |
| `header` / `referer` / `user-agent` | 下载请求头与 UA |
| `checksum` | 映射到校验算法与期望值 |
| `max-concurrent-downloads` | 传统模式改并行任务数；自动模式改线程预算 |
| `max-overall-download-limit` / `max-overall-upload-limit` | 全局下载/上传限速 |
| `max-tries` | 最大重试次数（有上限） |
| `enable-dht` / `bt-max-peers` / `seed-ratio` / `seed-time` | 映射到 BT 设置 |

`max-concurrent-downloads`、`max-overall-*`、`max-tries` 与 BT 相关键属于**全局选项**，通过 `aria2.changeGlobalOption` 生效；`dir`、`out`、`pause`、`select-file`、`split`、`header` 等属于任务级选项。**未逐项实现的选项会被忽略**，例如部分 Aria2 专有调度/会话选项。迁移前请阅读 [Aria2 RPC 对接](/ecosystem/aria2-rpc/) 的兼容范围。

### 不能直接迁移的内容

- `aria2.session` 中的活动任务不会被导入；请通过前端重新添加，或让原 Aria2 完成任务。
- Aria2 的 DHT/torrent 缓存、`--save-session` 历史不会自动复用。
- Aria2 的 RPC 密钥格式与 limedl 的 Token 机制相似但不完全等同；请重新配置前端。

---

## 相关文档

- [Aria2 RPC 对接](/ecosystem/aria2-rpc/)：方法、鉴权与客户端配置。
- [自更新通道与回滚](/advanced/self-update-channels/)：升级/回滚前的备份注意。
- [磁盘介质探测与缓冲调优](/advanced/disk-media-and-buffer-tuning/)：目录覆盖与路径规范化。
- [常见问题](/faq)：数据目录、升级与平台限制。
