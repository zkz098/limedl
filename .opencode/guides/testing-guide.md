# Testing Guide — limedl

## 模块职责

测试策略、mock 模式、运行命令和 CI 描述的汇总。

## 涉及文件

- `crates/limedl-core/src/tests/` — 核心下载引擎集成测试（manager_tests 等）
- `crates/limedl-native/src/` — 桌面 UI 桥接与逻辑单测
- `crates/limedl-native/src/ui_tests/` — Slint 进程内 UI 测试（L1，见下）
- `.github/workflows/ci.yml` — CI 配置文件

### 测试布局约定

1. **默认内联**：小的 `#[cfg(test)] mod tests { ... }` 直接放在生产文件尾部（< ~150 行），测试就近于实现。
2. **大了就外移**：超过 ~150 行的内联测试块必须移到同级 `tests.rs`（模块目录内），生产文件只保留 `#[cfg(test)] mod tests;`。例：`settings/tests.rs`、`http_executor/tests.rs`、`scheduler/tests.rs`、`task_lifecycle/tests.rs`。测试通过 `use super::*;` 仍能访问私有项，模块路径（如 `settings::tests::*`）保持不变。
3. **跨模块 E2E 放 crate 级**：需要真实 mock server / 多个子系统协作的测试放 `crates/limedl-core/src/tests/`（由 `lib.rs` 的 `mod tests` 引入）。
4. **巨型测试文件按场景拆分**：单文件超过 ~800 行或 ~30 个测试时，拆成同名目录（`mod.rs` 放导入与共享 fixture，`<场景>.rs` 放测试，每个文件 `use super::*;`）。已拆：`tests/manager_tests/`、`tests/http_executor_tests/`、`tests/scheduler_tests/`、`bt_backend/tests/`、`buffer_pool/tests/`、`database/tests/`。拆分类时注意把 `#[test]`/`#[tokio::test]` 属性与函数一起搬走。
5. **需要真实窗口的 UI 测试放 `crates/limedl-native/src/ui_tests/`**：`mod.rs` 是 fixture（建窗口 + 按 id 找元素 + 点击），场景按 `shell.rs`（窗口外壳/对话框）、`list.rs`（任务列表）拆分。规格与约束见下节。

## 数据流向

```
代码变更 → CI 触发（纯文档改动被 paths-ignore 跳过）:
  ├─ check-windows (Windows): clippy --workspace -D warnings
  ├─ test-windows-core (Windows): limedl-core 测试（nextest）
  ├─ test-windows-native (Windows): limedl-native 测试（nextest）
  ├─ check-macos (macOS): clippy → core 测试 → native 测试（nextest）
  ├─ check-rust (Linux): coverage（core + native 两份 lcov）→ clippy → SonarQube 扫描 → limedl-native 测试（nextest）
  ├─ bench-rust (Linux): cargo bench (aimd + rate_limiter)
  └─ supply-chain (Linux): cargo deny check + cargo audit
```

Windows 拆成**三个**并行 job 是因为它是最慢的平台：`cargo clippy` 只做 check、无法与测试
构建共享产物，串行只会累加墙钟时间。实测（热缓存）job 墙钟：clippy ≈ 272 s、limedl-core 测试
≈ 298 s、native 测试 ≈ 1012 s（macOS ≈ 879 s），瓶颈就是 Windows 的 native 测试 job。

那 1012 s 里**测试执行只占 ~3 s**（本地 16 核实测：114 个测试 3.06 s，编译+链接 115 s）：
大头是构建加 rust-cache 的**回写**（该 job 846 MB 的条目耗时 514 s，而 limedl-core 451 MB 的
只要 35 s）。所以“把测试搬到别的平台”并不能缩短 Windows 的时间，能省的是缓存条目本身——
见下面的缓存约定。同一 ref 的旧 run 由 `concurrency` 直接取消。

**limedl-native 的测试在三个平台都跑**：`test-windows-native`、`check-macos`，以及 Linux 的
`check-rust`（在原 job 里多加一步 `cargo nextest run -p limedl-native`，不另开 job）。测试本身
是平台无关逻辑 + `i-slint-backend-testing` 的进程内 UI 测试（无窗口、无 display），三个平台跑
同一套断言就是覆盖（路径分隔符、文件锁、临时目录）；而各平台的 `cfg(target_os = ...)` 分支只有
对应平台会**执行** —— Linux 那批（`.desktop` 自启、tray 失败文案、xdg-open、AppImage 自更新
守卫）只在这条腿上跑，所以它不能退化成只做 clippy。并进 `check-rust` 而不是单开 job 是缓存账：
仓库已贴着 GitHub 10 GB 上限，新 job = 新 rust-cache key = 再多 0.6-0.9 GB 条目，而
`check-rust` 的 242 s 远低于关键路径（1012 s）。

