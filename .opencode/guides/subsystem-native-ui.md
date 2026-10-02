# Subsystem: Native UI (limedl-native)

Slint-based lightweight desktop client (`crates/limedl-native`) that talks to the
same `limedl-core` engine, without a webview. It is the only desktop UI (the former
Tauri/Vue desktop shell was retired).

## 模块职责

提供桌面端原生 UI：任务列表（卡片/表格）、新建任务对话框、Inspector、设置中心、Labs（CDN + URL 重写）、首启向导、托盘、通知、自启、单实例、拖拽/磁链 IPC、剪贴板监听、休眠抑制与自更新。

## 涉及文件

| 文件                     | 职责                                                                                                                                                   |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/main.rs`            | 应用装配：上下文初始化、后台任务启动、托盘、事件循环、外观与视图偏好应用                                                                               |
| `src/ui_boot.rs`         | UI 装配：窗口构造、初始外观/视图偏好、`AppContext` 组装与回调注册；由 `main()` 与进程内 UI 测试共用（OS/事件循环相关的部分留在 `main()`）              |
| `src/ui_tests/`          | 进程内 UI 测试（L1）：`mod.rs` fixture（建窗口/找元素/点击/按键/断言助手/录制式 backend），`shell.rs`、`list.rs`、`labs.rs`、`settings.rs`、`new_task.rs`、`inspector.rs`、`toast.rs`、`layout.rs`，以及需要事件循环的 `async_contracts.rs`（详见 `.opencode/guides/testing-guide.md`） |
| `src/platform_adapter.rs`| 平台集成：跨平台单实例激活监听 + Windows 专属拖拽/WM_COPYDATA 窗口子类化与重试挂载                                                                      |
| `src/bridge/`            | 纯映射层：`DownloadSummary` → `TaskItem`/`InspectorInfo`、`AppSettings` ↔ `SettingsFormData`（`forms/` 按设置分区拆分：`combo`/`enums`/`to_form`/`from_form`/`speed_limit`）、`TaskStore`（筛选/排序/多选）、排序与列/限速计划工具函数 |
| `src/handlers/`          | 业务事件回调处理器，每个子系统一个目录：`task/`（列表/多选/批量/单任务/剪贴板）、`settings/`（对话框/限速计划/目录介质覆盖/路径）、`labs/`（对话框/CDN/重写规则）、`new_task/`（对话框/提交/载荷入口）、以及 `inspector.rs`、`updater.rs`、`setup_wizard.rs`、`window.rs`。共享的绑定样板在 `handlers/common.rs`；每个 `register()` 只做绑定编排，回调体（超过 ~25 行的一律）提取为同模块的命名 `fn`（如 `submit_single`、`finish_setup`、`factory_reset`） |
| `src/event_stream/`      | 后台监听：`bus.rs`（`DownloadEvent` 每个变体一个函数）、`pollers.rs`（剪贴板 + BT 状态 + Inspector 轮询）、`tray.rs`（托盘菜单/左键激活）                                                      |
| `src/settings_sync.rs`   | 保存设置后的共享副作用：OS 自启同步、Aria2 RPC 热重载、把设置推入 UI（设置对话框与首启向导共用）                                                                                     |
| `src/i18n/`             | 语言枚举（`language.rs`）与 `format_*` 本地化辅助，按域拆分：`task.rs`（列表/状态）、`dialogs.rs`（新建任务/批量）、`tray.rs`（托盘/通知）、`toast.rs`（全部 toast）、`validation.rs`（设置校验）、`cdn.rs`、`rewrite.rs`、`schedule.rs`；全部在 `mod.rs` 重新导出，调用点仍是 `i18n::format_*` |
| `src/update/`            | minisign 校验的多通道自更新（见 `subsystem-self-update.md`；`mod.rs` + `tests.rs`）                                                                                           |
| `src/migrate/`           | Tauri → Native 数据迁移（`mod.rs` + `tests.rs`）                                                                                                       |
| `src/autostart.rs`       | 开机自启（Win 注册表 / MSIX StartupTask / XDG .desktop / LaunchAgent）                                                                                 |
| `src/single_instance.rs` | 单实例（Win mutex + WM_COPYDATA；其他平台回环 TCP）                                                                                                    |
| `src/platform_win.rs`    | 窗口子类化（`WM_DROPFILES`、`WM_COPYDATA`）+ 窗口几何持久化（全平台）+ OS 描述文案                                                                        |
| `src/protocol.rs`        | `magnet:` / `limedl://` 协议注册（HKCU）                                                                                                               |
| `src/power.rs`           | 下载中抑制系统休眠                                                                                                                                     |
| `ui/appwindow.slint`     | 主窗口：侧边栏、工具栏、卡片/表格、所有弹层                                                                                                            |
| `ui/components/settings_dialog.slint` | 设置中心外壳：属性/回调、页签栏、页脚与各页签的装配（`settings/tab_*.slint`）                                                              |
| `ui/components/settings/` | 设置页签组件：`tab_{appearance,download,proxy,schedule,bt,io,log,aria2,about}.slint` + `shared.slint`（FormTip / ColumnCheck）。`tab_io` 的目录介质覆盖编辑器与 `tab_schedule` 的限速计划同构：行存在 UI model 里，Save 时由 `bridge/forms/{disk_override,speed_limit}.rs` 解析              |
| `ui/components/*.slint`  | 各对话框与复用组件（labs/inspector/new_task/priority_menu…）                                                                                  |
| `ui/theme.slint`         | 由 `scripts/generate-theme-slint.ps1` 从主题映射表生成的配色 token                                                                                     |

