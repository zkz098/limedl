import { test as base } from "@playwright/test";

/**
 * Tauri E2E test fixtures.
 *
 * Since Tauri runs as a native desktop app (not a browser tab), tests connect to
 * the Chromium webview that Tauri embeds. Currently the Tauri app must be launched
 * manually (e.g. via `bun run tauri dev`) before running tests.
 *
 * Future enhancement: auto-launch the Tauri app using Playwright's `_electron`
 * module or a custom fixture with child_process + Tauri CLI.
 */
export const test = base.extend({
  // Future: add custom fixtures here (e.g. auto-launch app, DB helpers)
});

export { expect } from "@playwright/test";