### CI job 命名规范

`name:`（GitHub UI 与 check 名）统一为 `<范围/动作> (<平台>[, 细节])`：

- 平台永远是最后一个括号里的**第一个 token**，取值限定 `Linux` / `macOS` / `Windows` /
  `windows-x86_64`。
- 动作词汇限定：Clippy / Tests / Coverage / Benchmarks / Supply chain / Release notes / Warm cache / Sign & guard。
- **不要在 `name:` 里写 `: `**：YAML 中值里出现 `": "` 必须加引号；子限定用逗号（`Tests (Windows, limedl-core)`）。
- **job id 不随显示名变化**，保持稳定，便于 `needs:`、`gh run view` 与文档引用。

### CI 缓存 / RUSTFLAGS 约定

- **`setup-rust-toolchain` 必须带 `cache: false`**：该 action 默认（`cache: true`）
  内部会跑一遍 rust-cache，与工作流里显式的 `swatinem/rust-cache` 重复，等于每个 job
  存两份 1-2 GB 缓存。仓库缓存上限 10 GB，重复条目把 release 缓存挤掉后，每次发版
  都要冷编译 Slint 图形栈。
- **`rustflags: ""`**：该 action 默认导出 `RUSTFLAGS=-D warnings`，而 RUSTFLAGS 一旦
  存在就会**整体覆盖** `.cargo/config.toml` 的 rustflags（target-cpu / rust-lld 全部失效）。留空后 config.toml 是唯一的 flag 来源，所有步骤共用
  同一份指纹，不再出现同 job 内反复全量重编。
- 告警仍然是硬失败：`CARGO_BUILD_WARNINGS: deny`（cargo 的 `build.warnings`，与
  RUSTFLAGS 正交）配合 clippy 的 `-- -D warnings`，**每个 Rust job 都设**，包括
  `test-windows-native`。它曾经是逐 job 的例外：用 rust-lld 链接 Skia 版 limedl-native
  必然告警（Skia 重复的 ICU 符号 `ubrk_getLocaleByType`，靠 `/FORCE:MULTIPLE` 收敛），
  所以那个 job 只能放弃该项。Skia 依赖与 Skia 渲染器都移除后，这条例外连同
  `/FORCE:MULTIPLE` 一起删掉了。
- rust-cache 的 key 由 rust 工具链 + 上述 RUST*/CARGO*/CC*/CFLAGS*/CXX*/CMAKE* 环境变量
  + `.cargo/config.toml` + 外部依赖 hash 组成，**不含源码**；只有恢复不完整（key 不
    完全匹配）时才会回写缓存，完整命中时不会覆盖。
- 桌面 release 构建的缓存由 `.github/workflows/warm-release-cache.yml` 在 main 上预热
  （`windows-x86_64` 与 `darwin-aarch64` 两条腿），与 `release.yml` 的 `build-native` /
  `build-native-macos` 共用同一 key（`add-job-id-key: false`）。**预热 job 是这些 key 唯一的写入者**：
  `release.yml` 三条腿都带 `save-if: ${{ github.ref_type == 'branch' }}`，tag run 只恢复不写——
  tag 作用域的条目（`refs/heads/refs/tags/vX.Y.Z`）只有同一 tag 重跑才读得到，等于每次发版白写
  2.3 GB 进 10 GB 的仓库上限（GitHub 按 last access 淘汰，7 天未访问直接删）。代价：同一 tag 的
  **重跑**会冷启动，首次发版由 main 的预热条目覆盖。
- **job 级 env 会分裂 rust-cache 的 envHash**：`CARGO_BUILD_WARNINGS: deny` 这类 job 级变量
  （前缀命中上面那份列表）会让两个 job 无法共享同一 key（实测：`check-rust`/`bench-rust` 是
  `357705c9`，`supply-chain`/`release-native-linux-x86_64` 是 `df9a423c`）。要合并 key
  （`shared-key` 或 `add-job-id-key: false`）时，把这类变量挂到具体 step 的 `env:` 上，cache 步骤
  就看不到它了。

