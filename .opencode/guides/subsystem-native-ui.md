# Subsystem: Native UI (limedl-native)

Slint-based lightweight desktop client (`crates/limedl-native`) that talks to the
same `limedl-core` engine as the Tauri edition, without a webview.

## 模块职责

提供桌面端原生 UI：任务列表（卡片/表格）、新建任务对话框、Inspector、设置中心、Labs（CDN + URL 重写）、首启向导、托盘、通知、自启、单实例、拖拽/磁链 IPC、剪贴板监听、休眠抑制与自更新。

## 涉及文件

| 文件 | 职责 |
| --- | --- |
| `src/main.rs` | 应用装配：后台任务、Slint 回调 → `Dispatcher`、托盘、事件循环、外观/视图偏好应用 |
| `src/bridge.rs` | 纯映射层：`DownloadSummary` → `TaskItem`/`InspectorInfo`、`AppSettings` ↔ `SettingsFormData`、`TaskStore`（筛选/排序/多选）、排序与列/限速计划工具函数 |
| `src/i18n.rs` | 语言枚举、`format_*` 本地化辅助（含设置校验错误、托盘文案、优先级标签） |
| `src/update.rs` | minisign 校验的多通道自更新（见 `subsystem-self-update.md`） |
| `src/autostart.rs` | 开机自启（Win 注册表 / MSIX StartupTask / XDG .desktop / LaunchAgent） |
| `src/single_instance.rs` | 单实例（Win mutex + WM_COPYDATA；其他平台回环 TCP） |
| `src/platform_win.rs` | Win32 窗口子类化（`WM_DROPFILES`、`WM_COPYDATA`）+ OS 描述文案 |
| `src/protocol.rs` | `magnet:` / `limedl://` 协议注册（HKCU） |
| `src/power.rs` | 下载中抑制系统休眠 |
| `ui/appwindow.slint` | 主窗口：侧边栏、工具栏、卡片/表格、所有弹层 |
| `ui/components/*.slint` | 各对话框与复用组件（settings/labs/inspector/new_task/priority_menu…） |
| `ui/theme.slint` | 由 `scripts/generate-theme-slint.ps1` 从主题映射表生成的配色 token |

## 数据流向

```
UI 事件（callback）
  → main.rs 的 on_* 处理器
  → limedl_core::Dispatcher / Aria2RpcServer
  → EventBus::publish()
  → main.rs 的 EventBus 订阅任务（DownloadEvent 全覆盖，无 catch-all）
  → slint::invoke_from_event_loop → set_* 属性 → Slint 重绘
```

- 列表状态由 `bridge::TaskStore` 持有；`refresh_ui()` 把计数/排序/筛选后的行推给 `MainWindow.tasks`。
- 设置对话框的权威数据是 `SettingsFormData`；**列表类编辑器**（Labs 重写规则、限速计划）把上行文本保存在 UI model 中，保存时再由 Rust 解析（`parse_speed_limit_slots`、`slint_to_url_rewrite_rules`）。

## 关键约定

- **i18n 双轨**：`.slint` 内文案用 `@tr(...)`（`lang/{en,zh_CN}/LC_MESSAGES`）；Rust 侧动态文案必须走 `i18n::format_*`，禁止硬编码中文——否则英文界面会泄漏中文（设置校验、托盘、通知首当其冲）。
- **EventBus 事件必须显式处理**：`DownloadEvent` 匹配是穷尽的（无 `_ => {}`），新增变体会在编译期报错。`Warning` → 警告 toast（5s 去重窗口，因为反吸血按 peer 触发）。
- **新属性/新列**：视图偏好（`compactView`、`visibleColumns`、`sortKey`、`sortDirection`）持久化在 `AppSettings.appearance`；列 key 使用 Web 端同一套字符串（`file/size/downloaded/status/progress/speed/priority/uploadSpeed/seeds/eta`），保证 settings.json 两端互通。`file` 列始终可见。
- **任务优先级**：`Priority::{High,Normal,Low}` ↔ `"high"/"normal"/"low"`；表格徽标点击或右键菜单“Set Priority”打开 `PriorityMenu`，经 `Dispatcher::set_priority` 落库。
- **数据目录**：默认 `%LOCALAPPDATA%\limedl`（macOS/Linux 同规范），可用 `LIMEDL_DATA_DIR` 覆盖（与 `limedl-server` 一致，便于隔离测试）。首次启动会从 Tauri 的 `com.zkz20.limedl` 目录迁移 `settings.json`。
- **窗口钩子安装时机**：Slint 的 OS 窗口在事件循环启动后才存在，`platform_win::try_install_window_hooks` 必须由 UI 线程定时器重试挂载（一次性调用会静默失败，导致拖拽与磁链 IPC 失效）。

## 测试

```powershell
cargo test -p limedl-native      # bridge/i18n/update/power 纯逻辑测试（CI: check-windows 已执行）
cargo clippy -p limedl-native --all-targets -- -D warnings
```

Slint 编译期校验（`build.rs` → `slint-build`）会捕获 `.slint` 语法/类型/图标路径错误，因此没有单独的 UI 快照测试；交互行为靠 `bridge.rs` 的映射与状态机测试覆盖。

## 手动冒烟

```powershell
$env:LIMEDL_DATA_DIR = "$env:TEMP\limedl-smoke"
cargo run -p limedl-native
```

日志在 `<data_dir>\downloads\logs\limedl.log`（级别由 `settings.logging.level` 控制；调试时设为 `info`）。
