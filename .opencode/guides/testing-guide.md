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
代码变更 → CI 触发（6 job 矩阵）:
  ├─ lint-typescript (ubuntu): pnpm install → oxlint → vue-tsc → vitest
  ├─ check-windows: cargo clippy -D warnings → per-crate test
  │   (core: test-utils,aria2-rpc / server: 无额外 feature / limedl-native: 单独 step)
  ├─ check-macos: 同 check-windows（macOS-14）
  ├─ check-rust (ubuntu): clippy → ts-rs freshness check → per-crate coverage
  ├─ bench-rust: cargo bench (aimd + rate_limiter)
  └─ supply-chain: cargo deny check + cargo audit
```

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