### CI 测试执行 / 构建速度

- **Rust 测试统一用 `cargo nextest`**（由 `taiki-e/install-action` 安装，版本在 workflow 里
  pin）：nextest 为每个测试启动独立进程，既并行执行，也消除了 libtest 单进程共享全局状态
  带来的兄弟测试互扰。注意 `cargo nextest run` **不执行 doctest** —— 目前 workspace 没有 doctest；若将来新增，
  需在 `check-rust` 补一个 `cargo test --doc` 步骤。覆盖率 job 同样走 nextest（`cargo llvm-cov nextest`，
  加 `--no-fail-fast` 保证有测试失败时覆盖率并集不被截断）。
- **`[profile.test] debug = false`**（根 `Cargo.toml`）：CI 每个 job 都要编译并链接测试二进制，
  而依赖早已通过 `[profile.dev.package."*"]` 跳过 debug info，workspace 自己 crate 的
  line tables 只剩开销。本地要断点/行号：`cargo test --profile dev`；覆盖率的
  `cargo llvm-cov nextest --cargo-profile dev` 也是为了 lcov 的行号归属（`--profile` 在 nextest 里
  选的是 nextest profile，Cargo profile 要用 `--cargo-profile`）。release 产物用的是独立 profile，
  不受影响。
- **覆盖率是硬门禁**：`check-rust` 里 `cargo llvm-cov nextest ... --fail-under-lines 87` 跑 core，低于 87% 直接让 job 变红（Sonar 的质量门在 Free 计划里挂不上自定义条件，只是参考）。这个数只算**产品代码**：cargo-llvm-cov 从 0.6.22 起默认 ignore 掉 `tests/` 目录与 `tests.rs`/`*_tests.rs` 文件，而门禁引入时用的 88/21397 行是 0.6.21 含测试代码的量法；CI 曾因为 install-action 拉 `latest`（跳到 0.9.0）在代码没变的情况下变红，所以工具版本已固定为 `cargo-llvm-cov@0.9.0`，改数字前先看 ci.yml 里的测算注释。同一步再为 limedl-native 生成第二份 lcov，用 `--ignore-filename-regex 'crates.limedl-core'` 去掉 llvm-cov 顺带插桩的 core（否则 core 的行会被重复导入）。两步都带 `--no-cfg-coverage`：native 侧 `tiny-xlib` 会把 `cfg(coverage)` 变成 nightly-only 的 `#![feature(coverage_attribute)]`（stable 直接 E0554），而且两步 RUSTFLAGS 一致才能让第二步复用第一步插桩好的共享依赖。两份报告写在各自 crate 目录（`crates/limedl-core/lcov-core.info`、`crates/limedl-native/lcov-native.info`）：Sonar 的 Rust analyzer 按每个 Cargo manifest 一个模块解析 `sonar.rust.lcov.reportPaths`，只会到该模块自己的 base dir 找报告。`scripts/`、`website/`、`xtask` 在 `sonar.coverage.exclusions` 里排除——llvm-cov 无法插桩 PS1/Astro，计 0% 只会拉低分母。
- **覆盖率目前只有行覆盖**：cargo-llvm-cov 的 `--branch` 是 unstable，stable 工具链直接拒绝（`--branch flag requires nightly toolchain`），所以 LCOV 没有 BRDA，Sonar 也没有条件覆盖。要分支覆盖得先把这两个 llvm-cov 步骤换成 pinned nightly，见 ci.yml 注释。
- **Windows job 排除 Defender 实时扫描**（`Add-MpPreference -ExclusionPath`，best-effort、
  失败不挂 job）：Defender 会逐个扫描 cargo 写入 `target/` 的多 GB 文件，是 Windows 相对
  Linux 的主要惩罚项。新增 Windows 步骤时不要导出 `RUSTFLAGS`/`CARGO*`/`CC*`/`CMAKE*`
  环境变量，否则会分裂 rust-cache key。

## 设计决策与约定

### Rust 测试

