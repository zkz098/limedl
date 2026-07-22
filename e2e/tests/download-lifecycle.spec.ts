/**
 * Download lifecycle E2E tests for NAS WebUI.
 *
 * Tests cover the complete lifecycle of a single download task:
 * creation, pause/resume, cancel, progress updates, completion, and failure.
 *
 * IMPORTANT: Each test uses seedDownloadTask once (single download) to avoid
 * the multi-dialog overlay issue that occurs with sequential composer dialogs.
 *
 * Pause/resume/cancel buttons live in the DetailPanel (bottom panel),
 * NOT inline in the table row. Tests click a row to select it, which
 * opens the DetailPanel, then interact with the action buttons there.
 */

import { test, expect } from "../fixtures";
import { expectTaskVisible, expectTaskState, expectProgressValue, expectSpeedDisplay } from "../helpers/download-asserts";
import { seedDownloadTask, makeMockSummary, makeMockProgress } from "../helpers/task-helpers";

test.describe("download lifecycle", () => {
  const TASK_ID = "lifecycle-001";

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

  test("download shows correct initial state after creation", async ({ page }) => {
    // Verify task row is visible and state is "downloading" (confirmed in beforeEach)

    // Click the row to open the detail panel
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    const detailPanel = page.locator(".detail-panel");
    await expect(detailPanel).toBeVisible();

    // File name should appear in the detail panel header
    await expect(detailPanel.locator(".detail-panel__filename")).toContainText("10mb.bin");

    // Status badge should show "Downloading"
    await expect(detailPanel.locator(".detail-panel__title .ui-badge")).toContainText("Downloading");

    // Switch to the Files tab to verify the URL is displayed
    await page.getByRole("button", { name: "Files" }).click();
    await expect(detailPanel.locator(".detail-panel__body")).toContainText(
      "http://127.0.0.1:9876/10mb.bin",
    );
  });

  test("download pause changes state to paused and resume restores downloading", async ({
    page,
    wsMocker,
  }) => {
    // Select the row to open the DetailPanel
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    // --- Pause ---
    const pausePromise = wsMocker.waitForMethod("download.pause");
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    await pausePromise;

    expect(wsMocker.respondToMethod("download.pause", makeMockSummary(TASK_ID, {
      state: "paused",
      downloadedBytes: 1_000_000,
      connectionCount: 0,
    }))).toBe(true);

    await expectTaskState(page, TASK_ID, "paused");

    // --- Resume ---
    const resumePromise = wsMocker.waitForMethod("download.resume");
    await page.getByRole("button", { name: "Resume" }).click();
    await resumePromise;

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

  test("download cancel removes task from queue", async ({ page, wsMocker }) => {
    // Select the row to open the DetailPanel
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    // Intercept the download.cancel RPC
    const cancelPromise = wsMocker.waitForMethod("download.cancel");

    // Click the Cancel button in the DetailPanel actions area
    await page.getByRole("button", { name: "Cancel" }).click();

    const cancelParams = await cancelPromise;
    expect(cancelParams).toHaveProperty("taskId", TASK_ID);

    // Respond to the RPC so the frontend resolves and removes the task
    expect(wsMocker.respondToMethod("download.cancel", makeMockSummary(TASK_ID, {
      state: "canceled",
      downloadedBytes: 0,
      connectionCount: 0,
    }))).toBe(true);

    // Update the auto-response so any list refreshes don't re-add the task
    wsMocker.setAutoResponse("download.list", []);

    // The task row should be removed from the queue
    await expect(page.locator(`[data-testid="download-row-${TASK_ID}"]`)).not.toBeVisible();
  });

  test("download progress bar updates correctly", async ({ page, wsMocker }) => {
    // Task was seeded with 10 MB total, 0 downloaded. beforeEach sent
    // a progress event at 1 MB (10%). Successive events advance the bar.

    // --- 25% progress ---
    wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
      downloadedBytes: 2_500_000,
      speedBytesPerSecond: 5_000_000,
      etaSeconds: 2,
    }));
    await expectProgressValue(page, TASK_ID, 20);

    // --- 50% progress ---
    wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
      downloadedBytes: 5_000_000,
      speedBytesPerSecond: 5_000_000,
      etaSeconds: 1,
    }));
    await expectProgressValue(page, TASK_ID, 45);

    // --- 75% progress ---
    wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
      downloadedBytes: 7_500_000,
      speedBytesPerSecond: 5_000_000,
      etaSeconds: 1,
    }));
    await expectProgressValue(page, TASK_ID, 70);
  });

  test("download completes with success state", async ({ page, wsMocker }) => {
    // Send an updated event with completed state and full download size
    wsMocker.sendEvent("updated", makeMockSummary(TASK_ID, {
      state: "completed",
      downloadedBytes: 10_000_000,
      connectionCount: 0,
    }));

    // The status badge in the table row should reflect the completed state
    await expectTaskState(page, TASK_ID, "completed");

    // Open the detail panel to verify the badge shows "Completed"
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();
    await expect(page.locator(".detail-panel__title .ui-badge")).toContainText("Completed");
  });

  test("download fails with error state", async ({ page, wsMocker }) => {
    const errorMessage = "Connection reset by peer";

    // Send an updated event with failed state and error message
    wsMocker.sendEvent("updated", makeMockSummary(TASK_ID, {
      state: "failed",
      error: errorMessage,
      downloadedBytes: 1_000_000,
      connectionCount: 0,
    }));

    // The status badge in the table row should reflect the failed state
    await expectTaskState(page, TASK_ID, "failed");

    // Open the detail panel to verify the error message is displayed
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    // The raw error string "Connection reset by peer" does not match any
    // known error pattern in toFriendlyError, so it is rendered verbatim.
    await expect(page.locator(".detail-panel__body")).toContainText(errorMessage);
  });

  test("download shows speed and ETA during active transfer", async ({ page, wsMocker }) => {
    // beforeEach already has the task at "downloading" state with
    // speedBytesPerSecond: 5_000_000 and etaSeconds: 2.
    // Send a new progress event with a longer ETA to verify speed display.
    wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
      downloadedBytes: 2_000_000,
      speedBytesPerSecond: 5_000_000,
      etaSeconds: 120,
    }));

    // The speed display should show a non-zero value with a unit suffix
    // (e.g. "4.77 MB/s") using the expectSpeedDisplay helper.
    await expectSpeedDisplay(page, TASK_ID);

    // The task row should still be visible and in "downloading" state
    await expectTaskState(page, TASK_ID, "downloading");
  });

  test("download refreshes status on manual refresh", async ({ page, wsMocker }) => {
    // Ensure download.list auto-response still includes the current task
    // (seedDownloadTask in beforeEach already set this, but be explicit)
    wsMocker.setAutoResponse("download.list", [makeMockSummary(TASK_ID)]);

    // Click the Refresh button; use .first() to avoid strict mode
    // when multiple buttons match the name
    await page.getByRole("button", { name: "Refresh" }).first().click();

    // The frontend calls download.list to refresh the queue.
    // The auto-response returns the current task summary.
    await wsMocker.waitForMethod("download.list");

    // The frontend then calls download.status to get the latest state.
    // Respond with a mock summary confirming the task is still downloading.
    await wsMocker.waitForMethod("download.status");
    wsMocker.respondToMethod("download.status", makeMockSummary(TASK_ID, {
      state: "downloading",
      downloadedBytes: 1_000_000,
    }));

    // Verify the task row persists after the refresh cycle
    await expectTaskVisible(page, TASK_ID);
    await expectTaskState(page, TASK_ID, "downloading");
  });
});