## 数据流向

```
UI 事件（callback）
  → handlers/ 的 on_* 处理器（绑定样板统一走 `handlers::common`）
  → limedl_core::Dispatcher / Aria2RpcServer
  → EventBus::publish()
  → main.rs 的 EventBus 订阅任务（DownloadEvent 全覆盖，无 catch-all）
  → slint::invoke_from_event_loop → set_* 属性 → Slint 重绘
```

- 列表状态由 `bridge::TaskStore` 持有；`refresh_ui()` 把计数/排序/筛选后的行推给 `MainWindow.tasks`。
- 设置对话框的权威数据是 `SettingsFormData`；**列表类编辑器**（Labs 重写规则、限速计划、IO 页签的目录介质覆盖）把上行文本保存在 UI model 中，保存时再由 Rust 解析（`parse_speed_limit_slots`、`slint_to_url_rewrite_rules`、`parse_disk_type_overrides`）。行内编辑通过 `set_row_data` 就地更新，不重建整个模型，否则正在输入的输入框会丢光标。

## 关键约定

- **i18n 双轨**：`.slint` 内文案用 `@tr(...)`（`lang/{en,zh_CN,zh_TW}/LC_MESSAGES`）；Rust 侧动态文案必须走 `i18n::format_*`，禁止硬编码中文——否则英文界面会泄漏中文（设置校验、托盘、通知首当其冲）。
- **EventBus 事件必须显式处理**：`DownloadEvent` 匹配是穷尽的（无 `_ => {}`），新增变体会在编译期报错。`Warning` → 警告 toast（5s 去重窗口，因为反吸血按 peer 触发）。新增事件变体时在 `event_stream/bus.rs` 里加一个 `on_*` 函数，不要在 match 里内联长逻辑。
- **回调绑定样板走 `handlers/common.rs`**：不要再写 `main_window.as_weak()` + `if let Some(ui) = ui_weak.upgrade()`；用 `with_ui` / `read_ui`（需读值）/ `mutate_store` / `reload_tasks` / `refresh_after_removal` / `spawn_action` / `spawn_batch_action`。每个子系统模块只负责“这个回调做什么”，样板不进业务代码。
- **一个回调一个函数**：`register()` 只做编排（各子模块的 `register` 列表）；跨回调共享的流程（如批量删除、torrent 预览、校验和探测、CDN 应用节点）必须提成命名函数，历史上它们曾以 2–4 份拷贝散在同一个 800 行函数里。
- **保存设置的副作用走 `settings_sync.rs`**：自启同步、Aria2 RPC 热重载、推设置到 UI（语言/托盘/外观/默认目录）只有一份实现，设置对话框与首启向导共用；新增“保存后要做的事”请加在这里，不要在两处各写一遍。
- **新属性/新列**：视图偏好（`compactView`、`visibleColumns`、`sortKey`、`sortDirection`）持久化在 `AppSettings.appearance`；列 key 使用 Web 端同一套字符串（`file/size/downloaded/status/progress/speed/priority/uploadSpeed/seeds/eta`），保证 settings.json 两端互通。`file` 列始终可见。
- **破坏性动作两段式，且关闭对话框就解除武装**：关于页的 “Factory Reset” 用 `reset_confirm` 做二次确认，该属性挂在 `MainWindow`（不是对话框）上，并在 `on_close_settings` 与 `settings_sync::push_ui(close_settings)` 里重置。只靠 Cancel/Confirm 清除的话，Escape 关掉再打开会直接停在 “Confirm Reset” 上，离清空数据目录只差一次点击（`ui_tests/async_contracts/dialogs.rs` 钉住这个生命周期）。
- **自定义控件声明 a11y**：`ToggleSwitch`、`PrimaryButton`/`SecondaryButton`/`DangerButton` 都写了 `accessible-role` + `accessible-label`/`accessible-enabled`（`Text` 的 `accessible-label` 默认就是它的 `text`）。这既是屏幕阅读器需要的，也是 L1 测试读“用户看到什么”（文案、禁用态）的唯一抓手。
- **任务优先级**：`Priority::{High,Normal,Low}` ↔ `"high"/"normal"/"low"`；表格徽标点击或右键菜单“Set Priority”打开 `PriorityMenu`，经 `Dispatcher::set_priority` 落库。
- **数据目录**：默认 `%LOCALAPPDATA%\limedl`（macOS/Linux 同规范），可用 `LIMEDL_DATA_DIR` 覆盖（与 `limedl-server` 一致，便于隔离测试）。首次启动会从 Tauri 的 `com.zkz20.limedl` 目录迁移 `settings.json`。
- **窗口钩子安装时机**：Slint 的 OS 窗口在事件循环启动后才存在，`platform_win::try_install_window_hooks` 必须由 UI 线程定时器重试挂载（一次性调用会静默失败，导致拖拽与磁链 IPC 失效）。

