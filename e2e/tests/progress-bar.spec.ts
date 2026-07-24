/**
 * Download progress bar E2E tests for NAS WebUI.
 *
 * Verifies that the progress bar, speed display, and state badges update
 * correctly in response to server-pushed download events.
 */

import { test, expect } from "../fixtures";
import {
  expectTaskVisible,
  expectTaskState,
  expectProgressValue,
  expectSpeedDisplay,
} from "../helpers/download-asserts";
import { seedDownloadTask, makeMockSummary, makeMockProgress } from "../helpers/task-helpers";

test.describe("download progress bar", () => {
  const TASK_ID = "test-progress-001";
  const TOTAL_BYTES = 10_000_000;

  test.beforeEach(async ({ page, wsMocker: _wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();
  });

  test("progress bar shows percentage and speed during download", async ({ page, wsMocker }) => {
    await seedDownloadTask(page, wsMocker, TASK_ID, { url: "http://127.0.0.1:9876/10mb.bin" });
    await expectTaskVisible(page, TASK_ID);

    // Send progress at 50%
    wsMocker.sendEvent(
      "progress",
      makeMockProgress(TASK_ID, {
        downloadedBytes: 5_000_000,
        speedBytesPerSecond: 2_500_000,
        etaSeconds: 2,
      }),
    );

    // Assert progress bar shows ~50%
    await expectProgressValue(page, TASK_ID, 45);

    // Assert speed display shows a value
    await expectSpeedDisplay(page, TASK_ID);
  });

  test("progress bar reaches 100% and task shows completed", async ({ page, wsMocker }) => {
    await seedDownloadTask(page, wsMocker, TASK_ID, {
      url: "http://127.0.0.1:9876/1mb.bin",
      fileName: "1mb.bin",
      totalBytes: 1_048_576,
    });
    await expectTaskVisible(page, TASK_ID);

    // Send completion via updated event
    wsMocker.sendEvent(
      "updated",
      makeMockSummary(TASK_ID, {
        state: "completed",
        url: "http://127.0.0.1:9876/1mb.bin",
        fileName: "1mb.bin",
        destinationPath: "/tmp/limedl-test/1mb.bin",
        totalBytes: 1_048_576,
        downloadedBytes: 1_048_576,
      }),
    );

    // Assert completed state
    await expectTaskState(page, TASK_ID, "completed");
  });

  test("multiple rapid progress updates render without errors", async ({ page, wsMocker }) => {
    await seedDownloadTask(page, wsMocker, TASK_ID, { url: "http://127.0.0.1:9876/10mb.bin" });
    await expectTaskVisible(page, TASK_ID);

    // Rapid-fire 10 progress updates
    for (let i = 1; i <= 10; i++) {
      wsMocker.sendEvent(
        "progress",
        makeMockProgress(TASK_ID, {
          downloadedBytes: Math.round((TOTAL_BYTES / 10) * i),
          speedBytesPerSecond: 1_000_000 * i,
          etaSeconds: 10 - i,
        }),
      );
    }

    // Final state should be visible and not errored
    await expectTaskVisible(page, TASK_ID);
    // The task should still be "downloading" (not completed — that requires an updated event)
    await expectTaskState(page, TASK_ID, "downloading");
  });
});