- 单元测试：内联在源码文件底部 `#[cfg(test)] mod tests`。
- 集成测试：`crates/limedl-core/src/tests/`（manager_tests.rs 等，使用本地 axum HTTP mock 服务器 + tempfile 临时目录）。
- 每 crate 独立测试命令（core 带 `test-utils,aria2-rpc`，limedl-native 走单独 step）。
- **全局状态必须独占一个测试文件**：cargo 把单个 `tests/*.rs` 当做一个进程跑，而 `tracing_subscriber::fmt().init()` 之类的调用会占用进程级全局槽，同文件内的兄弟测试会与之竞争并 panic。这类测试放独立文件：`tests/logging_reload_repro.rs`（干净进程）与 `tests/logging_preinstalled_subscriber.rs`（预装全局订阅者）就是例子。
- Windows 上必须先初始化 MSVC 环境（vcvarsall.bat x64），否则 clippy/test 因链接器失败。
- 依赖：axum（HTTP mock）、tempfile、ntest（超时注解）。

### Slint UI 测试（L1 进程内 / L2 MCP）

两层，按“能不能只花更小的代价就抓住你想抓的回归”选：

**L1 —— 进程内 UI 测试**（`crates/limedl-native/src/ui_tests/`，跟着 `cargo nextest run -p limedl-native` 一起跑）。
通过 `src/ui_boot.rs::build_ui` 构造**真实**的 `MainWindow` 与**真实**的回调接线，然后用
`i-slint-backend-testing` 的 testing backend 按 `.slint` 的 `id:` 找元素、模拟点击与按键。无窗口、
无 display，因此没有字体/DPI 平台的 flaky。覆盖的是 `bridge/` 单测看不到的那一段：按钮接错回调、
对话框的 `is_open` 没人写、属性绑定写错，以及**操作的爆炸半径**（删记录还是连文件一起删）。

场景文件：`shell.rs`（工具栏/对话框/Esc 层级/快捷键/向导外观实时预览）、`list.rs`（列表、选择、
shift 范围选择、批量栏、右键菜单、表格列）、`labs.rs`（重写规则编辑器、CDN 内联校验）、
`settings.rs`（限速计划、设置对话框）、`new_task.rs`（重置契约、批量计数、torrent 预选）、
`inspector.rs`、`toast.rs`、`layout.rs`（几何不变量 + 英文标签下的同一套）、`updater.rs`（关于页的
更新相位/安装渠道渲染）。需要事件循环的那一批在 `async_contracts/` 目录里按界面分组：
`bus.rs`（引擎 → 窗口：`DownloadEvent` 订阅者 + 周期重同步）、`selection.rs`（爆炸半径、批量、
remove/purge、失败回滚）、`hotkeys.rs`（`Delete`/`Shift+Delete`/`Space` + 模态吞键）、`rows.rs`
（卡片/表格行内按钮、双击行为）、`new_task.rs`（提交载荷 + 深链/magnet/拖入/批量等外部入口）、
`toolbar.rs`（`Pause All`/`Resume All`/`Clear Completed`）、`dialogs.rs`（设置/实验室/向导保存成功与
拒绝、工厂重置确认门）。

两种 fixture，选错会直接报错：

- `with_ui` / `with_settings`：**默认**。每线程装一次 `init_no_event_loop()`（mock 时间、无线程队列），
  因此一个进程里可以跑任意多个测试（`cargo test` 和 nextest 都可）。**断言必须同步**：timers 不触发、
  `invoke_from_event_loop` 不投递。fixture 会 `enter()` 一个 current-thread runtime 让回调里的
  `tokio::spawn` 不 panic，但从不驱动它。
- `with_language(Language::EnUs, …)`：同一套窗口，但用英文目录。`build_ui` 通过
  `slint::select_bundled_translation` 应用它，所以 `@tr` 字符串真的会切换（`layout.rs` 靠这个看英文
  标签的布局）。该选择是**进程级**的：之后每个新窗口都会重新应用自己的语言，而中途切语言的测试
  （向导的语言卡片）必须自己切回去——`cargo test` 同进程跑兄弟测试，nextest 不会。
- `with_ui_async` + `TestUi::pump_until` / `pump`：需要“回调里 spawn 的后台结果”时用。Slint 的
event-loop proxy 是**全局** `OnceCell`，所以 `init_integration_test_with_mock_time()` 一个进程只能装一次
  —— 所有这类场景必须在**同一个** `#[test]`（`async_contracts::event_loop_contracts`）里，每个场景用
  `new_window()` 拿自己的窗口。新增场景 = 在 `async_contracts/` 对应的分组文件加一个 `pub(super) async fn`，
  再在 `mod.rs` 的 `event_loop_contracts` 调用列表里加一行；不要新开 `#[test]`。
  一次 pump 轮次 = 推进 mock 时钟 → 让 tokio 跑 spawned 任务 → 跑 Slint 事件循环排空队列；`pump_until`
  有轮次上限，不收敛就失败（不会挂死 CI）。

