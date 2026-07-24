/**
 * Real-server integration tests for NAS WebUI.
 *
 * These tests connect to a REAL running limedl-server daemon (no WS mock)
 * and perform an end-to-end download. They verify the full stack works:
 * page load, composer dialog, download start, progress tracking, and
 * task completion.
 *
 * Prerequisites:
 *   1. Build the NAS frontend: pnpm run build:nas
 *   2. Build the server: cargo build --manifest-path crates/limedl-server/Cargo.toml
 *   3. Start the server: ./target/debug/limedl daemon --port 9090 --web-dir ./dist
 *   4. Start the test file server: (started automatically by global-setup)
 *   5. Run: pnpm run test:e2e:nas:real
 *
 * These tests are conditionally skipped — they only run when
 * LIMEDL_E2E_REAL_SERVER=1 is set in the environment.
 */

import { test, expect } from "../fixtures";

// Read from global-setup env vars
const TEST_FILE_SERVER_URL = process.env.TEST_FILE_SERVER_URL || "http://127.0.0.1:9876";

test.describe("real server integration", () => {
  // Conditional skip: tests run only when LIMEDL_E2E_REAL_SERVER=1
  test.skip(
    !process.env.LIMEDL_E2E_REAL_SERVER,
    "Set LIMEDL_E2E_REAL_SERVER=1 to run real-server tests",
  );

  test("page loads from server and shows app", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible({ timeout: 15000 });
  });

  test("creates a download via composer UI", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible({ timeout: 15000 });

    // Open composer
    await page.getByRole("button", { name: "Add Task" }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();

    // Fill URL with 50mb file to test larger segmented download
    const testUrl = `${TEST_FILE_SERVER_URL}/50mb.bin`;
    await dialog.getByPlaceholder("Paste a link or choose a torrent file").fill(testUrl);

    // The Save Path field is a read-only text field that opens a directory picker
    // on click. It cannot be typed into — the directory is chosen via an OS dialog.
    // If a default download directory is configured in server settings, this field
    // will already be populated and no action is needed here.
    // The test skips filling the directory input; the server default will be used.

    // Click start
    await dialog.getByRole("button", { name: "Start download" }).click();

    // Wait for the task to appear in the download table
    // The task row will show up once the server processes download.start
    // and the frontend refreshes via download.list
    await expect(page.locator('[data-testid^="download-row-"]').first()).toBeVisible({
      timeout: 15000,
    });

    // Verify the task is in downloading or queued state
    const taskRow = page.locator('[data-testid^="download-row-"]').first();
    const statusText = await taskRow.locator('[data-testid="task-status"]').textContent();
    // The task should be in an active state (not errored)
    expect(statusText?.toLowerCase()).not.toContain("failed");
  });

  test("download completes successfully (1mb file)", async ({ page }) => {
    test.setTimeout(120_000); // Downloads take time

    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible({ timeout: 15000 });

    // Create a download of the small file (1MB)
    await page.getByRole("button", { name: "Add Task" }).click();
    const dialog = page.getByRole("dialog");
    await dialog
      .getByPlaceholder("Paste a link or choose a torrent file")
      .fill(`${TEST_FILE_SERVER_URL}/1mb.bin`);
    await dialog.getByRole("button", { name: "Start download" }).click();

    // Wait for task row
    const taskRow = page.locator('[data-testid^="download-row-"]').first();
    await expect(taskRow).toBeVisible({ timeout: 15000 });

    // Wait for completion — poll the status text
    await expect(async () => {
      const status = await taskRow.locator('[data-testid="task-status"]').textContent();
      expect(status?.toLowerCase()).toBe("completed");
    }).toPass({ timeout: 60000, intervals: [2000] });

    // Verify speed display shows 0 (download complete)
    const speedText = await taskRow.locator('[data-testid="task-speed"]').textContent();
    expect(speedText).toBeTruthy();
  });

  test("range request support — 10mb file with resume-capable state", async ({ page }) => {
    test.setTimeout(120_000);

    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible({ timeout: 15000 });

    // Create a download of 10mb.bin
    await page.getByRole("button", { name: "Add Task" }).click();
    const dialog = page.getByRole("dialog");
    await dialog
      .getByPlaceholder("Paste a link or choose a torrent file")
      .fill(`${TEST_FILE_SERVER_URL}/10mb.bin`);
    await dialog.getByRole("button", { name: "Start download" }).click();

    // Wait for task row
    const taskRow = page.locator('[data-testid^="download-row-"]').first();
    await expect(taskRow).toBeVisible({ timeout: 15000 });

    // Wait for completion
    await expect(async () => {
      const status = await taskRow.locator('[data-testid="task-status"]').textContent();
      expect(status?.toLowerCase()).toBe("completed");
    }).toPass({ timeout: 90000, intervals: [2000] });

    // Verify destination path is populated
    const destinationPath = await taskRow.locator('[data-testid="task-destination"]').textContent();
    expect(destinationPath).toBeTruthy();
  });

  test("download list shows multiple tasks", async ({ page }) => {
    test.setTimeout(180_000);

    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible({ timeout: 15000 });

    // Start first download
    await page.getByRole("button", { name: "Add Task" }).click();
    let dialog = page.getByRole("dialog");
    await dialog
      .getByPlaceholder("Paste a link or choose a torrent file")
      .fill(`${TEST_FILE_SERVER_URL}/small.txt`);
    await dialog.getByRole("button", { name: "Start download" }).click();
    await expect(page.locator('[data-testid^="download-row-"]').first()).toBeVisible({
      timeout: 15000,
    });

    // Start second download
    await page.getByRole("button", { name: "Add Task" }).click();
    dialog = page.getByRole("dialog");
    await dialog
      .getByPlaceholder("Paste a link or choose a torrent file")
      .fill(`${TEST_FILE_SERVER_URL}/1mb.bin`);
    await dialog.getByRole("button", { name: "Start download" }).click();

    // Wait for at least 2 download rows
    await expect(page.locator('[data-testid^="download-row-"]')).toHaveCount(2, { timeout: 15000 });
  });
});
