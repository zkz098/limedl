# Testing Guide — limedl

## 模块职责

测试策略、mock 模式、运行命令和 CI 描述的汇总。

## 涉及文件

- `src/__tests__/` — 前端测试（Vitest + jsdom）
- `src/__tests__/mocks/tauri-mock.ts` — Tauri IPC mock 系统
- `src/__tests__/fixtures/downloads.ts` — Mock DownloadSummary 工厂
- `crates/limedl-core/src/tests/` — Rust 集成测试（manager_tests 等）
- `e2e/` — Playwright E2E 测试（CI 暂不执行）
- `e2e/playwright.config.ts` — E2E 配置
- `.github/workflows/ci.yml` — CI 配置文件

## 数据流向

```
代码变更 → CI 触发（6 job 矩阵）:
  ├─ lint-typescript (ubuntu): pnpm install → oxlint → vue-tsc → vitest
  ├─ check-windows: cargo clippy -D warnings → 3× per-crate test
  │   (core: test-utils,aria2-rpc / server: 无额外 feature / tauri: test-utils)
  ├─ check-macos: 同 check-windows（macOS-14）
  ├─ check-rust (ubuntu): clippy → ts-rs freshness check → 3× per-crate test
  ├─ bench-rust: cargo bench (aimd + rate_limiter)
  └─ supply-chain: cargo deny check + cargo audit
```

## 设计决策与约定

### 前端测试（Vitest）

- Tauri IPC 调用通过 `src/__tests__/mocks/tauri-mock.ts` 模拟。核心模式：`vi.mock("@tauri-apps/api/core")` → `mockTauriCommandValue()` 注册返回值 / `mockTauriCommand()` 注册动态 handler。
- i18n 通过 `vi.mock("path/to/i18n")` 模拟，返回原始 key 或带插值。
- Composables 接受 refs 作为参数（非全局状态），通过 helper factory 创建。
- 运行：`pnpm run test`，可选 `--watch` 或指定文件。

### Rust 测试

- 单元测试：内联在源码文件底部 `#[cfg(test)] mod tests`。
- 集成测试：`crates/limedl-core/src/tests/`（manager_tests.rs 等，使用本地 axum HTTP mock 服务器 + tempfile 临时目录）。
- 每 crate 独立测试命令（core 带 `test-utils,aria2-rpc`，server 无额外 feature，tauri 带 `test-utils`）。
- Windows 上必须先初始化 MSVC 环境（vcvarsall.bat x64），否则 clippy/test 因链接器失败。
- 依赖：axum（HTTP mock）、tempfile、ntest（超时注解）。

### E2E 测试（Playwright）

- 框架已配置但 CI 暂不执行（需要桌面环境）。
- 运行前提：`pnpm run tauri dev`（终端 1），然后 `pnpm run test:e2e`（终端 2）。
- 配置：Chromium 固定、headless、60 秒超时、失败时截图。
- 使用 `data-testid` 属性定位元素（当前 smoke 测试使用 CSS class fallback）。
- 测试需要真实 URL 或本地文件服务器（Tauri webview 不支持 localhost mock）。

### CI 已知警告

- Windows release build 的 `LNK4078`（多 `.rsrc` 段）警告——`build.rs` 手工嵌入 ComCtl32 v6 manifest 与 tauri-winres 重复嵌入。**无害**，不要删除 build.rs 的 manifest 代码。

### 测试编写优先级

1. Rust 集成测试（manager_tests.rs 扩展）：完整下载流程（单流、多流、断点续传、checksum 验证、取消/暂停）
2. E2E 测试（e2e/tests/）：核心用户流程
3. Rust 单元测试：buffer_pool、scheduler、database 关键逻辑
4. 前端 composable 测试：useLimedl、useDownloadActions、useDownloadForm

### ts-rs 绑定新鲜度

CI 的 check-rust job 中：`cargo test --features ts export_typescript_bindings` 后执行 `git diff --exit-code src/types/generated/ src/lib/ws/generated/`，确保生成的 `.ts` 文件与 Rust 源同步。修改 Rust 序列化类型后必须运行此步骤并提交生成的文件。
