/**
 * WebSocket connection lifecycle tests for NAS WebUI.
 *
 * Tests that the frontend establishes a WebSocket connection on page load,
 * can send JSON-RPC requests and receive responses, and can handle
 * server-pushed events.
 */

import { test, expect } from "../fixtures";
import { makeMockSummary } from "../helpers/task-helpers";

test.describe("WebSocket connection", () => {
  test("establishes WebSocket on page load", async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();
    // wsMocker.connected should be true after install + page load triggers WS connect
    expect(wsMocker.isConnected).toBe(true);
  });

  test("handles download.start and receives taskId response", async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Open composer
    await page.getByRole("button", { name: "Add Task" }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();

    // Fill URL
    const urlInput = dialog.getByPlaceholder("Paste a link or choose a torrent file");
    await urlInput.fill("http://127.0.0.1:9876/10mb.bin");

    // Set up the mock BEFORE clicking start — intercept the download.start call
    const startPromise = wsMocker.waitForMethod("download.start");

    // Click start
    await dialog.getByRole("button", { name: "Start download" }).click();

    const startParams = await startPromise;
    expect(startParams).toBeDefined();
    expect(startParams).toHaveProperty("url", "http://127.0.0.1:9876/10mb.bin");

    // Respond with taskId using respondToMethod which auto-matches the request ID.
    // TaskIdResult: { kind, id }
    const responded = wsMocker.respondToMethod("download.start", {
      kind: "http",
      id: "test-conn-001",
    });
    expect(responded).toBe(true);

    // After submitStart succeeds, the frontend calls download.list and download.status.
    // waitForMethod buffers messages that arrive before the call, so it's safe to await sequentially.
    await wsMocker.waitForMethod("download.list");
    wsMocker.respondToMethod("download.list", []);

    await wsMocker.waitForMethod("download.status");
    wsMocker.respondToMethod("download.status", makeMockSummary("test-conn-001"));
  });

  test("receives server-pushed events after connection", async ({ page, wsMocker }) => {
    const errors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text());
      }
    });

    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Send a progress event directly (simulating server push without a prior request).
    // This tests that the event dispatcher works without errors.
    wsMocker.sendEvent("progress", {
      id: "test-conn-002",
      state: "downloading",
      downloadedBytes: 1000,
      totalBytes: 10000,
      speedBytesPerSecond: 500000,
      etaSeconds: 18,
      connectionCount: 4,
      allocatedThreadCount: 4,
      error: null,
      uploadedBytes: 0,
      uploadSpeedBytesPerSecond: 0,
      peerCount: 0,
      degraded: false,
      diskType: "ssd",
      flushing: false,
    });

    // No console errors should have been triggered by the push event
    expect(errors).toHaveLength(0);
  });
});
