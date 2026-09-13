# Testing Guide — limedl

## 模块职责

测试策略、mock 模式、运行命令和 CI 描述的汇总。

## 涉及文件

- `src/__tests__/` — 前端测试（Vitest + jsdom）
- `src/__tests__/mocks/invoke-mock.ts` — IPC invoke mock 系统（`mockCommand` / `mockCommandValue` / `createMockInvoke`）
- `src/__tests__/fixtures/downloads.ts` — Mock DownloadSummary 工厂
- `crates/limedl-core/src/tests/` — Rust 集成测试（manager_tests 等）
- `e2e/` — Playwright E2E 测试（CI 暂不执行）
- `e2e/playwright.config.ts` — E2E 配置
- `.github/workflows/ci.yml` — CI 配置文件

## 数据流向

```
代码变更 → CI 触发（9 job 矩阵；纯文档改动被 paths-ignore 跳过）:
  ├─ lint-typescript (ubuntu): pnpm install → oxlint → vue-tsc → vitest
  ├─ e2e-nas-webui (ubuntu): build:nas → release limedl-server → Playwright (nas-webui)
  ├─ check-windows (windows): clippy --workspace -D warnings → server --features tls
  ├─ test-windows-core (windows): limedl-core 测试（nextest）
  ├─ test-windows-native (windows): server 测试 → limedl-native 测试（nextest）
  ├─ check-macos (macOS-14): clippy → core 测试 → server 测试（nextest）
  ├─ check-rust (ubuntu): clippy → ts-rs freshness check → per-crate coverage
  ├─ bench-rust: cargo bench (aimd + rate_limiter)
  └─ supply-chain: cargo deny check + cargo audit
```

Windows 拆成**三个**并行 job 是因为它是最慢的平台：`cargo clippy` 只做 check、无法与测试
构建共享产物，串行只会累加墙钟时间。实测单步耗时：clippy ≈ 2.7 min、limedl-core 测试
≈ 2.6 min、server + native 测试 ≈ 5.8 min；拆开后 Windows 关键路径从 ~6.3 min 降到
~5.8 min，整条流水线的瓶颈随之变成 macOS（≈ 6.9 min）。同一 ref 的旧 run 由
`concurrency` 直接取消。

### CI 缓存 / RUSTFLAGS 约定

- **`setup-rust-toolchain` 必须带 `cache: false`**：该 action 默认（`cache: true`）
  内部会跑一遍 rust-cache，与工作流里显式的 `swatinem/rust-cache` 重复，等于每个 job
  存两份 1-2 GB 缓存。仓库缓存上限 10 GB，重复条目把 release 缓存挤掉后，每次发版
  都要冷编译 Slint/Skia（实测 997 s）。
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
  - `.cargo/config.toml` + 外部依赖 hash 组成，**不含源码**；只有恢复不完整（key 不
    完全匹配）时才会回写缓存，完整命中时不会覆盖。
- 桌面 release 构建的缓存由 `.github/workflows/warm-release-cache.yml` 在 main 上预热，
  与 `release.yml` 的 `build-native` 共用同一 key（`add-job-id-key: false`）；
  NAS 构建的 WebUI 由 `release.yml` 的 `build-frontend` 构建一次后用 artifact 分发。

### CI 测试执行 / 构建速度

- **Rust 测试统一用 `cargo nextest`**（由 `taiki-e/install-action` 安装，版本在 workflow 里
  pin）：nextest 为每个测试启动独立进程，既并行执行，也消除了 libtest 单进程共享全局状态
  带来的兄弟测试互扰（历史上需要“一个测试独占一个文件”的 workaround）。注意
  `cargo nextest run` **不执行 doctest** —— 目前 workspace 没有 doctest；若将来新增，
  需在 `check-rust` 补一个 `cargo test --doc` 步骤。覆盖率 job 仍走 `cargo llvm-cov`。
- **`[profile.test] debug = false`**（根 `Cargo.toml`）：CI 每个 job 都要编译并链接测试二进制，
  而依赖早已通过 `[profile.dev.package."*"]` 跳过 debug info（`cargo test --no-run -v` 可见
  `-C strip=debuginfo`，且该 override 会被 `test` profile 继承），workspace 自己 crate 的
  line tables 只剩开销。实测仅重建 `limedl-core` 测试目标：`debug = false` 5.9 s，
  保留 line tables 15.6 s。本地要断点/行号：`cargo test --profile dev`；覆盖率的
  `cargo llvm-cov --profile dev` 也是为了 lcov 的行号归属。release 产物用的是独立 profile，
  不受影响。
- **Windows job 排除 Defender 实时扫描**（`Add-MpPreference -ExclusionPath`，best-effort、
  失败不挂 job）：Defender 会逐个扫描 cargo 写入 `target/` 的多 GB 文件，是 Windows 相对
  Linux 的主要惩罚项。新增 Windows 步骤时不要导出 `RUSTFLAGS`/`CARGO*`/`CC*`/`CMAKE*`
  环境变量，否则会分裂 rust-cache key（同上文约定）。

## 设计决策与约定

### 前端测试（Vitest）

