# Testing Guide — limedl

## 模块职责

测试策略、mock 模式、运行命令和 CI 描述的汇总。

## 涉及文件

- `crates/limedl-core/src/tests/` — 核心下载引擎集成测试（manager_tests 等）
- `crates/limedl-native/src/` — 桌面 UI 桥接与逻辑单测
- `.github/workflows/ci.yml` — CI 配置文件

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

### 下载完整性 / 损坏检测测试（Rust 集成 E2E）

针对"下载完成后 SHA 与源不一致"这类偶发数据损坏 bug，提供字节级 oracle 层：

- `tests/corruption_oracle_tests.rs` — 走完整引擎后**独立重读落盘文件**，用 SHA-256 与源内容比对（而非只看 `state == Completed`）。参数矩阵覆盖 多线程×大小（含参差尾块）×校验模式×迭代；确定性内容（seed 42）下任何一次失败都是真实引擎非确定性，视为 bug 报告而非 flaky。
- `tests/adversarial_interception_tests.rs` — 用坏服务器（range 错位 / 每段首字节翻转）验证：提供了 `expected_checksum` 时必须 `Failed` 拦截，且失败时保留 `.corrupt` 临时文件供取证。
- `tests/resume_corruption_tests.rs` — pause→resume 后仍字节级一致（多线程半途打断 + 带宽限速下的中/尾段暂停）。
- `tests/buffer_integrity_tests.rs` — SSD/HDD 写合并缓冲在**写入异常**下的完整性：通过 `buffer_pool::fault`（仅 test-utils 编译）对后台 flush 批量写注入确定性 I/O 失败，验证缓冲把失败标记为 degraded、`flush_all` 报错、已写入字节不损坏。

> 引擎未显式传 `expected_checksum` 时不做自动比对（产品行为，测试仅快照该现状并由 oracle 独立标记坏文件）——不要为了让这些测试全绿而绕过损坏检测。