## 数据迁移与数据目录

- 默认数据根目录：`%LOCALAPPDATA%\limedl`（macOS/Linux 同规范）；`LIMEDL_DATA_DIR` 可覆盖（与 `limedl-server` 一致）。
- 启动时 `migrate::migrate_tauri_data_if_needed(base_dir, state_dir)` 会从 Tauri 版数据目录（`com.zkz20.limedl`，可用 `LIMEDL_TAURI_DATA_DIR` 覆盖）一次性导入：`settings.json`、`downloads.db`（含 `-wal`/`-shm`）、`torrents/`、`bt_files/`。
- 规则：只拷贝不移动；已存在的目标文件绝不覆盖；进度记录在 `<base_dir>/.migrated-from-tauri.json`（部分失败下次重试）。
- **启动安全网**：拷贝后校验 `settings.json` 可解析、`downloads.db` 可被 `limedl_core::database::Database::open` 打开；不合格的文件会被隔离为 `*.rejected-<ts>` 并以默认值启动，避免迁移反而把应用钉死在启动失败。

## 窗口几何持久化（全平台）

`platform_win.rs` 承担了 `WindowGeometry`（位置/尺寸/最大化）的存取，两条实现路径：

- **Windows**：`GetWindowPlacement` / `SetWindowPos` + `MonitorFromRect`。选 Win32 是因为它能拿到*未最大化*时的尺寸，并能用真实显示器列表判断保存的矩形是否还在屏内。恢复时若矩形不在任何显示器上，会调用 `center_window_on_monitor` 居中。
- **macOS / Linux**：走 Slint 自己的 `Window::position` / `set_position` / `size` / `set_size` / `is_maximized` / `set_maximized`（winit 后端）。Slint 不暴露显示器几何，因此无法居中，也没有 `MonitorFromRect` 这类判断——改用 `MAX_PLAUSIBLE_COORD`（±20000 物理像素）启发式：真实多显示器布局不会超出这个范围，而残留在已断开显示器上的坐标会，于是被判定为过期并放弃恢复，交给窗口管理器放置。
- 单位统一为**物理像素**，与 `Window::position()` 的返回值和 Win32 的 `RECT` 一致。
- 调用点在 `main.rs` 中**位于 `#[cfg(windows)]` 块之外**，独立于 Win32 窗口钩子：钩子在非 Windows 上永远挂载失败（返回 `false`），早期把恢复逻辑放在钩子成功分支里会导致 macOS/Linux 每次启动都是默认尺寸和位置。
- **重试**：Slint 的 OS 窗口在事件循环启动后才创建，因此 `apply_restored_window_placement` 返回 `bool`（是否已应用），由 `main.rs` 的 `schedule_window_placement_restore` 用 250ms 定时器重试。Windows 在句柄未就绪时返回 `false`；其他平台在“已请求最大化但窗口还没真的最大化”时返回 `false`（winit 会丢弃窗口显示前发出的请求）。这个定时器也是 `--hidden` 托盘启动能恢复位置的原因——那条路径永远不 `show()`。
- 已最大化且位置已生效时立即返回 `true`（不再重试），避免与用户手动取消最大化相互打架。
- 保存时机：Windows 由子类化过程在 `WM_EXITSIZEMOVE` / `WM_SIZE` 触发；其他平台只在正常退出（关闭请求 / 事件循环返回后）调用 `save_current_window_geometry`。
## 静默启动与关闭行为

