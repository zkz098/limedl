import { defineConfig } from "@playwright/test";

/**
 * Playwright E2E configuration for Tauri downloader
 *
 * IMPORTANT: These tests require a running Tauri app. They do NOT launch the app automatically.
 *
 * Prerequisites:
 *   1. Run `bun run tauri dev` in a separate terminal (starts Vite on port 1420 + Tauri window)
 *   2. Or run `bun run tauri build` and launch the built binary
 *   3. Then run `bun run test:e2e`
 *
 * CI: These tests are NOT run in CI yet because they need a desktop environment.
 *      Future: use `xvfb-run` on Linux or Tauri's test harness.
 */
export default defineConfig({
  testDir: "./tests",
  timeout: 60_000,
  retries: 0,
  workers: 1,
  reporter: [
    ["html", { outputFolder: "playwright-report" }],
    ["list"],
  ],
  use: {
    // Tauri uses Chromium on Windows/Linux, WebKit on macOS.
    // We pin chromium so tests are consistent across dev machines.
    browserName: "chromium",
    headless: true,
    // Vite dev server runs on port 1420 when using `bun run tauri dev`
    baseURL: "http://localhost:1420",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
});
