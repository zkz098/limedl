import { defineConfig } from "@playwright/test";

/**
 * Playwright E2E configuration for limedl.
 *
 * Projects:
 *   1. tauri-desktop   — connects to Vite dev server (port 1420) for Tauri tests
 *   2. nas-webui       — connects to limedl-server (port 9090) for NAS WebUI tests
 *   3. nas-webui-firefox  — same as nas-webui but on Firefox
 *   4. nas-webui-webkit   — same as nas-webui but on WebKit (Safari)
 *   5. real-server     — runs real-server integration tests (chromium only)
 *                        requires a running limedl-server daemon
 *
 * Global setup starts TestFileServer on port 9876 and sets env vars.
 *
 * Tauri desktop tests require a running Tauri app (e.g. `pnpm run tauri dev`).
 * NAS WebUI tests require a running limedl-server (e.g. `cargo run --bin limedl-server daemon`).
 * Real-server tests also require LIMEDL_E2E_REAL_SERVER=1 to be set.
 */

// Determine browser for the default nas-webui project
const defaultBrowserName = (process.env.PLAYWRIGHT_BROWSER || "chromium") as "chromium" | "firefox" | "webkit";

export default defineConfig({
  testDir: "./tests",
  timeout: 60_000,

  globalSetup: "./global-setup",
  globalTeardown: "./global-teardown",

  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 2 : 1,

  reporter: [["html", { outputFolder: "playwright-report" }], ["list"]],

  use: {
    headless: true,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },

  projects: [
    {
      name: "tauri-desktop",
      use: {
        browserName: "chromium",
        baseURL: "http://localhost:1420",
      },
    },
    {
      name: "nas-webui",
      testMatch: ["**/*.spec.ts", "!**/integration-real.spec.ts"],
      use: {
        browserName: defaultBrowserName,
        baseURL: "http://localhost:9090",
      },
    },
    {
      name: "nas-webui-firefox",
      testMatch: ["**/*.spec.ts", "!**/integration-real.spec.ts"],
      use: {
        browserName: "firefox",
        baseURL: "http://localhost:9090",
      },
    },
    {
      name: "nas-webui-webkit",
      testMatch: ["**/*.spec.ts", "!**/integration-real.spec.ts"],
      use: {
        // WebKit requires the --headed flag or a display server on Linux CI.
        // On headless Linux, set PLAYWRIGHT_WEBKIT_HEADLESS=1 or use Xvfb.
        browserName: "webkit",
        baseURL: "http://localhost:9090",
      },
    },
    {
      name: "real-server",
      testMatch: ["**/integration-real.spec.ts"],
      use: {
        browserName: "chromium",
        baseURL: "http://localhost:9090",
      },
    },
  ],
});