- 自启注册（Windows `Run` / Linux `.desktop` / macOS LaunchAgent）统一附加 `--hidden`；`autostart::sync_from_settings` 会在路径过期时自动重写注册（`registered_for_current_exe`）。
- **MSIX / Store 通道例外**：`AppxManifest.xml` 的 `windows.startupTask` 无法携带参数，因此改由 `platform_win::launched_at_logon(150s)` 推断——比较本进程与 `GetShellWindow()`（explorer）的创建时间，登录后 150s 内、且 `autostart=true`、无命令行载荷、存在包身份时，按登录启动处理并隐藏窗口。（`WTSQuerySessionInformation(WTSLogonTime)` 在现行 Windows 上返回 `ERROR_NOT_SUPPORTED`，不可用。）
- 启动参数：`limedl-native [--hidden] [<url|magnet|path|limedl://…>]`。首启向导始终可见（`should_start_hidden` 要求 `setup_completed`）。
- 事件循环使用 `slint::run_event_loop_until_quit()`（默认循环以“可见窗口数”为退出条件，而我们的托盘由 `tray-icon`/`muda` 提供，Slint 不感知）。
- 关闭行为由 `Window::on_close_requested` 显式实现：`close_behavior = minimizeToTray` → `hide()`；`exit` → `quit_event_loop()`。
- 窗口钩子（拖拽/WM_COPYDATA）在窗口首次显示后才可能安装成功，因此采用 250ms 定时重试直到成功（隐藏启动期间会持续重试）。

## 构建依赖：rfd 对话框后端为 xdg-portal

`rfd` 只允许 `gtk3` 与 `xdg-portal` 二选一，两个都打开时其 `build.rs` 直接 panic（`You can't enable both`）。当前固定为：

```toml
rfd = { version = "0.16", default-features = false, features = ["xdg-portal"] }
```

- 历史原因：已删除的 `src-tauri` 通过 `tauri-plugin-dialog` 默认打开 `rfd/gtk3`，Cargo 在整包构建时统一 feature，所以本 crate 当时必须跟随同一后端。**该约束已解除**，现在是主动选择 `xdg-portal`。
- 选它的理由：去掉**构建期** GTK 依赖。`gtk3` 需要 `libgtk-3-dev` + `pkg-config`（`gtk-sys` 走 pkg-config）；`xdg-portal` 只通过 D-Bus 与 `xdg-desktop-portal` 通信（ashpd/zbus，纯 Rust），并在 Wayland 及 Flatpak/Snap 沙箱里给出正确的原生文件选择器。
- 运行时特征：需要一个运行中的 portal 服务（GNOME/KDE/XFCE 默认自带 `xdg-desktop-portal-*-gtk`）。裸 X11、无 portal 的环境会直接报 portal 错误——没有 GTK 构建的二进制无法退回 GTK 对话框。
- **不需要**额外的 runtime feature：rfd 通过 `pollster::block_on` 驱动 ashpd，而 ashpd 的默认 feature 就是 `tokio`。
- 这些 feature 只在 Linux 生效（gtk/ashpd 依赖都声明在 Linux 的 `target.'cfg(...)'.dependencies` 下），Windows/macOS 的依赖图与行为不变。

