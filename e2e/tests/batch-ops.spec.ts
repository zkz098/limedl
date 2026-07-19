/**
 * Batch operations E2E tests for NAS WebUI.
 *
 * These tests use ws-mocker to simulate multiple download tasks and
 * batch RPC operations. They verify:
 *   - Multi-select mode toggle
 *   - Select all / deselect all
 *   - Pause all selected tasks
 *   - Resume all selected tasks
 *   - Clear completed tasks
 */

import { test, expect } from "../fixtures";
import { makeMockSummary, makeMockProgress, seedDownloadTask } from "../helpers/task-helpers";
import { expectTaskVisible, expectTaskState } from "../helpers/download-asserts";

test.describe("batch operations", () => {
  const TASK_IDS = [
    "http:batch-001",
    "http:batch-002",
    "http:batch-003",
  ];

  test.beforeEach(async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Seed three download tasks
    for (const taskId of TASK_IDS) {
      await seedDownloadTask(page, wsMocker, taskId);
      await expectTaskVisible(page, taskId);

      // Send progress for each to show "downloading" state
      wsMocker.sendEvent("progress", makeMockProgress(taskId, {
        downloadedBytes: 500_000,
        speedBytesPerSecond: 2_000_000,
      }));
    }
  });

  test("enables multi-select mode and selects all tasks", async ({ page }) => {
    // The "Multi-select" button is in the toolbar
    const multiSelectBtn = page.getByRole("button", { name: "Multi-select" });
    await expect(multiSelectBtn).toBeVisible();
    await multiSelectBtn.click();

    // Now batch action buttons should appear
    await expect(page.getByRole("button", { name: "Select all" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Pause all" })).toBeVisible();

    // Click "Select all"
    await page.getByRole("button", { name: "Select all" }).click();

    // After selecting all, the button should change to "Deselect all"
    await expect(page.getByRole("button", { name: "Deselect all" })).toBeVisible();

    // Click "Deselect all" to return
    await page.getByRole("button", { name: "Deselect all" }).click();
    await expect(page.getByRole("button", { name: "Select all" })).toBeVisible();
  });

  test("pauses all downloading tasks", async ({ page, wsMocker }) => {
    // Enable multi-select mode
    await page.getByRole("button", { name: "Multi-select" }).click();

    // Select all tasks
    await page.getByRole("button", { name: "Select all" }).click();

    // Set up interception: pauseAll sends download.pause for each downloading task
    // There are 3 downloading tasks, so we expect 3 calls
    const pausePromises = TASK_IDS.map(() => wsMocker.waitForMethod("download.pause"));

    // Click "Pause all"
    await page.getByRole("button", { name: "Pause all" }).click();

    // Respond to each pause call
    for (const taskId of TASK_IDS) {
      const params = await pausePromises[TASK_IDS.indexOf(taskId)];
      expect(params).toHaveProperty("taskId", taskId);
      wsMocker.respondToMethod("download.pause", makeMockSummary(taskId, {
        state: "paused",
        connectionCount: 0,
        downloadedBytes: 500_000,
      }));
    }

    // Verify each task shows paused state
    for (const taskId of TASK_IDS) {
      await expectTaskState(page, taskId, "paused");
    }
  });

  test("resumes all paused tasks", async ({ page, wsMocker }) => {
    // First pause all tasks
    await page.getByRole("button", { name: "Multi-select" }).click();
    await page.getByRole("button", { name: "Select all" }).click();

    // Pause them
    const pausePromises = TASK_IDS.map(() => wsMocker.waitForMethod("download.pause"));
    await page.getByRole("button", { name: "Pause all" }).click();

    for (const taskId of TASK_IDS) {
      await pausePromises[TASK_IDS.indexOf(taskId)];
      wsMocker.respondToMethod("download.pause", makeMockSummary(taskId, {
        state: "paused",
        connectionCount: 0,
        downloadedBytes: 500_000,
      }));
    }

    for (const taskId of TASK_IDS) {
      await expectTaskState(page, taskId, "paused");
    }

    // Now resume all — but first deselect and reselect (resume all works on
    // paused tasks regardless of selection in the implementation)
    // Set up interception for download.resume calls
    const resumePromises = TASK_IDS.map(() => wsMocker.waitForMethod("download.resume"));

    // Click "Resume all"
    await page.getByRole("button", { name: "Resume all" }).click();

    // Respond to each resume call
    for (const taskId of TASK_IDS) {
      const params = await resumePromises[TASK_IDS.indexOf(taskId)];
      expect(params).toHaveProperty("taskId", taskId);
      wsMocker.respondToMethod("download.resume", makeMockSummary(taskId, {
        state: "downloading",
        connectionCount: 4,
        downloadedBytes: 500_000,
        speedBytesPerSecond: 2_000_000,
      }));

      // Send progress to update the UI
      wsMocker.sendEvent("progress", makeMockProgress(taskId, {
        downloadedBytes: 750_000,
        speedBytesPerSecond: 2_000_000,
      }));
    }

    // Verify each task shows downloading state
    for (const taskId of TASK_IDS) {
      await expectTaskState(page, taskId, "downloading");
    }
  });

  test("clears completed tasks", async ({ page, wsMocker }) => {
    // Send updated events to set all tasks to "completed" state
    for (const taskId of TASK_IDS) {
      wsMocker.sendEvent("updated", makeMockSummary(taskId, {
        state: "completed",
        downloadedBytes: 10_000_000,
        connectionCount: 0,
        speedBytesPerSecond: 0,
      }));
    }

    // Verify all tasks show completed
    for (const taskId of TASK_IDS) {
      await expectTaskState(page, taskId, "completed");
    }

    // Enable multi-select mode
    await page.getByRole("button", { name: "Multi-select" }).click();

    // Set up interception for download.remove calls (clear completed)
    const removePromises = TASK_IDS.map(() => wsMocker.waitForMethod("download.remove"));

    // Click "Clear completed"
    await page.getByRole("button", { name: "Clear completed" }).click();

    // Respond to each remove call
    for (const taskId of TASK_IDS) {
      await removePromises[TASK_IDS.indexOf(taskId)];
      wsMocker.respondToMethod("download.remove", makeMockSummary(taskId, {
        state: "completed",
      }));
    }

    // All tasks should be removed from the view
    for (const taskId of TASK_IDS) {
      await expect(page.locator(`[data-testid="download-row-${taskId}"]`)).not.toBeVisible();
    }
  });
});
