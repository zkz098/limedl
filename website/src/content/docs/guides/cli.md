---
title: 命令行与协议关联
description: 掌握 limedl 的命令行调用参数、静默启动、单实例 IPC 机制与原生协议关联集成
---

# 命令行与协议关联

虽然 limedl 是一款原生桌面图形化客户端，但其底层具备完备的命令行入口（CLI）与操作系统级协议挂钩（Deep Links）。

你可以通过终端脚本、自动化任务、外部下载工具或第三方程序直接调用 limedl 派发下载任务。

---

## 可执行程序路径

在不同操作系统中，可执行程序通常位于以下位置：

- **Windows**:
  - 安装版：`C:\Users\<用户名>\AppData\Local\Programs\limedl-native\limedl-native.exe`
  - 便携版：解压目录下的 `limedl-native.exe`
- **macOS**:
  - `/Applications/limedl.app/Contents/MacOS/limedl-native`
- **Linux**:
  - 便携包：解压后的 `./limedl-native`
  - AppImage：`./limedl-native-v0.5.0-linux-x86_64.AppImage`
  - DEB 安装后：系统全局命令 `limedl-native`（软链至 `/usr/bin/limedl-native`）

---

## 命令行参数语法

```bash
limedl-native [OPTIONS] [PAYLOAD]
```

### 支持的选项 (Options)

| 参数标志 | 说明 | 典型应用场景 |
| :--- | :--- | :--- |
| `--hidden` | 静默启动。直接最小化至系统托盘，不弹出主窗口。 | 配合操作系统开机启动项、自动化常驻服务或 NAS 后台挂机。 |
| `-h`, `--help` | 打印命令行帮助信息。 | 快速查阅支持的参数列表。 |
| `-V`, `--version` | 输出当前客户端版本号。 | CI 脚本或自动化工具核对版本。 |

### 支持的载荷类型 (Payload)

你可以直接在命令末尾附加任意合法的下载目标：

#### 1. 提交 HTTP/HTTPS 下载链接
```powershell
# Windows
limedl-native.exe "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso"

# Linux
limedl-native "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso"
```

#### 2. 提交 Magnet 磁力链接
```bash
limedl-native "magnet:?xt=urn:btih:d2b0e9a72c38e21e64c81979360cb53527655f02&dn=Ubuntu"
```

#### 3. 提交本地 .torrent 种子文件路径
```bash
limedl-native "D:\Downloads\linuxmint-22-cinnamon-64bit.iso.torrent"
```

#### 4. 提交本地 .metalink / .meta4 多镜像文件路径
```bash
limedl-native "D:\Downloads\fedora-workstation.meta4"
```

#### 5. 提交 limedl:// 原生深度链接
```bash
limedl-native "limedl://download?url=https://example.com/file.zip&filename=file.zip"
```

---

## 单实例机制与 IPC 瞬间接管 (Single Instance IPC)

当你通过终端多次执行 `limedl-native` 命令时，系统**绝不会启动多个互相冲突的进程**。

limedl 内置了基于平台特性的高可靠单实例与进程间通信（IPC）机制：
- **Windows 平台**：
  采用操作系统级命名互斥体（Named Mutex）识别是否已有实例运行。若检测到已有主进程，新进程会通过 Win32 `WM_COPYDATA` 消息通道将命令载荷毫秒级传输给主窗口，并唤醒聚焦主界面，随后新进程立即退出。
- **macOS 与 Linux 平台**：
  采用本地回环网络（Loopback TCP）握手认证通道，将新收到的 URL 或种子路径安全派发给主进程事件循环。

这意味着你可以在任何 Python、Bash 自动化脚本或外部下载器中随意调用 `limedl-native <URL>`，下载任务都会瞬时汇聚在同一个桌面上统一管理。

---

## 协议关联与网页一键唤醒 (Deep Links)

安装 limedl 时，客户端会自动向操作系统注册协议处理程序（Protocol Handlers）：

### 1. 磁力链接关联 (`magnet:`)
点击网页中的任何 `magnet:?xt=...` 链接时，浏览器会弹出“正在打开 limedl”对话框。确认后，limedl 窗口会自动弹至前台并开始解析 DHT 与种子元数据。

### 2. 专属协议唤醒 (`limedl://`)
支持第三方网页和私有系统通过 URL Scheme 一键唤醒 limedl。

**语法格式**：
```text
limedl://download?url=<ENCODED_URL>&filename=<OPTIONAL_NAME>&referer=<OPTIONAL_REFERER>
```

**参数说明**：
- `url` *(必填)*：经 URL Encode 编码的目标资源真实下载链接。
- `filename` *(可选)*：预设保存的文件名。
- `referer` *(可选)*：防盗链所需的 HTTP Referer 标头。

**HTML 网页调用示例**：
```html
<a href="limedl://download?url=https%3A%2F%2Fexample.com%2Fbigfile.zip&filename=demo.zip">
  使用 limedl 极速下载
</a>
```

---

## 环境变量控制

limedl 允许通过设置环境变量来改变运行行为，特别适合多配置隔离与便携模式：

| 环境变量 | 默认值 | 作用与说明 |
| :--- | :--- | :--- |
| `LIMEDL_DATA_DIR` | `%LOCALAPPDATA%\limedl` | 覆盖默认数据存储根目录。若将其指向便携 U 盘的相对路径，即可实现完全的绿色隔离运行。 |
| `LIMEDL_TAURI_DATA_DIR` | `%LOCALAPPDATA%\com.zkz20.limedl` | 覆盖老版本历史数据迁移源路径。 |
| `RUST_LOG` | `info` | 控制日志输出详细级别。排查故障时可设置为 `limedl_core=debug,limedl_native=trace`。 |