写新测试时：

- **每个测试独立线程**：Slint 的 context 是 thread-local，mock backend 必须**每线程**装一次。不要改成
  进程级 `Once`：那样只会让第一个线程有 platform，其余测试的 `MainWindow::new()` 会“platform not
  initialized”。
- **元素查找依赖 debug info**：`build.rs` 在 `PROFILE=debug` 时自动打开
  （`slint_build::CompilerConfiguration::with_debug_info`）；release 不带（省体积）。缺失时不会报错，
  只会打印 warning 并“找不到元素”，表现为 `no element with id …` 后跟实际存在的 id 列表。
- **`id:` 是测试契约**：新测试通常要顺手给 `.slint` 元素加 `id:`（snake_case：`ta_set`、`close_btn`、
  `cat_downloading`、`batch_purge_btn`…）。`for` 循环里的 id（`TaskTable::ta_row`）用 `find_all()`
  按模型顺序取第 N 个：`click_nth` / `right_click_nth`。
- **优先点真控件**（`click`），`invoke_*` 只用于两种情况：一个 id 对应多个元素而不关心具体哪个，
  或者被测价值在 Rust 侧的状态机（如重写规则的索引维护）。
- **断言爆炸半径**，不只看属性：`TestUi::core`（`RecordingBackend`）记录了 UI 对引擎的每一次调用，
  `assert_eq!(ui.core.purges(), vec![…])` 才能区分“删了记录”与“删了文件”。它注册在 `TaskKind::Http`
  上，种子任务必须用 `http_task(n, …)`（合法 UUID，否则 `TaskAction` 静默跳过）。
- **提交载荷断言 `ui.core.starts()`**：`CoreCall::Start` 带一份 `StartCall`（url/dir/file_name/
  checksum/expected_checksum/selected_file_indices），是“表单收集到的东西真的变成了
  `StartDownloadRequest`”的唯一抓手。两点注意：`start()` 返回**合成的 Ok id**（成功路径要关对话框、清
  URL、弹 toast），而其余 mutation 故意返回 `NotFound`（失败回滚靠它）；`.torrent` / `magnet:` 会被
  `Dispatcher::start` 归类为 BT、送到未注册的后端，载荷测试要用 http URL（选文件列表是由
  `preview_state` + ctx 缓存驱动的，与 URL 无关）。设置/实验室/向导的保存走的是同一个
  `Dispatcher::save_settings`，用 `ui.core.settings_pushes()` 计数。
- **断言助手**：`assert_inside_window`（附带 `id_tree()` 便于定位失败）、`assert_min_size`、
  `assert_no_overlap`、`assert_toast`/`toasts`/`dismiss_toast`、`bounds`/`window_logical_size`。
- **窗口与 DPI**：`set_window_size` 用逻辑像素；逻辑窗口尺寸要读**根元素**（`window_logical_size()` 内部
  用 `ElementQuery::from_root`）而不能拿 `Window::size()/scale_factor()` 除——testing backend 的
  `set_size` 固定按 scale 1.0 换算，而 `scale_factor()` 报的是宿主机 DPI（CI 与笔记本不一致）。
- **键盘**：`click` 到文本框会把焦点交给它，上行到不了根 `FocusScope`；快捷键测试要在**不点击**的前提下
  `press_keys(&[Key::Control.into(), 'n'])`（此时焦点在 `root_focus` 上）。`type_text` 往当前焦点输入。
  这两个 helper 是自己用公开的 `Window::dispatch_event` 实现的：crate 的 `internal` feature **无法编译**
  （`configure_test_fonts` 用 `include_dir!` 引了一个只存在于 Slint 源码仓库的路径），所以进程内
  **没有**截图 / `take_debug_log` / `set_locale`——像素与调试日志继续走 L2/MCP。
- **裁剪 = 不在元素树里**：`is_visible()` 在元素几何完全落在裁剪矩形外时为 false，元素查询会跳过它。
  所以靠下的控件要先滚进视野（关于页是 `ScrollView`，危险区在最底部）：往 `SettingsDialog::modal`
  中间派发 `WindowEvent::PointerScrolled`（`delta_y` 负值向下）再 `pump`（Flickable 的滚轮滚动是动画）
  ——见 `async_contracts/dialogs.rs` 的 `scroll_danger_zone_into_view`。同一原因：testing backend 的窗口
  默认 **800x600**，要整页可见先 `set_window_size(1100, 1600)`（设置对话框最高 820px）。
