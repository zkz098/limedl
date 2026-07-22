/**
 * Pause and resume lifecycle E2E tests for NAS WebUI.
 *
 * Tests that the pause/resume buttons in the detail panel send the correct
 * JSON-RPC methods and that the UI reflects state changes from server responses.
 *
 * IMPORTANT: Pause/resume buttons live in the DetailPanel (bottom panel),
 * NOT inline in the table row. The test clicks a row to select it, which
 * opens the DetailPanel, then interacts with the pause/resume buttons there.
 */

import { test, expect } from "../fixtures";
import { expectTaskVisible, expectTaskState } from "../helpers/download-asserts";
import { seedDownloadTask, makeMockSummary, makeMockProgress } from "../helpers/task-helpers";

test.describe("pause and resume", () => {
  const TASK_ID = "test-pause-001";

  test.beforeEach(async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    await seedDownloadTask(page, wsMocker, TASK_ID);
    await expectTaskVisible(page, TASK_ID);

    // Send initial progress so the task shows "downloading" state
    wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
      downloadedBytes: 1_000_000,
      speedBytesPerSecond: 5_000_000,
      etaSeconds: 2,
    }));

    await expectTaskState(page, TASK_ID, "downloading");
  });

  test("pause button in detail panel sends download.pause RPC", async ({ page, wsMocker }) => {
    // Click the row to ensure it's selected and the DetailPanel is open.
    // After submitStart, the task is auto-selected, but click explicitly for reliability.
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    // Set up interception for download.pause
    const pausePromise = wsMocker.waitForMethod("download.pause");

    // Click pause button in the DetailPanel actions area.
    // Use exact:true because sidebar "Paused" category button also matches "Pause".
    await page.getByRole("button", { name: "Pause", exact: true }).click();

    const pauseParams = await pausePromise;
    expect(pauseParams).toHaveProperty("taskId", TASK_ID);

    // Respond with a snapshot confirming the paused state.
    // This resolves the frontend's invoke promise and updates the UI.
    expect(wsMocker.respondToMethod("download.pause", makeMockSummary(TASK_ID, {
      state: "paused",
      downloadedBytes: 1_000_000,
      connectionCount: 0,
    }))).toBe(true);

    await expectTaskState(page, TASK_ID, "paused");
  });

  test("resume button sends download.resume RPC and restores downloading state", async ({
    page,
    wsMocker,
  }) => {
    // Select the row to open the DetailPanel
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    // --- First pause ---
    const pausePromise = wsMocker.waitForMethod("download.pause");
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    await pausePromise;

    expect(wsMocker.respondToMethod("download.pause", makeMockSummary(TASK_ID, {
      state: "paused",
      downloadedBytes: 1_000_000,
      connectionCount: 0,
    }))).toBe(true);

    await expectTaskState(page, TASK_ID, "paused");

    // --- Now resume ---
    const resumePromise = wsMocker.waitForMethod("download.resume");

    // Click the Resume button in the DetailPanel
    await page.getByRole("button", { name: "Resume" }).click();

    const resumeParams = await resumePromise;
    expect(resumeParams).toHaveProperty("taskId", TASK_ID);

    // Respond with a snapshot confirming downloading state
    expect(wsMocker.respondToMethod("download.resume", makeMockSummary(TASK_ID, {
      state: "downloading",
      downloadedBytes: 1_000_000,
      connectionCount: 4,
      speedBytesPerSecond: 5_000_000,
      etaSeconds: 2,
    }))).toBe(true);

    // Send a progress event to show the task is actively downloading
    wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
      downloadedBytes: 2_000_000,
      speedBytesPerSecond: 5_000_000,
      etaSeconds: 2,
    }));

    await expectTaskState(page, TASK_ID, "downloading");
  });

  test("error state is displayed when server reports failure", async ({ page, wsMocker }) => {
    // Select the row
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    // Send an updated event with error state directly (no RPC needed here)
    wsMocker.sendEvent("updated", makeMockSummary(TASK_ID, {
      state: "failed",
      downloadedBytes: 1_000_000,
      connectionCount: 0,
      error: "Connection reset by peer",
      requestedThreadCount: 4,
      desiredThreadCount: 0,
      allocatedThreadCount: 0,
    }));

    await expectTaskState(page, TASK_ID, "failed");
  });
});
