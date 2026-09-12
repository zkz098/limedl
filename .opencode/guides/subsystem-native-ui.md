# Subsystem: Native UI (limedl-native)

Slint-based lightweight desktop client (`crates/limedl-native`) that talks to the
same `limedl-core` engine, without a webview. It is the only desktop UI (the former
Tauri/Vue desktop shell was retired).

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

## 数据迁移与数据目录

- 默认数据根目录：`%LOCALAPPDATA%\limedl`（macOS/Linux 同规范）；`LIMEDL_DATA_DIR` 可覆盖（与 `limedl-server` 一致）。
- 启动时 `migrate::migrate_tauri_data_if_needed(base_dir, state_dir)` 会从 Tauri 版数据目录（`com.zkz20.limedl`，可用 `LIMEDL_TAURI_DATA_DIR` 覆盖）一次性导入：`settings.json`、`downloads.db`（含 `-wal`/`-shm`）、`torrents/`、`bt_files/`。
- 规则：只拷贝不移动；已存在的目标文件绝不覆盖；进度记录在 `<base_dir>/.migrated-from-tauri.json`（部分失败下次重试）。
- **启动安全网**：拷贝后校验 `settings.json` 可解析、`downloads.db` 可被 `limedl_core::database::Database::open` 打开；不合格的文件会被隔离为 `*.rejected-<ts>` 并以默认值启动，避免迁移反而把应用钉死在启动失败。

## 静默启动与关闭行为

- 自启注册（Windows `Run` / Linux `.desktop` / macOS LaunchAgent）统一附加 `--hidden`；`autostart::sync_from_settings` 会在路径过期时自动重写注册（`registered_for_current_exe`）。
- **MSIX / Store 通道例外**：`AppxManifest.xml` 的 `windows.startupTask` 无法携带参数，因此改由 `platform_win::launched_at_logon(150s)` 推断——比较本进程与 `GetShellWindow()`（explorer）的创建时间，登录后 150s 内、且 `autostart=true`、无命令行载荷、存在包身份时，按登录启动处理并隐藏窗口。（`WTSQuerySessionInformation(WTSLogonTime)` 在现行 Windows 上返回 `ERROR_NOT_SUPPORTED`，不可用。）
- 启动参数：`limedl-native [--hidden] [<url|magnet|path|limedl://…>]`。首启向导始终可见（`should_start_hidden` 要求 `setup_completed`）。
- 事件循环使用 `slint::run_event_loop_until_quit()`（默认循环以“可见窗口数”为退出条件，而我们的托盘由 `tray-icon`/`muda` 提供，Slint 不感知）。
- 关闭行为由 `Window::on_close_requested` 显式实现：`close_behavior = minimizeToTray` → `hide()`；`exit` → `quit_event_loop()`。
- 窗口钩子（拖拽/WM_COPYDATA）在窗口首次显示后才可能安装成功，因此采用 250ms 定时重试直到成功（隐藏启动期间会持续重试）。

## 构建依赖：rfd 对话框后端固定为 gtk3

`rfd` 只允许 `gtk3` 与 `xdg-portal` 二选一，两个都打开时其 `build.rs` 直接 panic（`You can't enable both`）。当前 workspace 固定为：

```toml
rfd = { version = "0.16", default-features = false, features = ["gtk3"] }
```

- 历史原因：已删除的 `src-tauri` 通过 `tauri-plugin-dialog` 默认打开 `rfd/gtk3`，而 Cargo 在整包构建（`cargo clippy --workspace --all-targets`、`cargo llvm-cov`、`cargo test --workspace`）时统一 feature，所以本 crate 当时必须显式跟随同一个后端，否则会同时启用 `gtk3` 与 `xdg-portal` 而 panic。
- **该约束已解除**：现在可以把 workspace 里唯一启用 `rfd` 的 crate 改成 `xdg-portal` 后端（`default-features = false, features = ["xdg-portal"]`，需要运行时存在 portal 服务），或者保留 gtk3；两种选择都不要让两个 feature 同时生效。
- 这些 feature 只在 Linux 生效（gtk/ashpd 依赖都声明在 Linux 的 `target.'cfg(...)'.dependencies` 下），Windows/macOS 的依赖图与行为不变。
- `gtk3` 后端在 Linux 上需要 `libgtk-3-dev`（`gtk-sys` 走 pkg-config；CI 的 Rust 作业已安装）。对话框由 `rfd` 自建的 GTK 线程（`gtk_init_check` + `gtk_main_iteration`）驱动，不要求宿主已有 GTK 主循环，Slint 应用可直接用 `AsyncFileDialog`。

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