注意：**托盘**仍需 GTK 构建依赖。`tray-icon` 在 Linux 上默认启用 `gtk` feature（→ `gtk` + `libappindicator`，dlopen 调用 appindicator 库），所以 Linux 构建 / CI 仍需 `libgtk-3-dev`；去掉的是 rfd 带来的那一份，不是全部。

- **已接受的上游告警（TODO：上游迁到 gtk-rs 0.20 后复查）**：这条 `gtk 0.18` 链会把 `glib 0.18.x` 带进来，而它带着一条 unsoundness 告警（GHSA-wrw7-89jp-8q8g / RUSTSEC-2024-0429：`glib::VariantStrIter` 的迭代器实现可能解引用 NULL ⇒ 崩溃）。已在 GitHub 上以 “Vulnerable code is not actually used” dismiss：limedl 从不直接调用 glib，触发它需要迭代 `VariantStrIter`，影响是崩溃而不是可控输入；`cargo audit` 对 unsound 类只 warn、`cargo deny` 的 `unsound` lint 默认也是 warn，所以 supply-chain job 一直是绿的。钉住它的是 `libappindicator 0.9.0`（2023-10，最后一个版本，硬依赖 `glib ^0.18`）：今天最新的 `tray-icon 0.26.0` / `muda 0.21.0` 仍要求 `gtk ^0.18`，`cargo update -p glib` 无可达版本。复查时机：`libappindicator` 发版，或改用 `tray-icon` 的可选 `ksni` 后端（会整条去掉 gtk/libappindicator，但托盘菜单要改写，且只能在 Linux 桌面上验证）。

## Linux 桌面版

- 仓库：`x86_64-unknown-linux-gnu`（`.cargo/config.toml` 已把该 target 定为 `x86-64-v3`，与 Windows 桌面一致，需 2013+ CPU）。选 gnu 而非 musl：Slint 已经链接系统库（GL/X11/Wayland、托盘 appindicator），静态 musl 买不到可移植性。代价是 glibc 下限 —— 构建机是 `ubuntu-latest`，因此二进制需要 glibc >= 2.39（Ubuntu 24.04+）。
- 发布产物：
  - `limedl-native-v{V}-linux-x86_64-portable.tar.gz`：由 `scripts/package-linux.sh` 生成，含唯一顶层目录 `limedl-native/`（二进制 + README），便携解压运行。
  - `limedl-native-v{V}-linux-x86_64.deb`：由 `scripts/package-deb.sh` 生成，标准 Debian/Ubuntu 安装包，集成 `/usr/bin/limedl-native`、`.desktop` 与多尺寸应用图标。
  - `limedl-native-v{V}-linux-x86_64.AppImage`：由 `scripts/package-appimage.sh` 生成，跨发行版免安装单文件，内嵌 `AppRun`、桌面项与图标。
- 运行时托盘依赖：缺失 appindicator 时 `TrayIconBuilder::build()` 会失败，而托盘是唯一常驻 UI（关闭到托盘、`--hidden` 自启都落在它上面），因此不降级而是报错退出，并在 `tray_init_failure_message` 中给出 apt/dnf/pacman 包名。
- 自启：`~/.config/autostart/limedl-native.desktop`（XDG），带 `--hidden`。
- 单实例：回环 TCP（`open:`/`show` 协议），文件管理器打开走 `xdg-open`。
- 休眠抑制仍是空实现（`power.rs` 在非 Windows 不做任何事）—— 即 Linux/macOS 上不会阻止系统休眠。

## 测试

```powershell
cargo nextest run -p limedl-native   # CI 的口径；含 UI 测试，cargo test -p limedl-native 同样可跑
cargo clippy -p limedl-native --all-targets -- -D warnings
```

