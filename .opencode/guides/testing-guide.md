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
  ├─ check-rust (Linux): clippy → per-crate coverage
  ├─ bench-rust (Linux): cargo bench (aimd + rate_limiter)
  └─ supply-chain (Linux): cargo deny check + cargo audit
```

Windows 拆成**三个**并行 job 是因为它是最慢的平台：`cargo clippy` 只做 check、无法与测试
构建共享产物，串行只会累加墙钟时间。实测单步耗时：clippy ≈ 2.7 min、limedl-core 测试
≈ 2.6 min、native 测试 ≈ 3 min；拆开后整条流水线的瓶颈在 macOS（≈ 6.9 min）。同一 ref 的旧 run 由
`concurrency` 直接取消。

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
  都要冷编译 Slint/Skia。
- **`rustflags: ""`**：该 action 默认导出 `RUSTFLAGS=-D warnings`，而 RUSTFLAGS 一旦
  存在就会**整体覆盖** `.cargo/config.toml` 的 rustflags（target-cpu / rust-lld /
  /FORCE:MULTIPLE 全部失效）。留空后 config.toml 是唯一的 flag 来源，所有步骤共用
  同一份指纹，不再出现同 job 内反复全量重编。
- 告警仍然是硬失败：`CARGO_BUILD_WARNINGS: deny`（cargo 的 `build.warnings`，与
  RUSTFLAGS 正交）配合 clippy 的 `-- -D warnings`。它是**逐 job** 设置的，不是 workflow
  级：cargo 的 `build.warnings` 同样会把**链接器告警**当错误，而用 rust-lld 链接 Skia
  版 limedl-native 必然告警（Skia 重复的 ICU 符号 `ubrk_getLocaleByType`，靠
  `/FORCE:MULTIPLE` 收敛）。因此 `test-windows-native` 不设该项（告警可见但不失败），
  Windows 的 lint 门禁仍由 `check-windows` 的全 workspace clippy 负责。
- rust-cache 的 key 由 rust 工具链 + 上述 RUST*/CARGO*/CC*/CFLAGS*/CXX*/CMAKE* 环境变量
  + `.cargo/config.toml` + 外部依赖 hash 组成，**不含源码**；只有恢复不完整（key 不
    完全匹配）时才会回写缓存，完整命中时不会覆盖。
- 桌面 release 构建的缓存由 `.github/workflows/warm-release-cache.yml` 在 main 上预热，
  与 `release.yml` 的 `build-native` 共用同一 key（`add-job-id-key: false`）。

### CI 测试执行 / 构建速度

- **Rust 测试统一用 `cargo nextest`**（由 `taiki-e/install-action` 安装，版本在 workflow 里
  pin）：nextest 为每个测试启动独立进程，既并行执行，也消除了 libtest 单进程共享全局状态
  带来的兄弟测试互扰。注意 `cargo nextest run` **不执行 doctest** —— 目前 workspace 没有 doctest；若将来新增，
  需在 `check-rust` 补一个 `cargo test --doc` 步骤。覆盖率 job 仍走 `cargo llvm-cov`。
- **`[profile.test] debug = false`**（根 `Cargo.toml`）：CI 每个 job 都要编译并链接测试二进制，
  而依赖早已通过 `[profile.dev.package."*"]` 跳过 debug info，workspace 自己 crate 的
  line tables 只剩开销。本地要断点/行号：`cargo test --profile dev`；覆盖率的
  `cargo llvm-cov --profile dev` 也是为了 lcov 的行号归属。release 产物用的是独立 profile，
  不受影响。
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

场景文件：`shell.rs`（工具栏/对话框/Esc 层级/快捷键）、`list.rs`（列表、选择、批量栏、右键菜单、
表格列）、`labs.rs`（重写规则编辑器、CDN 内联校验）、`settings.rs`（限速计划、设置对话框）、
`new_task.rs`（重置契约、批量计数、torrent 预选）、`inspector.rs`、`toast.rs`、`layout.rs`（几何
不变量，含两条已知缺陷的 characterization）、`async_contracts.rs`（需要事件循环的那一批：爆炸半径、
破坏性热键 `Delete`/`Shift+Delete`/`Space`、失败回滚、新建任务的提交载荷）。

两种 fixture，选错会直接报错：

- `with_ui` / `with_settings`：**默认**。每线程装一次 `init_no_event_loop()`（mock 时间、无线程队列），
  因此一个进程里可以跑任意多个测试（`cargo test` 和 nextest 都可）。**断言必须同步**：timers 不触发、
  `invoke_from_event_loop` 不投递。fixture 会 `enter()` 一个 current-thread runtime 让回调里的
  `tokio::spawn` 不 panic，但从不驱动它。
- `with_ui_async` + `TestUi::pump_until` / `pump`：需要“回调里 spawn 的后台结果”时用。Slint 的
event-loop proxy 是**全局** `OnceCell`，所以 `init_integration_test_with_mock_time()` 一个进程只能装一次
  —— 所有这类场景必须在**同一个** `#[test]`（`async_contracts::event_loop_contracts`）里，每个场景用
  `new_window()` 拿自己的窗口。新增场景就加个 `async fn scenario_*(…)` 并列进去，不要新开 `#[test]`。
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
  `preview_state` + ctx 缓存驱动的，与 URL 无关）。
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
- **点对话框里的控件有两个坑**：打开新建任务对话框会 spawn 一个系统剪贴板预填（它只填空的 URL 字段，
  但宿主机剪贴板内容不可控）——先在断言前 `pump` 排空它；`NewTaskDialog::modal` 的高度有 200ms 动画
  （470 ↔ 600px），状态翻转后立刻点击底部按钮会因按压/抬起的坐标跨越移动中的页脚而被丢弃，需先
  `pump` 到动画结束。
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

> 引擎未显式传 `expected_checksum` 时不做自动比对（产品行为，测试仅快照该现状并由 oracle 独立标记坏文件）——不要为了让这些测试全绿而绕过损坏检测。
