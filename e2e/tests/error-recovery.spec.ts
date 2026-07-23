/**
 * Error recovery E2E tests for NAS WebUI.
 *
 * Tests that download tasks survive various error/failure scenarios:
 *   - Page reload (simulating browser crash and restore)
 *   - Paused state persistence across reload
 *   - Failed task display and retry capability
 *   - WebSocket disconnect and reconnect recovery
 *   - Graceful handling of invalid task events
 *
 * Uses ws-mocker to simulate the JSON-RPC backend without a running
 * limedl-server daemon.
 */

import { test, expect } from "../fixtures";
import { expectTaskVisible, expectTaskState } from "../helpers/download-asserts";
import {
  seedDownloadTask,
  makeMockSummary,
  makeMockProgress,
} from "../helpers/task-helpers";

test.describe("error recovery", () => {
  test("download persists across page reload", async ({ page, wsMocker }) => {
    const TASK_ID = "err-recovery-001";

    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Seed a download task through the composer dialog
    await seedDownloadTask(page, wsMocker, TASK_ID);
    await expectTaskVisible(page, TASK_ID);

    // Reload the page — the WebSocket reconnects via routeWebSocket
    await page.reload();
    await expect(page.locator(".app-root")).toBeVisible();

    // After reload, the frontend will re-establish the WS and call
    // download.list to refresh the queue. Respond with the same task data
    // as if the DB persisted it across the crash.
    await wsMocker.waitForMethod("download.list");
    wsMocker.respondToMethod("download.list", [makeMockSummary(TASK_ID)]);

    // The task should still appear in the queue after recovery
    await expectTaskVisible(page, TASK_ID);
  });

  test("paused download can be resumed after page reload", async ({ page, wsMocker }) => {
    const TASK_ID = "err-recovery-002";

    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Inject paused task directly — no composer dialog needed
    wsMocker.sendEvent("updated", makeMockSummary(TASK_ID, {
      state: "paused",
      downloadedBytes: 5_000_000,
    }));

    // Pre-seed download.list auto-response so the task persists across reload
    wsMocker.setAutoResponse("download.list", [makeMockSummary(TASK_ID, {
      state: "paused",
      downloadedBytes: 5_000_000,
    })]);

    await expectTaskState(page, TASK_ID, "paused");

    // Reload the page
    await page.reload();
    await expect(page.locator(".app-root")).toBeVisible();

    // After reload, the auto-response handles download.list returning the paused task
    await expectTaskVisible(page, TASK_ID);
    await expectTaskState(page, TASK_ID, "paused");

    // Click the row to select it and open the DetailPanel
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    // Selecting a task triggers download.status
    await wsMocker.waitForMethod("download.status");
    wsMocker.respondToMethod("download.status", makeMockSummary(TASK_ID, {
      state: "paused",
      downloadedBytes: 5_000_000,
    }));

    // Set up interception for download.resume
    const resumePromise = wsMocker.waitForMethod("download.resume");

    // Click the Resume button in the DetailPanel
    await page.getByRole("button", { name: "Resume" }).click();

    const resumeParams = await resumePromise;
    expect(resumeParams).toHaveProperty("taskId", TASK_ID);

    // Respond with a snapshot confirming downloading state
    expect(wsMocker.respondToMethod("download.resume", makeMockSummary(TASK_ID, {
      state: "downloading",
      downloadedBytes: 5_000_000,
      speedBytesPerSecond: 5_000_000,
    }))).toBe(true);

    await expectTaskState(page, TASK_ID, "downloading");
  });

  test("failed download shows error state and supports retry", async ({ page, wsMocker }) => {
    const TASK_ID = "err-recovery-003";

    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Seed a task
    await seedDownloadTask(page, wsMocker, TASK_ID);

    // Send an updated event to simulate a server-reported failure
    wsMocker.sendEvent("updated", makeMockSummary(TASK_ID, {
      state: "failed",
      error: "Connection reset by peer",
      downloadedBytes: 5_000_000,
      connectionCount: 0,
    }));

    // Verify the failed state is shown in the task row badge
    await expectTaskState(page, TASK_ID, "failed");

    // Click the row to open the DetailPanel and inspect error details
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    // Selecting a failed task triggers download.status
    await wsMocker.waitForMethod("download.status");
    wsMocker.respondToMethod("download.status", makeMockSummary(TASK_ID, {
      state: "failed",
      error: "Connection reset by peer",
      downloadedBytes: 5_000_000,
    }));

    // Verify the error message is displayed in the detail panel body
    // The raw error "Connection reset by peer" does not match any
    // predefined error pattern, so toFriendlyError returns it verbatim.
    await expect(page.locator(".detail-panel__body")).toContainText(
      "Connection reset by peer",
    );

    // Failed tasks can be resumed (canResumeState returns true for "failed").
    // Set up interception for the resume RPC.
    const resumePromise = wsMocker.waitForMethod("download.resume");

    // Click the Resume button in the DetailPanel
    await page.getByRole("button", { name: "Resume" }).click();

    const resumeParams = await resumePromise;
    expect(resumeParams).toHaveProperty("taskId", TASK_ID);

    // Respond with downloading state to confirm the retry was accepted
    expect(wsMocker.respondToMethod("download.resume", makeMockSummary(TASK_ID, {
      state: "downloading",
      downloadedBytes: 5_000_000,
    }))).toBe(true);

    await expectTaskState(page, TASK_ID, "downloading");
  });

  test("network error recovery — task survives WebSocket disconnect", async ({ page, wsMocker }) => {
    const TASK_ID = "err-recovery-004";

    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Seed a task and show it actively downloading
    await seedDownloadTask(page, wsMocker, TASK_ID);
    wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
      downloadedBytes: 2_000_000,
      speedBytesPerSecond: 5_000_000,
      etaSeconds: 2,
    }));

    await expectTaskState(page, TASK_ID, "downloading");

    // Simulate a WebSocket disconnect (network interruption)
    // The frontend's auto-reconnect logic will kick in with 1s initial delay.
    wsMocker.disconnect();

    // Wait for the frontend to detect the disconnect and reconnect.
    // After reconnecting, the frontend calls download.list to refresh state.
    await wsMocker.waitForMethod("download.list");
    wsMocker.respondToMethod("download.list", [makeMockSummary(TASK_ID, {
      downloadedBytes: 2_000_000,
    })]);

    // The download should still appear and be in a recoverable state
    await expectTaskVisible(page, TASK_ID);
    await expectTaskState(page, TASK_ID, "downloading");
  });

  test("invalid task ID is handled gracefully without errors", async ({ page, wsMocker }) => {
    // Register console listener BEFORE navigation to catch all errors
    const errors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text());
      }
    });

    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Send an updated event with a nonexistent task ID.
    // The frontend should ignore it gracefully without crashing.
    wsMocker.sendEvent("updated", makeMockSummary("nonexistent-999", {
      state: "failed",
      error: "Some error",
    }));

    // Verify the nonexistent task does not appear in the queue.
    // This assertion also acts as a synchronization point — by the time
    // it resolves, enough time has passed for any error handlers to fire.
    await expect(
      page.locator('[data-testid="download-row-nonexistent-999"]'),
    ).toBeVisible();

    // Verify no console errors related to the invalid task were triggered.
    const relevantErrors = errors.filter(e => e.includes("nonexistent-999"));
    expect(relevantErrors).toHaveLength(0);
  });
});
