# Testing Guide — downloader

> Core test patterns, mock setup, and commands. Focus on patterns, not exhaustive examples.

## Quick Reference

| Layer | Command | Framework | Location |
|---|---|---|---|
| Frontend unit | `bun run test` | Vitest + jsdom | `src/__tests__/` |
| Rust unit | `cargo test --workspace` | Rust `#[test]` | 内联 `#[cfg(test)]` |
| Rust integration | `cargo test --workspace` | Rust `#[test]` | `src-tauri/src/download/tests/` |
| E2E | (pending setup) | Playwright | `e2e/` |

**Before any Rust test on Windows**, initialize MSVC:
```powershell
& "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
```

---

## Frontend Testing (Vitest)

### Mock Pattern

All Tauri IPC calls go through a mock system in `src/__tests__/mocks/tauri-mock.ts`:

```ts
import { vi } from "vitest";

// 1. Mock the Tauri invoke function
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import {
  createMockInvoke,
  mockTauriCommand,
  mockTauriCommandValue,
  resetTauriMocks,
} from "../mocks/tauri-mock";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  resetTauriMocks();
  mockInvoke.mockImplementation(createMockInvoke());
});

afterEach(() => {
  vi.clearAllMocks();
});
```

### Mocking Tauri Commands

```ts
// 注册返回值（最简单）
mockTauriCommandValue("download_list", [
  { id: "1", fileName: "test.zip", state: "downloading", ... }
]);

// 注册动态 handler（需要逻辑时）
mockTauriCommand("download_start", (args) => {
  if (!args?.url) throw new Error("URL required");
  return "http:generated-uuid";
});
```

### Mocking i18n

```ts
vi.mock("../../i18n", () => ({
  t: vi.fn((key: string) => key),           // 返回原始 key
  // 或带插值:
  t: vi.fn((key: string, options?: Record<string, unknown>) => {
    if (options) return `${key} ${JSON.stringify(options)}`;
    return key;
  }),
}));
```

### Test Fixtures

`src/__tests__/fixtures/downloads.ts` provides `createMockDownloadTask()` for building mock `DownloadSummary` objects:

```ts
import { createMockDownloadTask } from "../fixtures/downloads";

const task = createMockDownloadTask({
  id: "task-1",
  fileName: "test.zip",
  state: "downloading",
  downloadedBytes: 5000000,
  totalBytes: 10000000,
});
```

### Composables Testing Pattern

Composables accept refs as parameters (not global state). Create a helper factory:

```ts
function createList() {
  return useDownloadList({
    downloads: ref([]),
    selectedId: ref(null),
    selectedSnapshot: ref(null),
    allowAutoSelect: ref(true),
    isAutoRefreshing: ref(false),
    ensureSelection: vi.fn(),
    setMessage: vi.fn(),
    setError: vi.fn(),
  });
}
```

### Running

```bash
bun run test                # 运行所有测试
bun run test -- --watch     # watch 模式
bun run test path/to/test   # 运行单个测试文件
```

---

## Rust Testing

### Organization

- **单元测试**: 内联在源码文件底部 `#[cfg(test)] mod tests { ... }`
- **集成测试**: `src-tauri/src/download/tests/manager_tests.rs`（使用本地 HTTP mock 服务器）

### Test Helpers (manager_tests.rs pattern)

```rust
use axum::{Router, extract::State, routing::get};
use tempfile::tempdir;

// 创建本地 HTTP 测试服务器
fn single_file_state(path: &str, bytes: Arc<Vec<u8>>, etag: &str, delay_ms: u64) -> TestState {
    // 返回可 clone 的 TestState，包含文件数据和模拟延迟
}

// 启动 axum 服务器 + 创建 DownloadManager
// → 发送下载请求
// → 验证文件内容、校验和、状态转换
```

### Running

```bash
cargo test --workspace                          # 所有测试
cargo test --workspace -- --test-threads=1      # 单线程
cargo test -p downloader_lib                    # 仅 Rust 库
cargo test -p downloader_lib -- manager         # 按名称过滤
cargo test -p downloader_lib -- --nocapture     # 显示 println 输出
```

### Key Test Dependencies

```toml
[dev-dependencies]
axum        # HTTP mock 服务器
tempfile    # 临时目录
ntest       # #[timeout(ms)] 测试超时注解
```

---