- **读文本/启用态用 a11y**：`Text` 默认带 `accessible-role: text` + `accessible-label: text`，共享的
  `PrimaryButton`/`SecondaryButton`/`DangerButton` 也声明了 `accessible-role`/label/enabled，所以
  `ui.find(id).accessible_label()/accessible_enabled()` 是“用户看到什么”的断言口（`updater.rs` 用它钉住
  相位 → 文案/按钮矩阵）。`accessible-action-set-value` 仍不可用（见下）。
- **修饰键点击**：指针事件本身不带 modifiers，core 用**当前按住的修饰键**填 `PointerEvent.modifiers`，
  所以 `TestUi::shift_click_nth`（按下 Shift → 点击 → 抬起）就是真实的 shift 范围选择，
  `mock_single_click` 表达不了。
- **点对话框里的控件有两个坑**：打开新建任务对话框会 spawn 一个系统剪贴板预填（它只填空的 URL 字段，
  但宿主机剪贴板内容不可控）——先在断言前 `pump` 排空它；`NewTaskDialog::modal` 的高度有 200ms 动画
  （470 ↔ 600px），状态翻转后立刻点击底部按钮会因按压/抬起的坐标跨越移动中的页脚而被丢弃，需先
  `pump` 到动画结束。
- **表格是横向裁切的，不是横向滚动的**：列比窗口宽时右侧数据列被裁，操作列固定在右边缘
  （`root.width - 138px`）。所以表格相关断言要在声明的最小窗口（1100x660）下写：`assert_inside_window
  ("TaskTable::ta_explorer")` 必须通过，而右侧数据列不在元素树里（被裁）。
- **`assert_toast` 要求“恰好一条”**，只适合确定性的单条通知：向导 `finish_setup` 会在路上额外触发
  `sync_aria2_rpc` 之类的副作用 toast，那里要改成在 `ui.toasts()` 里找那一条 success（见
  `the_setup_wizard_persists_its_form_and_remembers_where_it_was`）。
- **输入框类控件不能用 `set_accessible_value`**：`accessible-action-set-value` 是需要 .slint 显式声明的
  回调，app 自写的 `SearchInput` 没声明（只有 std-widgets 的 `LineEdit` 有），所以 `TestUi::search()`
  是“点入焦点 + `type_text`”。
- **排序/种子顺序**：列表默认按创建时间**降序**，`http_task(n, …)` 故意让 `created_at_ms` 随 n 递减，
  这样第 n 个任务的显示行号就是 n；批量操作作用在 `HashSet` 选择集上，断言要**排序后再比**。
- 依赖版本：`i-slint-backend-testing` **不遵守 semver**，在根 `Cargo.toml` 里与 `slint`/`slint-build`
  用 `=x.y.z` 钉死，升级时三者一起改。

**L2 —— MCP server**（无测试代码；给 agent 和人工观察真实运行的窗口）：

```cmd
set "SLINT_EMIT_DEBUG_INFO=1"
set "SLINT_MCP_PORT=8080"
cargo run -p limedl-native --features slint/mcp
```

（`set "V=值"` 的引号写法是必需的：写成 `set V=值 && …` 时 cmd 会把 `&&` 前的空格算进值里，
`SLINT_MCP_PORT` 会变成 `"8080 "`，端口解析失败后 server 静默不启动。）

`http://127.0.0.1:8080/mcp` 是 MCP Streamable HTTP（JSON-RPC），用 `curl` 就能调：元素树、
`take_screenshot`、click、drag、type、按键事件。它不是 CI 断言手段（端口、时序、unstable API），
而是“让 agent 自己点开 UI 看状态”：

- `--features slint/mcp` **只走命令行**，不要写进 `[features]` 表：它会把 `prost`/`protox` 拖进 release 依赖。
- **元素内省需要编译器嵌入的 debug 元数据**：`build.rs` 在 `PROFILE=debug` 时自动打开，所以 `cargo run` /
  `cargo test` 免配置；release 需要在**构建时**设 `SLINT_EMIT_DEBUG_INFO=1`（`CompilerConfiguration::new()`
  会读它，`build.rs` 只在 debug 下强制打开、不会把 release 的显式设置覆盖掉）。没有元数据时所有 id
  查找静默返回空，只打一条 warning。