- IPC 调用通过 `src/__tests__/mocks/invoke-mock.ts` 模拟。核心模式：`vi.mock("#invoke", () => ({ invoke: vi.fn() }))` → `mockCommandValue()` 注册返回值 / `mockCommand()` 注册动态 handler / `resetInvokeMocks()` 在每个测试前清空。模块级函数（如 `src/lib/ipc/*-api.ts` 里的包装）可直接用 `vi.mock` 替换。
- i18n 通过 `vi.mock("path/to/i18n")` 模拟，返回原始 key 或带插值。
- Composables 接受 refs 作为参数（非全局状态），通过 helper factory 创建。
- 运行：`pnpm run test`，可选 `--watch` 或指定文件。

### Rust 测试

- 单元测试：内联在源码文件底部 `#[cfg(test)] mod tests`。
- 集成测试：`crates/limedl-core/src/tests/`（manager_tests.rs 等，使用本地 axum HTTP mock 服务器 + tempfile 临时目录）。
- 每 crate 独立测试命令（core 带 `test-utils,aria2-rpc`，server 无额外 feature，limedl-native 走单独 step 并覆写 `RUSTFLAGS`）。
- **全局状态必须独占一个测试文件**：cargo 把单个 `tests/*.rs` 当做一个进程跑，而 `tracing_subscriber::fmt().init()` 之类的调用会占用进程级全局槽，同文件内的兄弟测试会与之竞争并 panic（CI 曾因此偶发红）。这类测试放独立文件：`tests/logging_reload_repro.rs`（干净进程）与 `tests/logging_preinstalled_subscriber.rs`（预装全局订阅者）就是例子。
- Windows 上必须先初始化 MSVC 环境（vcvarsall.bat x64），否则 clippy/test 因链接器失败。
- 依赖：axum（HTTP mock）、tempfile、ntest（超时注解）。

### 下载完整性 / 损坏检测测试（Rust 集成 E2E）

针对"下载完成后 SHA 与源不一致"这类偶发数据损坏 bug，提供字节级 oracle 层：

- `tests/corruption_oracle_tests.rs` — 走完整引擎后**独立重读落盘文件**，用 SHA-256 与源内容比对（而非只看 `state == Completed`）。参数矩阵覆盖 多线程×大小（含参差尾块）×校验模式×迭代；确定性内容（seed 42）下任何一次失败都是真实引擎非确定性，视为 bug 报告而非 flaky。
- `tests/adversarial_interception_tests.rs` — 用坏服务器（range 错位 / 每段首字节翻转）验证：提供了 `expected_checksum` 时必须 `Failed` 拦截，且失败时保留 `.corrupt` 临时文件供取证。
- `tests/resume_corruption_tests.rs` — pause→resume 后仍字节级一致（多线程半途打断 + 带宽限速下的中/尾段暂停）。
- `tests/buffer_integrity_tests.rs` — SSD/HDD 写合并缓冲在**写入异常**下的完整性：通过 `buffer_pool::fault`（仅 test-utils 编译）对后台 flush 批量写注入确定性 I/O 失败，验证缓冲把失败标记为 degraded、`flush_all` 必须报错、**已写入字节不损坏**（只丢失败批次的数据并以错误上报，绝不静默成功）；另有一条全管道 E2E（强制 SSD ping-pong 缓冲 + 多线程）复现真实报告的 `SSD ping-pong buffer flush failed`，必须 `Failed` 而非静默 `Completed`。故障按下载 id 定点、并全局串行锁隔离，避免并行测试互相干扰。

配套 harness 新增端点：`/file/range-shifted/{shift}`（Content-Range 错位）与 `/file/range-bitflip`（每段内容翻转），用于模拟真实世界里 CDN/代理返回错误字节的损坏源。

> 引擎未显式传 `expected_checksum` 时不做自动比对（产品行为，测试仅快照该现状并由 oracle 独立标记坏文件）——不要为了让这些测试全绿而绕过损坏检测。

### E2E 测试（Playwright）

- 框架已配置；CI 只跑 `nas-webui` 项目（需要真实或 mock 的 `limedl-server`）。
- 运行前提：启动 `limedl-server daemon`（如 `cargo run --bin limedl-server daemon -- --user e2e --pass e2epass`），然后 `pnpm run test:e2e:nas`。
- 配置：Chromium 固定、headless、60 秒超时、失败时截图。
- 使用 `data-testid` 属性定位元素（当前 smoke 测试使用 CSS class fallback）。
- 需要真实 URL 或本地文件服务器（`e2e/server/test-file-server.ts`，由 global-setup 启动）。

### CI 已知警告

目前无未解决的警告（详见 `troubleshooting.md`；旧条目均随 Tauri 版退役而消失）。

### 测试编写优先级

1. Rust 集成测试（manager_tests.rs 扩展）：完整下载流程（单流、多流、断点续传、checksum 验证、取消/暂停）
2. E2E 测试（e2e/tests/）：核心用户流程
3. Rust 单元测试：buffer_pool、scheduler、database 关键逻辑
4. 前端 composable 测试：useLimedl、useDownloadActions、useDownloadForm

### ts-rs 绑定新鲜度

CI 的 check-rust job 中：`cargo test --features ts export_typescript_bindings` 后执行 `git diff --exit-code src/types/generated/ src/lib/ws/generated/`，确保生成的 `.ts` 文件与 Rust 源同步。修改 Rust 序列化类型后必须运行此步骤并提交生成的文件。