## E2E Testing (Playwright)

### Current State

- 框架已配置：`e2e/` 目录存在，含 `playwright.config.ts`、`fixtures.ts`、`tests/smoke.spec.ts`
- Playwright 依赖 `@playwright/test` 安装在根 `package.json` devDependencies 中
- 运行前需先启动 Tauri 应用：`bun run tauri dev`（在另一个终端）

### E2E 脚本

定义在根 `package.json`：

| 命令 | 用途 |
|---|---|
| `bun run test:e2e` | 运行所有 E2E 测试（headless） |
| `bun run test:e2e:ui` | 打开 Playwright UI 模式运行测试 |

### 运行前提

```bash
# 终端 1: 启动 Tauri 开发模式
bun run tauri dev

# 终端 2: 运行 E2E 测试
bun run test:e2e
```

### 配置 (`e2e/playwright.config.ts`)

- 测试目录：`./tests`
- 超时：60 秒/测试
- 重试：0（本地和 CI 均无重试）
- Reporters：`html`（输出到 `playwright-report/`）+ `list`
- 浏览器：Chromium 固定（Tauri Windows/Linux 使用 Chromium）
- 截图：仅在失败时
- `baseURL`：`http://localhost:1420`（Vite dev server 端口）
- `headless: true`
- CI：目前不运行，因为需要桌面环境。将来可使用 `xvfb-run`（Linux）或 Tauri 测试工具

### Fixtures (`e2e/fixtures.ts`)

```ts
import { test as base } from "@playwright/test";

export const test = base.extend({
  // 未来：添加自定义 fixture（如自动启动 app、DB helpers）
});

export { expect } from "@playwright/test";
```

当前 fixture 只导出基础 `test` 和 `expect`。Tauri 应用需手动启动（通过 `bun run tauri dev`），测试通过 `page.goto("/")` 连接到 Vite dev server。

### Writing E2E Tests

```ts
// e2e/tests/download.spec.ts — 示例结构
import { test, expect } from "../fixtures";

test("add and start HTTP download", async ({ page }) => {
  // 1. 输入下载 URL
  await page.fill('[data-testid="url-input"]', "https://example.com/file.zip");
  // 2. 选择目标目录
  await page.click('[data-testid="browse-dir"]');
  // 3. 点击开始下载
  await page.click('[data-testid="start-download"]');
  // 4. 验证任务出现在队列中
  await expect(page.locator('[data-testid="download-row"]')).toBeVisible();
  // 5. 验证状态变为 downloading
  await expect(page.locator('[data-testid="status-badge"]')).toContainText("downloading");
});
```

### 现有 Smoke Tests (`e2e/tests/smoke.spec.ts`)

- `page loads and renders the app root`：验证 `#app` 挂载点、`.app-root` 元素可见、页面标题为 "Downloader"
- `main UI elements are present on the home view`：验证侧边栏和主内容区域存在

### Tauri E2E 注意事项

- Tauri webview 不支持 `localhost` 的本地 mock 服务器（安全限制），需要真实 URL 或本地文件服务器
- 使用 `data-testid` 属性定位元素，不要依赖 CSS 类名（smoke 测试目前使用 CSS class fallback，因为尚未添加 `data-testid`）
- 下载测试需要：一个可访问的测试文件服务器 + 足够的磁盘空间

---

## CI

CI（`.github/workflows/ci.yml`）在 Ubuntu 上运行，流水线如下：

1. `bun install --frozen-lockfile`
2. `bun run lint` (oxlint)
3. `bunx vue-tsc --noEmit`
4. `bun run test` (Vitest)
5. `cargo check --workspace`
6. `cargo clippy --workspace -- -D warnings`
7. `cargo test --workspace`

---

## Test Writing Priorities

当前测试覆盖不足。推荐优先级：

1. **Rust 集成测试**（`manager_tests.rs` 扩展）：覆盖完整下载流程（单流、多流、断点续传、校验和验证、取消/暂停）
2. **E2E 测试**（`e2e/tests/`）：覆盖核心用户流程（添加下载、暂停/恢复、删除、设置页面）
3. **Rust 单元测试**：`buffer_pool.rs`、`scheduler.rs`、`database.rs` 的关键逻辑
4. **前端 composable 测试**：`useDownloader.ts`、`useDownloadActions.ts`、`useDownloadForm.ts`