- **先退出正在运行的 limedl**：单实例守卫会让第二个进程通知主实例后立即退出（exit 0、无报错），
  端口永远不会打开 —— 看起来像 server 挂了。用 `Get-Process limedl-native` 确认。
- **不要加 `--hidden`**：MCP server 由“首个窗口 shown”的钩子启动，而 `--hidden`（在 `setup_completed`
  为真时）根本不会 `show()` 窗口，所以 server 不会起来；MSIX 登录启动同理。
- 建议用 `LIMEDL_DATA_DIR=%TEMP%\limedl-mcp` 隔离会话，不必动真实 settings/数据库。
- 无 display（CI/容器）加 `SLINT_BACKEND=headless`；该取值只在 `mcp` feature 编译进来时存在，
  Slint 自标 unstable，只给自动化用。
- server 只绑 `127.0.0.1`、无鉴权、会校验 `Origin`，是本地开发工具，不要外暴。
- `slint/mcp` 属于开源 `slint` crate；Python 的 `slint_testing`（`testing.slint.dev` 的
  “GUI Test Framework”）是**商业**授权产品，不要假设可用。

### 下载完整性 / 损坏检测测试（Rust 集成 E2E）

针对"下载完成后 SHA 与源不一致"这类偶发数据损坏 bug，提供字节级 oracle 层：

- `tests/corruption_oracle_tests.rs` — 走完整引擎后**独立重读落盘文件**，用 SHA-256 与源内容比对（而非只看 `state == Completed`）。参数矩阵覆盖 多线程×大小（含参差尾块）×校验模式×迭代；确定性内容（seed 42）下任何一次失败都是真实引擎非确定性，视为 bug 报告而非 flaky。
- `tests/adversarial_interception_tests.rs` — 用坏服务器（range 错位 / 每段首字节翻转）验证：提供了 `expected_checksum` 时必须 `Failed` 拦截，且失败时保留 `.corrupt` 临时文件供取证。
- `tests/resume_corruption_tests.rs` — pause→resume 后仍字节级一致（多线程半途打断 + 带宽限速下的中/尾段暂停）。
- `tests/buffer_integrity_tests.rs` — SSD/HDD 写合并缓冲在**写入异常**下的完整性：通过 `buffer_pool::fault`（仅 test-utils 编译）对后台 flush 批量写注入确定性 I/O 失败，验证缓冲把失败标记为 degraded、`flush_all` 报错、已写入字节不损坏。
- `file_ops::tests::finalize_temp_file_cross_device_*` — 跨文件系统最终化的完整性（Linux 用 `/dev/shm` 与 `tempdir()` 两个设备，macOS 单卷时跳过）：目标文件逐字节一致、无 `.finalizing.*` 残留；staging 无法分配时 finalize 必须失败且源文件原封不动。
- `tests/persistence_tests.rs::downloading_survives_restart_without_stale_claims` — 进程被杀（状态仍是 `Downloading`）后的恢复：进度与 chunk 图保留、连接/分配计数归零、**陈旧 chunk claim 被清空**（否则重启后续传会跳过这些块）。
- `checksum::detect` 的解析单测 — 恶意/畸形 checksum 文档不得产生错误的 `expected_checksum`：注释/PGP 噪声行、GNU/BSD 变体、名字不匹配、畸形行全部拒绝；HTTP 探测忽略 HTML 404 页面，并可从目录清单（`SHA256SUMS`）中取出匹配项。

> 引擎未显式传 `expected_checksum` 时不做自动比对（产品行为，测试仅快照该现状并由 oracle 独立标记坏文件）——不要为了让这些测试全绿而绕过损坏检测。

### 日志轮转 / 保留测试

`logging.rs` 的 `#[cfg(test)] mod tests` 只测文件系统层，不碰进程级 `LOGGER_CONTROL`（`init_logging` 只能成功一次），所以可与其它 lib 测试同进程安全运行：