- **Slint 编译期校验**（`build.rs` → `slint-build`）捕获 `.slint` 语法/类型/图标路径错误。
- **进程内 UI 测试**（`src/ui_tests/`，详见 `.opencode/guides/testing-guide.md`）：真实窗口 + 真实回调，覆盖工具栏/对话框开关、Esc 层级、快捷键（Ctrl+N/A/F）与快捷键守卫、列表选择与批量栏、右键菜单、表格列、重写规则编辑器、限速计划、新建任务重置契约与**提交载荷**、toast 队列、几何不变量（含英文标签下同一套），以及“删除记录 vs 删除文件”这类爆炸半径断言：`Delete`/`Shift+Delete`/`Space` 热键、卡片与表格的行内按钮、`Pause All`/`Resume All`/`Clear Completed`、筛选下的全选与批量删除、失败回滚、双击行为、设置/实验室保存成功路径、首次运行向导（`with_language(EnUs, …)` 是唯一的英文布局入口）。它们按 `.slint` 的 `id:` 找元素，**所以 `id:` 是测试契约的一部分**（重命名要同步改测试）：新增交互控件时顺手给个 snake_case 的 `id:`。
- **测试抓到过的真实 bug**（新增用例时就往这些方向看）：
  - `handle_key_escape` 所在 `FocusScope` 的“对话框打开就拦下快捷键”是裸 `if … { reject }`，值被丢弃 ⇒ Ctrl+A / Space / Delete 会作用到模态**背后**的列表（Delete 会删选中项）。现在它是 `else if` 链的一环。
  - 表格列头点击调用 `set_sort_field`，它不重置方向 ⇒ 换列可能变成降序，与注释承诺的“新列升序”不符。现在用 `apply_sort(field, true)`。
  - 窗口在 `min-width`（1000px）下工具栏溢出行右边界，`ta_new_task` 被裁掉约 18px ⇒ 已修：`min-width` 抬到 1100px，英文标签下也在 `layout.rs` 里钉住。
  - 新建任务对话框的 modal 高度是固定的（`preview_state == "ready"` 时 600px），而展开 torrent 文件列表后的内容更高 ⇒ 已修：对话框主体放进 ScrollView，页脚（“Start Download”）永远留在 modal 里。
  - 表格视图的行内操作列在默认与最小窗口宽度下被裁出可视区（表格没有横向滚动）⇒ 已修：操作列固定到右边缘（`root.width - 138px`），数据列在它后面被裁；文件列 `min-width` 从 260px 收到 140px，使默认列集在最小窗口下也能露出前几列。
  - 向导第 2 步的三个 `LanguageCard` 原本是同一行里三个 `width: 50%`（共 150% + 间距）⇒ `en-US` 卡片在任何窗口宽度下都溢出 modal，鼠标点不到英语。已修：三张卡改为 `horizontal-stretch: 1` 均分。
- **仍然接受的限制**：表格列比窗口宽时，右侧的数据列会被裁掉（操作列永远可见）。这里没有横向滚动，因为 Slint 1.17 的 `ScrollView` 无法为 `for` 循环内容推导 `viewport-width`（flickable pass 会跳过 repeated 元素，见 `passes/flickable.rs` 里的 #407），而显式绑定会被编译器生成的默认绑定覆盖——所以需要更多数据列时请把窗口拉宽或在设置里关掉几列。
- 断言可以读到的几何来自布局（无渲染器也是真实字体度量），但**没有进程内截图**：testing crate 的 `internal` feature 在 crates.io 上无法编译（见 testing-guide）。像素与视觉回归仍走 L2/MCP 的 `take_screenshot`。

## 手动冒烟

```powershell
$env:LIMEDL_DATA_DIR = "$env:TEMP\limedl-smoke"
cargo run -p limedl-native
```

日志在 `<data_dir>\downloads\logs\limedl.log`（级别由 `settings.logging.level` 控制；调试时设为 `info`）。

需要让 agent（或脚本）自己点开真实窗口看状态时，改用 MCP server（详见`.opencode/guides/testing-guide.md`的 L2 节）：

```cmd
set SLINT_EMIT_DEBUG_INFO=1
set SLINT_MCP_PORT=8080
cargo run -p limedl-native --features slint/mcp
```