- `rotate_startup_logs_shifts_without_losing_content` — 右移轮转必须从**最大编号降序** rename，测试逐文件断言内容映射；升序会先把 `.1` 覆写到 `.2`，静默丢掉最旧的日志。
- `perform_startup_rotation_skips_while_lock_is_held` — 用 `File::try_lock` 模拟第二个实例持锁，断言轮转被跳过、释放后才执行（桌面 + NAS 同机并发的安全网）。
- `cleanup_by_count_keeps_exactly_the_limit`（边界 `num > count`，`Some(0)` 清空）与 `cleanup_by_age_removes_only_old_files`（用 `File::set_modified` 造 10 天前的 mtime，当前日志也参与）。
- `dynamic_file_writer_honors_enabled_and_survives_open_failure` — 禁用时写入即丢弃、启用时创建父目录、打开失败（路径是目录）降级为 sink 而非 panic（失败只走 stderr，这是日志模块的约定）。

### Dispatcher / Aria2 RPC 测试

- `tests/dispatcher_tests.rs` — 门面矩阵：生命周期事件的发射（pause/resume/cancel/remove/purge/set_priority）、`status/list/has_active_downloads` 聚合、无服务时的降级分支、`save_settings` 对 ConcurrencyManager / BufferPool / 持久化设置的同步（Traditional vs Automatic 两种线程上限）、CDN 禁用时 `clear()` 真的执行、`resolve_mirror_urls` 重写规则、`fetch_tracker_list` 归一化、`probe_checksum` 从 URL 推导文件名。辅助函数 `make_manager`/`inject_download` 为 `pub(crate)`：aria2 测试复用它们覆盖 `resolve_gid` 与 GID 缓存逐出（避免复制 ManagedDownload fixture）。
- `aria2_rpc/tests.rs` — 纯函数 + `process_jsonrpc_message` 的解析错误/版本错误/未知方法/成功四条分支，以及 `resolve_gid` 的扫描→缓存→`aria2.remove` 逐出链路。
- `aria2_rpc/e2e_tests.rs` — 真实 HTTP 服务器：handler 矩阵、multicall 响应形状、secret 全方法门控、CORS 白名单与不可解析配置的 localhost 回退、端口冲突报错；magnet 经 addUri 路由到 BT、addTorrent 非法 base64 拒绝；changeOption 的 pause/拒绝矩阵；keys 字段过滤；getUris 镜像列表；removeDownloadResult 单条删除；以及“内存淘汰后终态任务回查 DB”的 11 任务场景。
- `aria2_rpc/tests.rs` 另有：URI 分类、`parse_select_file` 1→0 基转换、`filter_status_keys`、`BtFileStatus → aria2 files` 映射，以及“每个生命周期转换只发一次 aria2 通知”的契约测试（HTTP 真实下载驱动 pause/unpause/remove）。

### BT backend 测试

`bt_backend/tests/` 用 `make_backend()`（关掉 DHT/LSD/UPnP/PEX/uTP，端口 0）起真实 irontide session，按场景拆文件：

- `queries.rs` — 预览/peers/trackers/pieces/file 列表、文件选择、限速、runtime status、pending summary。所有走 `block_in_place` 的用例必须用 `#[tokio::test(flavor = "multi_thread")]`（current-thread 下 `block_in_place` panic）。torrent fixture 是 `tests/mod.rs` 里手写的 bencode（`multi_file_torrent_bytes()`，piece hash 全零：解析只需要结构），用来覆盖“有 metadata”那条路；`single_file_torrent_bytes()` 覆盖没有 `files` 列表的单文件形状（依赖 `get_torrent_files()` 自己合成条目）；magnet 覆盖“没 metadata”那条路。
- `alerts.rs` — `handle_alert`（从 `alert_bridge_loop` 抽出的映射本体，`pub(super)`）直接喂合成 `AlertKind`，逐条断言映射表（aria2 通知名、gid、Updated/Progress 形状）；只打日志的告警必须不发事件。`setup_alert_bridge()` 在 return 前就 `session.subscribe()`，所以“setup 后立刻 start torrent”不会丢 `TorrentAdded`——最后一个用例就靠这个顺序做端到端断言。
- `anti_leech.rs` / `uploads.rs` — 后台循环用 `spawn_*_loop()` + “interval 第一次 tick 立即触发”跑一轮 sweep：断言 ban/slot-state 的清理、过期 ban 的 sweep、限制清零后的 unpause；需要真实上传量才能命中的 pause/ban-leecher 分支离线覆盖不到（`get_peer_info` 为空时循环提前 return）。
- 循环类测试的同步方式：能等事件就等事件（`rx.recv()` + timeout），否则 `wait_until()` 轮询状态；不要靠 sleep 猜时长。
